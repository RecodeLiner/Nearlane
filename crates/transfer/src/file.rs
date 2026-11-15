use std::{
    io::{Read, Write},
    path::{Component, Path},
};

use nearlane_protocol::{
    secure::SecureChannel,
    wire::{self, MAX_CHUNK, PayloadChunk, PayloadEvent, PayloadKind},
};
use sha2::{Digest as _, Sha256};
use tempfile::NamedTempFile;

use crate::{Cancellation, FileOffer, FileSummary, ReceivedFile, TransferError};

/// Validate a portable single basename. Nested folders, path traversal, Windows
/// device names/alternate streams and hidden control characters are not accepted.
pub fn validate_filename(name: &str) -> Result<(), TransferError> {
    if name.is_empty()
        || name.len() > 255
        || name.ends_with(['.', ' '])
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
    {
        return Err(TransferError::Filename);
    }
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(TransferError::Filename);
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(TransferError::Filename);
    }
    Ok(())
}

/// Stream an already-accepted file over an authenticated channel using Nearby
/// FILE chunks, then wait for the receiver's matching PAYLOAD_ACK. Memory usage
/// is bounded by chunk/frame limits, independently of the file's total length.
pub fn send_file<S: Read + Write>(
    channel: &mut SecureChannel<S>,
    source: &mut impl Read,
    offer: &FileOffer,
    max_file_bytes: u64,
    cancellation: &Cancellation,
) -> Result<FileSummary, TransferError> {
    validate_offer(offer, max_file_bytes)?;
    let mut offset = 0;
    let mut index = 0;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; MAX_CHUNK];
    while offset < offer.size {
        if cancellation.is_cancelled() {
            let _ = channel.send(&wire::encode_cancel(offer.payload_id, offset)?);
            return Err(TransferError::Cancelled);
        }
        let length = (offer.size - offset).min(MAX_CHUNK as u64) as usize;
        source.read_exact(&mut buffer[..length]).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                TransferError::SourceChanged
            } else {
                e.into()
            }
        })?;
        digest.update(&buffer[..length]);
        channel.send(&wire::encode_data(&PayloadChunk {
            id: offer.payload_id,
            kind: PayloadKind::File,
            total_size: offer.size,
            offset,
            index,
            body: buffer[..length].to_vec(),
            last: false,
            file_name: Some(offer.name.clone()),
            parent_folder: None,
        })?)?;
        offset += length as u64;
        index += 1;
    }
    if cancellation.is_cancelled() {
        let _ = channel.send(&wire::encode_cancel(offer.payload_id, offset)?);
        return Err(TransferError::Cancelled);
    }
    let mut extra = [0];
    if source.read(&mut extra)? != 0 {
        return Err(TransferError::SourceChanged);
    }
    // The explicit empty terminal chunk also represents a zero-length file.
    channel.send(&wire::encode_data(&PayloadChunk {
        id: offer.payload_id,
        kind: PayloadKind::File,
        total_size: offer.size,
        offset,
        index,
        body: Vec::new(),
        last: true,
        file_name: Some(offer.name.clone()),
        parent_folder: None,
    })?)?;
    match wire::decode_payload(&channel.receive()?)? {
        PayloadEvent::Acknowledged(id) if id == offer.payload_id => Ok(FileSummary {
            bytes: offset,
            sha256: digest.finalize().into(),
        }),
        PayloadEvent::Cancelled(id) if id == offer.payload_id => Err(TransferError::Cancelled),
        _ => Err(TransferError::Sequence),
    }
}

/// Receive a previously accepted, peer-verified offer. Stage in a private random
/// file, require contiguous chunks and exact completion, then publish atomically
/// without replacing existing files. Any earlier error drops the partial file.
pub fn receive_file<S: Read + Write>(
    channel: &mut SecureChannel<S>,
    offer: &FileOffer,
    directory: &Path,
    max_file_bytes: u64,
    cancellation: &Cancellation,
) -> Result<ReceivedFile, TransferError> {
    let mut sink = FileSink::new(offer, directory, max_file_bytes)?;
    loop {
        if cancellation.is_cancelled() {
            let _ = channel.send(&wire::encode_cancel(offer.payload_id, sink.offset)?);
            return Err(TransferError::Cancelled);
        }
        match wire::decode_payload(&channel.receive()?)? {
            PayloadEvent::Data(chunk) => {
                if sink.apply(&chunk)? {
                    break;
                }
            }
            PayloadEvent::Cancelled(id) if id == offer.payload_id => {
                return Err(TransferError::Cancelled);
            }
            PayloadEvent::Failed(id) if id == offer.payload_id => {
                return Err(TransferError::Rejected);
            }
            _ => return Err(TransferError::Sequence),
        }
    }
    let file = sink.finish()?;
    // A lost acknowledgement can leave a complete saved file at the receiver;
    // it must never cause the sender to claim confirmed delivery.
    channel.send(&wire::encode_ack(offer.payload_id))?;
    Ok(file)
}

fn validate_offer(offer: &FileOffer, max_file_bytes: u64) -> Result<(), TransferError> {
    validate_filename(&offer.name)?;
    if offer.size > max_file_bytes || offer.size > i64::MAX as u64 {
        return Err(TransferError::SizeLimit);
    }
    Ok(())
}

pub(crate) struct FileSink {
    offer: FileOffer,
    temporary: NamedTempFile,
    destination: std::path::PathBuf,
    offset: u64,
    digest: Sha256,
    complete: bool,
    failed: bool,
}

