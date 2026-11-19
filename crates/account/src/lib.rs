//! Browser OAuth runtime. Configuration stays local; bearer grants stay in memory.
//!
//! A granted permission does not establish Quick Share device trust. This crate
//! never advertises a device or enables Contacts/OwnDevices visibility.

use nearlane_core::Text;
mod config;
mod exchange;
mod identity;
mod loopback;
mod saved;

use std::{
    fmt,
    net::TcpListener,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub use config::ClientConfig;
use identity::PrivateSharing;
pub use identity::{CertificateSummary, IdentityError, PrivateStatus};
use nearlane_auth::{AuthorizationAttempt, TokenGrant, TokenRequest};
pub use saved::SavedLoginStatus;
use saved::{SavedLogin, Signal};

/// Sanitized failures. No provider response, URL, code or token is retained here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginError {
    /// Missing, malformed or insufficiently private local configuration.
    Configuration,
    /// Validated application parameters could not be saved outside the package.
    ConfigurationStorage,
    /// Could not bind or use the IPv4 loopback listener.
    Listener,
    /// Secure entropy or callback validation failed.
    Callback,
    /// The browser could not be launched by the desktop.
    Browser,
    /// Access was declined in the provider's browser page.
    Denied,
    /// The attempt was cancelled and any result discarded.
    Cancelled,
    /// No valid callback arrived within the allowed time.
    TimedOut,
    /// The HTTPS request failed. Details are deliberately not exposed.
    Network,
    /// Google rejected the code exchange.
    TokenRejected,
    /// The response did not contain a valid Nearby bearer grant.
    InvalidGrant,
    /// The background worker stopped unexpectedly.
    WorkerStopped,
    /// An authorization attempt is already in progress.
    Busy,
    /// The OS credential store could not be used securely.
    SecureStorage,
    /// Google invalidated the offline grant; browser consent is required again.
    RefreshRejected,
}

impl LoginError {
    /// Deferred, sanitized explanation for a localized presentation layer.
    pub fn text(&self) -> Text {
        match self {
            Self::ConfigurationStorage => {
                Text::new("Could not save sign-in configuration in the local application folder.")
            }
            Self::Configuration => {
                Text::new("Local sign-in configuration is missing, invalid, or not private.")
            }
            Self::Listener => Text::new("Could not open a local browser callback listener."),
            Self::Callback => Text::new("Could not validate the sign-in attempt."),
            Self::Browser => Text::new("Could not open the system browser."),
            Self::Denied => Text::new("Access was declined in the browser."),
            Self::Cancelled => Text::new("Sign-in cancelled."),
            Self::TimedOut => Text::new("Sign-in timed out. You can try again."),
            Self::Network => Text::new("Could not reach Google's token endpoint securely."),
            Self::TokenRejected => {
                Text::new("Google rejected the token exchange. No session was enabled.")
            }
            Self::InvalidGrant => {
                Text::new("Google did not return the required Quick Share permission.")
            }
            Self::WorkerStopped => Text::new("The sign-in worker stopped unexpectedly."),
            Self::Busy => Text::new("A sign-in attempt is already in progress."),
            Self::SecureStorage => Text::new(
                "The system credential store is unavailable or locked. Unlock it and retry saved sign-in.",
            ),
            Self::RefreshRejected => {
                Text::new("The saved Google permission expired or was revoked. Sign in again.")
            }
        }
    }
}
impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text().fmt(f)
    }
}
impl std::error::Error for LoginError {}

/// Safe account progress information; this is not a sharing capability flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountStatus {
    /// No usable local application configuration.
    Unconfigured,
    /// Ready to start a browser authorization attempt.
    Ready,
    /// Restoring or renewing an expired saved session without a browser.
    Restoring,
    /// Listener is bound; waiting for consent in the browser.
    WaitingForBrowser,
    /// The validated one-use code is being exchanged over HTTPS.
    Exchanging,
    /// A Nearby permission was granted; certificate exchange remains unverified.
    Authorized,
    /// Local sign-out completed. Provider-wide consent is separate.
    SignedOut,
    /// A bounded session expired; reauthorization is required.
    Expired,
    /// A sanitized failure, including cancellation or provider denial.
    Failed(LoginError),
}

impl AccountStatus {
    /// Short description suitable for a UI, with no sensitive values.
    pub fn message(self) -> String {
        self.text().to_string()
    }

