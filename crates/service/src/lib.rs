//! Application state for the CLI and desktop, with explicit private-mode gating.
use nearlane_core::Text;
mod certificates;
mod discovery;
mod receiving;
mod transfers;
pub use certificates::CertificateRow;
use discovery::Discovery;
pub use discovery::DiscoveryStatus;
use nearlane_discovery::{DiscoveryError, Search};
use transfers::Transfers;

use nearlane_account::Account;
pub use nearlane_account::{
    AccountStatus, BrowserAuthorization, IdentityError, LoginError, PrivateStatus, SavedLoginStatus,
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
    transfers: Transfers,
    receiving: receiving::Receiving,
    continuous_discovery: bool,
    discovery_retry: std::time::Instant,
    receive_on_ready: bool,
    confirm_each_file: bool,
    pending_send: Option<(nearlane_discovery::OwnDevice, std::time::Instant)>,
}

impl Default for Service {
    fn default() -> Self {
        Self::from_config(nearlane_platform::configuration::oauth_config_path().as_deref())
    }
}

impl Service {
    /// Construct offline with an explicit configuration path. `None` creates
    /// an unconfigured service without inspecting a personal runtime profile.
    pub fn from_config(path: Option<&std::path::Path>) -> Self {
        let account = path
            .map(Account::from_local_config)
            .unwrap_or_else(Account::unconfigured);
        Self {
            visibility: Visibility::Hidden,
            capabilities: Capabilities::default(),
            account,
            private_setup_started: false,
            discovery: Discovery::default(),
            transfers: Transfers::default(),
            receiving: receiving::Receiving::default(),
            continuous_discovery: false,
            discovery_retry: std::time::Instant::now(),
            receive_on_ready: false,
            confirm_each_file: true,
            pending_send: None,
        }
    }
}

impl Service {
    /// Desktop startup/retry only; the default service constructor stays offline.
    pub fn restore_saved_sign_in(&mut self) -> Result<(), LoginError> {
        self.account.restore_saved_sign_in()
    }
    /// Safe system-vault status for UI display.
    pub fn saved_login_status(&self) -> SavedLoginStatus {
        self.account.saved_login_status()
    }
    /// Whether sign-out must also remove a possibly saved offline grant.
    pub fn has_saved_login(&self) -> bool {
        self.account.has_saved_login()
    }
    /// Load application configuration when signed out and no operation is pending.
    pub fn load_account_config(&mut self, path: &std::path::Path) {
        if self.account.busy() || self.account.status() == AccountStatus::Authorized {
            return;
        }
        self.account = Account::from_local_config(path);
        self.pending_send = None;
        self.transfers.reset();
        self.receiving.reset();
        self.discovery.reset();
        self.private_setup_started = false;
    }

    /// Persist an explicitly chosen JSON in the runtime configuration directory.
    /// Existing trust remains untouched if parsing or persistence fails.
    pub fn import_account_config(&mut self, source: &std::path::Path) -> Result<(), LoginError> {
        if self.account.busy() || self.account.status() == AccountStatus::Authorized {
            return Err(LoginError::Busy);
        }
        let destination = nearlane_platform::configuration::oauth_config_path()
            .ok_or(LoginError::ConfigurationStorage)?;
        self.import_config_to(source, &destination)
    }
    /// Import to an explicit runtime path owned by the application controller.
    /// This is also the isolated-test boundary; no implicit profile fallback.
    pub fn import_config_to(
        &mut self,
        source: &std::path::Path,
        destination: &std::path::Path,
    ) -> Result<(), LoginError> {
        if self.account.busy() || self.account.status() == AccountStatus::Authorized {
            return Err(LoginError::Busy);
        }
        let config = nearlane_account::ClientConfig::import(source, destination)?;
        self.account = Account::from_client_config(config);
        self.pending_send = None;
        self.transfers.reset();
        self.receiving.reset();
        self.discovery.reset();
        self.private_setup_started = false;
        Ok(())
    }

    /// Start a browser login without changing discovery or sharing permissions.
    pub fn begin_sign_in(&mut self) -> Result<BrowserAuthorization, LoginError> {
        let url = self.account.begin()?;
        self.private_setup_started = false;
        Ok(url)
    }

