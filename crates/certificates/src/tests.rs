#![allow(clippy::unwrap_used)]

use nearlane_core::Visibility;
use prost::Message as _;

use crate::{test_vectors as v, *};

const NOW: i64 = 1_881_702_000;

fn google_certificate() -> wire::Certificate {
    wire::Certificate {
        id: v::CERTIFICATE_ID.to_vec(),
        secret: v::SECRET_KEY.to_vec(),
        public_key: v::PUBLIC_KEY.to_vec(),
        start: Some(wire::Timestamp {
            seconds: NOW,
            nanos: 0,
        }),
        end: Some(wire::Timestamp {
            seconds: NOW + VALIDITY_SECONDS,
            nanos: 0,
        }),
        metadata_key: Vec::new(),
        encrypted_metadata: v::ENCRYPTED_METADATA.to_vec(),
        metadata_key_tag: v::METADATA_TAG.to_vec(),
        self_share: false,
        binding_id: String::new(),
    }
}

#[test]
fn matches_google_metadata_crypto_vectors() {
    assert_eq!(
        crypto::metadata_tag(&v::METADATA_KEY).unwrap(),
        v::METADATA_TAG
    );
    assert_eq!(
        *crypto::crypt_metadata_key(&v::SECRET_KEY, &v::SALT, &v::METADATA_KEY).unwrap(),
        v::ENCRYPTED_METADATA_KEY
    );
    let plaintext =
        crypto::decrypt_metadata(&v::ENCRYPTED_METADATA, &v::METADATA_KEY, &v::SECRET_KEY).unwrap();
    assert_eq!(
        crypto::encrypt_metadata(&plaintext, &v::METADATA_KEY, &v::SECRET_KEY).unwrap(),
        v::ENCRYPTED_METADATA
    );
    assert_eq!(
        crypto::authentication_hash(b"sample", &v::SECRET_KEY).unwrap(),
        v::PAYLOAD_HASH
    );
}

#[test]
fn decrypts_google_certificate_and_verifies_rfc6979_signature() {
    let cert = RemoteCertificate::decode(&google_certificate().encode_to_vec()).unwrap();
    let mut bytes = [0; 16];
    bytes[..2].copy_from_slice(&v::SALT);
    bytes[2..].copy_from_slice(&v::ENCRYPTED_METADATA_KEY);
    let metadata = cert
        .identify(EncryptedMetadataKey::from_bytes(&bytes).unwrap(), NOW)
        .unwrap();
    assert_eq!(metadata.device_name(), "device_name");
    assert_eq!(
        metadata.bluetooth_address(),
        Some([0x4e, 0x65, 0x61, 0x72, 0x62, 0x79])
    );
    assert!(cert.verify_raw(b"sample", &v::SAMPLE_SIGNATURE));
    assert!(!cert.verify_raw(b"tampered", &v::SAMPLE_SIGNATURE));
    assert!(!format!("{cert:?} {metadata:?}").contains("device_name"));
}

#[test]
fn generated_certificate_signs_role_bound_peer_proof_and_rotates_advertisements() {
    let mut local = LocalCertificate::generate(
        DeviceMetadata::named("Nearlane test").unwrap(),
        Visibility::OwnDevices,
        NOW,
    )
    .unwrap();
    let remote = RemoteCertificate::decode(&local.publication_bytes().unwrap()).unwrap();
    assert!(remote.for_own_device());
    let a = local.advertisement_key().unwrap();
    let b = local.advertisement_key().unwrap();
    assert_ne!(&a.to_bytes()[..2], &b.to_bytes()[..2]);
    assert_eq!(
        remote.identify(a, NOW).unwrap().device_name(),
        "Nearlane test"
    );
    let token = [0x5a; 32];
    let signature = local.sign_role(&token, PeerRole::Sender).unwrap();
    assert!(remote.verify_role(&token, PeerRole::Sender, &signature, NOW));
    assert!(!remote.verify_role(&token, PeerRole::Receiver, &signature, NOW));
    assert!(!remote.verify_role(&[0x5b; 32], PeerRole::Sender, &signature, NOW));
    assert_eq!(
        local.authentication_hash(&token).unwrap(),
        remote.authentication_hash(&token).unwrap()
    );
}

#[test]
fn tampered_metadata_commitments_and_stale_certificates_are_rejected() {
    let key = EncryptedMetadataKey {
        salt: v::SALT,
        encrypted: v::ENCRYPTED_METADATA_KEY,
    };
    let mut cert = google_certificate();
    cert.metadata_key_tag[0] ^= 1;
    let remote = RemoteCertificate::decode(&cert.encode_to_vec()).unwrap();
    assert!(matches!(
        remote.identify(key, NOW),
        Err(CertificateError::Integrity)
    ));
    let mut cert = google_certificate();
    cert.encrypted_metadata[0] ^= 1;
    let remote = RemoteCertificate::decode(&cert.encode_to_vec()).unwrap();
    assert!(matches!(
        remote.identify(key, NOW),
        Err(CertificateError::Integrity)
    ));
    let remote = RemoteCertificate::decode(&google_certificate().encode_to_vec()).unwrap();
    assert!(remote.valid_at(NOW - CLOCK_SKEW_SECONDS));
    assert!(!remote.valid_at(NOW - CLOCK_SKEW_SECONDS - 1));
    assert!(matches!(
        remote.identify(key, NOW + VALIDITY_SECONDS + CLOCK_SKEW_SECONDS),
        Err(CertificateError::Expired)
    ));
}

#[test]
fn malformed_key_lengths_times_and_oversized_credentials_fail_before_use() {
    let mut cert = google_certificate();
    cert.secret.truncate(31);
    assert!(matches!(
        RemoteCertificate::decode(&cert.encode_to_vec()),
        Err(CertificateError::Key)
    ));
    let mut cert = google_certificate();
    cert.end.as_mut().unwrap().nanos = -1;
    assert!(matches!(
        RemoteCertificate::decode(&cert.encode_to_vec()),
        Err(CertificateError::Time)
    ));
    assert!(matches!(
        RemoteCertificate::decode(&vec![0; 16 * 1024 + 1]),
        Err(CertificateError::Limit)
    ));
    assert!(
        LocalCertificate::generate(
            DeviceMetadata::named("test").unwrap(),
            Visibility::Hidden,
            NOW
        )
        .is_err()
    );
}
