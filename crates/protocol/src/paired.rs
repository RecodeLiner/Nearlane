//! Original paired-key adapter following Google's public sharing/proto/wire_format.proto
//! and paired_key_verification_runner.cc, revision 770a030be8fbebbcda5bf71ba718c35f5350c27c.
//! An advertisement match alone is not live proof of key possession.

use nearlane_certificates::{LocalCertificate, PeerRole, RemoteCertificate};
use prost::Message as _;
use std::fmt;
use subtle::ConstantTimeEq as _;

/// Sanitized failures. Unknown or UNABLE peers never gain private access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairError {
    /// Conflicting/missing fields, unsupported frame or bounded encoding failure.
    Invalid,
    /// A local or peer certificate is not active now.
    Expired,
    /// Both credentials must be restricted to own-device sharing.
    NotOwnDevice,
    /// Signature or proof of the local discovery key did not match.
    Authentication,
    /// Peer did not confirm successful mutual verification.
    Rejected,
}
impl fmt::Display for PairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Private peer verification failed: {self:?}")
    }
}
impl std::error::Error for PairError {}

/// Encode role-bound signature and proof of the peer's shared certificate.
/// `remote` must come from the authenticated API and a matched advertisement.
pub fn encode_proof(
    local: &LocalCertificate,
    remote: &RemoteCertificate,
    token: &[u8],
    role: PeerRole,
    now: i64,
) -> Result<Vec<u8>, PairError> {
    check_certificates(local, remote, now)?;
    let proof = Encryption {
        signed: Some(
            local
                .sign_role(token, role)
                .map_err(|_| PairError::Invalid)?,
        ),
        hash: Some(
            remote
                .authentication_hash(token)
                .map_err(|_| PairError::Invalid)?
                .to_vec(),
        ),
        optional_signed: None,
        qr: None,
        provisioning: None,
    };
    Ok(frame(3, Some(proof.encode_to_vec()), None))
}

/// Check possession of the trusted peer's signing key and knowledge of this
/// device's certificate, bound to this UKEY2 session and the opposite role.
/// Local success still requires a successful paired-key result from the peer.
pub fn verify_proof(
    bytes: &[u8],
    local: &LocalCertificate,
    remote: &RemoteCertificate,
    token: &[u8],
    local_role: PeerRole,
    now: i64,
) -> Result<(), PairError> {
    check_certificates(local, remote, now)?;
    let proof = Encryption::decode(body(bytes, 3)?.as_slice()).map_err(|_| PairError::Invalid)?;
    let signature = proof.signed.as_deref().ok_or(PairError::Invalid)?;
    let hash = proof.hash.as_deref().ok_or(PairError::Invalid)?;
    if hash.len() != 6
        || signature.len() > 80
        || proof.optional_signed.as_ref().is_some_and(|s| s.len() > 80)
        || proof.qr.is_some()
    {
        return Err(PairError::Invalid);
    }
    let expected = local
        .authentication_hash(token)
        .map_err(|_| PairError::Invalid)?;
    let peer_role = if local_role == PeerRole::Sender {
        PeerRole::Receiver
    } else {
        PeerRole::Sender
    };
    let valid_signature = remote.verify_role(token, peer_role, signature, now)
        || proof
            .optional_signed
            .as_ref()
            .is_some_and(|s| remote.verify_role(token, peer_role, s, now));
    if !bool::from(expected.as_slice().ct_eq(hash)) || !valid_signature {
        return Err(PairError::Authentication);
    }
    Ok(())
}

/// Encode sharing PAIRED_KEY_RESULT without fabricating an OS identity.
pub fn encode_result(success: bool) -> Vec<u8> {
    frame(
        4,
        None,
        Some(
            ResultFrame {
                status: Some(if success { 1 } else { 2 }),
            }
            .encode_to_vec(),
        ),
    )
}
/// Require explicit SUCCESS; UNABLE, SHOULD_PROVISION and absent status fail.
pub fn require_success(bytes: &[u8]) -> Result<(), PairError> {
    let result = ResultFrame::decode(body(bytes, 4)?.as_slice()).map_err(|_| PairError::Invalid)?;
    if result.status == Some(1) {
        Ok(())
    } else {
        Err(PairError::Rejected)
    }
}
fn check_certificates(
    local: &LocalCertificate,
    remote: &RemoteCertificate,
    now: i64,
) -> Result<(), PairError> {
    if !local.for_own_device() || !remote.for_own_device() {
        return Err(PairError::NotOwnDevice);
    }
    if !local.active_at(now) || !remote.valid_at(now) {
        return Err(PairError::Expired);
    }
    Ok(())
}

