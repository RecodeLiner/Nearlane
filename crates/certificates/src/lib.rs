//! Quick Share device certificates, based on Google's published Nearby code.
//!
//! These are sharing credentials, not X.509 certificates. Treat symmetric
//! discovery keys and decrypted metadata as sensitive. No network or persistence.

mod crypto;
mod local;
mod remote;
#[cfg(test)]
mod test_vectors;
#[cfg(test)]
mod tests;
mod wire;

use std::fmt;

pub use local::LocalCertificate;
pub use remote::{DeviceMetadata, RemoteCertificate};

/// Rotating advertisement key: a two-byte salt and fourteen encrypted bytes.
/// This is broadcast data, but Debug omits it to avoid creating a tracking log.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct EncryptedMetadataKey {
    pub(crate) salt: [u8; 2],
    pub(crate) encrypted: [u8; 14],
}
impl EncryptedMetadataKey {
    /// Parse exactly the key bytes carried by the advertisement.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CertificateError> {
        if bytes.len() != 16 {
            return Err(CertificateError::Encoding);
        }
        Ok(Self {
            salt: bytes[..2]
                .try_into()
                .map_err(|_| CertificateError::Encoding)?,
            encrypted: bytes[2..]
                .try_into()
                .map_err(|_| CertificateError::Encoding)?,
        })
    }
    /// Serialize for a Nearby advertisement, never as an account credential.
    pub fn to_bytes(self) -> [u8; 16] {
        let mut out = [0; 16];
        out[..2].copy_from_slice(&self.salt);
        out[2..].copy_from_slice(&self.encrypted);
        out
    }
}
impl fmt::Debug for EncryptedMetadataKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncryptedMetadataKey([REDACTED])")
    }
}

/// Private certificate validity, matching Google's published implementation.
pub const VALIDITY_SECONDS: i64 = 72 * 60 * 60;
/// Allowed public-certificate clock skew, matching Google's implementation.
pub const CLOCK_SKEW_SECONDS: i64 = 30 * 60;

/// Role prefixes bind paired-key signatures to sender vs receiver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerRole {
    /// Sender signs prefix 0x01 followed by the UKEY2 authentication token.
    Sender,
    /// Receiver signs prefix 0x02 followed by the same token.
    Receiver,
}

impl PeerRole {
    pub(crate) fn signed_payload(self, token: &[u8]) -> Result<Vec<u8>, CertificateError> {
        if token.len() != 32 {
            return Err(CertificateError::Key);
        }
        let mut payload = Vec::with_capacity(33);
        payload.push(if self == Self::Sender { 1 } else { 2 });
        payload.extend_from_slice(token);
        Ok(payload)
    }
}

/// Sanitized errors, without key bytes, metadata, names or remote responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateError {
    /// Invalid timestamps or clock state.
    Time,
    /// Malformed protobuf or a bounded field is missing.
    Encoding,
    /// Invalid key or signature encoding/length.
    Key,
    /// OS entropy could not be obtained.
    Entropy,
    /// Commitment, signature or authenticated decryption failed.
    Integrity,
    /// The certificate is not valid at this time, including clock tolerance.
    Expired,
    /// A size or rotation limit was exceeded.
    Limit,
    /// This visibility or metadata shape is unsupported.
    Unsupported,
}
impl fmt::Display for CertificateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Quick Share certificate rejected: {self:?}")
    }
}
impl std::error::Error for CertificateError {}

/// Current Unix time, without an implicit fallback if the system clock is wrong.
pub fn now_seconds() -> Result<i64, CertificateError> {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| CertificateError::Time)?
            .as_secs(),
    )
    .map_err(|_| CertificateError::Time)
}

pub(crate) fn valid_at(start: i64, end: i64, now: i64) -> bool {
    start < end
        && now >= start.saturating_sub(CLOCK_SKEW_SECONDS)
        && now < end.saturating_add(CLOCK_SKEW_SECONDS)
}
