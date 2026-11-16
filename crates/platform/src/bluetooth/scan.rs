//! Portable Nearby advertisement observation. Never pairs, connects, powers a radio
//! on, or treats a local device name as an authenticated identity.
use base64::{Engine as _, engine::general_purpose::URL_SAFE};
use btleplug::{
    api::{Central, CentralEvent, CentralState, Manager as _, Peripheral, ScanFilter},
    platform::Manager,
};
use futures_util::StreamExt;
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

const SERVICE: Uuid = Uuid::from_u128(0x0000fef3_0000_1000_8000_00805f9b34fb);

/// Safe radio status without native error strings or identifying addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanState {
    /// Native initialization is pending.
    Starting,
    /// The adapter accepted the scan request.
    Scanning,
    /// Bluetooth is disabled in OS settings.
    PoweredOff,
    /// Radio or permission unavailable.
    Unavailable,
    /// macOS requires a packaged application with a Bluetooth usage description.
    MissingBundle,
}
/// One cancellable scan with a bounded queue of protocol-sized packets.
pub struct BluetoothScan {
    stop: Arc<AtomicBool>,
    state: Arc<AtomicU8>,
    packets: mpsc::Receiver<Vec<u8>>,
}
impl Drop for BluetoothScan {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl BluetoothScan {
    /// Start only after application-level authentication and private-mode gating.
    pub fn start() -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(AtomicU8::new(0));
        let (tx, packets) = mpsc::sync_channel(64);
        let cancel = Arc::clone(&stop);
        let status = Arc::clone(&state);
        std::thread::Builder::new()
            .name("nearlane-ble-scan".into())
            .spawn(move || {
                #[cfg(target_os = "macos")]
                if !has_bluetooth_bundle() {
                    status.store(4, Ordering::Release);
                    return;
                }
                let cancelled = Arc::clone(&cancel);
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok()
                    .map(|rt| rt.block_on(scan(cancel, Arc::clone(&status), tx)));
                if (!matches!(result, Some(Ok(()))) || !cancelled.load(Ordering::Acquire))
                    && status.load(Ordering::Acquire) != 2
                {
                    status.store(3, Ordering::Release);
                }
            })?;
        Ok(Self {
            stop,
            state,
            packets,
        })
    }
    /// Nonblocking sanitized status.
    pub fn state(&self) -> ScanState {
        match self.state.load(Ordering::Acquire) {
            0 => ScanState::Starting,
            1 => ScanState::Scanning,
            2 => ScanState::PoweredOff,
            4 => ScanState::MissingBundle,
            _ => ScanState::Unavailable,
        }
    }
    /// At most 64 packets per poll; callers still verify account certificates.
    pub fn drain(&self) -> Vec<Vec<u8>> {
        self.packets.try_iter().take(64).collect()
    }
}
#[cfg(target_os = "macos")]
fn has_bluetooth_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent()?.parent().map(|p| p.join("Info.plist")))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|info| {
            info.len() <= 65536 && info.contains("NSBluetoothAlwaysUsageDescription")
        })
}
async fn bounded<T>(
    future: impl std::future::Future<Output = btleplug::Result<T>>,
) -> Result<T, ()> {
    tokio::time::timeout(Duration::from_secs(3), future)
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
}
async fn scan(
    stop: Arc<AtomicBool>,
    state: Arc<AtomicU8>,
    tx: mpsc::SyncSender<Vec<u8>>,
) -> Result<(), ()> {
    let manager = bounded(Manager::new()).await?;
    let adapter = bounded(manager.adapters())
        .await?
        .into_iter()
        .next()
        .ok_or(())?;
    if matches!(
        bounded(adapter.adapter_state()).await?,
        CentralState::PoweredOff
    ) {
        state.store(2, Ordering::Release);
        return Ok(());
    }
    let mut events = bounded(adapter.events()).await?;
    bounded(adapter.start_scan(ScanFilter {
        services: vec![SERVICE],
    }))
    .await?;
    state.store(1, Ordering::Release);
    let mut seen = HashSet::new();
    let mut epoch = Instant::now();
    let mut processed = 0;
    while !stop.load(Ordering::Acquire) {
        let event = match tokio::time::timeout(Duration::from_millis(200), events.next()).await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => continue,
        };
        if epoch.elapsed() >= Duration::from_secs(60) {
            epoch = Instant::now();
            processed = 0;
            seen.clear();
        }
        processed += 1;
        if processed > 8192 {
            continue;
        }
        let data = match event {
            CentralEvent::StateUpdate(CentralState::PoweredOff) => {
                state.store(2, Ordering::Release);
                break;
            }
            CentralEvent::ServiceDataAdvertisement { service_data, .. } => {
                service_data.get(&SERVICE).cloned()
            }
            CentralEvent::DeviceDiscovered(id)
            | CentralEvent::DeviceUpdated(id)
            | CentralEvent::ServicesAdvertisement { id, .. } => {
                if let Ok(device) = bounded(adapter.peripheral(&id)).await {
                    bounded(device.properties())
                        .await
                        .ok()
                        .flatten()
                        .and_then(|p| {
                            packet(
                                p.service_data.get(&SERVICE).map(Vec::as_slice),
                                p.services.contains(&SERVICE),
                                p.local_name.as_deref(),
                            )
                        })
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(data) = data.filter(|p| matches!(p.len(), 25 | 27))
            && seen.len() < 512
            && seen.insert(data.clone())
        {
            let _ = tx.try_send(data);
        }
    }
    let _ = bounded(adapter.stop_scan()).await;
    Ok(())
}
fn packet(data: Option<&[u8]>, service_present: bool, name: Option<&str>) -> Option<Vec<u8>> {
    if let Some(data) = data.filter(|d| matches!(d.len(), 25 | 27)) {
        return Some(data.to_vec());
    }
    // Apple Nearby advertising encodes service data in the local name. That
    // name is protocol bytes only and is never shown as a device identity.
    let name = name.filter(|n| service_present && n.len() <= 44)?;
    URL_SAFE
        .decode(name)
        .ok()
        .filter(|d| matches!(d.len(), 25 | 27))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn name_fallback_requires_nearby_uuid_and_exact_packet_size() {
        let bytes = [7; 25];
        let name = URL_SAFE.encode(bytes);
        assert_eq!(packet(None, true, Some(&name)), Some(bytes.to_vec()));
        assert!(packet(None, false, Some(&name)).is_none());
        assert!(packet(None, true, Some("My phone")).is_none());
        assert!(packet(Some(&[0; 24]), true, None).is_none());
    }
}
