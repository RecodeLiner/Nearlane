//! Original bounded decoder of Google's two-layer fast BLE advertisement.
//! Reference: google/nearby 770a030b, connections/implementation/ble_advertisement
//! and connections/implementation/mediums/ble/ble_advertisement. No OS APIs.
use crate::match_incoming;
use nearlane_certificates::RemoteCertificate;
use std::sync::Arc;

/// Match a FEF3 service-data packet to current own-device certificates. This
/// yields only a discovery hint; it supplies no address or permission to send.
/// Unknown/public/malformed packets are discarded without exposing their names.
pub fn match_bluetooth(
    data: &[u8],
    certificates: &[Arc<RemoteCertificate>],
) -> Option<(Arc<RemoteCertificate>, String)> {
    let info = private_info(data)?;
    match_incoming(info, certificates)
}
fn private_info(data: &[u8]) -> Option<&[u8]> {
    if !matches!(data.len(), 25 | 27)
        || !matches!(data[0] >> 5, 1 | 2)
        || !matches!((data[0] >> 2) & 7, 1 | 2)
        || data[0] & 3 != 2 // Fast advertisement, not a second profile.
        || data[1] != 23
    {
        return None;
    }
    let connection = &data[2..25];
    if connection[0] != 0x23 // Connections v1, point-to-point.
        || !connection[1..5].iter().all(u8::is_ascii_alphanumeric)
        || connection[5] != 17
        || connection[6] & 0x10 == 0
    // Private endpoint, no public-name fallback.
    {
        return None;
    }
    Some(&connection[6..23])
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use nearlane_certificates::{DeviceMetadata, LocalCertificate, now_seconds};
    use nearlane_core::Visibility;
    #[test]
    fn nested_fast_packet_matches_only_an_owned_certificate() {
        let mut local = LocalCertificate::generate(
            DeviceMetadata::named("Synthetic BLE peer").unwrap(),
            Visibility::OwnDevices,
            now_seconds().unwrap(),
        )
        .unwrap();
        let key = local.advertisement_key().unwrap();
        let remote =
            Arc::new(RemoteCertificate::decode(&local.publication_bytes().unwrap()).unwrap());
        let mut bytes = vec![0x2a, 23, 0x23, b'T', b'E', b'S', b'T', 17, 0x16];
        bytes.extend_from_slice(&key.to_bytes());
        bytes.extend_from_slice(&[0x01, 0x02]);
        assert_eq!(
            match_bluetooth(&bytes, std::slice::from_ref(&remote))
                .unwrap()
                .1,
            "Synthetic BLE peer"
        );
        assert!(match_bluetooth(&bytes, &[]).is_none());
        for index in [0, 1, 2, 7, 8, 9] {
            let mut invalid = bytes.clone();
            invalid[index] ^= 0xff;
            assert!(match_bluetooth(&invalid, std::slice::from_ref(&remote)).is_none());
        }
        for length in 0..25 {
            assert!(match_bluetooth(&bytes[..length], std::slice::from_ref(&remote)).is_none());
        }
    }
}