    /// Deferred text for a localized presentation layer.
    pub fn text(self) -> Text {
        match self {
            Self::Unconfigured => Text::new("Local sign-in configuration is not available."),
            Self::Ready => Text::new("Ready to sign in through your browser."),
            Self::Restoring => Text::new("Restoring saved Google permission…"),
            Self::WaitingForBrowser => {
                Text::new("Complete sign-in in your browser. Choose a test account.")
            }
            Self::Exchanging => Text::new("Checking the permission returned by Google…"),
            Self::Authorized => Text::new(
                "Quick Share permission granted. Receiving starts only when you enable it.",
            ),
            Self::SignedOut => Text::new("Signed out. This device is hidden."),
            Self::Expired => Text::new("The in-memory session expired. Sign in again."),
            Self::Failed(error) => error.text(),
        }
    }
}

/// An authorization URL for the system-browser launcher.
pub struct BrowserAuthorization(url::Url);
impl BrowserAuthorization {
    /// Pass directly to the browser opener and keep out of logs and UI state.
    pub fn expose_url(&self) -> &str {
        self.0.as_str()
    }
}
impl fmt::Debug for BrowserAuthorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BrowserAuthorization([REDACTED])")
    }
}

struct Session {
    _grant: Arc<TokenGrant>,
    expires_at: Instant,
    registration: String,
}
impl Session {
    fn new(grant: TokenGrant, registration: Option<String>) -> Result<Self, LoginError> {
        if !grant.has_sharing_permission() {
            return Err(LoginError::InvalidGrant);
        }
        Ok(Self {
            expires_at: Instant::now() + Duration::from_secs(grant.expires_in()),
            _grant: Arc::new(grant),
            registration: match registration {
                Some(id) => id,
                None => identity::registration_id().map_err(|_| LoginError::WorkerStopped)?,
            },
        })
    }
}

struct PendingLogin {
    cancelled: Arc<AtomicBool>,
    exchanging: Arc<AtomicBool>,
    result: mpsc::Receiver<Result<Session, LoginError>>,
}

impl PendingLogin {
    fn start(
        config: ClientConfig,
        timeout: Duration,
        exchange: impl FnOnce(TokenRequest) -> Result<TokenGrant, LoginError> + Send + 'static,
    ) -> Result<(Self, BrowserAuthorization), LoginError> {
        // Bind before constructing the redirect and launching the browser. Never
        // bind 0.0.0.0 or let another process select the callback port for us.
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| LoginError::Listener)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| LoginError::Listener)?;
        let attempt = AuthorizationAttempt::new(
            listener
                .local_addr()
                .map_err(|_| LoginError::Listener)?
                .port(),
        )
        .map_err(|_| LoginError::Callback)?;
        let url = attempt
            .authorization_url(&config.client_id)
            .map_err(|_| LoginError::Configuration)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let exchanging = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancelled);
        let worker_exchange = Arc::clone(&exchanging);
        let (sender, result) = mpsc::channel();
        std::thread::Builder::new()
            .name("nearlane-signin".into())
            .spawn(move || {
                let result = loopback::run(
                    listener,
                    attempt,
                    config,
                    worker_cancel,
                    worker_exchange,
                    timeout,
                    exchange,
                )
                .and_then(|grant| {
                    if !grant.has_sharing_permission() {
                        return Err(LoginError::InvalidGrant);
                    }
                    Session::new(grant, None)
                });
                // Exactly one terminal result is sent; dropped/cancelled UIs do not
                // retain grants in a detached global store.
                let _ = sender.send(result);
            })
            .map_err(|_| LoginError::WorkerStopped)?;
        Ok((
            Self {
                cancelled,
                exchanging,
                result,
            },
            BrowserAuthorization(url),
        ))
    }
}

impl Drop for PendingLogin {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

/// Owns one cancellable browser attempt and at most one in-memory bearer grant.
pub struct Account {
    config: Option<ClientConfig>,
    pending: Option<PendingLogin>,
    session: Option<Session>,
    status: AccountStatus,
    private: PrivateSharing,
    saved: Option<SavedLogin>,
    saved_status: SavedLoginStatus,
    next_refresh: Option<Instant>,
    next_restore: Option<Instant>,
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Account")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl Account {
    /// Load only application parameters. Never reads browser profiles or account cookies.
    pub fn from_local_config(path: &Path) -> Self {
        Self::with_config(ClientConfig::load(path).ok())
    }
    /// Offline state when no usable per-account configuration path is available.
    pub fn unconfigured() -> Self {
        Self::with_config(None)
    }
    /// Use a validated, locally imported configuration without reopening its file.
    pub fn from_client_config(config: ClientConfig) -> Self {
        Self::with_config(Some(config))
    }

