//! Initial, bounded subset of Google's published Nearby/Quick Share protobufs.
//!
//! Field numbers come from google/nearby revision
//! 770a030be8fbebbcda5bf71ba718c35f5350c27c (Apache-2.0). Original Rust adapter.

use std::fmt;

use prost::Message as _;

/// Maximum file/bytes body in one streaming chunk.
pub const MAX_CHUNK: usize = 64 * 1024;

/// Payload families implemented by this initial adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadKind {
    /// A small serialized Quick Share control message.
    Bytes,
    /// File content streamed independently of its introduction metadata.
    File,
}

/// A validated single chunk. Debug omits its contents and filename.
pub struct PayloadChunk {
    /// Opaque per-transfer payload identifier.
    pub id: i64,
    /// Underlying Nearby payload family.
    pub kind: PayloadKind,
    /// Advertised length of the complete payload.
    pub total_size: u64,
    /// Sequential byte offset of this chunk.
    pub offset: u64,
    /// Sequential chunk index, including the separate terminal chunk.
    pub index: u32,
    /// Bounded chunk body, not the full file.
    pub body: Vec<u8>,
    /// Whether this is the terminal chunk.
    pub last: bool,
    /// Optional transport filename; never used directly as an output path.
    pub file_name: Option<String>,
    /// Folder transfers are not supported by the initial file receiver.
    pub parent_folder: Option<String>,
}

impl fmt::Debug for PayloadChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PayloadChunk")
            .field("id", &self.id)
            .field("offset", &self.offset)
            .field("body_bytes", &self.body.len())
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

/// Nearby payload-layer events; unrelated frame families are rejected here.
#[derive(Debug)]
pub enum PayloadEvent {
    /// A validated payload fragment.
    Data(PayloadChunk),
    /// Peer confirmed reception of the payload with this ID.
    Acknowledged(i64),
    /// Peer cancelled this payload.
    Cancelled(i64),
    /// Peer reported a payload error.
    Failed(i64),
}

/// Safe parser errors without raw peer data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireError {
    /// Missing, conflicting, malformed or out-of-range fields.
    Invalid,
    /// A valid-looking message outside the supported subset.
    Unsupported,
    /// The bounded message/chunk size was exceeded.
    Size,
}
impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Nearby wire frame rejected: {self:?}")
    }
}
impl std::error::Error for WireError {}

/// Encode an official Nearby PAYLOAD_TRANSFER/DATA envelope.
pub fn encode_data(chunk: &PayloadChunk) -> Result<Vec<u8>, WireError> {
    validate_chunk(chunk)?;
    Ok(envelope(Transfer {
        packet_type: Some(1),
        header: Some(Header {
            id: Some(chunk.id),
            kind: Some(match chunk.kind {
                PayloadKind::Bytes => 1,
                PayloadKind::File => 2,
            }),
            total_size: Some(chunk.total_size as i64),
            file_name: chunk.file_name.clone(),
            parent_folder: chunk.parent_folder.clone(),
        }),
        chunk: Some(Chunk {
            flags: Some(i32::from(chunk.last)),
            offset: Some(chunk.offset as i64),
            body: (!chunk.body.is_empty()).then(|| chunk.body.clone()),
            index: Some(chunk.index as i32),
        }),
        control: None,
    })
    .encode_to_vec())
}

/// Encode Nearby's explicit PAYLOAD_ACK, whose header only needs the payload ID.
pub fn encode_ack(id: i64) -> Vec<u8> {
    envelope(Transfer {
        packet_type: Some(3),
        header: Some(Header {
            id: Some(id),
            ..Default::default()
        }),
        ..Default::default()
    })
    .encode_to_vec()
}

/// Encode a payload cancellation without introducing a custom protocol message.
pub fn encode_cancel(id: i64, offset: u64) -> Result<Vec<u8>, WireError> {
    let offset = i64::try_from(offset).map_err(|_| WireError::Invalid)?;
    Ok(envelope(Transfer {
        packet_type: Some(2),
        header: Some(Header {
            id: Some(id),
            ..Default::default()
        }),
        control: Some(Control {
            event: Some(2),
            offset: Some(offset),
        }),
        ..Default::default()
    })
    .encode_to_vec())
}

