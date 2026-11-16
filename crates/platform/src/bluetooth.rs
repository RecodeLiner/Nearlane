//! Optional native wake advertisement; packet semantics are independent of OS.
#[cfg(target_os = "linux")]
mod linux;
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};

/// State of this process's advertisement, never proof that a phone was found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeState {
    /// Registration is pending.
    Starting,
    /// The OS accepted the local advertisement.
    Advertising,
    /// This bounded lease ended or was withdrawn.
    Stopped,
    /// The radio, permission or advertising slot is unavailable.
    Unavailable,
}
/// A silent Nearby initiation packet. No device name, account key or notify bit.
/// Layout facts: Google's public FastInitBleBeacon API, revision 770a030b.
pub fn silent_initiation_data() -> [u8; 24] {
    let mut data = [0; 24];
    data[..4].copy_from_slice(&[0xfc, 0x12, 0x8e, 0x04]);
    // Google's FastInit V1 final field: require_bt_advertising (MSB), then
    // self_only_advertising and six reserved bits. Ask the phone to publish its
    // connection endpoint while keeping silent type and no certificate claim.
    data[23] = 0x80;
    data
}
/// Whether this adapter can emit the required service data. CoreBluetooth scan
/// support is provided separately by the desktop's portable Qt Bluetooth layer.
pub const fn wake_supported() -> bool {
    cfg!(target_os = "linux")
}
/// An owned, cancellable ten-minute registration. Drop withdraws only this lease.
pub struct WakeAdvertisement {
    stop: Arc<AtomicBool>,
    state: Arc<AtomicU8>,
}
impl fmt::Debug for WakeAdvertisement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WakeAdvertisement")
            .field("state", &self.state())
            .finish()
    }
}
impl Drop for WakeAdvertisement {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl WakeAdvertisement {
    /// Start only for an authorized foreground send surface. Does not power on
    /// Bluetooth or change pairing, global discoverability or sharing visibility.
    pub fn start() -> Option<Self> {
        if !wake_supported() {
            return None;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(AtomicU8::new(0));
        #[cfg(target_os = "linux")]
        {
            let cancel = Arc::clone(&stop);
            let status = Arc::clone(&state);
            std::thread::Builder::new()
                .name("nearlane-ble-wake".into())
                .spawn(move || {
                    let result = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .ok()
                        .map(|rt| rt.block_on(linux::run(cancel, Arc::clone(&status))));
                    status.store(
                        if matches!(result, Some(Ok(()))) { 2 } else { 3 },
                        Ordering::Release,
                    );
                })
                .ok()?;
        }
        Some(Self { stop, state })
    }
    /// Registration status without adapter addresses or raw OS error strings.
    pub fn state(&self) -> WakeState {
        match self.state.load(Ordering::Acquire) {
            0 => WakeState::Starting,
            1 => WakeState::Advertising,
            2 => WakeState::Stopped,
            _ => WakeState::Unavailable,
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn wake_packet_is_silent_and_does_not_claim_a_certificate_or_uwb_support() {
        let data = super::silent_initiation_data();
        assert_eq!(&data[..3], &[0xfc, 0x12, 0x8e]);
        assert_eq!(data[3], 4);
        assert!(data[4..23].iter().all(|b| *b == 0));
        assert_eq!(data[23], 0x80);
        assert_eq!(data.len() + 4, 28); // AD header and UUID fit legacy 31-byte budget.
    }
}
