//! Fixed-origin Nearby Identity RPCs and in-memory own-device credentials.
use nearlane_core::Text;
mod context;
mod proto;
mod rpc;
#[cfg(test)]
mod tests;

use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

use nearlane_auth::TokenGrant;
use nearlane_certificates::CertificateError;

use context::IdentityContext;
use rpc::GoogleIdentityClient;

pub(crate) fn registration_id() -> Result<String, IdentityError> {
    context::device_id()
}

/// Bounded certificate exchange counts. No device/account names or keys.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CertificateSummary {
    /// Non-expired credentials Google marked as the same account's devices.
    pub own_device_certificates: usize,
    /// Credentials currently within their validity window and clock tolerance.
    pub active_certificates: usize,
    /// Malformed public-certificate records rejected by validation.
    pub rejected_certificates: usize,
}

/// Sanitized Nearby Identity failures; provider messages/metadata are discarded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// A fixed-origin TLS connection or local runtime could not be established.
    Network,
    /// Only the numeric gRPC status is retained, never its potentially private text.
    RpcStatus(i32),
    /// Invalid local request or credential framing.
    Encoding,
    /// Certificate cryptographic/structural validation failed.
    Certificate(CertificateError),
    /// Bounded page/count/byte policy was exceeded.
    Limit,
    /// Cancellation requested; it cannot undo an already accepted publication.
    Cancelled,
    /// Operation or cleanup exceeded its overall deadline.
    TimedOut,
    /// Sign in before preparing private device credentials.
    NoSession,
    /// Another login/setup/cleanup operation is pending.
    Busy,
    /// The bounded background worker stopped.
    Worker,
}
impl IdentityError {
    /// Deferred, sanitized explanation for a localized presentation layer.
    pub fn text(&self) -> Text {
        match self {
            Self::Network => Text::new("Could not reach Nearby Identity securely."),
            Self::RpcStatus(16) => {
                Text::new("Nearby Identity did not accept the account token. Sign in again.")
            }
            Self::RpcStatus(7) => Text::new("Google denied access to Nearby Identity (gRPC 7)."),
            Self::RpcStatus(code) => Text::with_args(
                "Nearby Identity rejected the request (gRPC %1).",
                [Text::literal(code)],
            ),
            Self::Encoding => Text::new("The private-device request could not be prepared."),
            Self::Certificate(_) => Text::new("Device certificate validation failed."),
            Self::Limit => Text::new("Certificate exchange exceeded a safety limit."),
            Self::Cancelled => Text::new("Private-device setup was cancelled."),
            Self::TimedOut => Text::new("Private-device setup timed out."),
            Self::NoSession => Text::new("Sign in before preparing private devices."),
            Self::Busy => Text::new("A private-device operation is already running."),
            Self::Worker => Text::new("The private-device worker stopped unexpectedly."),
        }
    }
}
impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text().fmt(f)
    }
}
impl std::error::Error for IdentityError {}

/// Server exchange state, separate from real private-mode discovery/handshake.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PrivateStatus {
    /// Not registered/prepared during this local account session.
    #[default]
    Idle,
    /// Generating/publishing own-device credentials and reading bounded pages.
    Preparing,
    /// Publication and a complete certificate query succeeded.
    Ready(CertificateSummary),
    /// Setup failed; retained local keys/name allow an idempotent retry.
    Failed(IdentityError),
    /// Local trust is gone; a bounded withdrawal request is finishing.
    SigningOut,
    /// Local keys are gone. Remote withdrawal was not confirmed.
    CleanupUnconfirmed,
}
impl PrivateStatus {
    /// Sanitized progress for the UI; Ready does not promise Android transfer.
    pub fn message(self) -> String {
        self.text().to_string()
    }

