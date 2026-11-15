//! Original Nearby connection/control adapters, following Google's published
//! offline_wire_formats.proto and base_pcp_handler.cc at revision
//! 770a030be8fbebbcda5bf71ba718c35f5350c27c. No account identity from plain frames.
use crate::wire::WireError;
use prost::Message as _;

/// Nearby control events around encrypted payload traffic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Control {
    /// Payload is parsed separately by the bounded payload decoder.
    Payload,
    /// Keep-alive requests need a same-sequence acknowledgement.
    KeepAlive {
        /// Peer already acknowledges, rather than requests.
        ack: bool,
        /// Peer sequence, not an encryption sequence.
        sequence: u32,
    },
    /// Decline an upgrade without using peer-supplied network credentials.
    Upgrade {
        /// Requested medium code, if supplied.
        medium: Option<i32>,
    },
    /// Peer asks to close; no further file completion may be inferred.
    Disconnect {
        /// Safe-disconnect request.
        request: bool,
        /// Safe-disconnect acknowledgement.
        ack: bool,
    },
}

/// Private sharing endpoint info: version zero, no public name, computer form
/// factor, and the rotating encrypted metadata key for the local certificate.
pub fn private_endpoint_info(key: nearlane_certificates::EncryptedMetadataKey) -> Vec<u8> {
    // Advertisement ShareTargetType::kLaptop = 3 occupies bits 1..3.
    let mut out = vec![0x16];
    out.extend_from_slice(&key.to_bytes());
    out
}

/// Initial plain request; advertise only the implemented Wi-Fi LAN medium.
pub fn request(endpoint: &[u8; 4], info: &[u8], nonce: i32) -> Result<Vec<u8>, WireError> {
    if info.len() != 17 || !matches!(info[0], 0x10 | 0x16) {
        return Err(WireError::Invalid);
    }
    let body = Request {
        endpoint: Some(endpoint.to_vec()),
        name: Some(info.to_vec()),
        nonce: Some(nonce),
        mediums: vec![5],
        info: Some(info.to_vec()),
        keep_interval: Some(5000),
        keep_timeout: Some(30000),
    };
    Ok(envelope(1, 0, body.encode_to_vec()))
}
/// Validate a plain request for the owned-peer test receiver; never trust its
/// unverified name. Production currently implements outgoing connections only.
pub fn request_info(bytes: &[u8]) -> Result<Vec<u8>, WireError> {
    let body = Request::decode(single(bytes, 1, 0)?.as_slice()).map_err(|_| WireError::Invalid)?;
    if body.endpoint.as_ref().is_none_or(|id| id.len() != 4) {
        return Err(WireError::Invalid);
    }
    let info = body.info.or(body.name).ok_or(WireError::Invalid)?;
    if info.len() != 17 || !matches!(info[0], 0x10 | 0x16) {
        return Err(WireError::Invalid);
    }
    Ok(info)
}
/// Plain transport acceptance after UKEY2 and before encrypted traffic. The
/// advertised safe-disconnect version supports received-payload ACKs and closing.
pub fn acceptance() -> Vec<u8> {
    envelope(
        2,
        1,
        Response {
            status: Some(0),
            response: Some(1),
            os: Some(Os {
                kind: Some(host_os()),
            }),
            safe_version: Some(6),
        }
        .encode_to_vec(),
    )
}
// Public wire OS codes; do not impersonate another host platform.
fn host_os() -> i32 {
    if cfg!(target_os = "windows") {
        3
    } else if cfg!(target_os = "macos") {
        4
    } else if cfg!(target_os = "linux") {
        100
    } else {
        0
    }
}

