//! Synthetic mailbox tests: no socket, Google request or real account credential.
#![allow(clippy::unwrap_used)]
use super::*;

fn queued(result: WorkResult) -> PrivateSharing {
    let (sender, receiver) = mpsc::channel();
    sender.send(result).ok().unwrap();
    let json = format!(
        r#"{{"access_token":"synthetic-access","token_type":"Bearer","expires_in":60,"scope":"{}"}}"#,
        nearlane_auth::NEARBY_SCOPE
    );
    PrivateSharing {
        context: None,
        pending: Some(Pending {
            receiver,
            grant: Arc::new(TokenGrant::from_response(&json).unwrap()),
            cancelled: Arc::new(AtomicBool::new(false)),
            logout: Arc::new(AtomicBool::new(false)),
        }),
        status: PrivateStatus::Preparing,
    }
}

#[test]
fn sign_out_discards_late_success_and_requests_worker_cleanup() {
    let context = IdentityContext::new().unwrap();
    // No publication reached the network, so no second worker is needed.
    let mut private = queued(WorkResult {
        context: Some(context),
        outcome: Ok(CertificateSummary::default()),
        cleanup: None,
    });
    private.sign_out(None);
    assert!(
        private
            .pending
            .as_ref()
            .unwrap()
            .logout
            .load(Ordering::Acquire)
    );
    assert!(
        private
            .pending
            .as_ref()
            .unwrap()
            .cancelled
            .load(Ordering::Acquire)
    );
    private.poll(false);
    assert_eq!(private.status(), PrivateStatus::Idle);
    assert!(!private.busy());
    assert!(private.context.is_none());
}

#[test]
fn unconfirmed_cleanup_never_restores_local_keys_or_ready_status() {
    let mut context = IdentityContext::new().unwrap();
    context.publication_attempted = true;
    let mut private = queued(WorkResult {
        context: Some(context),
        outcome: Ok(CertificateSummary::default()),
        cleanup: Some(Err(IdentityError::TimedOut)),
    });
    private.sign_out(None);
    private.poll(false);
    assert_eq!(private.status(), PrivateStatus::CleanupUnconfirmed);
    assert!(private.context.is_none());
    assert!(!private.busy());
}

#[test]
fn failed_setup_retains_context_for_idempotent_retry_only_while_signed_in() {
    let mut private = queued(WorkResult {
        context: Some(IdentityContext::new().unwrap()),
        outcome: Err(IdentityError::RpcStatus(7)),
        cleanup: None,
    });
    private.poll(true);
    assert_eq!(
        private.status(),
        PrivateStatus::Failed(IdentityError::RpcStatus(7))
    );
    assert!(private.context.is_some());
    private.sign_out(None);
    assert!(private.context.is_none());
}

#[test]
fn lost_worker_reports_failure_without_enabling_private_mode() {
    let mut private = queued(WorkResult {
        context: None,
        outcome: Err(IdentityError::Worker),
        cleanup: None,
    });
    private
        .pending
        .as_ref()
        .unwrap()
        .receiver
        .try_recv()
        .ok()
        .unwrap();
    private.poll(true);
    assert_eq!(
        private.status(),
        PrivateStatus::Failed(IdentityError::Worker)
    );
    assert!(private.context.is_none());
}