    /// Deferred text for a localized presentation layer.
    pub fn text(self) -> Text {
        match self {
            Self::Idle => Text::new("Own-device certificates have not been prepared."),
            Self::Preparing => Text::new("Preparing temporary certificates for your own devices…"),
            Self::Ready(summary) => Text::with_args(
                "This device is registered. Received %1 own-device certificates (%2 active; %3 malformed records rejected). Live private verification is required for each connection.",
                [
                    Text::literal(summary.own_device_certificates),
                    Text::literal(summary.active_certificates),
                    Text::literal(summary.rejected_certificates),
                ],
            ),
            Self::Failed(error) => error.text(),
            Self::SigningOut => Text::new(
                "Signed out locally. Asking Google to clear this device's published certificates…",
            ),
            Self::CleanupUnconfirmed => Text::new(
                "Signed out locally. Clearing the temporary device credentials at Google was not confirmed; they may remain until expiry.",
            ),
        }
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Setup,
    Cleanup,
}
struct WorkResult {
    context: Option<IdentityContext>,
    outcome: Result<CertificateSummary, IdentityError>,
    cleanup: Option<Result<(), IdentityError>>,
}
struct Pending {
    receiver: mpsc::Receiver<WorkResult>,
    grant: Arc<TokenGrant>,
    cancelled: Arc<AtomicBool>,
    logout: Arc<AtomicBool>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

/// No credentials or identifying cache values are exposed through Debug.
#[derive(Default)]
pub(crate) struct PrivateSharing {
    context: Option<IdentityContext>,
    pending: Option<Pending>,
    status: PrivateStatus,
}
impl fmt::Debug for PrivateSharing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrivateSharing")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl PrivateSharing {
    pub(crate) fn transfer_identity(
        &mut self,
    ) -> Result<nearlane_certificates::OwnDeviceIdentity, IdentityError> {
        if self.busy() || !matches!(self.status, PrivateStatus::Ready(_)) {
            return Err(IdentityError::Busy);
        }
        self.context
            .as_mut()
            .ok_or(IdentityError::NoSession)?
            .transfer_identity()
    }
    pub(crate) fn is_local_certificate(&self, id: &[u8; 32]) -> bool {
        self.context
            .as_ref()
            .is_some_and(|c| c.is_local_certificate(id))
    }
    pub(crate) fn remote_certificates(&self) -> Vec<Arc<nearlane_certificates::RemoteCertificate>> {
        self.context
            .as_ref()
            .map(IdentityContext::remote_certificates)
            .unwrap_or_default()
    }
    pub(crate) fn status(&self) -> PrivateStatus {
        self.status
    }
    pub(crate) fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn prepare(
        &mut self,
        grant: Arc<TokenGrant>,
        registration: &str,
    ) -> Result<(), IdentityError> {
        if self.busy() {
            return Err(IdentityError::Busy);
        }
        if let Err(error) = self.spawn(grant, Mode::Setup, Some(registration.to_owned())) {
            self.status = PrivateStatus::Failed(error);
            return Err(error);
        }
        self.status = PrivateStatus::Preparing;
        Ok(())
    }

    pub(crate) fn sign_out(&mut self, grant: Option<Arc<TokenGrant>>) {
        if let Some(pending) = &self.pending {
            pending.logout.store(true, Ordering::Release);
            pending.cancelled.store(true, Ordering::Release);
            self.status = PrivateStatus::SigningOut;
        } else if self
            .context
            .as_ref()
            .is_some_and(|c| c.publication_attempted)
        {
            if let Some(grant) = grant {
                if self.spawn(grant, Mode::Cleanup, None).is_ok() {
                    self.status = PrivateStatus::SigningOut;
                    return;
                }
            }
            self.context = None;
            self.status = PrivateStatus::CleanupUnconfirmed;
        } else {
            self.context = None;
            self.status = PrivateStatus::Idle;
        }
    }

