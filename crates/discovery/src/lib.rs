//! Bounded Wi-Fi LAN discovery, verified BLE hints and private advertisements.
//! Only API-delivered own-device certificates may authenticate candidate names.
use nearlane_core::Text;
mod advertisement;
mod bluetooth;
mod publication;
mod watch;
pub use bluetooth::match_bluetooth;
pub use publication::PrivateAdvertisement;
pub use watch::Watch;

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};
use nearlane_certificates::{RemoteCertificate, now_seconds};
use std::{
    fmt,
    net::Ipv4Addr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// Sanitized failures; no peer names, addresses or raw network data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryError {
    /// No currently valid trusted own-device credentials are available.
    NoCertificates,
    /// The local mDNS adapter could not start or stopped unexpectedly.
    Network,
    /// An advertisement has an unsupported or malformed bounded shape.
    Advertisement,
    /// The bounded event/peer policy was exceeded.
    Limit,
    /// Discovery was cancelled or the account signed out.
    Cancelled,
    /// Background worker could not start or stopped unexpectedly.
    Worker,
}
impl DiscoveryError {
    /// Deferred, sanitized explanation for a localized presentation layer.
    pub fn text(&self) -> Text {
        match self {
            Self::NoCertificates => {
                Text::new("Prepare current own-device certificates before searching.")
            }
            Self::Network => Text::new(
                "Local-network discovery could not run. Check the network and mDNS access.",
            ),
            Self::Advertisement => Text::new("An unsupported Nearby advertisement was rejected."),
            Self::Limit => Text::new("Local-network discovery reached its event limit."),
            Self::Cancelled => Text::new("Local-network discovery was cancelled."),
            Self::Worker => Text::new("The local-network discovery worker stopped."),
        }
    }
}
impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text().fmt(f)
    }
}
impl std::error::Error for DiscoveryError {}

/// Authenticate incoming endpoint metadata against current own-device records.
/// Any plaintext advertised name is ignored; live signatures are still required.
pub fn match_incoming(
    info: &[u8],
    certificates: &[Arc<RemoteCertificate>],
) -> Option<(Arc<RemoteCertificate>, String)> {
    if certificates.len() > 256 {
        return None;
    }
    let key = advertisement::decode_info(info, *b"INCM").ok()?.key;
    let now = now_seconds().ok()?;
    certificates
        .iter()
        .filter(|c| c.for_own_device())
        .find_map(|c| {
            c.identify(key, now)
                .ok()
                .map(|m| (Arc::clone(c), m.device_name().to_owned()))
        })
}

/// Candidate authenticated by an advertisement commitment and metadata AEAD.
/// This still requires a live paired-key signature proof before any file use.
#[derive(Clone)]
pub struct OwnDevice {
    service: String,
    name: String,
    certificate: Arc<RemoteCertificate>,
    endpoint: [u8; 4],
    address: Ipv4Addr,
    port: u16,
    discovered: Instant,
}
impl OwnDevice {
    /// Restrict connection attempts to recently observed, matched advertisements.
    pub fn is_recent(&self) -> bool {
        self.discovered.elapsed() <= Duration::from_secs(120)
    }
    /// Certificate-bound discovery destination for the trusted transfer worker.
    /// Never expose the address to QML or follow later peer-supplied redirects.
    pub fn socket_address(&self) -> std::net::SocketAddrV4 {
        std::net::SocketAddrV4::new(self.address, self.port)
    }
    /// Authenticated metadata name, suitable for plain-text UI display.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Trusted certificate matched to the advertisement, for a future live
    /// signature verifier. It is never exposed to QML or serialized.
    pub fn certificate(&self) -> &RemoteCertificate {
        &self.certificate
    }
}
impl fmt::Debug for OwnDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnDevice([REDACTED])")
    }
}

/// Explicit scan operation with owned cancellation. Dropping cancels and discards
/// late results; its worker checks cancellation at least every 100 milliseconds.
pub struct Search {
    cancelled: Arc<AtomicBool>,
    result: mpsc::Receiver<Result<Vec<OwnDevice>, DiscoveryError>>,
}
impl fmt::Debug for Search {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Search([REDACTED])")
    }
}
impl Drop for Search {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
impl Search {
    /// Search only the NearbySharing DNS-SD service for 20 seconds. No TCP
    /// connection, file listener, service publication or Google request occurs.
    pub fn start(certificates: Vec<Arc<RemoteCertificate>>) -> Result<Self, DiscoveryError> {
        if certificates.len() > 256 {
            return Err(DiscoveryError::Limit);
        }
        let now = now_seconds().map_err(|_| DiscoveryError::NoCertificates)?;
        let certificates: Vec<_> = certificates
            .into_iter()
            .filter(|c| c.for_own_device() && c.valid_at(now))
            .collect();
        if certificates.is_empty() {
            return Err(DiscoveryError::NoCertificates);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancelled);
        let (sender, result) = mpsc::channel();
        std::thread::Builder::new()
            .name("nearlane-discovery".into())
            .spawn(move || {
                let outcome = discover(certificates, &worker_cancel);
                let _ = sender.send(outcome);
            })
            .map_err(|_| DiscoveryError::Worker)?;
        Ok(Self { cancelled, result })
    }
    /// Observe completion without blocking the UI thread.
    pub fn poll(&self) -> Option<Result<Vec<OwnDevice>, DiscoveryError>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(DiscoveryError::Worker)),
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
}