/// Decode and validate a bounded payload event before the file layer sees it.
pub fn decode_payload(bytes: &[u8]) -> Result<PayloadEvent, WireError> {
    if bytes.len() > crate::secure::MAX_PLAINTEXT {
        return Err(WireError::Size);
    }
    let frame = NearbyFrame::decode(bytes).map_err(|_| WireError::Invalid)?;
    if frame.version != Some(1) {
        return Err(WireError::Unsupported);
    }
    let v1 = frame.v1.ok_or(WireError::Invalid)?;
    if v1.kind != Some(3) || v1.other_frames().any(Option::is_some) {
        return Err(WireError::Unsupported);
    }
    let transfer = v1.transfer.ok_or(WireError::Invalid)?;
    let header = transfer.header.ok_or(WireError::Invalid)?;
    let id = header.id.ok_or(WireError::Invalid)?;
    match transfer.packet_type {
        Some(1) => {
            if transfer.control.is_some() {
                return Err(WireError::Invalid);
            }
            let chunk = transfer.chunk.ok_or(WireError::Invalid)?;
            let flags = chunk.flags.unwrap_or(0);
            if flags & !1 != 0 {
                return Err(WireError::Unsupported);
            }
            let data = PayloadChunk {
                id,
                kind: match header.kind {
                    Some(1) => PayloadKind::Bytes,
                    Some(2) => PayloadKind::File,
                    _ => return Err(WireError::Unsupported),
                },
                total_size: u64::try_from(header.total_size.ok_or(WireError::Invalid)?)
                    .map_err(|_| WireError::Invalid)?,
                offset: u64::try_from(chunk.offset.ok_or(WireError::Invalid)?)
                    .map_err(|_| WireError::Invalid)?,
                index: u32::try_from(chunk.index.unwrap_or(0)).map_err(|_| WireError::Invalid)?,
                body: chunk.body.unwrap_or_default(),
                last: flags & 1 != 0,
                file_name: header.file_name,
                parent_folder: header.parent_folder,
            };
            validate_chunk(&data)?;
            Ok(PayloadEvent::Data(data))
        }
        Some(2) => {
            if transfer.chunk.is_some() {
                return Err(WireError::Invalid);
            }
            let control = transfer.control.ok_or(WireError::Invalid)?;
            if control.offset.is_some_and(|o| o < 0) {
                return Err(WireError::Invalid);
            }
            match control.event {
                Some(1) => Ok(PayloadEvent::Failed(id)),
                Some(2) => Ok(PayloadEvent::Cancelled(id)),
                Some(3) => Ok(PayloadEvent::Acknowledged(id)),
                _ => Err(WireError::Unsupported),
            }
        }
        Some(3) if transfer.chunk.is_none() && transfer.control.is_none() => {
            Ok(PayloadEvent::Acknowledged(id))
        }
        _ => Err(WireError::Unsupported),
    }
}

fn validate_chunk(chunk: &PayloadChunk) -> Result<(), WireError> {
    if chunk.body.len() > MAX_CHUNK || chunk.index > i32::MAX as u32 {
        return Err(WireError::Size);
    }
    let end = chunk
        .offset
        .checked_add(chunk.body.len() as u64)
        .ok_or(WireError::Invalid)?;
    if chunk.total_size > i64::MAX as u64
        || end > chunk.total_size
        || (chunk.last && end != chunk.total_size)
        || (!chunk.last && chunk.body.is_empty())
    {
        return Err(WireError::Invalid);
    }
    if chunk.file_name.as_ref().is_some_and(|s| s.len() > 255)
        || chunk.parent_folder.as_ref().is_some_and(|s| s.len() > 1024)
    {
        return Err(WireError::Size);
    }
    Ok(())
}

fn envelope(transfer: Transfer) -> NearbyFrame {
    NearbyFrame {
        version: Some(1),
        v1: Some(NearbyV1 {
            kind: Some(3),
            transfer: Some(transfer),
            ..Default::default()
        }),
    }
}

/// Single-file introduction metadata. Remote names must pass the file layer's
/// containment checks before any output file is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    /// Displayed basename, never a path.
    pub name: String,
    /// Nearby FILE payload ID.
    pub payload_id: i64,
    /// Advertised number of file bytes.
    pub size: u64,
    /// Quick Share attachment identifier.
    pub attachment_id: i64,
    /// Bounded descriptive MIME type; it does not control filesystem execution.
    pub mime_type: String,
}

