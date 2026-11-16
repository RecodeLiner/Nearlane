use nearlane_core::Text;
use nearlane_platform::bluetooth::{BluetoothScan, ScanState, WakeAdvertisement, WakeState};
use nearlane_service::{AccountStatus, PrivateStatus, Service};
use std::time::{Duration, Instant};

pub(crate) struct Radio {
    scan: Option<BluetoothScan>,
    wake: Option<WakeAdvertisement>,
    scan_retry: Instant,
    wake_retry: Instant,
}
impl Default for Radio {
    fn default() -> Self {
        Self {
            scan: None,
            wake: None,
            scan_retry: Instant::now(),
            wake_retry: Instant::now(),
        }
    }
}
impl Radio {
    pub(crate) fn stop(&mut self) {
        self.scan = None;
        self.wake = None;
    }
    pub(crate) fn update(&mut self, service: &mut Service, live: bool, foreground: bool) -> Text {
        let enabled = live
            && service.continuous_discovery()
            && service.account_status() == AccountStatus::Authorized
            && matches!(service.private_status(), PrivateStatus::Ready(_));
        if !enabled {
            self.stop();
            return Text::new("Bluetooth discovery is off.");
        }
        if self.scan.is_none() && Instant::now() >= self.scan_retry {
            self.scan = BluetoothScan::start().ok();
            self.scan_retry = Instant::now() + Duration::from_secs(30);
        }
        let state = self
            .scan
            .as_ref()
            .map(BluetoothScan::state)
            .unwrap_or(ScanState::Unavailable);
        if let Some(scan) = &self.scan {
            for data in scan.drain() {
                service.observe_bluetooth(&data);
            }
        }
        if !foreground || service.transfer_busy() || state != ScanState::Scanning {
            self.wake = None;
        } else {
            if self
                .wake
                .as_ref()
                .is_some_and(|w| matches!(w.state(), WakeState::Unavailable | WakeState::Stopped))
            {
                self.wake = None;
                self.wake_retry = Instant::now() + Duration::from_secs(30);
            }
            if self.wake.is_none() && Instant::now() >= self.wake_retry {
                self.wake = WakeAdvertisement::start();
                self.wake_retry = Instant::now() + Duration::from_secs(30);
            }
        }
        if matches!(state, ScanState::Unavailable | ScanState::PoweredOff)
            && Instant::now() >= self.scan_retry
        {
            self.scan = None;
        }
        Text::new(match state {
            ScanState::Starting => "Allow Bluetooth access in the system dialog.",
            ScanState::Scanning
                if self
                    .wake
                    .as_ref()
                    .is_some_and(|w| w.state() == WakeState::Advertising) =>
            {
                "Bluetooth discovery and silent discovery signal are active."
            }
            ScanState::Scanning => {
                "Bluetooth discovery is active. Transfers need an available Wi-Fi endpoint."
            }
            ScanState::PoweredOff => "Bluetooth is turned off in system settings.",
            ScanState::MissingBundle => {
                "For Bluetooth on macOS, launch the installed Nearlane.app."
            }
            ScanState::Unavailable => "Bluetooth is unavailable. Wi-Fi discovery continues.",
        })
    }
}