#[derive(Clone, PartialEq, prost::Message)]
struct Frame {
    #[prost(int32, optional, tag = "1")]
    version: Option<i32>,
    #[prost(message, optional, tag = "2")]
    v1: Option<V1>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct V1 {
    #[prost(int32, optional, tag = "1")]
    kind: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    introduction: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    response: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    encryption: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "5")]
    result: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "6")]
    certificates: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "7")]
    progress: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    bindings: Option<Vec<u8>>,
}
fn frame(kind: i32, encryption: Option<Vec<u8>>, result: Option<Vec<u8>>) -> Vec<u8> {
    Frame {
        version: Some(1),
        v1: Some(V1 {
            kind: Some(kind),
            encryption,
            result,
            ..Default::default()
        }),
    }
    .encode_to_vec()
}
fn body(bytes: &[u8], kind: i32) -> Result<Vec<u8>, PairError> {
    if bytes.len() > 4096 {
        return Err(PairError::Invalid);
    }
    let frame = Frame::decode(bytes).map_err(|_| PairError::Invalid)?;
    if frame.version != Some(1) {
        return Err(PairError::Invalid);
    }
    let v1 = frame.v1.ok_or(PairError::Invalid)?;
    if v1.kind != Some(kind)
        || [
            &v1.introduction,
            &v1.response,
            &v1.certificates,
            &v1.progress,
            &v1.bindings,
        ]
        .into_iter()
        .any(Option::is_some)
    {
        return Err(PairError::Invalid);
    }
    match (kind, v1.encryption, v1.result) {
        (3, Some(bytes), None) | (4, None, Some(bytes)) => Ok(bytes),
        _ => Err(PairError::Invalid),
    }
}
#[derive(Clone, PartialEq, prost::Message)]
struct Encryption {
    #[prost(bytes = "vec", optional, tag = "1")]
    signed: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    hash: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    optional_signed: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    qr: Option<Vec<u8>>,
    #[prost(bool, optional, tag = "5")]
    provisioning: Option<bool>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ResultFrame {
    #[prost(int32, optional, tag = "1")]
    status: Option<i32>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use nearlane_certificates::{DeviceMetadata, VALIDITY_SECONDS};
    use nearlane_core::Visibility;
    const NOW: i64 = 1881702000;
    fn identity() -> (LocalCertificate, RemoteCertificate) {
        let local = LocalCertificate::generate(
            DeviceMetadata::named("synthetic").unwrap(),
            Visibility::OwnDevices,
            NOW,
        )
        .unwrap();
        let remote = RemoteCertificate::decode(&local.publication_bytes().unwrap()).unwrap();
        (local, remote)
    }
    #[test]
    fn binds_signing_and_certificate_knowledge_to_session_and_role() {
        let (a, a_public) = identity();
        let (b, b_public) = identity();
        let token = [5; 32];
        let proof = encode_proof(&a, &b_public, &token, PeerRole::Sender, NOW).unwrap();
        verify_proof(&proof, &b, &a_public, &token, PeerRole::Receiver, NOW).unwrap();
        assert!(verify_proof(&proof, &b, &a_public, &[6; 32], PeerRole::Receiver, NOW).is_err());
        assert!(verify_proof(&proof, &b, &a_public, &token, PeerRole::Sender, NOW).is_err());
        let (_, wrong_public) = identity();
        assert!(verify_proof(&proof, &b, &wrong_public, &token, PeerRole::Receiver, NOW).is_err());
        let wrong_hash = encode_proof(&a, &wrong_public, &token, PeerRole::Sender, NOW).unwrap();
        assert!(verify_proof(&wrong_hash, &b, &a_public, &token, PeerRole::Receiver, NOW).is_err());
        assert_eq!(
            verify_proof(
                &proof,
                &b,
                &a_public,
                &token,
                PeerRole::Receiver,
                NOW + VALIDITY_SECONDS
            ),
            Err(PairError::Expired)
        );
    }
    #[test]
    fn rejects_conflicting_frames_unable_and_missing_confirmation() {
        assert_eq!(encode_result(true), [8, 1, 18, 6, 8, 4, 42, 2, 8, 1]);
        require_success(&encode_result(true)).unwrap();
        assert_eq!(
            require_success(&encode_result(false)),
            Err(PairError::Rejected)
        );
        for status in [None, Some(0), Some(3), Some(4)] {
            assert_eq!(
                require_success(&frame(
                    4,
                    None,
                    Some(ResultFrame { status }.encode_to_vec())
                )),
                Err(PairError::Rejected)
            );
        }
        let mut parsed = Frame::decode(encode_result(true).as_slice()).unwrap();
        parsed.v1.as_mut().unwrap().introduction = Some(vec![]);
        assert_eq!(
            require_success(&parsed.encode_to_vec()),
            Err(PairError::Invalid)
        );
        assert_eq!(require_success(&vec![0; 4097]), Err(PairError::Invalid));
    }
}
