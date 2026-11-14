use nearlane_discovery::{DiscoveryError, OwnDevice, Search};

/// Safe discovery state; finding a candidate does not enable private sharing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DiscoveryStatus {
    /// No active search; cached device names have been cleared.
    #[default]
    Idle,
    /// Explicit twenty-second local-network search is running.
    Searching,
    /// Number of advertisements matching trusted own-device certificates.
    Found(usize),
    /// Sanitized failure without raw network data or device identifiers.
    Failed(DiscoveryError),
}
impl DiscoveryStatus {
    /// Plain progress text; no account/device identity values.
    pub fn message(self) -> String {
        match self {
            Self::Idle => "Ready to search your local network after certificate setup.".into(),
            Self::Searching => "Looking for your own Quick Share devices for 20 seconds…".into(),
            Self::Found(0) => "No matching Wi-Fi LAN advertisements found. Check the same network and phone's Quick Share settings. Bluetooth discovery is not implemented yet.".into(),
            Self::Found(n) => format!("Found {n} own-device candidates. Their advertisement metadata is authenticated; live connection verification is still required."),
            Self::Failed(error) => error.to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Discovery {
    pub(crate) search: Option<Search>,
    pub(crate) devices: Vec<OwnDevice>,
    pub(crate) status: DiscoveryStatus,
}
impl Discovery {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
    pub(crate) fn poll(&mut self) {
        let Some(result) = self.search.as_ref().and_then(Search::poll) else {
            return;
        };
        self.search = None;
        match result {
            Ok(devices) => {
                self.status = DiscoveryStatus::Found(devices.len());
                self.devices = devices;
            }
            Err(error) => {
                self.status = DiscoveryStatus::Failed(error);
                self.devices.clear();
            }
        }
    }
}
