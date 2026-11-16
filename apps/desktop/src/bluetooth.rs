//! Portable Qt radio adapter; core policy never depends on BlueZ or Qt types.
use crate::backend::ffi;
use nearlane_core::Text;
use nearlane_platform::bluetooth::{WakeAdvertisement, WakeState};
use nearlane_service::{AccountStatus, PrivateStatus, Service};
use std::time::{Duration, Instant};

pub(super) struct Discovery {
    native: Option<cxx::UniquePtr<ffi::BluetoothDiscovery>>,
    wake: Option<WakeAdvertisement>,
    retry: Instant,
}
impl Default for Discovery {
    fn default() -> Self {
        Self {
            native: None,
            wake: None,
            retry: Instant::now(),
        }
    }
}
impl Discovery {
    pub(super) fn stop(&mut self) {
        self.wake = None;
        if let Some(native) = self.native.as_mut().and_then(|n| n.as_mut()) {
            native.set_scanning(false);
        }
    }
    pub(super) fn update(
        &mut self,
        service: &mut Service,
        enabled: bool,
        foreground: bool,
    ) -> Text {
        let enabled = enabled
            && service.continuous_discovery()
            && service.account_status() == AccountStatus::Authorized
            && matches!(service.private_status(), PrivateStatus::Ready(_));
        if enabled && self.native.is_none() {
            self.native = Some(ffi::create_bluetooth_discovery());
        }
        let state = if let Some(mut native) = self.native.as_mut().and_then(|n| n.as_mut()) {
            native.as_mut().set_scanning(enabled);
            for value in native.as_mut().drain_advertisements().iter().take(64) {
                if let Some(bytes) = decode(&value.to_string()) {
                    service.observe_bluetooth(&bytes);
                }
            }
            native.scan_status()
        } else {
            0
        };
        if !enabled || !foreground || service.transfer_busy() || state != 2 {
            self.wake = None;
            self.retry = Instant::now();
        } else {
            if self
                .wake
                .as_ref()
                .is_some_and(|w| matches!(w.state(), WakeState::Unavailable | WakeState::Stopped))
            {
                self.wake = None;
                self.retry = Instant::now() + Duration::from_secs(30);
            }
            if self.wake.is_none() && Instant::now() >= self.retry {
                self.wake = WakeAdvertisement::start();
                self.retry = Instant::now() + Duration::from_secs(30);
            }
        }
        Text::new(match state {
            0 => "Bluetooth discovery is off.",
            1 => "Allow Bluetooth access in the system dialog.",
            2 if self
                .wake
                .as_ref()
                .is_some_and(|w| w.state() == WakeState::Advertising) =>
            {
                "Bluetooth discovery and silent discovery signal are active."
            }
            2 => "Bluetooth discovery is active. Transfers need an available Wi-Fi endpoint.",
            3 => "Bluetooth is turned off in system settings.",
            4 => "Bluetooth permission is denied. Check system settings.",
            6 => "For Bluetooth on macOS, launch the installed Nearlane.app.",
            _ => "Bluetooth is unavailable. Wi-Fi discovery continues.",
        })
    }
}
fn decode(value: &str) -> Option<Vec<u8>> {
    if !matches!(value.len(), 50 | 54) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|b| {
            let high = (b[0] as char).to_digit(16)?;
            let low = (b[1] as char).to_digit(16)?;
            Some(((high << 4) | low) as u8)
        })
        .collect()
}
