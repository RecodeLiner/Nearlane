use std::fmt;

use p256::{
    ecdsa::{Signature, VerifyingKey, signature::Verifier as _},
    pkcs8::DecodePublicKey as _,
};
use prost::Message as _;
use subtle::ConstantTimeEq as _;
use zeroize::Zeroizing;

use crate::{CertificateError, EncryptedMetadataKey, PeerRole, crypto, valid_at, wire};

/// Authenticated decrypted metadata. Debug does not print personal/device data.
#[derive(Clone)]
pub struct DeviceMetadata(pub(crate) wire::Metadata);
impl fmt::Debug for DeviceMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceMetadata([REDACTED])")
    }
}
impl DeviceMetadata {
    /// Minimal local metadata. Account names and Bluetooth addresses are omitted.
    pub fn named(device_name: &str) -> Result<Self, CertificateError> {
        let mut metadata = wire::Metadata::default();
        metadata.device_name = Some(device_name.to_owned());
        metadata.vendor_id = Some(0);
        Self::from_wire(metadata)
    }

    pub(crate) fn from_wire(metadata: wire::Metadata) -> Result<Self, CertificateError> {
        let name = metadata
            .device_name
            .as_deref()
            .ok_or(CertificateError::Encoding)?;
        if name.is_empty()
            || name.len() > 128
            || name.chars().any(char::is_control)
            || metadata
                .bluetooth_mac
                .as_ref()
                .is_some_and(|m| m.len() != 6)
            || [
                &metadata.full_name,
                &metadata.icon_url,
                &metadata.gaia_id,
                &metadata.account_name,
                &metadata.model_name,
                &metadata.icon_token,
            ]
            .into_iter()
            .any(|s| {
                s.as_ref()
                    .is_some_and(|s| s.len() > 2048 || s.chars().any(char::is_control))
            })
        {
            return Err(CertificateError::Encoding);
        }
        Ok(Self(metadata))
    }

    /// Name to show only after advertisement authentication succeeds.
    pub fn device_name(&self) -> &str {
        self.0.device_name.as_deref().unwrap_or_default()
    }

    /// Optional six-byte Bluetooth address from authenticated metadata.
    pub fn bluetooth_address(&self) -> Option<[u8; 6]> {
        self.0.bluetooth_mac.as_deref()?.try_into().ok()
    }
}

/// Server-delivered certificate. Construction alone does not make it trusted;
/// production cache entries must originate from the fixed authenticated API.
pub struct RemoteCertificate {
    id: [u8; 32],
    secret: Zeroizing<[u8; 32]>,
    public_key: VerifyingKey,
    encrypted_metadata: Vec<u8>,
    metadata_tag: [u8; 32],
    start: i64,
    end: i64,
    self_share: bool,
}
impl fmt::Debug for RemoteCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RemoteCertificate([REDACTED])")
    }
}

impl RemoteCertificate {
    /// Parse one bounded credential. Do not supply an arbitrary peer's certificate
    /// as trusted input; use only certificates delivered by the authenticated API.
    pub fn decode(bytes: &[u8]) -> Result<Self, CertificateError> {
        if bytes.len() > 16 * 1024 {
            return Err(CertificateError::Limit);
        }
        let mut certificate =
            wire::Certificate::decode(bytes).map_err(|_| CertificateError::Encoding)?;
        let start = certificate.start.take().ok_or(CertificateError::Time)?;
        let end = certificate.end.take().ok_or(CertificateError::Time)?;
        if !(0..1_000_000_000).contains(&start.nanos)
            || !(0..1_000_000_000).contains(&end.nanos)
            || start.seconds >= end.seconds
            || end.seconds.saturating_sub(start.seconds) > 31 * 86400
        {
            return Err(CertificateError::Time);
        }
        if certificate.encrypted_metadata.len() < 16
            || certificate.encrypted_metadata.len() > 8 * 1024
            || !matches!(certificate.metadata_key.len(), 0 | 14)
            || certificate.binding_id.len() > 1024
        {
            return Err(CertificateError::Encoding);
        }
        let id = certificate
            .id
            .as_slice()
            .try_into()
            .map_err(|_| CertificateError::Key)?;
        let secret = Zeroizing::new(
            certificate
                .secret
                .as_slice()
                .try_into()
                .map_err(|_| CertificateError::Key)?,
        );
        let public_key = VerifyingKey::from_public_key_der(&certificate.public_key)
            .map_err(|_| CertificateError::Key)?;
        let metadata_tag = certificate
            .metadata_key_tag
            .as_slice()
            .try_into()
            .map_err(|_| CertificateError::Key)?;
        Ok(Self {
            id,
            secret,
            public_key,
            encrypted_metadata: std::mem::take(&mut certificate.encrypted_metadata),
            metadata_tag,
            start: start.seconds,
            end: end.seconds,
            self_share: certificate.self_share,
        })
    }

    /// Opaque cache identifier; never display it as proof of a Google identity.
    pub fn id(&self) -> &[u8; 32] {
        &self.id
    }
    /// Whether Google marked this credential as belonging to the same account.
    pub fn for_own_device(&self) -> bool {
        self.self_share
    }
    /// Public expiry including the published obfuscation, before clock tolerance.
    pub fn expires_at(&self) -> i64 {
        self.end
    }
    /// Validity including Google's half-hour clock tolerance.
    pub fn valid_at(&self, now: i64) -> bool {
        valid_at(self.start, self.end, now)
    }

    /// Match an encrypted advertisement key, commitment and AES-GCM metadata.
    /// This identifies a candidate; ECDSA paired-key checking is still required.
    pub fn identify(
        &self,
        key: EncryptedMetadataKey,
        now: i64,
    ) -> Result<DeviceMetadata, CertificateError> {
        if !self.valid_at(now) {
            return Err(CertificateError::Expired);
        }
        let metadata_key = crypto::crypt_metadata_key(&self.secret, &key.salt, &key.encrypted)?;
        if !bool::from(crypto::metadata_tag(metadata_key.as_ref())?.ct_eq(&self.metadata_tag)) {
            return Err(CertificateError::Integrity);
        }
        let plaintext = crypto::decrypt_metadata(
            &self.encrypted_metadata,
            metadata_key.as_ref(),
            self.secret.as_ref(),
        )?;
        let metadata =
            wire::Metadata::decode(plaintext.as_slice()).map_err(|_| CertificateError::Encoding)?;
        DeviceMetadata::from_wire(metadata)
    }

    /// Verify the peer's role-bound signature using a Google-delivered key.
    pub fn verify_role(&self, token: &[u8], role: PeerRole, signature: &[u8], now: i64) -> bool {
        if !self.valid_at(now) {
            return false;
        }
        let Ok(payload) = role.signed_payload(token) else {
            return false;
        };
        self.verify_raw(&payload, signature)
    }

    pub(crate) fn verify_raw(&self, payload: &[u8], signature: &[u8]) -> bool {
        if signature.len() > 80 {
            return false;
        }
        let Ok(signature) = Signature::from_der(signature) else {
            return false;
        };
        self.public_key.verify(payload, &signature).is_ok()
    }

    /// Proof sent to this peer of knowledge of its server-distributed certificate.
    pub fn authentication_hash(&self, token: &[u8]) -> Result<[u8; 6], CertificateError> {
        if token.len() != 32 {
            return Err(CertificateError::Key);
        }
        crypto::authentication_hash(token, self.secret.as_ref())
    }
}