/// Require explicit transport acceptance, returning its safe-disconnect feature
/// version. File ACK support (version 6) is optional, separate from acceptance
/// and cryptographic peer verification inside the encrypted stream.
pub fn require_acceptance(bytes: &[u8]) -> Result<i32, WireError> {
    let r = Response::decode(single(bytes, 2, 1)?.as_slice()).map_err(|_| WireError::Invalid)?;
    if r.response != Some(1) && !(r.response.is_none() && r.status == Some(0)) {
        return Err(WireError::Unsupported);
    }
    if r.status.is_some_and(|s| s != 0) {
        return Err(WireError::Unsupported);
    }
    let version = r.safe_version.unwrap_or(0);
    if version < 0 {
        return Err(WireError::Invalid);
    }
    Ok(version)
}
/// Decode only the supported transport-control families; reject conflicts.
pub fn control(bytes: &[u8]) -> Result<Control, WireError> {
    let frame = decode(bytes)?;
    let v1 = frame.v1.ok_or(WireError::Invalid)?;
    match v1.kind {
        Some(3) => {
            single(bytes, 3, 2)?;
            Ok(Control::Payload)
        }
        Some(4) => {
            let u =
                Upgrade::decode(single(bytes, 4, 3)?.as_slice()).map_err(|_| WireError::Invalid)?;
            if u.event != Some(1) {
                return Err(WireError::Unsupported);
            }
            Ok(Control::Upgrade {
                medium: u.path.and_then(|p| p.medium),
            })
        }
        Some(5) => {
            let k =
                Keep::decode(single(bytes, 5, 4)?.as_slice()).map_err(|_| WireError::Invalid)?;
            Ok(Control::KeepAlive {
                ack: k.ack.unwrap_or(false),
                sequence: k.sequence.unwrap_or(0),
            })
        }
        Some(6) => {
            let d = Disconnect::decode(single(bytes, 6, 5)?.as_slice())
                .map_err(|_| WireError::Invalid)?;
            Ok(Control::Disconnect {
                request: d.request.unwrap_or(false),
                ack: d.ack.unwrap_or(false),
            })
        }
        _ => Err(WireError::Unsupported),
    }
}
/// Reply to a keep-alive without inventing an application payload.
pub fn keep_alive(ack: bool, sequence: u32) -> Vec<u8> {
    envelope(
        5,
        4,
        Keep {
            ack: Some(ack),
            sequence: Some(sequence),
        }
        .encode_to_vec(),
    )
}
/// Explicitly refuse bandwidth redirection and remain on the chosen LAN socket.
pub fn decline_upgrade(medium: Option<i32>) -> Vec<u8> {
    envelope(
        4,
        3,
        Upgrade {
            event: Some(5),
            path: medium.map(|m| Path { medium: Some(m) }),
        }
        .encode_to_vec(),
    )
}
/// Safe-disconnect request/ack, without asserting payload delivery.
pub fn disconnect(request: bool, ack: bool) -> Vec<u8> {
    envelope(
        6,
        5,
        Disconnect {
            request: Some(request),
            ack: Some(ack),
        }
        .encode_to_vec(),
    )
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
    request: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    response: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    payload: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "5")]
    upgrade: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "6")]
    keep: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "7")]
    disconnect: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    paired: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "9")]
    auth: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "10")]
    auth_result: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "11")]
    resume: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "12")]
    reconnect: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "13")]
    retry: Option<Vec<u8>>,
}
impl V1 {
    fn fields(&self) -> [&Option<Vec<u8>>; 12] {
        [
            &self.request,
            &self.response,
            &self.payload,
            &self.upgrade,
            &self.keep,
            &self.disconnect,
            &self.paired,
            &self.auth,
            &self.auth_result,
            &self.resume,
            &self.reconnect,
            &self.retry,
        ]
    }
}
fn decode(bytes: &[u8]) -> Result<Frame, WireError> {
    if bytes.len() > crate::secure::MAX_PLAINTEXT {
        return Err(WireError::Size);
    }
    let f = Frame::decode(bytes).map_err(|_| WireError::Invalid)?;
    if f.version != Some(1) {
        return Err(WireError::Unsupported);
    }
    Ok(f)
}
fn single(bytes: &[u8], kind: i32, field: usize) -> Result<Vec<u8>, WireError> {
    let v = decode(bytes)?.v1.ok_or(WireError::Invalid)?;
    if v.kind != Some(kind)
        || v.fields()
            .iter()
            .enumerate()
            .any(|(i, f)| i != field && f.is_some())
    {
        return Err(WireError::Invalid);
    }
    v.fields()[field].clone().ok_or(WireError::Invalid)
}
fn envelope(kind: i32, field: usize, bytes: Vec<u8>) -> Vec<u8> {
    let mut v = V1 {
        kind: Some(kind),
        ..Default::default()
    };
    match field {
        0 => v.request = Some(bytes),
        1 => v.response = Some(bytes),
        3 => v.upgrade = Some(bytes),
        4 => v.keep = Some(bytes),
        5 => v.disconnect = Some(bytes),
        _ => unreachable!(),
    }
    Frame {
        version: Some(1),
        v1: Some(v),
    }
    .encode_to_vec()
}
#[derive(Clone, PartialEq, prost::Message)]
struct Request {
    #[prost(bytes = "vec", optional, tag = "1")]
    endpoint: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "2")]
    name: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "4")]
    nonce: Option<i32>,
    #[prost(int32, repeated, packed = "false", tag = "5")]
    mediums: Vec<i32>,
    #[prost(bytes = "vec", optional, tag = "6")]
    info: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "8")]
    keep_interval: Option<i32>,
    #[prost(int32, optional, tag = "9")]
    keep_timeout: Option<i32>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Response {
    #[prost(int32, optional, tag = "1")]
    status: Option<i32>,
    #[prost(int32, optional, tag = "3")]
    response: Option<i32>,
    #[prost(message, optional, tag = "4")]
    os: Option<Os>,
    #[prost(int32, optional, tag = "7")]
    safe_version: Option<i32>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Os {
    #[prost(int32, optional, tag = "1")]
    kind: Option<i32>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Keep {
    #[prost(bool, optional, tag = "1")]
    ack: Option<bool>,
    #[prost(uint32, optional, tag = "2")]
    sequence: Option<u32>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Disconnect {
    #[prost(bool, optional, tag = "1")]
    request: Option<bool>,
    #[prost(bool, optional, tag = "2")]
    ack: Option<bool>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Upgrade {
    #[prost(int32, optional, tag = "1")]
    event: Option<i32>,
    #[prost(message, optional, tag = "2")]
    path: Option<Path>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Path {
    #[prost(int32, optional, tag = "1")]
    medium: Option<i32>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn official_response_and_keepalive_field_layout() {
        let os = if cfg!(target_os = "windows") {
            3
        } else if cfg!(target_os = "macos") {
            4
        } else if cfg!(target_os = "linux") {
            100
        } else {
            0
        };
        assert_eq!(
            acceptance(),
            [8, 1, 18, 14, 8, 2, 26, 10, 8, 0, 24, 1, 34, 2, 8, os, 56, 6]
        );
        require_acceptance(&acceptance()).unwrap();
        assert_eq!(
            keep_alive(false, 7),
            [8, 1, 18, 8, 8, 5, 50, 4, 8, 0, 16, 7]
        );
        assert_eq!(
            control(&keep_alive(false, 7)).unwrap(),
            Control::KeepAlive {
                ack: false,
                sequence: 7
            }
        );
    }
    #[test]
    fn conflicting_subframes_are_rejected_but_legacy_acceptance_is_supported() {
        let mut f = Frame::decode(acceptance().as_slice()).unwrap();
        f.v1.as_mut().unwrap().keep = Some(Vec::new());
        assert!(require_acceptance(&f.encode_to_vec()).is_err());
        assert_eq!(
            require_acceptance(&envelope(
                2,
                1,
                Response {
                    status: Some(0),
                    response: Some(1),
                    os: None,
                    safe_version: Some(4)
                }
                .encode_to_vec()
            ))
            .unwrap(),
            4
        );
        assert!(
            require_acceptance(&envelope(
                2,
                1,
                Response {
                    status: Some(0),
                    response: Some(2),
                    os: None,
                    safe_version: Some(6)
                }
                .encode_to_vec()
            ))
            .is_err()
        );
    }
}
