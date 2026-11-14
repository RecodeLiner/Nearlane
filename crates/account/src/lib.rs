//! Browser OAuth runtime. Configuration stays local; bearer grants stay in memory.
//!
//! A granted permission does not establish Quick Share device trust. This crate
//! never advertises a device or enables Contacts/OwnDevices visibility.

mod config;
mod exchange;
mod loopback;

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
use nearlane_auth::{AuthorizationAttempt, TokenGrant, TokenRequest};

/// Sanitized failures. No provider response, URL, code or token is retained here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginError {
    /// Missing, malformed or insufficiently private local configuration.
    Configuration,
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
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => {
                "Local sign-in configuration is missing, invalid, or not private."
            }
            Self::Listener => "Could not open a local browser callback listener.",
            Self::Callback => "Could not validate the sign-in attempt.",
            Self::Browser => "Could not open the system browser.",
            Self::Denied => "Access was declined in the browser.",
            Self::Cancelled => "Sign-in cancelled.",
            Self::TimedOut => "Sign-in timed out. You can try again.",
            Self::Network => "Could not reach Google's token endpoint securely.",
            Self::TokenRejected => "Google rejected the token exchange. No session was enabled.",
            Self::InvalidGrant => "Google did not return the required Quick Share permission.",
            Self::WorkerStopped => "The sign-in worker stopped unexpectedly.",
            Self::Busy => "A sign-in attempt is already in progress.",
        })
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
    /// Listener is bound; waiting for consent in the browser.
    WaitingForBrowser,
    /// The validated one-use code is being exchanged over HTTPS.
    Exchanging,
    /// A Nearby permission was granted; certificate exchange remains unverified.
    Authorized,
    /// A bounded session expired; reauthorization is required.
    Expired,
    /// A sanitized failure, including cancellation or provider denial.
    Failed(LoginError),
}

impl AccountStatus {
    /// Short description suitable for a UI, with no sensitive values.
    pub fn message(self) -> String {
        match self {
            Self::Unconfigured => "Local sign-in configuration is not available.".into(),
            Self::Ready => "Ready to sign in through your browser.".into(),
            Self::WaitingForBrowser => "Complete sign-in in your browser. Choose a test account.".into(),
            Self::Exchanging => "Checking the permission returned by Google…".into(),
            Self::Authorized => "Quick Share permission granted. Device certificate exchange is still required; this device remains hidden.".into(),
            Self::Expired => "The in-memory session expired. Sign in again.".into(),
            Self::Failed(error) => error.to_string(),
        }
    }
}

/// An authorization URL exposed only to the system-browser launcher, never QML.
pub struct BrowserAuthorization(url::Url);
impl BrowserAuthorization {
    /// Hand directly to the OS browser opener; do not log it or put it in QML.
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
    _grant: TokenGrant,
    expires_at: Instant,
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
                    Ok(Session {
                        expires_at: Instant::now() + Duration::from_secs(grant.expires_in()),
                        _grant: grant,
                    })
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
        let config = ClientConfig::load(path).ok();
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
    }

    /// Bind a loopback listener and start a worker; the caller opens the returned
    /// URL on the UI thread. No account request is sent until a valid callback.
    pub fn begin(&mut self) -> Result<BrowserAuthorization, LoginError> {
        if self.busy() {
            return Err(LoginError::Busy);
        }
        let config = self.config.clone().ok_or(LoginError::Configuration)?;
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
                        self.session = Some(session);
                        self.status = AccountStatus::Authorized;
                    }
                    Err(error) => self.status = AccountStatus::Failed(error),
                }
            } else if pending.exchanging.load(Ordering::Acquire) {
                self.status = AccountStatus::Exchanging;
            }
        }
        if self
            .session
            .as_ref()
            .is_some_and(|s| Instant::now() >= s.expires_at)
        {
            self.session = None;
            self.status = AccountStatus::Expired;
        }
    }

    /// Cancel the attempt and discard its result. An already-sent HTTPS request
    /// may finish within its timeout, but its grant cannot re-enable the session.
    pub fn cancel(&mut self, reason: LoginError) {
        self.pending = None;
        self.session = None;
        self.status = AccountStatus::Failed(reason);
    }

    /// Clear local memory only. This does not revoke grants in the Google account.
    pub fn clear_session(&mut self) {
        self.pending = None;
        self.session = None;
        self.status = if self.configured() {
            AccountStatus::Ready
        } else {
            AccountStatus::Unconfigured
        };
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
                _grant: TokenGrant::from_response(&json).unwrap(),
                expires_at: Instant::now() - Duration::from_secs(1),
            }),
            status: AccountStatus::Authorized,
        };
        account.poll();
        assert_eq!(account.status(), AccountStatus::Expired);
        assert!(account.session.is_none());
    }
}
