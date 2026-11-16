//! One serialized worker orders vault writes, renewal and deletion. A late save
//! cannot overtake sign-out; generation checks discard late authentication events.
use crate::{ClientConfig, LoginError};
use nearlane_auth::TokenGrant;
use nearlane_core::Text;
use nearlane_platform::credentials::{SecretStore, SystemStore};
use std::{fmt, sync::mpsc};
use zeroize::{Zeroize as _, Zeroizing};

/// Safe saved-login progress; no token, account name or vault identifier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SavedLoginStatus {
    /// No saved grant is known.
    #[default]
    NotSaved,
    /// Reading this app's vault entry and renewing its access token.
    Restoring,
    /// Renewing an existing session without a browser.
    Refreshing,
    /// Saving the offline grant to the OS vault.
    Saving,
    /// Offline grant was saved successfully.
    Saved,
    /// Google did not provide a refresh token for this session.
    MissingRefreshToken,
    /// The OS vault could not be used; no plaintext fallback is attempted.
    Unavailable,
    /// A transient renewal failure leaves the saved entry intact.
    RestoreFailed(LoginError),
    /// Restoration was explicitly cancelled for this run.
    Paused,
    /// Removing the saved grant after local sign-out.
    Clearing,
    /// Local sign-out completed but vault deletion was not confirmed.
    ClearFailed,
}
impl SavedLoginStatus {
    /// Plain interface explanation of persistence and its limitations.
    pub fn message(self) -> String {
        self.text().to_string()
    }

    /// Deferred text for a localized presentation layer.
    pub fn text(self) -> Text {
        match self {
            Self::NotSaved => {
                Text::new("Sign in once to remember this account in the system credential store.")
            }
            Self::Restoring => {
                Text::new("Restoring saved sign-in through the system credential store…")
            }
            Self::Refreshing => Text::new("Renewing access automatically…"),
            Self::Saving => Text::new("Saving offline access in the system credential store…"),
            Self::Saved => Text::new(
                "Sign-in is remembered in the system credential store. Access renews automatically.",
            ),
            Self::MissingRefreshToken => Text::new(
                "This session is temporary: Google did not issue offline access. Sign out and sign in again to request it.",
            ),
            Self::Unavailable => Text::new(
                "The system credential store is unavailable or locked. Sign-in could not be remembered; no token file was created.",
            ),
            Self::RestoreFailed(_) => Text::new(
                "Automatic sign-in could not complete. The saved grant was kept; retry or sign out.",
            ),
            Self::Paused => {
                Text::new("Saved sign-in is paused for this run. Retry or sign out to forget it.")
            }
            Self::Clearing => Text::new(
                "Signed out locally. Removing saved sign-in from the system credential store…",
            ),
            Self::ClearFailed => Text::new(
                "Signed out locally, but removing saved sign-in was not confirmed. Keep the app open and retry Sign out after unlocking the credential store.",
            ),
        }
    }
}

struct Record {
    token: Zeroizing<String>,
    registration: String,
}
impl Record {
    fn from_grant(grant: &TokenGrant, registration: &str) -> Option<Self> {
        Some(Self {
            token: Zeroizing::new(grant.refresh_token()?.to_owned()),
            registration: registration.into(),
        })
    }
    fn encode(&self) -> Result<Zeroizing<Vec<u8>>, ()> {
        let mut json = serde_json::json!({"version":1,"scope":nearlane_auth::NEARBY_SCOPE,"refresh_token":self.token.as_str(),"registration":self.registration});
        let result = serde_json::to_vec(&json)
            .map(Zeroizing::new)
            .map_err(|_| ());
        if let serde_json::Value::String(token) = &mut json["refresh_token"] {
            token.zeroize();
        }
        result
    }
    fn decode(bytes: &[u8]) -> Result<Self, ()> {
        if bytes.len() > 32 * 1024 {
            return Err(());
        }
        let mut json: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| ())?;
        let token = match json["refresh_token"].take() {
            serde_json::Value::String(t) => Zeroizing::new(t),
            _ => return Err(()),
        };
        let registration = json["registration"].as_str().ok_or(())?;
        if json["version"].as_u64() != Some(1)
            || json["scope"].as_str() != Some(nearlane_auth::NEARBY_SCOPE)
            || token.is_empty()
            || token.len() > 16 * 1024
            || token.chars().any(char::is_control)
            || registration.len() != 10
            || !registration.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Err(());
        }
        Ok(Self {
            token,
            registration: registration.into(),
        })
    }
}
enum Work {
    Restore,
    Renew(Record),
    Save(Record),
    Forget,
}
struct Command {
    generation: u64,
    work: Work,
}
enum Outcome {
    Empty,
    Authorized {
        grant: TokenGrant,
        registration: String,
        stored: bool,
    },
    Saved(bool),
    Forgotten(bool),
    StoreUnavailable,
    Failed(LoginError),
    Revoked {
        cleared: bool,
    },
}
struct Event {
    generation: u64,
    outcome: Outcome,
}
pub(crate) enum Signal {
    Empty,
    Authorized(TokenGrant, String),
    Failed(LoginError),
}