    fn with_config(config: Option<ClientConfig>) -> Self {
        let status = if config.is_some() {
            AccountStatus::Ready
        } else {
            AccountStatus::Unconfigured
        };
        Self {
            config,
            pending: None,
            session: None,
            status,
            private: PrivateSharing::default(),
            saved: None,
            saved_status: SavedLoginStatus::NotSaved,
            next_refresh: None,
            next_restore: None,
        }
    }

    /// Sanitized current state. Call poll to observe completions and expiry.
    pub fn status(&self) -> AccountStatus {
        self.status
    }
    /// Whether local desktop application parameters were loaded successfully.
    pub fn configured(&self) -> bool {
        self.config.is_some()
    }
    /// Whether one browser or network operation is in progress.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
            || self.private.busy()
            || self.saved.as_ref().is_some_and(SavedLogin::busy)
    }

    /// Safe persistence progress; no vault/account values are exposed.
    pub fn saved_login_status(&self) -> SavedLoginStatus {
        self.saved
            .as_ref()
            .map(SavedLogin::status)
            .unwrap_or(self.saved_status)
    }
    /// Whether an OS-vault entry might exist, including failed deletion.
    pub fn has_saved_login(&self) -> bool {
        self.saved.as_ref().is_some_and(SavedLogin::may_have_saved)
            || self.saved_status == SavedLoginStatus::ClearFailed
    }
    fn ensure_saved(&mut self) -> Result<&mut SavedLogin, LoginError> {
        if self.saved.is_none() {
            let config = self.config.clone().ok_or(LoginError::Configuration)?;
            self.saved = Some(SavedLogin::native(config).inspect_err(|_| {
                self.saved_status = SavedLoginStatus::Unavailable;
            })?);
        }
        self.saved.as_mut().ok_or(LoginError::WorkerStopped)
    }
    /// Called explicitly by the desktop at startup; CLI/config construction stays
    /// offline. Only this app's OS-vault entry and fixed Google token API are used.
    pub fn restore_saved_sign_in(&mut self) -> Result<(), LoginError> {
        if self.busy() {
            return Err(LoginError::Busy);
        }
        self.next_restore = None;
        if let Some(session) = &self.session {
            if Instant::now() < session.expires_at {
                return Err(LoginError::Busy);
            }
            let grant = Arc::clone(&session._grant);
            let id = session.registration.clone();
            self.ensure_saved()?.renew(&grant, &id);
        } else {
            self.ensure_saved()?.restore();
        }
        self.status = AccountStatus::Restoring;
        Ok(())
    }
    fn schedule_refresh(&mut self) {
        self.next_refresh = self
            .session
            .as_ref()
            .filter(|s| s._grant.refresh_token().is_some())
            .map(|s| {
                let life = s._grant.expires_in();
                Instant::now() + Duration::from_secs(life.saturating_sub(120).max(life / 2).max(1))
            });
    }

    /// Server-side certificate exchange state, independent of peer trust flags.
    pub fn private_status(&self) -> PrivateStatus {
        self.private.status()
    }

    /// Whether certificate setup or owned-resource cleanup is in progress.
    pub fn private_busy(&self) -> bool {
        self.private.busy()
    }

    /// In-memory trusted API cache for the owned-device discovery worker only.
    /// Keep credentials out of serialized data, UI state and logs.
    pub fn own_device_certificates(&self) -> Vec<Arc<nearlane_certificates::RemoteCertificate>> {
        if self.status == AccountStatus::Authorized
            && self
                .session
                .as_ref()
                .is_some_and(|s| Instant::now() < s.expires_at)
        {
            self.private.remote_certificates()
        } else {
            Vec::new()
        }
    }

    /// Whether this session owns the private key for a cached certificate.
    pub fn is_local_certificate(&self, id: &[u8; 32]) -> bool {
        self.private.is_local_certificate(id)
    }

    /// Issue private signing/ad material to one trusted transfer worker while
    /// account authorization is current; discovery alone cannot obtain this.
    pub fn transfer_identity(
        &mut self,
    ) -> Result<nearlane_certificates::OwnDeviceIdentity, IdentityError> {
        if self.status != AccountStatus::Authorized
            || self
                .session
                .as_ref()
                .is_none_or(|s| Instant::now() >= s.expires_at)
        {
            return Err(IdentityError::NoSession);
        }
        self.private.transfer_identity()
    }

