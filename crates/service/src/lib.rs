//! Application state shared by CLI and Qt, with explicit private-mode gating.

use nearlane_account::Account;
pub use nearlane_account::{AccountStatus, BrowserAuthorization, LoginError};
use nearlane_core::{Capabilities, UnavailableVisibility, Visibility};

/// Initial implementation reports unsupported features honestly and stays hidden.
#[derive(Debug)]
pub struct Service {
    visibility: Visibility,
    capabilities: Capabilities,
    account: Account,
}

impl Default for Service {
    fn default() -> Self {
        let path = std::env::var_os("NEARLANE_OAUTH_CONFIG")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("work/private/oauth-client.json"));
        Self {
            visibility: Visibility::Hidden,
            capabilities: Capabilities::default(),
            account: Account::from_local_config(&path),
        }
    }
}

impl Service {
    /// Load application configuration only, discarding any previous local session.
    pub fn load_account_config(&mut self, path: &std::path::Path) {
        self.account = Account::from_local_config(path);
    }

    /// Start a browser login without changing discovery or sharing permissions.
    pub fn begin_sign_in(&mut self) -> Result<BrowserAuthorization, LoginError> {
        self.account.begin()
    }

    /// Cancel the current attempt and discard any late token response.
    pub fn cancel_sign_in(&mut self, reason: LoginError) {
        self.account.cancel(reason);
    }

    /// Forget only the local in-memory session; provider grants are not revoked.
    pub fn clear_account(&mut self) {
        self.account.clear_session();
    }

    /// Reset sharing state before discarding account credentials. Published
    /// Quick Share resets discovery/connections and trust data at sign-out;
    /// these adapters do not yet exist in this prototype. No public fallback.
    pub fn sign_out(&mut self) {
        self.visibility = Visibility::Hidden;
        self.capabilities = Capabilities::default();
        self.account.sign_out();
    }

    /// Nonblocking observation of login progress and session expiry.
    pub fn poll_account(&mut self) {
        self.account.poll();
    }

    /// Account authorization status, separate from device trust/capabilities.
    pub fn account_status(&self) -> AccountStatus {
        self.account.status()
    }

    /// Whether local application parameters are available for browser login.
    pub fn account_configured(&self) -> bool {
        self.account.configured()
    }

    /// Whether a browser login or code exchange is pending.
    pub fn account_busy(&self) -> bool {
        self.account.busy()
    }

    /// Current effective policy; never a speculative UI selection.
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// The protocol backend is not connected yet, so all account features are off.
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    /// Change visibility only when the necessary trust has been established.
    pub fn set_visibility(&mut self, requested: Visibility) -> Result<(), UnavailableVisibility> {
        self.capabilities.require(requested)?;
        self.visibility = requested;
        Ok(())
    }

    /// A truthful status suitable for display without exposing private data.
    pub fn status(&self) -> String {
        format!(
            "Platform: {:?}. Hidden. Google certificate exchange is not verified.",
            nearlane_platform::current()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_selection_does_not_change_effective_visibility() {
        let mut service = Service::default();
        assert!(service.set_visibility(Visibility::Contacts).is_err());
        assert_eq!(service.visibility(), Visibility::Hidden);
    }

    #[test]
    fn sign_out_resets_trust_and_visibility_before_relogin() {
        let mut service = Service {
            capabilities: Capabilities {
                account_certificates: true,
                contacts: true,
            },
            ..Service::default()
        };
        service.set_visibility(Visibility::Contacts).unwrap();
        service.sign_out();
        assert_eq!(service.visibility(), Visibility::Hidden);
        assert_eq!(service.capabilities(), Capabilities::default());
        assert_eq!(service.account_status(), AccountStatus::SignedOut);
        assert!(service.set_visibility(Visibility::OwnDevices).is_err());
    }
}
