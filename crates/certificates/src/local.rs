use std::{collections::HashSet, fmt};

use nearlane_core::Visibility;
use p256::{
    ecdsa::{Signature, SigningKey, signature::Signer as _},
    pkcs8::EncodePublicKey as _,
};
use prost::Message as _;
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

use crate::{
    CertificateError, DeviceMetadata, EncryptedMetadataKey, PeerRole, VALIDITY_SECONDS, crypto,
    wire,
};

/// Private signing/discovery keys of this installation. Neither Clone nor Debug
/// exposes secrets. The signing key is never included in publication bytes.
pub struct LocalCertificate {
    signing: SigningKey,
    secret: Zeroizing<[u8; 32]>,
    metadata_key: Zeroizing<[u8; 14]>,
    id: [u8; 32],
    metadata: DeviceMetadata,
    visibility: Visibility,
    start: i64,
    end: i64,
    public_start: i64,
    public_end: i64,
    salts: HashSet<u16>,
}
impl fmt::Debug for LocalCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LocalCertificate([REDACTED])")
    }
}

impl LocalCertificate {
    /// Opaque certificate identifier for internal cache binding, not account proof.
    pub fn id(&self) -> &[u8; 32] {
        &self.id
    }
    /// Generate immutable metadata and independent per-certificate keys. A new
    /// certificate is required when metadata changes: AES-GCM nonce reuse with
    /// changed plaintext under the same key is not allowed.
    pub fn generate(
        metadata: DeviceMetadata,
        visibility: Visibility,
        start: i64,
    ) -> Result<Self, CertificateError> {
        if !matches!(visibility, Visibility::OwnDevices | Visibility::Contacts) {
            return Err(CertificateError::Unsupported);
        }
        let end = start
            .checked_add(VALIDITY_SECONDS)
            .ok_or(CertificateError::Time)?;
        let mut signing = None;
        for _ in 0..128 {
            if let Ok(key) = SigningKey::from_slice(crypto::random::<32>()?.as_ref()) {
                signing = Some(key);
                break;
            }
        }
        let signing = signing.ok_or(CertificateError::Entropy)?;
        let secret = crypto::random::<32>()?;
        let metadata_key = crypto::random::<14>()?;
        let id = Sha256::digest(secret.as_ref()).into();
        let jitter = crypto::random::<4>()?;
        let before = u16::from_be_bytes([jitter[0], jitter[1]]) as i64 % 7200;
        let after = u16::from_be_bytes([jitter[2], jitter[3]]) as i64 % 7200;
        Ok(Self {
            signing,
            secret,
            metadata_key,
            id,
            metadata,
            visibility,
            start,
            end,
            public_start: start.checked_sub(before).ok_or(CertificateError::Time)?,
            public_end: end.checked_add(after).ok_or(CertificateError::Time)?,
            salts: HashSet::new(),
        })
    }

    /// Serialize the sharing credential uploaded to Google. This includes
    /// discovery secrets (as required by the protocol), not the ECDSA private key.
    /// Keep the result in trusted transport memory, out of logs, storage and UI state.
    pub fn publication_bytes(&self) -> Result<Zeroizing<Vec<u8>>, CertificateError> {
        let public_key = self
            .signing
            .verifying_key()
            .to_public_key_der()
            .map_err(|_| CertificateError::Key)?;
        let metadata = Zeroizing::new(self.metadata.0.encode_to_vec());
        let encrypted_metadata =
            crypto::encrypt_metadata(&metadata, self.metadata_key.as_ref(), self.secret.as_ref())?;
        let certificate = wire::Certificate {
            id: self.id.to_vec(),
            secret: self.secret.to_vec(),
            public_key: public_key.as_bytes().to_vec(),
            start: Some(wire::Timestamp {
                seconds: self.public_start,
                nanos: 0,
            }),
            end: Some(wire::Timestamp {
                seconds: self.public_end,
                nanos: 0,
            }),
            metadata_key: self.metadata_key.to_vec(),
            encrypted_metadata,
            metadata_key_tag: crypto::metadata_tag(self.metadata_key.as_ref())?.to_vec(),
            self_share: self.visibility == Visibility::OwnDevices,
            binding_id: String::new(),
        };
        Ok(Zeroizing::new(certificate.encode_to_vec()))
    }

    /// Expiration used by the credential-publication wrapper.
    pub fn published_expiration(&self) -> i64 {
        self.public_end
    }

    /// Stable opaque credential tag for this certificate within this device.
    /// It is not used as a peer identity or as a cryptographic verification proof.
    pub fn credential_tag(&self) -> i64 {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&self.id[..8]);
        i64::from_le_bytes(bytes)
    }

    /// Whether the local certificate is active for signing (no clock tolerance).
    pub fn active_at(&self, now: i64) -> bool {
        now >= self.start && now < self.end
    }

    /// Whether this private key was generated exclusively for own-device use.
    pub fn for_own_device(&self) -> bool {
        self.visibility == Visibility::OwnDevices
    }

    /// Make a fresh rotating advertisement key. Do not reuse salts or exhaust
    /// more than half the two-byte space; regenerate certificates at that point.
    pub fn advertisement_key(&mut self) -> Result<EncryptedMetadataKey, CertificateError> {
        if self.salts.len() >= 32768 {
            return Err(CertificateError::Limit);
        }
        for _ in 0..128 {
            let salt = *crypto::random::<2>()?;
            if self.salts.insert(u16::from_be_bytes(salt)) {
                let encrypted =
                    *crypto::crypt_metadata_key(&self.secret, &salt, &self.metadata_key)?;
                return Ok(EncryptedMetadataKey { salt, encrypted });
            }
        }
        Err(CertificateError::Entropy)
    }

    /// Sign the role-prefixed UKEY2 authentication value for paired-key checking.
    pub fn sign_role(&self, token: &[u8], role: PeerRole) -> Result<Vec<u8>, CertificateError> {
        let payload = Zeroizing::new(role.signed_payload(token)?);
        let signature: Signature = self.signing.sign(&payload);
        Ok(signature.to_der().as_bytes().to_vec())
    }

    /// Six-byte proof that the peer knows this certificate's shared discovery key.
    pub fn authentication_hash(&self, token: &[u8]) -> Result<[u8; 6], CertificateError> {
        if token.len() != 32 {
            return Err(CertificateError::Key);
        }
        crypto::authentication_hash(token, self.secret.as_ref())
    }
}