    /// Cancel the current attempt and discard any late token response.
    pub fn cancel_sign_in(&mut self, reason: LoginError) {
        self.pending_send = None;
        self.transfers.reset();
        self.receiving.reset();
        self.visibility = Visibility::Hidden;
        self.capabilities = Capabilities::default();
        self.discovery.reset();
        self.account.cancel(reason);
    }

    /// Sign out locally and request withdrawal of this device's credentials.
    pub fn clear_account(&mut self) {
        self.sign_out();
    }

    /// Reset sharing state before discarding account credentials. Published
    /// Quick Share resets discovery/connections and trust data at sign-out;
    /// remembered local preferences never keep workers or trust alive.
    pub fn sign_out(&mut self) {
        self.pending_send = None;
        self.transfers.reset();
        self.receiving.reset();
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
            self.transfers.poll();
            self.receiving.poll(self.confirm_each_file);
        } else {
            self.pending_send = None;
            self.transfers.reset();
            self.discovery.reset();
            self.receiving.reset();
        }
        let advertised = self
            .receiving
            .job
            .as_ref()
            .is_some_and(|j| j.phase() != nearlane_transfer::ReceivePhase::Starting);
        self.visibility = if advertised {
            Visibility::OwnDevices
        } else {
            Visibility::Hidden
        };
        self.capabilities.account_certificates = advertised;
        if self.account.status() == AccountStatus::Authorized && !self.private_setup_started {
            self.private_setup_started = true;
            let _ = self.account.prepare_own_devices();
        }
        if self.continuous_discovery
            && self.account.status() == AccountStatus::Authorized
            && matches!(self.account.private_status(), PrivateStatus::Ready(_))
            && self.discovery.watch.is_none()
            && self.discovery.search.is_none()
            && std::time::Instant::now() >= self.discovery_retry
        {
            self.discovery_retry = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let certificates = self
                .account
                .own_device_certificates()
                .into_iter()
                .filter(|c| !self.account.is_local_certificate(c.id()))
                .collect();
            match nearlane_discovery::Watch::start(certificates) {
                Ok(watch) => self.discovery.watch = Some(watch),
                Err(error) => self.discovery.status = DiscoveryStatus::Failed(error),
            }
        }
        if let Some((peer, deadline)) = self.pending_send.clone() {
            if std::time::Instant::now() >= deadline {
                self.pending_send = None;
                self.transfers.message =
                    "The private receiver did not stop in time. Retry sending.".into();
            } else {
                self.launch_sender(peer);
            }
        }
        if (self.receive_on_ready || self.receiving.enabled)
            && !self.receiving.failed
            && self.receiving.job.is_none()
            && std::time::Instant::now() >= self.receiving.restart
            && self.account.status() == AccountStatus::Authorized
            && !self.account.busy()
            && matches!(self.account.private_status(), PrivateStatus::Ready(_))
            && !self.transfer_busy()
        {
            self.launch_receiver();
        }
    }

    /// Retry the same owned registration without changing device visibility.
    pub fn prepare_own_devices(&mut self) -> Result<(), IdentityError> {
        if self.transfer_busy() || self.receiving_active() {
            return Err(IdentityError::Busy);
        }
        self.discovery.reset();
        self.private_setup_started = true;
        self.account.prepare_own_devices()
    }

    /// Search only for advertisements matching the signed-in account's cache.
    /// This does not advertise, establish connections or enable file reception.
    pub fn find_own_devices(&mut self) {
        if self.discovery.search.is_some() || self.account.busy() || self.transfer_busy() {
            return;
        }
        self.discovery.reset();
        let result = if matches!(self.account.private_status(), PrivateStatus::Ready(_)) {
            Search::start(
                self.account
                    .own_device_certificates()
                    .into_iter()
                    .filter(|c| !self.account.is_local_certificate(c.id()))
                    .collect(),
            )
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
        self.continuous_discovery = false;
        self.discovery.reset();
    }
    /// Desktop background discovery; constructing the service/CLI stays offline.
    pub fn set_continuous_discovery(&mut self, enabled: bool) {
        self.continuous_discovery = enabled;
        if !enabled {
            self.discovery.reset();
        }
        self.discovery_retry = std::time::Instant::now();
    }
    /// Whether ongoing discovery was requested, independent of current login.
    pub fn continuous_discovery(&self) -> bool {
        self.continuous_discovery
    }
    /// Safe local-network search progress.
    pub fn discovery_status(&self) -> DiscoveryStatus {
        self.discovery.status
    }
    /// Authenticated candidate names only; no addresses, keys or tracking IDs.
    pub fn own_device_names(&self) -> Vec<&str> {
        self.discovery.devices.iter().map(|d| d.name()).collect()
    }

    /// Equality-only stamp for a displayed candidate list. It is not a peer
    /// identifier or authorization token. The application converts changes to
    /// a process-local generation before exposing a list to a frontend.
    pub fn device_list_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut stamp = std::collections::hash_map::DefaultHasher::new();
        for device in &self.discovery.devices {
            device.name().hash(&mut stamp);
            device.socket_address().hash(&mut stamp);
            device.certificate().id().hash(&mut stamp);
        }
        stamp.finish()
    }
    /// Bounded BLE hint input from an OS adapter. It never creates a routable
    /// transfer target; Wi-Fi discovery and live proofs are still required.
    pub fn observe_bluetooth(&mut self, data: &[u8]) {
        if !self.continuous_discovery
            || self.account.status() != AccountStatus::Authorized
            || !matches!(self.account.private_status(), PrivateStatus::Ready(_))
        {
            return;
        }
        let certificates = self
            .account
            .own_device_certificates()
            .into_iter()
            .filter(|c| !self.account.is_local_certificate(c.id()))
            .collect::<Vec<_>>();
        if let Some((certificate, name)) = nearlane_discovery::match_bluetooth(data, &certificates)
        {
            self.discovery
                .bluetooth
                .retain(|d| d.seen.elapsed() < std::time::Duration::from_secs(120));
            if let Some(old) = self
                .discovery
                .bluetooth
                .iter_mut()
                .find(|d| d.certificate.id() == certificate.id())
            {
                old.seen = std::time::Instant::now();
            } else if self.discovery.bluetooth.len() < 32 {
                self.discovery.bluetooth.push(discovery::BleObservation {
                    certificate,
                    name,
                    seen: std::time::Instant::now(),
                });
            }
        }
    }
    /// Verified own-device discovery names still waiting for a Wi-Fi endpoint.
    pub fn bluetooth_device_names(&self) -> Vec<&str> {
        if self.account.status() != AccountStatus::Authorized {
            return Vec::new();
        }
        self.discovery
            .bluetooth
            .iter()
            .filter(|d| {
                d.seen.elapsed() < std::time::Duration::from_secs(120)
                    && !self
                        .discovery
                        .devices
                        .iter()
                        .any(|p| p.certificate().id() == d.certificate.id())
            })
            .map(|d| d.name.as_str())
            .collect()
    }

    /// Remember only the explicitly selected local path; opening and validation
    /// happen on the transfer worker after an explicit recipient selection.
    pub fn select_send_file(&mut self, path: std::path::PathBuf) {
        if !self.transfer_busy() {
            self.transfers.source = Some(path);
        }
    }
    /// Safe selected basename for plain-text UI; no parent path is exposed.
    pub fn selected_send_name(&self) -> String {
        self.transfers
            .source
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
    /// Explicit outgoing action. The selected candidate must match account
    /// certificates; its live paired-key proof gates all offered file content.
    pub fn send_selected_file(&mut self, index: usize) {
        if self.transfer_busy() || self.account.busy() {
            return;
        }
        let Some(peer) = self.discovery.devices.get(index).cloned() else {
            self.transfers.message = "Find and select an own device first.".into();
            return;
        };
        if self.transfers.source.is_none() {
            self.transfers.message = "Choose a file first.".into();
            return;
        }
        if self.receiving.job.as_ref().is_some_and(|j| {
            !matches!(
                j.phase(),
                nearlane_transfer::ReceivePhase::Starting
                    | nearlane_transfer::ReceivePhase::Listening
            )
        }) {
            self.transfers.message = "Finish the incoming transfer before sending a file.".into();
            return;
        }
        // Cancel the idle listener without changing the saved receiving intent.
        // Its worker releases the signing handle asynchronously; poll retries
        // within a fixed deadline rather than blocking the GUI thread.
        self.receiving.stop();
        self.pending_send = Some((
            peer.clone(),
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        ));
        self.transfers.message = "Preparing to send from the background receiver…".into();
        self.launch_sender(peer);
    }
    fn launch_sender(&mut self, peer: nearlane_discovery::OwnDevice) {
        self.transfers.completed = false;
        let Some(path) = self.transfers.source.clone() else {
            self.pending_send = None;
            self.transfers.message = "Choose a file first.".into();
            return;
        };
        let identity = match self.account.transfer_identity() {
            Ok(identity) => identity,
            Err(IdentityError::Busy) => return,
            Err(_) => {
                self.pending_send = None;
                self.transfers.message =
                    "Current private device keys are unavailable or busy. Retry device setup."
                        .into();
                return;
            }
        };
        self.pending_send = None;
        match nearlane_transfer::SendJob::start(peer, identity, path) {
            Ok(job) => {
                self.transfers.message = job.progress().text();
                self.transfers.job = Some(job);
            }
            Err(error) => self.transfers.message = error.text(),
        }
    }
    /// Stop IO and discard late transfer results without changing visibility.
    pub fn cancel_transfer(&mut self) {
        self.pending_send = None;
        self.transfers.cancel();
    }
    /// Whether one outgoing worker is owned by this service.
    pub fn transfer_busy(&self) -> bool {
        self.transfers.job.is_some() || self.pending_send.is_some()
    }
    /// Whether the most recent attempt received the matching file ACK.
    pub fn transfer_completed(&self) -> bool {
        self.transfers.completed
    }
    /// Select the local destination through the local folder chooser.
    pub fn select_receive_directory(&mut self, path: std::path::PathBuf) {
        if !self.receiving_active() {
            self.receiving.directory = Some(path);
        }
    }
    /// Explicitly selected local folder only, never a remotely supplied destination.
    pub fn receive_directory(&self) -> String {
        self.receiving
            .directory
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
    /// Set desktop receiving policy. Construction and unauthenticated sessions
    /// stay offline; automatic approval still requires live own-device proofs.
    pub fn set_receiving_preferences(&mut self, enabled: bool, confirm_each_file: bool) {
        self.receive_on_ready = enabled;
        self.confirm_each_file = confirm_each_file;
        self.receiving.failed = false;
        if !enabled {
            self.stop_receiving();
        }
    }
    /// Whether private receiving should resume after login or an outgoing file.
    pub fn background_receiving(&self) -> bool {
        self.receive_on_ready
    }
    /// Whether verified own devices need an explicit decision for each file.
    pub fn confirm_each_file(&self) -> bool {
        self.confirm_each_file
    }
    /// Enable private reception now and after the next successful setup.
    pub fn start_receiving(&mut self) {
        self.receive_on_ready = true;
        self.receiving.failed = false;
        if self.receiving_active() || self.transfer_busy() || self.account.busy() {
            return;
        }
        self.receiving.enabled = true;
        self.launch_receiver();
    }
    fn launch_receiver(&mut self) {
        let Some(generation) = self.receiving.generation.checked_add(1) else {
            self.receiving.enabled = false;
            self.receiving.failed = true;
            self.receiving.message = "Restart the application before receiving again.".into();
            return;
        };
        self.receiving.saved = false;
        let Some(directory) = self.receiving.directory.clone() else {
            self.receiving.message = "Choose a destination folder first.".into();
            self.receiving.enabled = false;
            self.receiving.failed = true;
            return;
        };
        let identity = match self.account.transfer_identity() {
            Ok(i) => i,
            Err(IdentityError::Busy) => {
                self.receiving.restart =
                    std::time::Instant::now() + std::time::Duration::from_secs(1);
                return;
            }
            Err(_) => {
                self.receiving.enabled = false;
                self.receiving.failed = true;
                self.receiving.message = "Prepare current own-device keys before receiving.".into();
                return;
            }
        };
        match nearlane_transfer::ReceiveJob::start(
            identity,
            self.account.own_device_certificates(),
            directory,
            nearlane_transfer::RECEIVE_PORT,
        ) {
            Ok(job) => {
                self.receiving.enabled = true;
                self.receiving.decision_sent = false;
                self.receiving.generation = generation;
                self.receiving.message = job.phase().text();
                self.receiving.job = Some(job);
            }
            Err(error) => {
                self.receiving.enabled = false;
                self.receiving.failed = true;
                self.receiving.message = error.text();
            }
        }
    }
    /// Withdraw the advertisement, interrupt established IO and discard offers.
    pub fn stop_receiving(&mut self) {
        self.receive_on_ready = false;
        self.receiving.stop();
        self.visibility = Visibility::Hidden;
        self.capabilities = Capabilities::default();
    }
    /// Whether the explicitly enabled receiver owns its worker/listener.
    pub fn receiving_active(&self) -> bool {
        self.receiving.enabled || self.receiving.job.is_some()
    }
    /// Safe receiver phase or saved-file outcome.
    pub fn receive_status(&self) -> String {
        self.receiving.message.to_string()
    }
    /// Deferred reception status, preserving translation parameters.
    pub fn receive_text(&self) -> &Text {
        &self.receiving.message
    }
    /// Metadata of an own device that passed live mutual private proofs.
    pub fn incoming_offer(&self) -> Option<nearlane_transfer::IncomingOffer> {
        self.receiving.offer()
    }
    /// First local consent decision applies only to the current verified offer.
    pub fn decide_incoming(&mut self, accept: bool) {
        if let Some(id) = self.incoming_request_id() {
            let _ = self.decide_incoming_for(id, accept);
        }
    }
    /// Monotonic current-offer identity for native notifications. It is not a
    /// peer or account identifier and survives account reset to prevent reuse.
    pub fn incoming_request_id(&self) -> Option<u64> {
        self.receiving.offer().map(|_| self.receiving.generation)
    }
    /// Apply a notification action only to the still-current verified offer.
    pub fn decide_incoming_for(&mut self, id: u64, accept: bool) -> bool {
        let applied = self.incoming_request_id() == Some(id)
            && self
                .receiving
                .job
                .as_ref()
                .is_some_and(|j| j.decide(accept));
        if applied {
            self.receiving.decision_sent = true;
        }
        applied
    }
    /// Whether the latest receiving session durably saved a complete file.
    pub fn received_file_saved(&self) -> bool {
        self.receiving.saved
    }
    /// Completed incoming files during this app run, for one notification each.
    pub fn received_count(&self) -> u32 {
        self.receiving.count
    }
    /// Plain phase/status without paths, account identifiers or keys.
    pub fn transfer_status(&self) -> String {
        self.transfers.message.to_string()
    }
    /// Deferred sending status, preserving translation parameters.
    pub fn transfer_text(&self) -> &Text {
        &self.transfers.message
    }

    /// List certificate validity and only names established locally or by
    /// authenticated discovery. Unknown encrypted metadata is not guessed.
    pub fn certificate_rows(&self) -> Vec<CertificateRow> {
        let Ok(now) = nearlane_certificates::now_seconds() else {
            return Vec::new();
        };
        self.account
            .own_device_certificates()
            .iter()
            .enumerate()
            .map(|(index, cert)| {
                let device = if self.account.is_local_certificate(cert.id()) {
                    Some("This computer (Nearlane)".into())
                } else {
                    self.discovery
                        .devices
                        .iter()
                        .find(|d| d.certificate().id() == cert.id())
                        .map(|d| d.name().to_owned())
                };
                CertificateRow {
                    number: index + 1,
                    local: self.account.is_local_certificate(cert.id()),
                    device,
                    starts_at: cert.starts_at(),
                    expires_at: cert.expires_at(),
                    active: cert.valid_at(now),
                }
            })
            .collect()
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
        if requested == Visibility::Hidden {
            self.stop_receiving();
            return Ok(());
        }
        self.capabilities.require(requested)?;
        self.visibility = requested;
        Ok(())
    }

    /// A truthful status suitable for display without exposing private data.
    pub fn status(&self) -> String {
        self.status_text().to_string()
    }
    /// Deferred platform/visibility status for localized presentation.
    pub fn status_text(&self) -> Text {
        Text::with_args(
            "Platform: %1. %2 Experimental private file transfers over Wi-Fi LAN.",
            [
                Text::literal(format!("{:?}", nearlane_platform::current())),
                Text::new(if self.receiving_active() {
                    "Private receiver is enabled."
                } else {
                    "Hidden."
                }),
            ],
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn configuration_import_is_persistent_without_enabling_visibility_or_losing_a_valid_config_on_error()
     {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected.json");
        let destination = root.path().join("private/nearlane/oauth-client.json");
        std::fs::write(&source, br#"{"client_id":"synthetic-import"}"#).unwrap();
        let mut service = Service::default();
        service.import_config_to(&source, &destination).unwrap();
        assert!(service.account_configured());
        assert_eq!(service.account_status(), AccountStatus::Ready);
        assert_eq!(service.visibility(), Visibility::Hidden);
        let saved = std::fs::read(&destination).unwrap();
        std::fs::write(&source, b"{}").unwrap();
        assert_eq!(
            service.import_config_to(&source, &destination),
            Err(LoginError::Configuration)
        );
        assert!(service.account_configured());
        assert_eq!(service.account_status(), AccountStatus::Ready);
        assert_eq!(std::fs::read(&destination).unwrap(), saved);
        assert_eq!(service.visibility(), Visibility::Hidden);
    }
    #[test]
    fn background_receive_policy_survives_logout_but_never_opens_without_identity() {
        let mut service = Service::default();
        assert!(!service.background_receiving());
        assert!(service.confirm_each_file());
        service.select_receive_directory("synthetic-not-opened".into());
        service.set_receiving_preferences(true, false);
        for _ in 0..3 {
            service.poll_account();
        }
        assert!(service.background_receiving());
        assert!(!service.confirm_each_file());
        assert!(!service.receiving_active());
        assert!(service.receiving.job.is_none());
        assert_eq!(service.visibility(), Visibility::Hidden);
        service.sign_out();
        service.poll_account();
        assert!(service.background_receiving());
        assert!(!service.confirm_each_file());
        assert!(!service.decide_incoming_for(0, true));
        service.set_receiving_preferences(false, true);
        service.poll_account();
        assert!(!service.background_receiving());
        assert!(service.confirm_each_file());
        assert!(!service.receiving_active());
    }
    #[test]
    fn unauthenticated_bluetooth_input_cannot_create_a_transfer_target() {
        let mut service = Service::default();
        service.set_continuous_discovery(true);
        service.observe_bluetooth(&[0xff; 512]);
        assert!(service.bluetooth_device_names().is_empty());
        assert!(service.own_device_names().is_empty());
        assert_eq!(service.visibility(), Visibility::Hidden);
        service.sign_out();
        assert!(service.bluetooth_device_names().is_empty());
    }
    #[test]
    fn continuous_discovery_waits_for_login_and_can_be_stopped_without_a_network() {
        let mut service = Service::default();
        service.set_continuous_discovery(true);
        service.poll_account();
        assert!(service.continuous_discovery());
        assert!(service.discovery.watch.is_none());
        assert_eq!(service.visibility(), Visibility::Hidden);
        service.cancel_discovery();
        assert!(!service.continuous_discovery());
        assert!(service.own_device_names().is_empty());
    }
    #[test]
    fn unauthenticated_receiving_does_not_bind_and_logout_keeps_only_folder_preference() {
        let mut service = Service::default();
        service.select_receive_directory(std::path::PathBuf::from("synthetic-not-opened"));
        service.start_receiving();
        assert!(!service.receiving_active());
        assert!(service.incoming_offer().is_none());
        assert!(!service.decide_incoming_for(0, true));
        assert!(!service.decide_incoming_for(u64::MAX, false));
        assert_eq!(service.visibility(), Visibility::Hidden);
        service.sign_out();
        assert_eq!(service.receive_directory(), "synthetic-not-opened");
        assert!(!service.receiving_active());
    }
    #[test]
    fn unauthenticated_file_action_does_not_connect_and_sign_out_forgets_selection() {
        let mut service = Service::default();
        service.select_send_file(std::path::PathBuf::from("synthetic-not-opened.bin"));
        service.send_selected_file(0);
        assert!(!service.transfer_busy());
        assert_eq!(service.visibility(), Visibility::Hidden);
        service.sign_out();
        assert!(service.selected_send_name().is_empty());
    }
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