    /// Publish temporary own-device certificates and obtain a bounded cache from
    /// Google's fixed API. Only an explicit service command starts this operation.
    pub fn prepare_own_devices(&mut self) -> Result<(), IdentityError> {
        if self.busy() {
            return Err(IdentityError::Busy);
        }
        let session = self
            .session
            .as_ref()
            .filter(|s| Instant::now() < s.expires_at)
            .ok_or(IdentityError::NoSession)?;
        self.private
            .prepare(Arc::clone(&session._grant), &session.registration)
    }

    /// Bind a loopback listener and start a worker; the caller opens the returned
    /// URL on the UI thread. No account request is sent until a valid callback.
    pub fn begin(&mut self) -> Result<BrowserAuthorization, LoginError> {
        if self.busy() || self.session.is_some() || self.has_saved_login() {
            return Err(LoginError::Busy);
        }
        let config = self.config.clone().ok_or(LoginError::Configuration)?;
        self.next_restore = None;
        if let Some(saved) = &mut self.saved {
            saved.cancel();
        }
        let (pending, url) =
            PendingLogin::start(config, Duration::from_secs(300), exchange::exchange)?;
        self.session = None;
        self.pending = Some(pending);
        self.status = AccountStatus::WaitingForBrowser;
        Ok(url)
    }

