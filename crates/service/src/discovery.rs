use nearlane_core::Text;
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
        self.text().to_string()
    }

    /// Deferred text for a localized presentation layer.
    pub fn text(self) -> Text {
        match self {
            Self::Idle => Text::new("Ready to search your local network after certificate setup."),
            Self::Searching => {
                Text::new("Looking for your own Quick Share devices for 20 seconds…")
            }
            Self::Found(0) => Text::new(
                "No matching Wi-Fi LAN endpoints found. Check the same network and phone's private Quick Share settings; a Bluetooth hint alone is not a file-transfer endpoint.",
            ),
            Self::Found(n) => Text::with_args(
                "Found %1 own-device candidates. Their advertisement metadata is authenticated; live connection verification is still required.",
                [Text::literal(n)],
            ),
            Self::Failed(error) => error.text(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Discovery {
    pub(crate) search: Option<Search>,
    pub(crate) watch: Option<nearlane_discovery::Watch>,
    pub(crate) devices: Vec<OwnDevice>,
    pub(crate) status: DiscoveryStatus,
    pub(crate) bluetooth: Vec<BleObservation>,
}
pub(crate) struct BleObservation {
    pub(crate) certificate: std::sync::Arc<nearlane_certificates::RemoteCertificate>,
    pub(crate) name: String,
    pub(crate) seen: std::time::Instant,
}
impl std::fmt::Debug for BleObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BleObservation([REDACTED])")
    }
}
impl Discovery {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
    pub(crate) fn poll(&mut self) {
        if let Some(watch) = &self.watch {
            match watch.snapshot() {
                Ok(devices) => {
                    self.status = DiscoveryStatus::Found(devices.len());
                    self.devices = devices;
                }
                Err(error) => {
                    self.status = DiscoveryStatus::Failed(error);
                    self.watch = None;
                    self.devices.clear();
                }
            }
            return;
        }
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
