//! Continuous bounded discovery with replaceable snapshots, never an event queue.
use super::*;
use std::sync::Mutex;

/// A background DNS-SD browser. Drop cancels it; each snapshot contains only
/// current certificate-matched own devices. No receiver or TCP probe is started.
pub struct Watch {
    cancelled: Arc<AtomicBool>,
    snapshot: Arc<Mutex<Result<Vec<OwnDevice>, DiscoveryError>>>,
}
impl fmt::Debug for Watch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Watch([REDACTED])")
    }
}
impl Drop for Watch {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
impl Watch {
    /// Start continuous private discovery using only the current trusted cache.
    pub fn start(certificates: Vec<Arc<RemoteCertificate>>) -> Result<Self, DiscoveryError> {
        let now = now_seconds().map_err(|_| DiscoveryError::NoCertificates)?;
        if certificates.len() > 256 {
            return Err(DiscoveryError::Limit);
        }
        let certificates: Vec<_> = certificates
            .into_iter()
            .filter(|c| c.for_own_device() && c.valid_at(now))
            .collect();
        if certificates.is_empty() {
            return Err(DiscoveryError::NoCertificates);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let snapshot = Arc::new(Mutex::new(Ok(Vec::new())));
        let stop = Arc::clone(&cancelled);
        let output = Arc::clone(&snapshot);
        std::thread::Builder::new()
            .name("nearlane-watch".into())
            .spawn(move || {
                if let Err(error) = run(certificates, &stop, &output) {
                    let _ = output.lock().map(|mut s| *s = Err(error));
                }
            })
            .map_err(|_| DiscoveryError::Worker)?;
        Ok(Self {
            cancelled,
            snapshot,
        })
    }
    /// Replaceable bounded snapshot; stale endpoints are never returned.
    pub fn snapshot(&self) -> Result<Vec<OwnDevice>, DiscoveryError> {
        self.snapshot
            .lock()
            .map_err(|_| DiscoveryError::Worker)?
            .as_ref()
            .map(|v| v.iter().filter(|d| d.is_recent()).cloned().collect())
            .map_err(|e| *e)
    }
}

fn run(
    certificates: Vec<Arc<RemoteCertificate>>,
    stop: &AtomicBool,
    output: &Mutex<Result<Vec<OwnDevice>, DiscoveryError>>,
) -> Result<(), DiscoveryError> {
    let daemon = Daemon(ServiceDaemon::new().map_err(|_| DiscoveryError::Network)?);
    let ty = advertisement::service_type();
    let events = daemon.0.browse(&ty).map_err(|_| DiscoveryError::Network)?;
    let mut devices: Vec<OwnDevice> = Vec::new();
    let mut epoch = Instant::now();
    let mut processed = 0;
    while !stop.load(Ordering::Acquire) {
        if epoch.elapsed() >= Duration::from_secs(60) {
            epoch = Instant::now();
            processed = 0;
        }
        match events.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                processed += 1;
                if processed > 1024 {
                    return Err(DiscoveryError::Limit);
                }
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if let Some(device) = identify(&info, &ty, &certificates) {
                            if let Some(old) = devices
                                .iter_mut()
                                .find(|d| d.service.eq_ignore_ascii_case(&device.service))
                            {
                                *old = device;
                            } else if devices.len() < 32 {
                                devices.push(device);
                            } else {
                                return Err(DiscoveryError::Limit);
                            }
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, name) => {
                        devices.retain(|d| !d.service.eq_ignore_ascii_case(&name))
                    }
                    _ => {}
                }
            }
            Err(_) if events.is_disconnected() => return Err(DiscoveryError::Network),
            _ => {}
        }
        devices.retain(OwnDevice::is_recent);
        *output.lock().map_err(|_| DiscoveryError::Worker)? = Ok(devices.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_trust_cache_never_starts_network_watch() {
        assert!(matches!(
            Watch::start(Vec::new()),
            Err(DiscoveryError::NoCertificates)
        ));
    }

    #[test]
    #[ignore = "Opt-in native continuous discovery of an owned synthetic advertisement"]
    #[allow(clippy::unwrap_used)]
    fn native_watch_tracks_appearance_and_withdrawal() {
        use nearlane_certificates::{DeviceMetadata, LocalCertificate};
        let mut local = LocalCertificate::generate(
            DeviceMetadata::named("Synthetic watch device").unwrap(),
            nearlane_core::Visibility::OwnDevices,
            now_seconds().unwrap(),
        )
        .unwrap();
        let key = local.advertisement_key().unwrap();
        let certificate =
            Arc::new(RemoteCertificate::decode(&local.publication_bytes().unwrap()).unwrap());
        let listener = std::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
        let publication =
            PrivateAdvertisement::start(*b"NWAT", key, listener.local_addr().unwrap().port())
                .unwrap();
        let watch = Watch::start(vec![certificate]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        while watch.snapshot().unwrap().is_empty() {
            assert!(
                Instant::now() < deadline,
                "Owned advertisement was not found"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(
            watch.snapshot().unwrap()[0].name(),
            "Synthetic watch device"
        );
        drop(publication);
        let deadline = Instant::now() + Duration::from_secs(8);
        while !watch.snapshot().unwrap().is_empty() {
            assert!(
                Instant::now() < deadline,
                "Withdrawn endpoint stayed visible"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
