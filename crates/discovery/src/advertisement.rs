//! Original decoder of Google's public Wi-Fi LAN and sharing advertisements.
//! Reference: google/nearby 770a030be8fbebbcda5bf71ba718c35f5350c27c,
//! connections/implementation/wifi_lan_service_info.cc and sharing/advertisement.cc.

use base64::{
    Engine as _,
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use nearlane_certificates::EncryptedMetadataKey;
use sha2::{Digest as _, Sha256};

use crate::DiscoveryError;

/// Decoded discovery material. Peer names from public advertisements are ignored.
pub(crate) struct Advertisement {
    pub key: EncryptedMetadataKey,
    pub endpoint: [u8; 4],
}

pub(crate) fn service_type() -> String {
    let hash = Sha256::digest(b"NearbySharing");
    let hex: String = hash[..6].iter().map(|b| format!("{b:02X}")).collect();
    format!("_{hex}._tcp.local.")
}

pub(crate) fn decode(instance: &str, txt: &str) -> Result<Advertisement, DiscoveryError> {
    let name = base64(instance, 63)?;
    let hash = Sha256::digest(b"NearbySharing");
    if name.len() < 9
        || name[0] >> 5 != 1
        || !matches!(name[0] & 31, 1..=3)
        || name[5..8] != hash[..3]
    {
        return Err(DiscoveryError::Advertisement);
    }
    let after_uwb = 9_usize
        .checked_add(name[8] as usize)
        .ok_or(DiscoveryError::Advertisement)?;
    if after_uwb > name.len() || name.len() - after_uwb > 1 {
        return Err(DiscoveryError::Advertisement);
    }
    let info = base64(txt, 180)?;
    decode_info(
        &info,
        name[1..5]
            .try_into()
            .map_err(|_| DiscoveryError::Advertisement)?,
    )
}

fn base64(text: &str, bound: usize) -> Result<Vec<u8>, DiscoveryError> {
    if text.is_empty() || text.len() > bound {
        return Err(DiscoveryError::Advertisement);
    }
    URL_SAFE_NO_PAD
        .decode(text)
        .or_else(|_| URL_SAFE.decode(text))
        .map_err(|_| DiscoveryError::Advertisement)
}

pub(super) fn decode_info(info: &[u8], endpoint: [u8; 4]) -> Result<Advertisement, DiscoveryError> {
    if !(17..=131).contains(&info.len()) || info[0] >> 5 > 1 {
        return Err(DiscoveryError::Advertisement);
    }
    let key = EncryptedMetadataKey::from_bytes(&info[1..17])
        .map_err(|_| DiscoveryError::Advertisement)?;
    let mut cursor = 17;
    if info[0] & 16 == 0 {
        let length = *info.get(cursor).ok_or(DiscoveryError::Advertisement)? as usize;
        cursor += 1;
        if length == 0 || cursor + length > info.len() {
            return Err(DiscoveryError::Advertisement);
        }
        cursor += length; // Never use the unauthenticated advertised name.
    }
    while cursor < info.len() {
        let tail = &info[cursor..];
        if tail.len() < 2 || tail[1] as usize > tail.len() - 2 || (tail[0] == 2 && tail[1] != 1) {
            return Err(DiscoveryError::Advertisement);
        }
        cursor += 2 + tail[1] as usize;
    }
    Ok(Advertisement { key, endpoint })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn service_hash_and_private_versions_follow_reference_layout() {
        assert_eq!(service_type(), "_FC9F5ED42C8A._tcp.local.");
        for header in [0x10, 0x30] {
            let mut info = vec![header];
            info.extend_from_slice(&[3; 16]);
            assert_eq!(
                decode_info(&info, *b"test").unwrap().key.to_bytes(),
                [3; 16]
            );
        }
    }
    #[test]
    fn refuses_truncation_and_ignores_untrusted_public_names() {
        let mut info = vec![0];
        info.extend_from_slice(&[0; 16]);
        assert!(decode_info(&info, *b"test").is_err());
        info.extend_from_slice(&[3, b'f', b'a', b'k']);
        assert!(decode_info(&info, *b"test").is_ok());
        info.extend_from_slice(&[2, 4, 0]);
        assert!(decode_info(&info, *b"test").is_err());
        assert!(decode_info(&[0; 132], *b"test").is_err());
    }
}