struct Daemon(ServiceDaemon);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}
fn discover(
    certificates: Vec<Arc<RemoteCertificate>>,
    cancelled: &AtomicBool,
) -> Result<Vec<OwnDevice>, DiscoveryError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(DiscoveryError::Cancelled);
    }
    let daemon = Daemon(ServiceDaemon::new().map_err(|_| DiscoveryError::Network)?);
    let ty = advertisement::service_type();
    let receiver = daemon.0.browse(&ty).map_err(|_| DiscoveryError::Network)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut devices = Vec::new();
    let mut processed = 0;
    while Instant::now() < deadline {
        if cancelled.load(Ordering::Acquire) {
            return Err(DiscoveryError::Cancelled);
        }
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) {
            processed += 1;
            if processed > 1024 {
                return Err(DiscoveryError::Limit);
            }
            if let ServiceEvent::ServiceResolved(info) = event
                && let Some(device) = identify(&info, &ty, &certificates)
                && !devices.iter().any(|d: &OwnDevice| {
                    d.endpoint == device.endpoint
                        && d.address == device.address
                        && d.port == device.port
                })
            {
                if devices.len() >= 32 {
                    return Err(DiscoveryError::Limit);
                }
                devices.push(device);
            }
        } else if receiver.is_disconnected() {
            return Err(DiscoveryError::Network);
        }
    }
    Ok(devices)
}

fn identify(
    info: &ResolvedService,
    ty: &str,
    certificates: &[Arc<RemoteCertificate>],
) -> Option<OwnDevice> {
    if info.get_port() == 0 {
        return None;
    }
    let fullname = info.get_fullname();
    let split = fullname.len().checked_sub(ty.len())?;
    if !fullname.get(split..)?.eq_ignore_ascii_case(ty) {
        return None;
    }
    let instance = fullname.get(..split)?.strip_suffix('.')?;
    let ad = advertisement::decode(instance, info.get_property_val_str("n")?).ok()?;
    let address = info
        .get_addresses_v4()
        .into_iter()
        .filter(|a| a.is_private() || a.is_link_local())
        .min()?;
    let now = now_seconds().ok()?;
    for certificate in certificates {
        if !certificate.for_own_device() {
            continue;
        }
        if let Ok(metadata) = certificate.identify(ad.key, now) {
            return Some(OwnDevice {
                service: fullname.into(),
                name: metadata.device_name().into(),
                certificate: Arc::clone(certificate),
                endpoint: ad.endpoint,
                address,
                port: info.get_port(),
                discovered: Instant::now(),
            });
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use mdns_sd::ServiceInfo;
    use nearlane_certificates::{DeviceMetadata, LocalCertificate};
    use nearlane_core::Visibility;
    use sha2::{Digest as _, Sha256};
    use std::collections::HashMap;

    #[test]
    fn only_trusted_own_metadata_is_shown_and_no_public_name_is_trusted() {
        let now = now_seconds().unwrap();
        let mut local = LocalCertificate::generate(
            DeviceMetadata::named("Synthetic phone").unwrap(),
            Visibility::OwnDevices,
            now,
        )
        .unwrap();
        let certificate =
            Arc::new(RemoteCertificate::decode(&local.publication_bytes().unwrap()).unwrap());
        let mut endpoint = vec![0x10];
        endpoint.extend_from_slice(&local.advertisement_key().unwrap().to_bytes());
        let hash = Sha256::digest(b"NearbySharing");
        let mut instance = vec![0x23];
        instance.extend_from_slice(b"ABCD");
        instance.extend_from_slice(&hash[..3]);
        instance.push(0);
        let ty = advertisement::service_type();
        let instance = URL_SAFE_NO_PAD.encode(instance);
        let mut txt = HashMap::new();
        txt.insert("n".to_owned(), URL_SAFE_NO_PAD.encode(&endpoint));
        let mut info = ServiceInfo::new(
            &ty,
            &instance,
            "synthetic.local.",
            "192.168.1.7",
            12345,
            txt,
        )
        .unwrap()
        .as_resolved_service();
        assert!(identify(&info, &ty, &[]).is_none());
        let device = identify(&info, &ty, &[Arc::clone(&certificate)]).unwrap();
        assert_eq!(device.name(), "Synthetic phone");
        assert!(device.certificate.for_own_device());
        assert_eq!(format!("{device:?}"), "OwnDevice([REDACTED])");
        info.fullname = info.fullname.replace(&ty, &ty.to_ascii_lowercase());
        assert!(identify(&info, &ty, &[certificate]).is_some());
    }

    #[test]
    fn empty_credentials_do_not_start_a_network_worker_and_drop_cancels() {
        assert!(matches!(
            Search::start(Vec::new()),
            Err(DiscoveryError::NoCertificates)
        ));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (sender, result) = mpsc::channel();
        let search = Search {
            cancelled: Arc::clone(&cancelled),
            result,
        };
        drop(search);
        assert!(cancelled.load(Ordering::Acquire));
        assert!(sender.send(Ok(Vec::new())).is_err());
    }
}
