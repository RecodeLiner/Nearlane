//! Original private Wi-Fi LAN publication adapter. No plaintext device name.
use crate::{DiscoveryError, advertisement};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use mdns_sd::{DaemonEvent, ServiceDaemon, ServiceInfo};
use nearlane_certificates::EncryptedMetadataKey;
use sha2::{Digest as _, Sha256};
use std::{net::IpAddr, time::Duration};

/// An explicit receiver beacon. Drop withdraws this service and stops its daemon.
/// Only private/link-local IPv4 interfaces are advertised, never a public name.
pub struct PrivateAdvertisement {
    daemon: ServiceDaemon,
    name: String,
    monitor: mdns_sd::Receiver<DaemonEvent>,
}
impl std::fmt::Debug for PrivateAdvertisement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateAdvertisement([REDACTED])")
    }
}
impl Drop for PrivateAdvertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.name);
        let _ = self.daemon.shutdown();
    }
}
impl PrivateAdvertisement {
    /// Publish the bound receiver's port using fresh private advertisement keys.
    /// The caller owns the listener and must require live private authentication.
    pub fn start(
        endpoint: [u8; 4],
        key: EncryptedMetadataKey,
        port: u16,
    ) -> Result<Self, DiscoveryError> {
        if port == 0 || !endpoint.iter().all(u8::is_ascii_alphanumeric) {
            return Err(DiscoveryError::Advertisement);
        }
        let addresses: Vec<IpAddr> = if_addrs::get_if_addrs()
            .map_err(|_| DiscoveryError::Network)?
            .into_iter()
            .map(|i| i.ip())
            .filter(|a| matches!(a, IpAddr::V4(v) if v.is_private() || v.is_link_local()))
            .collect();
        if addresses.is_empty() {
            return Err(DiscoveryError::Network);
        }
        let (instance, txt) = encoded(endpoint, key);
        let host = format!("nearlane-{}.local.", String::from_utf8_lossy(&endpoint));
        let props = [("n", txt.as_str())];
        let info = ServiceInfo::new(
            &advertisement::service_type(),
            &instance,
            &host,
            addresses.as_slice(),
            port,
            &props[..],
        )
        .map_err(|_| DiscoveryError::Network)?;
        let daemon = ServiceDaemon::new().map_err(|_| DiscoveryError::Network)?;
        let monitor = daemon.monitor().map_err(|_| DiscoveryError::Network)?;
        let publication = Self {
            daemon,
            name: info.get_fullname().into(),
            monitor,
        };
        publication
            .daemon
            .register(info)
            .map_err(|_| DiscoveryError::Network)?;
        Ok(publication)
    }
    /// Wait briefly for an actual announcement; caller repeats with cancellation.
    pub fn announced(&self, timeout: Duration) -> Result<bool, DiscoveryError> {
        match self.monitor.recv_timeout(timeout) {
            Ok(DaemonEvent::Announce(_, _)) => Ok(true),
            Ok(DaemonEvent::Error(_) | DaemonEvent::NameChange(_)) => Err(DiscoveryError::Network),
            _ => Ok(false),
        }
    }
}
fn encoded(endpoint: [u8; 4], key: EncryptedMetadataKey) -> (String, String) {
    let hash = Sha256::digest(b"NearbySharing");
    let mut name = vec![0x23]; // Wi-Fi LAN v1, point-to-point PCP.
    name.extend_from_slice(&endpoint);
    name.extend_from_slice(&hash[..3]);
    name.extend_from_slice(&[0, 0]); // No UWB address or WebRTC upgrade.
    let mut info = vec![0x16]; // Private computer, no plaintext name.
    info.extend_from_slice(&key.to_bytes());
    (URL_SAFE_NO_PAD.encode(name), URL_SAFE_NO_PAD.encode(info))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn published_shape_matches_private_decoder_without_a_public_name() {
        let key = EncryptedMetadataKey::from_bytes(&[7; 16]).unwrap();
        let (instance, txt) = encoded(*b"TEST", key);
        let decoded = advertisement::decode(&instance, &txt).unwrap();
        assert_eq!(decoded.endpoint, *b"TEST");
        assert_eq!(decoded.key.to_bytes(), [7; 16]);
        assert_eq!(URL_SAFE_NO_PAD.decode(txt).unwrap().len(), 17);
    }

    #[test]
    #[ignore = "Opt-in native LAN announcement and self-resolution; synthetic key only"]
    fn native_private_advertisement_is_announced_and_resolved() {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let key = EncryptedMetadataKey::from_bytes(&[7; 16]).unwrap();
        let publication = PrivateAdvertisement::start(*b"NQAT", key, port).unwrap();
        let browser = ServiceDaemon::new().unwrap();
        let events = browser.browse(&advertisement::service_type()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut found = false;
        while std::time::Instant::now() < deadline {
            if let Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) =
                events.recv_timeout(Duration::from_millis(100))
            {
                if info.get_port() == port && info.get_hostname() == "nearlane-NQAT.local." {
                    let ty = advertisement::service_type();
                    let instance = info
                        .get_fullname()
                        .strip_suffix(&ty)
                        .unwrap()
                        .trim_end_matches('.');
                    let ad =
                        advertisement::decode(instance, info.get_property_val_str("n").unwrap())
                            .unwrap();
                    assert_eq!(ad.endpoint, *b"NQAT");
                    assert_eq!(ad.key.to_bytes(), [7; 16]);
                    found = true;
                    break;
                }
            }
        }
        let _ = browser.shutdown();
        drop(publication);
        assert!(found, "Native self-advertisement could not be resolved");
    }
}