    /// Observe background completion on the caller's thread, without blocking.
    pub fn poll(&mut self) {
        if let Some(pending) = &self.pending {
            let result = match pending.result.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(LoginError::WorkerStopped)),
            };
            if let Some(result) = result {
                self.pending = None;
                match result {
                    Ok(session) => {
                        if session._grant.refresh_token().is_some() {
                            let grant = Arc::clone(&session._grant);
                            if let Ok(saved) = self.ensure_saved() {
                                saved.save(&grant, &session.registration);
                            }
                        } else {
                            self.saved_status = SavedLoginStatus::MissingRefreshToken;
                            if let Some(saved) = &mut self.saved {
                                saved.save(&session._grant, &session.registration);
                            }
                        }
                        self.session = Some(session);
                        self.status = AccountStatus::Authorized;
                        self.schedule_refresh();
                    }
                    Err(error) => self.status = AccountStatus::Failed(error),
                }
            } else if pending.exchanging.load(Ordering::Acquire) {
                self.status = AccountStatus::Exchanging;
            }
        }
        if let Some(signal) = self.saved.as_mut().and_then(SavedLogin::poll) {
            match signal {
                Signal::Empty => {
                    if self.session.is_none() {
                        self.status = AccountStatus::Ready;
                    }
                }
                Signal::Authorized(grant, id) => match Session::new(grant, Some(id)) {
                    Ok(session) => {
                        self.session = Some(session);
                        self.status = AccountStatus::Authorized;
                        self.next_restore = None;
                        self.schedule_refresh();
                    }
                    Err(error) => self.status = AccountStatus::Failed(error),
                },
                Signal::Failed(LoginError::RefreshRejected) => {
                    self.private
                        .sign_out(self.session.as_ref().map(|s| Arc::clone(&s._grant)));
                    self.session = None;
                    self.next_refresh = None;
                    self.next_restore = None;
                    self.status = AccountStatus::Failed(LoginError::RefreshRejected);
                }
                Signal::Failed(error) => {
                    if self
                        .session
                        .as_ref()
                        .is_some_and(|s| Instant::now() < s.expires_at)
                    {
                        self.status = AccountStatus::Authorized;
                    } else {
                        self.status = AccountStatus::Failed(error);
                    }
                    self.next_refresh = self
                        .session
                        .as_ref()
                        .map(|_| Instant::now() + Duration::from_secs(60));
                    if self.session.is_none() && error == LoginError::Network {
                        self.next_restore = Some(Instant::now() + Duration::from_secs(60));
                    }
                }
            }
        }
        let refresh_due = self.next_refresh.is_some_and(|time| Instant::now() >= time);
        if refresh_due
            && !self.saved.as_ref().is_some_and(SavedLogin::pending)
            && let Some(session) = &self.session
        {
            let grant = Arc::clone(&session._grant);
            let id = session.registration.clone();
            if let Ok(saved) = self.ensure_saved() {
                saved.renew(&grant, &id);
            }
            self.next_refresh = None;
        }
        if self.next_restore.is_some_and(|time| Instant::now() >= time) && !self.busy() {
            let _ = self.restore_saved_sign_in();
        }
        if self
            .session
            .as_ref()
            .is_some_and(|s| Instant::now() >= s.expires_at)
        {
            if self
                .session
                .as_ref()
                .is_some_and(|s| s._grant.refresh_token().is_some())
            {
                self.status = if self.saved.as_ref().is_some_and(SavedLogin::busy) {
                    AccountStatus::Restoring
                } else {
                    AccountStatus::Expired
                };
            } else {
                self.private
                    .sign_out(self.session.as_ref().map(|s| Arc::clone(&s._grant)));
                self.session = None;
                self.status = AccountStatus::Expired;
            }
        }
        self.private.poll(self.session.is_some());
    }

    /// Cancel the attempt and discard its result. An already-sent HTTPS request
    /// may finish within its timeout, but its grant cannot re-enable the session.
    pub fn cancel(&mut self, reason: LoginError) {
        self.next_refresh = None;
        self.next_restore = None;
        if let Some(saved) = &mut self.saved {
            saved.cancel();
        }
        self.private
            .sign_out(self.session.as_ref().map(|s| Arc::clone(&s._grant)));
        self.pending = None;
        self.session = None;
        self.status = AccountStatus::Failed(reason);
    }

    /// Drop the session and request cleanup of the owned temporary registration.
    /// This does not revoke the account's project-wide OAuth grant.
    pub fn clear_session(&mut self) {
        self.sign_out();
        self.status = if self.configured() {
            AccountStatus::Ready
        } else {
            AccountStatus::Unconfigured
        };
    }

    /// Sign out and cancel any pending browser attempt. Provider-wide grant
    /// revocation is separate; Google browser cookies are not accessed.
    pub fn sign_out(&mut self) {
        self.next_refresh = None;
        self.next_restore = None;
        if let Some(saved) = &mut self.saved {
            saved.forget();
        }
        self.private
            .sign_out(self.session.as_ref().map(|s| Arc::clone(&s._grant)));
        self.pending = None;
        self.session = None;
        self.status = AccountStatus::SignedOut;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpStream,
    };

    fn config() -> ClientConfig {
        ClientConfig {
            client_id: zeroize::Zeroizing::new("synthetic-client".into()),
            client_secret: None,
        }
    }

    fn request(url: &url::Url, state: &str, code: &str) -> String {
        let redirect: String = url
            .query_pairs()
            .find(|(k, _)| k == "redirect_uri")
            .unwrap()
            .1
            .into();
        let redirect = url::Url::parse(&redirect).unwrap();
        let authority = format!("127.0.0.1:{}", redirect.port().unwrap());
        let mut stream = TcpStream::connect(&authority).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        write!(
            stream,
            "GET /oauth/callback?state={state}&code={code} HTTP/1.1\r\nHost: {authority}\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn one_valid_callback_survives_stray_requests_and_exchanges_once() {
        let (pending, url) = PendingLogin::start(config(), Duration::from_secs(3), |request| {
            assert!(request.expose_form().contains("code=synthetic-code"));
            let json = format!(r#"{{"access_token":"synthetic-access","token_type":"Bearer","expires_in":60,"scope":"{}"}}"#, nearlane_auth::NEARBY_SCOPE);
            TokenGrant::from_response(&json).map_err(|_| LoginError::InvalidGrant)
        }).unwrap();
        let state: String = url
            .0
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into();
        let invalid = request(&url.0, "wrong", "stray-code");
        assert!(invalid.starts_with("HTTP/1.1 400"));
        let valid = request(&url.0, &state, "synthetic-code");
        assert!(valid.starts_with("HTTP/1.1 200"));
        assert!(!valid.contains("synthetic-code"));
        let session = pending
            .result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        assert!(session.expires_at > Instant::now());
        let redirect = url
            .0
            .query_pairs()
            .find(|(k, _)| k == "redirect_uri")
            .unwrap()
            .1
            .into_owned();
        let port = url::Url::parse(&redirect).unwrap().port().unwrap();
        assert!(TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_err());
    }

    #[test]
    fn cancellation_and_timeout_close_listener_without_token_exchange() {
        for cancel in [true, false] {
            let (pending, _) = PendingLogin::start(config(), Duration::from_millis(100), |_| {
                panic!("must not exchange")
            })
            .unwrap();
            if cancel {
                pending.cancelled.store(true, Ordering::Release);
            }
            let error = pending
                .result
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .err()
                .unwrap();
            assert_eq!(
                error,
                if cancel {
                    LoginError::Cancelled
                } else {
                    LoginError::TimedOut
                }
            );
        }
    }

    #[test]
    fn cancelled_worker_completion_cannot_restore_account_session() {
        let (pending, _) = PendingLogin::start(config(), Duration::from_secs(2), |_| {
            panic!("must not exchange")
        })
        .unwrap();
        let mut account = Account {
            config: Some(config()),
            pending: Some(pending),
            session: None,
            status: AccountStatus::WaitingForBrowser,
            private: PrivateSharing::default(),
            saved: None,
            saved_status: SavedLoginStatus::NotSaved,
            next_refresh: None,
            next_restore: None,
        };
        account.cancel(LoginError::Cancelled);
        std::thread::sleep(Duration::from_millis(50));
        account.poll();
        assert_eq!(
            account.status(),
            AccountStatus::Failed(LoginError::Cancelled)
        );
        assert!(account.session.is_none());
    }

    #[test]
    fn cancellation_during_exchange_discards_late_grant() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (pending, url) = PendingLogin::start(config(), Duration::from_secs(3), move |_| {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let json = format!(r#"{{"access_token":"late-synthetic-token","token_type":"Bearer","expires_in":60,"scope":"{}"}}"#, nearlane_auth::NEARBY_SCOPE);
            TokenGrant::from_response(&json).map_err(|_| LoginError::InvalidGrant)
        }).unwrap();
        let state: String = url
            .0
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into();
        assert!(request(&url.0, &state, "synthetic-code").starts_with("HTTP/1.1 200"));
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        pending.cancelled.store(true, Ordering::Release);
        release_tx.send(()).unwrap();
        let result = pending.result.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(result, Err(LoginError::Cancelled)));
    }

    #[test]
    fn sdk_only_grant_cannot_enable_sharing_authorization() {
        let (pending, url) = PendingLogin::start(config(), Duration::from_secs(3), |_| {
            let json = format!(r#"{{"access_token":"synthetic-token","token_type":"Bearer","expires_in":60,"scope":"{}"}}"#, nearlane_auth::NEARBY_SDK_SCOPE);
            TokenGrant::from_response(&json).map_err(|_| LoginError::InvalidGrant)
        }).unwrap();
        let state: String = url
            .0
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into();
        request(&url.0, &state, "synthetic-code");
        assert!(matches!(
            pending.result.recv_timeout(Duration::from_secs(2)).unwrap(),
            Err(LoginError::InvalidGrant)
        ));
    }

    #[test]
    fn expired_session_is_removed_without_reauthentication() {
        let json = format!(
            r#"{{"access_token":"synthetic-token","token_type":"Bearer","expires_in":60,"scope":"{}"}}"#,
            nearlane_auth::NEARBY_SCOPE
        );
        let mut account = Account {
            config: Some(config()),
            pending: None,
            session: Some(Session {
                _grant: Arc::new(TokenGrant::from_response(&json).unwrap()),
                expires_at: Instant::now() - Duration::from_secs(1),
                registration: "TestDevice".into(),
            }),
            status: AccountStatus::Authorized,
            private: PrivateSharing::default(),
            saved: None,
            saved_status: SavedLoginStatus::NotSaved,
            next_refresh: None,
            next_restore: None,
        };
        account.poll();
        assert_eq!(account.status(), AccountStatus::Expired);
        assert!(account.session.is_none());
    }

    #[test]
    fn sign_out_cancels_pending_login_and_stays_signed_out() {
        let (pending, _) = PendingLogin::start(config(), Duration::from_secs(2), |_| {
            panic!("must not exchange")
        })
        .unwrap();
        let mut account = Account {
            config: Some(config()),
            pending: Some(pending),
            session: None,
            status: AccountStatus::WaitingForBrowser,
            private: PrivateSharing::default(),
            saved: None,
            saved_status: SavedLoginStatus::NotSaved,
            next_refresh: None,
            next_restore: None,
        };
        account.sign_out();
        std::thread::sleep(Duration::from_millis(50));
        account.poll();
        assert_eq!(account.status(), AccountStatus::SignedOut);
        assert!(account.session.is_none() && !account.busy());
        assert!(account.configured());
    }
}
