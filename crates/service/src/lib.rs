//! Application state shared by CLI and Qt, with explicit private-mode gating.
mod discovery;
use discovery::Discovery;
pub use discovery::DiscoveryStatus;
use nearlane_discovery::{DiscoveryError, Search};

use nearlane_account::Account;
pub use nearlane_account::{
    AccountStatus, BrowserAuthorization, IdentityError, LoginError, PrivateStatus,
};
use nearlane_core::{Capabilities, UnavailableVisibility, Visibility};

/// Initial implementation reports unsupported features honestly and stays hidden.
#[derive(Debug)]
pub struct Service {
    visibility: Visibility,
    capabilities: Capabilities,
    account: Account,
    private_setup_started: bool,
    discovery: Discovery,
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
            private_setup_started: false,
            discovery: Discovery::default(),
        }
    }
}

impl Service {
    /// Load application configuration when signed out and no operation is pending.
    pub fn load_account_config(&mut self, path: &std::path::Path) {
        if self.account.busy() || self.account.status() == AccountStatus::Authorized {
            return;
        }
        self.account = Account::from_local_config(path);
        self.discovery.reset();
        self.private_setup_started = false;
    }

    /// Start a browser login without changing discovery or sharing permissions.
    pub fn begin_sign_in(&mut self) -> Result<BrowserAuthorization, LoginError> {
        let url = self.account.begin()?;
        self.private_setup_started = false;
        Ok(url)
    }

    /// Cancel the current attempt and discard any late token response.
    pub fn cancel_sign_in(&mut self, reason: LoginError) {
        self.discovery.reset();
        self.account.cancel(reason);
    }

    /// Sign out locally and request withdrawal of this device's credentials.
    pub fn clear_account(&mut self) {
        self.sign_out();
    }

    /// Reset sharing state before discarding account credentials. Published
    /// Quick Share resets discovery/connections and trust data at sign-out;
    /// these adapters do not yet exist in this prototype. No public fallback.
    pub fn sign_out(&mut self) {
        self.discovery.reset();
        self.visibility = Visibility::Hidden;
        self.capabilities = Capabilities::default();
        self.account.sign_out();
        self.private_setup_started = false;
    }

    /// Nonblocking observation of login progress and session expiry.
    pub fn poll_account(&mut self) {
        self.account.poll();
        if self.account.status() == AccountStatus::Authorized {
            self.discovery.poll();
        } else {
            self.discovery.reset();
        }
        if self.account.status() == AccountStatus::Authorized && !self.private_setup_started {
            self.private_setup_started = true;
            let _ = self.account.prepare_own_devices();
        }
    }

    /// Retry the same owned registration without changing device visibility.
    pub fn prepare_own_devices(&mut self) -> Result<(), IdentityError> {
        self.discovery.reset();
        self.private_setup_started = true;
        self.account.prepare_own_devices()
    }

    /// Search only for advertisements matching the signed-in account's cache.
    /// This does not advertise, establish connections or enable file reception.
    pub fn find_own_devices(&mut self) {
        if self.discovery.search.is_some() || self.account.busy() {
            return;
        }
        self.discovery.reset();
        let result = if matches!(self.account.private_status(), PrivateStatus::Ready(_)) {
            Search::start(self.account.own_device_certificates())
        } else {
            Err(DiscoveryError::NoCertificates)
        };
        match result {
            Ok(search) => {
                self.discovery.search = Some(search);
                self.discovery.status = DiscoveryStatus::Searching;
            }
            Err(error) => self.discovery.status = DiscoveryStatus::Failed(error),
        }
    }

    /// Cancel discovery and forget candidates before any later search.
    pub fn cancel_discovery(&mut self) {
        self.discovery.reset();
    }
    /// Safe local-network search progress.
    pub fn discovery_status(&self) -> DiscoveryStatus {
        self.discovery.status
    }
    /// Authenticated candidate names only; no addresses, keys or tracking IDs.
    pub fn own_device_names(&self) -> Vec<&str> {
        self.discovery.devices.iter().map(|d| d.name()).collect()
    }

    /// Certificate server exchange status; it does not assert peer verification.
    pub fn private_status(&self) -> PrivateStatus {
        self.account.private_status()
    }

    /// Whether private setup/cleanup is currently pending.
    pub fn private_busy(&self) -> bool {
        self.account.private_busy()
    }

    /// Account authorization status, separate from device trust/capabilities.
    pub fn account_status(&self) -> AccountStatus {
        self.account.status()
    }

    /// Whether local application parameters are available for browser login.
    pub fn account_configured(&self) -> bool {
        self.account.configured()
    }

    /// Whether login, device preparation or sign-out cleanup is pending.
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
            "Platform: {:?}. Hidden. Nearby connection support is still being implemented.",
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
    fn discovery_without_own_credentials_does_not_start_or_enable_visibility() {
        let mut service = Service::default();
        service.find_own_devices();
        assert_eq!(
            service.discovery_status(),
            DiscoveryStatus::Failed(DiscoveryError::NoCertificates)
        );
        assert!(service.discovery.search.is_none());
        assert!(service.own_device_names().is_empty());
        assert_eq!(service.visibility(), Visibility::Hidden);
        service.sign_out();
        assert_eq!(service.discovery_status(), DiscoveryStatus::Idle);
        assert!(service.own_device_names().is_empty());
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