pub(crate) struct SavedLogin {
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<Event>,
    generation: u64,
    pending: bool,
    may_have_saved: bool,
    status: SavedLoginStatus,
}
impl fmt::Debug for SavedLogin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SavedLogin")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}
impl SavedLogin {
    pub(crate) fn native(config: ClientConfig) -> Result<Self, LoginError> {
        let store = SystemStore::for_client(&config.client_id);
        Self::start(store, move |token| crate::exchange::refresh(&config, token))
    }
    pub(crate) fn start(
        store: impl SecretStore,
        renew: impl FnMut(&str) -> Result<TokenGrant, LoginError> + Send + 'static,
    ) -> Result<Self, LoginError> {
        let (commands, receiver) = mpsc::channel::<Command>();
        let (sender, events) = mpsc::channel();
        std::thread::Builder::new()
            .name("nearlane-saved-login".into())
            .spawn(move || worker(store, renew, receiver, sender))
            .map_err(|_| LoginError::WorkerStopped)?;
        Ok(Self {
            commands,
            events,
            generation: 0,
            pending: false,
            may_have_saved: false,
            status: SavedLoginStatus::NotSaved,
        })
    }
    pub(crate) fn status(&self) -> SavedLoginStatus {
        self.status
    }
    pub(crate) fn may_have_saved(&self) -> bool {
        self.may_have_saved
    }
    pub(crate) fn busy(&self) -> bool {
        self.pending && self.status != SavedLoginStatus::Saving
    }
    pub(crate) fn pending(&self) -> bool {
        self.pending
    }
    pub(crate) fn restore(&mut self) {
        if self.pending {
            return;
        }
        self.may_have_saved = true;
        self.submit(Work::Restore, SavedLoginStatus::Restoring);
    }
    pub(crate) fn save(&mut self, grant: &TokenGrant, registration: &str) {
        if let Some(record) = Record::from_grant(grant, registration) {
            self.may_have_saved = true;
            self.submit(Work::Save(record), SavedLoginStatus::Saving);
        } else {
            self.status = SavedLoginStatus::MissingRefreshToken;
        }
    }
    pub(crate) fn renew(&mut self, grant: &TokenGrant, registration: &str) {
        if self.pending {
            return;
        }
        if let Some(record) = Record::from_grant(grant, registration) {
            self.submit(Work::Renew(record), SavedLoginStatus::Refreshing);
        }
    }
    pub(crate) fn forget(&mut self) {
        if self.pending && self.status == SavedLoginStatus::Clearing {
            return;
        }
        self.submit(Work::Forget, SavedLoginStatus::Clearing);
    }
    pub(crate) fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = false;
        self.status = if self.may_have_saved {
            SavedLoginStatus::Paused
        } else {
            SavedLoginStatus::NotSaved
        };
    }
    fn submit(&mut self, work: Work, status: SavedLoginStatus) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = self
            .commands
            .send(Command {
                generation: self.generation,
                work,
            })
            .is_ok();
        self.status = if self.pending {
            status
        } else {
            SavedLoginStatus::Unavailable
        };
    }
    pub(crate) fn poll(&mut self) -> Option<Signal> {
        // Each UI action queues at most one operation; bound draining as well.
        for _ in 0..16 {
            let event = match self.events.try_recv() {
                Ok(event) => event,
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.pending {
                        self.pending = false;
                        self.status = SavedLoginStatus::Unavailable;
                        return Some(Signal::Failed(LoginError::WorkerStopped));
                    }
                    return None;
                }
            };
            if event.generation != self.generation {
                continue;
            }
            self.pending = false;
            match event.outcome {
                Outcome::Empty => {
                    self.may_have_saved = false;
                    self.status = SavedLoginStatus::NotSaved;
                    return Some(Signal::Empty);
                }
                Outcome::Authorized {
                    grant,
                    registration,
                    stored,
                } => {
                    self.may_have_saved = true;
                    self.status = if stored {
                        SavedLoginStatus::Saved
                    } else {
                        SavedLoginStatus::Unavailable
                    };
                    return Some(Signal::Authorized(grant, registration));
                }
                Outcome::Saved(ok) => {
                    self.status = if ok {
                        SavedLoginStatus::Saved
                    } else {
                        SavedLoginStatus::Unavailable
                    }
                }
                Outcome::Forgotten(ok) => {
                    self.may_have_saved = !ok;
                    self.status = if ok {
                        SavedLoginStatus::NotSaved
                    } else {
                        SavedLoginStatus::ClearFailed
                    };
                }
                Outcome::StoreUnavailable => {
                    self.status = SavedLoginStatus::Unavailable;
                    return Some(Signal::Failed(LoginError::SecureStorage));
                }
                Outcome::Failed(error) => {
                    self.status = SavedLoginStatus::RestoreFailed(error);
                    return Some(Signal::Failed(error));
                }
                Outcome::Revoked { cleared } => {
                    self.may_have_saved = !cleared;
                    self.status = if cleared {
                        SavedLoginStatus::NotSaved
                    } else {
                        SavedLoginStatus::ClearFailed
                    };
                    return Some(Signal::Failed(LoginError::RefreshRejected));
                }
            }
        }
        None
    }
}