    pub(crate) fn poll(&mut self, logged_in: bool) {
        let result = match self.pending.as_ref().map(|p| p.receiver.try_recv()) {
            Some(Ok(result)) => result,
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.pending = None;
                self.status = if logged_in {
                    PrivateStatus::Failed(IdentityError::Worker)
                } else {
                    PrivateStatus::CleanupUnconfirmed
                };
                return;
            }
            _ => return,
        };
        let Some(pending) = self.pending.take() else {
            return;
        };
        if !logged_in || pending.logout.load(Ordering::Acquire) {
            // Sign-out can race with a result already in the mailbox. If the
            // worker did not see it, clear the same owned resource in a new job
            // before allowing another login; never restore the returned keys.
            if result.cleanup.is_none()
                && result
                    .context
                    .as_ref()
                    .is_some_and(|c| c.publication_attempted)
            {
                self.context = result.context;
                self.sign_out(Some(Arc::clone(&pending.grant)));
            } else {
                self.context = None;
                self.status = if matches!(result.cleanup, Some(Err(_))) {
                    PrivateStatus::CleanupUnconfirmed
                } else {
                    PrivateStatus::Idle
                };
            }
        } else {
            self.context = result.context;
            self.status = match result.outcome {
                Ok(summary) => PrivateStatus::Ready(summary),
                Err(error) => PrivateStatus::Failed(error),
            };
        }
    }

    fn spawn(
        &mut self,
        grant: Arc<TokenGrant>,
        mode: Mode,
        registration: Option<String>,
    ) -> Result<(), IdentityError> {
        let slot = Arc::new(Mutex::new(self.context.take()));
        let worker_slot = Arc::clone(&slot);
        let cancelled = Arc::new(AtomicBool::new(false));
        let logout = Arc::new(AtomicBool::new(matches!(mode, Mode::Cleanup)));
        let worker_cancel = Arc::clone(&cancelled);
        let worker_logout = Arc::clone(&logout);
        let worker_grant = Arc::clone(&grant);
        let (sender, receiver) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("nearlane-certificates".into())
            .spawn(move || {
                let context = worker_slot.lock().ok().and_then(|mut slot| slot.take());
                let result = run(
                    context,
                    &worker_grant,
                    mode,
                    &worker_cancel,
                    &worker_logout,
                    registration.as_deref(),
                );
                let _ = sender.send(result);
            });
        if spawned.is_err() {
            self.context = slot.lock().ok().and_then(|mut slot| slot.take());
            return Err(IdentityError::Worker);
        }
        self.pending = Some(Pending {
            receiver,
            grant,
            cancelled,
            logout,
        });
        Ok(())
    }
}

fn run(
    existing: Option<IdentityContext>,
    grant: &TokenGrant,
    mode: Mode,
    cancelled: &AtomicBool,
    logout: &AtomicBool,
    registration: Option<&str>,
) -> WorkResult {
    let mut context = match existing.map(Ok).unwrap_or_else(|| {
        registration
            .map(IdentityContext::for_registration)
            .unwrap_or_else(IdentityContext::new)
    }) {
        Ok(context) => context,
        Err(error) => {
            return WorkResult {
                context: None,
                outcome: Err(error),
                cleanup: None,
            };
        }
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            return WorkResult {
                context: Some(context),
                outcome: Err(IdentityError::Network),
                cleanup: None,
            };
        }
    };
    let (outcome, cleanup) = runtime.block_on(async {
        let mut client = match GoogleIdentityClient::connect(grant).await {
            Ok(client) => client,
            Err(error) => {
                return (
                    Err(error),
                    if matches!(mode, Mode::Cleanup) {
                        Some(Err(error))
                    } else {
                        None
                    },
                );
            }
        };
        let outcome = if matches!(mode, Mode::Setup) {
            tokio::time::timeout(
                Duration::from_secs(60),
                context.synchronize(&mut client, cancelled),
            )
            .await
            .unwrap_or(Err(IdentityError::TimedOut))
        } else {
            Ok(CertificateSummary::default())
        };
        let cleanup = if logout.load(Ordering::Acquire) {
            Some(
                tokio::time::timeout(Duration::from_secs(20), context.withdraw(&mut client))
                    .await
                    .unwrap_or(Err(IdentityError::TimedOut)),
            )
        } else {
            None
        };
        (outcome, cleanup)
    });
    WorkResult {
        context: Some(context),
        outcome,
        cleanup,
    }
}