impl FileSink {
    pub(crate) fn new(
        offer: &FileOffer,
        directory: &Path,
        max: u64,
    ) -> Result<Self, TransferError> {
        validate_offer(offer, max)?;
        let temporary = tempfile::Builder::new()
            .prefix(".nearlane-")
            .tempfile_in(directory)?;
        Ok(Self {
            offer: offer.clone(),
            temporary,
            destination: directory.join(&offer.name),
            offset: 0,
            digest: Sha256::new(),
            complete: false,
            failed: false,
        })
    }

    pub(crate) fn apply(&mut self, chunk: &PayloadChunk) -> Result<bool, TransferError> {
        let result = self.apply_checked(chunk);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn apply_checked(&mut self, chunk: &PayloadChunk) -> Result<bool, TransferError> {
        if self.complete
            || self.failed
            || chunk.id != self.offer.payload_id
            || chunk.kind != PayloadKind::File
            || chunk.total_size != self.offer.size
            || chunk.offset != self.offset
            || chunk.body.len() > MAX_CHUNK
            || chunk
                .file_name
                .as_ref()
                .is_some_and(|s| s != &self.offer.name)
            || chunk.parent_folder.as_ref().is_some_and(|s| !s.is_empty())
        {
            return Err(TransferError::Sequence);
        }
        let end = self
            .offset
            .checked_add(chunk.body.len() as u64)
            .ok_or(TransferError::Sequence)?;
        if end > self.offer.size
            || (chunk.last && end != self.offer.size)
            || (!chunk.last && chunk.body.is_empty())
        {
            return Err(TransferError::Sequence);
        }
        self.temporary.write_all(&chunk.body)?;
        self.digest.update(&chunk.body);
        self.offset = end;
        self.complete = chunk.last;
        Ok(self.complete)
    }

    pub(crate) fn finish(mut self) -> Result<ReceivedFile, TransferError> {
        if !self.complete || self.failed || self.offset != self.offer.size {
            return Err(TransferError::Sequence);
        }
        self.temporary.flush()?;
        self.temporary.as_file().sync_all()?;
        let summary = FileSummary {
            bytes: self.offset,
            sha256: self.digest.finalize().into(),
        };
        self.temporary
            .persist_noclobber(&self.destination)
            .map_err(|e| TransferError::Io(e.error))?;
        Ok(ReceivedFile {
            path: self.destination,
            summary,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn offer(name: &str, size: u64) -> FileOffer {
        FileOffer {
            name: name.into(),
            size,
            payload_id: 7,
            attachment_id: 8,
            mime_type: "application/octet-stream".into(),
        }
    }
    fn chunk(offset: u64, body: &[u8], last: bool) -> PayloadChunk {
        PayloadChunk {
            index: 0,
            id: 7,
            kind: PayloadKind::File,
            total_size: 4,
            offset,
            body: body.to_vec(),
            last,
            file_name: None,
            parent_folder: None,
        }
    }

    #[test]
    fn rejects_paths_control_characters_and_portable_device_names() {
        for name in [
            "",
            ".",
            "..",
            "../escape",
            "a/b",
            "a\\b",
            "/tmp/a",
            "C:stream",
            "CON",
            "nul.txt",
            "LPT1.log",
            "x\n.txt",
            "x.",
            "x ",
        ] {
            assert!(
                validate_filename(name).is_err(),
                "accepted unsafe fixture name"
            );
        }
        assert!(validate_filename("Фото 2026.txt").is_ok());
    }

    #[test]
    fn invalid_sequence_drops_staging_file_and_never_publishes_partial_data() {
        let directory = tempfile::tempdir().unwrap();
        let mut sink = FileSink::new(&offer("safe.txt", 4), directory.path(), 10).unwrap();
        assert!(!sink.apply(&chunk(0, b"ab", false)).unwrap());
        assert!(sink.apply(&chunk(1, b"cd", true)).is_err());
        assert!(sink.finish().is_err());
        assert_eq!(directory.path().read_dir().unwrap().count(), 0);
    }

    #[test]
    fn existing_destination_is_not_overwritten_even_by_a_complete_transfer() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("safe.txt");
        std::fs::write(&existing, b"keep-me").unwrap();
        let mut sink = FileSink::new(&offer("safe.txt", 4), directory.path(), 10).unwrap();
        assert!(sink.apply(&chunk(0, b"data", true)).unwrap());
        assert!(sink.finish().is_err());
        assert_eq!(std::fs::read(existing).unwrap(), b"keep-me");
        assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    }

    #[test]
    fn size_policy_is_checked_before_creating_a_file() {
        let directory = tempfile::tempdir().unwrap();
        assert!(FileSink::new(&offer("large.bin", 11), directory.path(), 10).is_err());
        assert_eq!(directory.path().read_dir().unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_destination_cannot_redirect_the_write() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("received");
        std::fs::create_dir(&directory).unwrap();
        let outside = root.path().join("outside.txt");
        std::fs::write(&outside, b"keep-me").unwrap();
        std::os::unix::fs::symlink(&outside, directory.join("safe.txt")).unwrap();
        let mut sink = FileSink::new(&offer("safe.txt", 4), &directory, 10).unwrap();
        sink.apply(&chunk(0, b"data", true)).unwrap();
        assert!(sink.finish().is_err());
        assert_eq!(std::fs::read(outside).unwrap(), b"keep-me");
    }
}