fn worker(
    mut store: impl SecretStore,
    mut renew: impl FnMut(&str) -> Result<TokenGrant, LoginError>,
    commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
) {
    while let Ok(command) = commands.recv() {
        let outcome = match command.work {
            Work::Forget => Outcome::Forgotten(store.delete().is_ok()),
            Work::Save(record) => Outcome::Saved(
                record
                    .encode()
                    .ok()
                    .is_some_and(|bytes| store.save(&bytes).is_ok()),
            ),
            Work::Restore => match store.load() {
                Ok(None) => Outcome::Empty,
                Ok(Some(bytes)) => match Record::decode(&bytes) {
                    Ok(record) => renew_record(record, &mut store, &mut renew),
                    Err(_) => Outcome::StoreUnavailable,
                },
                Err(_) => Outcome::StoreUnavailable,
            },
            Work::Renew(record) => renew_record(record, &mut store, &mut renew),
        };
        // Continue processing queued deletion even when a former observer left.
        let _ = events.send(Event {
            generation: command.generation,
            outcome,
        });
    }
}
fn renew_record(
    record: Record,
    store: &mut impl SecretStore,
    renew: &mut impl FnMut(&str) -> Result<TokenGrant, LoginError>,
) -> Outcome {
    match renew(&record.token) {
        Ok(grant) if grant.has_sharing_permission() => {
            let stored = Record::from_grant(&grant, &record.registration)
                .and_then(|r| r.encode().ok())
                .is_some_and(|bytes| store.save(&bytes).is_ok());
            Outcome::Authorized {
                grant,
                registration: record.registration,
                stored,
            }
        }
        Ok(_) => Outcome::Failed(LoginError::InvalidGrant),
        Err(LoginError::RefreshRejected) => Outcome::Revoked {
            cleared: store.delete().is_ok(),
        },
        Err(error) => Outcome::Failed(error),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use nearlane_platform::credentials::StoreError;
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    #[derive(Clone, Default)]
    struct MemoryStore {
        data: Arc<Mutex<Option<Zeroizing<Vec<u8>>>>>,
        fail_delete: bool,
    }
    impl SecretStore for MemoryStore {
        fn load(&mut self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError> {
            Ok(self
                .data
                .lock()
                .unwrap()
                .as_ref()
                .map(|v| Zeroizing::new(v.to_vec())))
        }
        fn save(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
            *self.data.lock().unwrap() = Some(Zeroizing::new(bytes.to_vec()));
            Ok(())
        }
        fn delete(&mut self) -> Result<(), StoreError> {
            if self.fail_delete {
                return Err(StoreError);
            }
            *self.data.lock().unwrap() = None;
            Ok(())
        }
    }
    fn grant(refresh: &str) -> TokenGrant {
        TokenGrant::from_response(&format!(r#"{{"access_token":"synthetic-access","refresh_token":"{refresh}","token_type":"Bearer","expires_in":60,"scope":"{}"}}"#, nearlane_auth::NEARBY_SCOPE)).unwrap()
    }
    fn finish(saved: &mut SavedLogin) -> Option<Signal> {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut signal = None;
        while saved.pending() && Instant::now() < deadline {
            if let Some(event) = saved.poll() {
                signal = Some(event);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!saved.pending());
        signal
    }

    #[test]
    fn restart_restores_scoped_grant_and_keeps_registration_while_rotating_token() {
        let store = MemoryStore::default();
        let mut saved =
            SavedLogin::start(store.clone(), |_| panic!("save must not renew")).unwrap();
        saved.save(&grant("synthetic-refresh"), "TestDevice");
        finish(&mut saved);
        assert_eq!(saved.status(), SavedLoginStatus::Saved);
        let record = Record::decode(store.data.lock().unwrap().as_ref().unwrap()).unwrap();
        assert_eq!(record.token.as_str(), "synthetic-refresh");
        assert!(
            !String::from_utf8_lossy(store.data.lock().unwrap().as_ref().unwrap())
                .contains("synthetic-access")
        );
        drop(saved);
        let mut restored = SavedLogin::start(store.clone(), |token| {
            assert_eq!(token, "synthetic-refresh");
            Ok(grant("rotated-synthetic-refresh"))
        })
        .unwrap();
        restored.restore();
        let Some(Signal::Authorized(grant, id)) = finish(&mut restored) else {
            panic!("not restored")
        };
        assert_eq!(id, "TestDevice");
        assert_eq!(grant.refresh_token(), Some("rotated-synthetic-refresh"));
        assert_eq!(
            Record::decode(store.data.lock().unwrap().as_ref().unwrap())
                .unwrap()
                .token
                .as_str(),
            "rotated-synthetic-refresh"
        );
        assert!(!format!("{restored:?}").contains("synthetic"));
    }

    #[test]
    fn queued_sign_out_wins_over_late_renewal_and_its_vault_write() {
        let mut store = MemoryStore::default();
        store
            .save(
                &Record::from_grant(&grant("synthetic-refresh"), "TestDevice")
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
            .unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut saved = SavedLogin::start(store.clone(), move |_| {
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            Ok(grant("late-rotated-refresh"))
        })
        .unwrap();
        saved.restore();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        saved.forget();
        release_tx.send(()).unwrap();
        assert!(finish(&mut saved).is_none());
        assert_eq!(saved.status(), SavedLoginStatus::NotSaved);
        assert!(!saved.may_have_saved());
        assert!(store.data.lock().unwrap().is_none());
    }

    #[test]
    fn network_failure_keeps_grant_but_revoked_grant_is_deleted() {
        for error in [LoginError::Network, LoginError::RefreshRejected] {
            let mut store = MemoryStore::default();
            store
                .save(
                    &Record::from_grant(&grant("synthetic-refresh"), "TestDevice")
                        .unwrap()
                        .encode()
                        .unwrap(),
                )
                .unwrap();
            let mut saved = SavedLogin::start(store.clone(), move |_| Err(error)).unwrap();
            saved.restore();
            assert!(matches!(finish(&mut saved), Some(Signal::Failed(e)) if e == error));
            assert_eq!(
                store.data.lock().unwrap().is_some(),
                error == LoginError::Network
            );
        }
    }

    #[test]
    fn malformed_scope_and_failed_deletion_do_not_claim_success() {
        let mut store = MemoryStore {
            fail_delete: true,
            ..Default::default()
        };
        store.save(br#"{"version":1,"scope":"openid","refresh_token":"synthetic","registration":"TestDevice"}"#).unwrap();
        let mut saved =
            SavedLogin::start(store.clone(), |_| panic!("malformed record must not renew"))
                .unwrap();
        saved.restore();
        assert!(matches!(
            finish(&mut saved),
            Some(Signal::Failed(LoginError::SecureStorage))
        ));
        saved.forget();
        finish(&mut saved);
        assert_eq!(saved.status(), SavedLoginStatus::ClearFailed);
        assert!(saved.may_have_saved());
    }

    #[test]
    fn account_refreshes_without_browser_and_late_result_cannot_undo_sign_out() {
        use crate::{Account, AccountStatus, ClientConfig, Session};
        let mut account =
            Account::from_local_config(std::path::Path::new("missing-synthetic-config"));
        account.config = Some(ClientConfig {
            client_id: Zeroizing::new("synthetic-client".into()),
            client_secret: None,
        });
        account.saved = Some(
            SavedLogin::start(MemoryStore::default(), |_| Ok(grant("renewed-refresh"))).unwrap(),
        );
        account.session =
            Some(Session::new(grant("synthetic-refresh"), Some("TestDevice".into())).unwrap());
        account.status = AccountStatus::Authorized;
        account.next_refresh = Some(Instant::now() - Duration::from_secs(1));
        account.poll();
        let deadline = Instant::now() + Duration::from_secs(2);
        while account.saved.as_ref().unwrap().pending() && Instant::now() < deadline {
            account.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(account.status(), AccountStatus::Authorized);
        assert_eq!(
            account.session.as_ref().unwrap()._grant.refresh_token(),
            Some("renewed-refresh")
        );
        assert_eq!(account.session.as_ref().unwrap().registration, "TestDevice");
        assert!(account.pending.is_none());
        account.sign_out();
        while account.saved.as_ref().unwrap().pending() && Instant::now() < deadline {
            account.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(account.status(), AccountStatus::SignedOut);
        assert!(account.session.is_none());
        assert!(!account.has_saved_login());
    }
}