/// Sharing-layer messages supported after peer verification.
#[derive(Debug, Eq, PartialEq)]
pub enum ShareMessage {
    /// One offered file.
    Introduction(FileMetadata),
    /// Receiver accepted this introduction.
    Accepted,
    /// Receiver declined this introduction.
    Rejected,
    /// Sharing session cancelled.
    Cancel,
}

/// Encode a bounded Quick Share sharing-layer message for a Nearby BYTES payload.
pub fn encode_share(message: &ShareMessage) -> Result<Vec<u8>, WireError> {
    let v1 = match message {
        ShareMessage::Introduction(meta) => {
            validate_metadata(meta)?;
            ShareV1 {
                kind: Some(1),
                introduction: Some(Introduction {
                    start_transfer: Some(true),
                    use_case: Some(1),
                    files: vec![FileMeta {
                        name: Some(meta.name.clone()),
                        payload_id: Some(meta.payload_id),
                        size: Some(meta.size as i64),
                        mime_type: Some(meta.mime_type.clone()),
                        id: Some(meta.attachment_id),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }
        }
        ShareMessage::Accepted | ShareMessage::Rejected => ShareV1 {
            kind: Some(2),
            response: Some(Response {
                status: Some(if *message == ShareMessage::Accepted {
                    1
                } else {
                    2
                }),
            }),
            ..Default::default()
        },
        ShareMessage::Cancel => ShareV1 {
            kind: Some(6),
            ..Default::default()
        },
    };
    Ok(ShareFrame {
        version: Some(1),
        v1: Some(v1),
    }
    .encode_to_vec())
}

/// Decode the initial one-file sharing subset. Multi-file, text, folders, Wi-Fi
/// credentials and app packages are explicitly unsupported at this stage.
pub fn decode_share(bytes: &[u8]) -> Result<ShareMessage, WireError> {
    if bytes.len() > MAX_CHUNK {
        return Err(WireError::Size);
    }
    let frame = ShareFrame::decode(bytes).map_err(|_| WireError::Invalid)?;
    if frame.version != Some(1) {
        return Err(WireError::Unsupported);
    }
    let v1 = frame.v1.ok_or(WireError::Invalid)?;
    if v1.encryption.is_some()
        || v1.key_result.is_some()
        || v1.certificates.is_some()
        || v1.progress.is_some()
        || v1.bindings.is_some()
    {
        return Err(WireError::Unsupported);
    }
    match v1.kind {
        Some(1) if v1.response.is_none() => {
            let intro = v1.introduction.ok_or(WireError::Invalid)?;
            if intro.files.len() != 1
                || !intro.text.is_empty()
                || !intro.wifi.is_empty()
                || !intro.apps.is_empty()
                || !intro.streams.is_empty()
            {
                return Err(WireError::Unsupported);
            }
            let file = intro.files.into_iter().next().ok_or(WireError::Invalid)?;
            if file.parent_folder.is_some_and(|s| !s.is_empty()) {
                return Err(WireError::Unsupported);
            }
            let meta = FileMetadata {
                name: file.name.ok_or(WireError::Invalid)?,
                payload_id: file.payload_id.ok_or(WireError::Invalid)?,
                size: u64::try_from(file.size.ok_or(WireError::Invalid)?)
                    .map_err(|_| WireError::Invalid)?,
                attachment_id: file.id.ok_or(WireError::Invalid)?,
                mime_type: file
                    .mime_type
                    .unwrap_or_else(|| "application/octet-stream".into()),
            };
            validate_metadata(&meta)?;
            Ok(ShareMessage::Introduction(meta))
        }
        Some(2) if v1.introduction.is_none() => match v1.response.and_then(|r| r.status) {
            Some(1) => Ok(ShareMessage::Accepted),
            Some(2..=5) => Ok(ShareMessage::Rejected),
            _ => Err(WireError::Unsupported),
        },
        Some(6) if v1.introduction.is_none() && v1.response.is_none() => Ok(ShareMessage::Cancel),
        _ => Err(WireError::Unsupported),
    }
}

fn validate_metadata(meta: &FileMetadata) -> Result<(), WireError> {
    if meta.name.is_empty()
        || meta.name.len() > 255
        || meta.mime_type.len() > 255
        || meta.size > i64::MAX as u64
    {
        return Err(WireError::Invalid);
    }
    Ok(())
}

/// Recognize bounded progress messages. Advisory progress never substitutes for
/// receiver acceptance or a matching file acknowledgement.
pub fn is_progress(bytes: &[u8]) -> bool {
    if bytes.len() > MAX_CHUNK {
        return false;
    }
    let Ok(frame) = ShareFrame::decode(bytes) else {
        return false;
    };
    let Some(v) = frame.v1 else {
        return false;
    };
    frame.version == Some(1)
        && v.kind == Some(7)
        && v.progress.is_some()
        && v.introduction.is_none()
        && v.response.is_none()
        && v.encryption.is_none()
        && v.key_result.is_none()
        && v.certificates.is_none()
        && v.bindings.is_none()
}

#[derive(Clone, PartialEq, prost::Message)]
struct NearbyFrame {
    #[prost(int32, optional, tag = "1")]
    version: Option<i32>,
    #[prost(message, optional, tag = "2")]
    v1: Option<NearbyV1>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct NearbyV1 {
    #[prost(int32, optional, tag = "1")]
    kind: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    request: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "3")]
    response: Option<Vec<u8>>,
    #[prost(message, optional, tag = "4")]
    transfer: Option<Transfer>,
    #[prost(bytes = "vec", optional, tag = "5")]
    upgrade: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "6")]
    keepalive: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "7")]
    disconnect: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    paired_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "9")]
    auth_message: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "10")]
    auth_result: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "11")]
    resume: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "12")]
    reconnect: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "13")]
    retry: Option<Vec<u8>>,
}
impl NearbyV1 {
    fn other_frames(&self) -> impl Iterator<Item = &Option<Vec<u8>>> {
        [
            &self.request,
            &self.response,
            &self.upgrade,
            &self.keepalive,
            &self.disconnect,
            &self.paired_key,
            &self.auth_message,
            &self.auth_result,
            &self.resume,
            &self.reconnect,
            &self.retry,
        ]
        .into_iter()
    }
}
#[derive(Clone, PartialEq, prost::Message)]
struct Transfer {
    #[prost(int32, optional, tag = "1")]
    packet_type: Option<i32>,
    #[prost(message, optional, tag = "2")]
    header: Option<Header>,
    #[prost(message, optional, tag = "3")]
    chunk: Option<Chunk>,
    #[prost(message, optional, tag = "4")]
    control: Option<Control>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Header {
    #[prost(int64, optional, tag = "1")]
    id: Option<i64>,
    #[prost(int32, optional, tag = "2")]
    kind: Option<i32>,
    #[prost(int64, optional, tag = "3")]
    total_size: Option<i64>,
    #[prost(string, optional, tag = "5")]
    file_name: Option<String>,
    #[prost(string, optional, tag = "6")]
    parent_folder: Option<String>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Chunk {
    #[prost(int32, optional, tag = "1")]
    flags: Option<i32>,
    #[prost(int64, optional, tag = "2")]
    offset: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "3")]
    body: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "4")]
    index: Option<i32>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Control {
    #[prost(int32, optional, tag = "1")]
    event: Option<i32>,
    #[prost(int64, optional, tag = "2")]
    offset: Option<i64>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ShareFrame {
    #[prost(int32, optional, tag = "1")]
    version: Option<i32>,
    #[prost(message, optional, tag = "2")]
    v1: Option<ShareV1>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ShareV1 {
    #[prost(int32, optional, tag = "1")]
    kind: Option<i32>,
    #[prost(message, optional, tag = "2")]
    introduction: Option<Introduction>,
    #[prost(message, optional, tag = "3")]
    response: Option<Response>,
    #[prost(bytes = "vec", optional, tag = "4")]
    encryption: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "5")]
    key_result: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "6")]
    certificates: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "7")]
    progress: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "8")]
    bindings: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Introduction {
    #[prost(message, repeated, tag = "1")]
    files: Vec<FileMeta>,
    #[prost(bytes = "vec", repeated, tag = "2")]
    text: Vec<Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "4")]
    wifi: Vec<Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "5")]
    apps: Vec<Vec<u8>>,
    #[prost(bool, optional, tag = "6")]
    start_transfer: Option<bool>,
    #[prost(bytes = "vec", repeated, tag = "7")]
    streams: Vec<Vec<u8>>,
    #[prost(int32, optional, tag = "8")]
    use_case: Option<i32>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct FileMeta {
    #[prost(string, optional, tag = "1")]
    name: Option<String>,
    #[prost(int64, optional, tag = "3")]
    payload_id: Option<i64>,
    #[prost(int64, optional, tag = "4")]
    size: Option<i64>,
    #[prost(string, optional, tag = "5")]
    mime_type: Option<String>,
    #[prost(int64, optional, tag = "6")]
    id: Option<i64>,
    #[prost(string, optional, tag = "7")]
    parent_folder: Option<String>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Response {
    #[prost(int32, optional, tag = "1")]
    status: Option<i32>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn introduction_sets_google_nearby_share_start_fields() {
        let encoded = encode_share(&ShareMessage::Introduction(FileMetadata {
            name: "data.bin".into(),
            payload_id: 7,
            size: 3,
            attachment_id: 8,
            mime_type: "application/octet-stream".into(),
        }))
        .unwrap();
        // IntroductionFrame: bool start_transfer = 6; NEARBY_SHARE use_case = 8.
        assert!(encoded.ends_with(&[48, 1, 64, 1]));
    }
    #[test]
    fn file_chunk_matches_published_field_number_fixture() {
        let chunk = PayloadChunk {
            id: 7,
            kind: PayloadKind::File,
            total_size: 3,
            offset: 0,
            index: 0,
            body: b"abc".to_vec(),
            last: false,
            file_name: None,
            parent_folder: None,
        };
        let fixture = [
            8, 1, 18, 27, 8, 3, 34, 23, 8, 1, 18, 6, 8, 7, 16, 2, 24, 3, 26, 11, 8, 0, 16, 0, 26,
            3, 97, 98, 99, 32, 0,
        ];
        assert_eq!(encode_data(&chunk).unwrap(), fixture);
        let PayloadEvent::Data(decoded) = decode_payload(&fixture).unwrap() else {
            panic!("wrong packet type")
        };
        assert_eq!(decoded.body, b"abc");
        assert_eq!(decoded.total_size, 3);
    }
    #[test]
    fn terminal_chunk_has_an_index_and_omits_an_empty_body_like_the_reference() {
        let chunk = PayloadChunk {
            id: 7,
            kind: PayloadKind::File,
            total_size: 94,
            offset: 94,
            index: 1,
            body: Vec::new(),
            last: true,
            file_name: None,
            parent_folder: None,
        };
        // PayloadChunk fields: flags=LAST_CHUNK, offset=94, index=1; no body.
        let fixture = [
            8, 1, 18, 22, 8, 3, 34, 18, 8, 1, 18, 6, 8, 7, 16, 2, 24, 94, 26, 6, 8, 1, 16, 94, 32,
            1,
        ];
        assert_eq!(encode_data(&chunk).unwrap(), fixture);
        let PayloadEvent::Data(decoded) = decode_payload(&fixture).unwrap() else {
            panic!("not data")
        };
        assert!(decoded.last);
        assert_eq!(decoded.index, 1);
        assert!(decoded.body.is_empty());
    }

    #[test]
    fn rejects_negative_offsets_conflicting_subframes_and_early_last() {
        let header = Header {
            id: Some(1),
            kind: Some(2),
            total_size: Some(3),
            ..Default::default()
        };
        for (offset, flags, control) in [
            (-1, 0, None),
            (0, 1, None),
            (
                0,
                0,
                Some(Control {
                    event: Some(2),
                    offset: Some(0),
                }),
            ),
        ] {
            let packet = envelope(Transfer {
                packet_type: Some(1),
                header: Some(header.clone()),
                chunk: Some(Chunk {
                    flags: Some(flags),
                    offset: Some(offset),
                    body: Some(b"x".to_vec()),
                    index: Some(0),
                }),
                control,
            });
            assert!(decode_payload(&packet.encode_to_vec()).is_err());
        }
    }
    #[test]
    fn recognizes_payload_ack_and_cancel_without_file_data() {
        assert!(matches!(
            decode_payload(&encode_ack(17)).unwrap(),
            PayloadEvent::Acknowledged(17)
        ));
        assert!(matches!(
            decode_payload(&encode_cancel(18, 0).unwrap()).unwrap(),
            PayloadEvent::Cancelled(18)
        ));
    }
}
