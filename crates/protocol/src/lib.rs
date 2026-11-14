//! Bounded framing for Nearby's length-prefixed transport.
//!
//! This does not yet implement UKEY2 or claim Quick Share interoperability.
//! It handles the four-byte big-endian length envelope only.

use std::io::{self, Read, Write};

/// Conservative initial bound before allocating space for untrusted frames.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Read one frame, refusing empty or oversized data before allocation.
pub fn read_frame(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    reader.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame length",
        ));
    }
    let mut frame = vec![0; length];
    reader.read_exact(&mut frame)?;
    Ok(frame)
}

/// Write an envelope, enforcing the same limits as the reader.
pub fn write_frame(writer: &mut impl Write, frame: &[u8]) -> io::Result<()> {
    if frame.is_empty() || frame.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid frame length",
        ));
    }
    writer.write_all(&(frame.len() as u32).to_be_bytes())?;
    writer.write_all(frame)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn frame_boundaries_survive_streaming() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, b"first").unwrap();
        write_frame(&mut bytes, b"second").unwrap();
        let mut reader = bytes.as_slice();
        assert_eq!(read_frame(&mut reader).unwrap(), b"first");
        assert_eq!(read_frame(&mut reader).unwrap(), b"second");
        assert!(reader.is_empty());
    }
    #[test]
    fn rejects_length_bombs_without_reading_payload() {
        for size in [0, MAX_FRAME_BYTES as u32 + 1, u32::MAX] {
            assert_eq!(
                read_frame(&mut size.to_be_bytes().as_slice())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    }
    #[test]
    fn truncated_payload_does_not_become_a_valid_frame() {
        assert_eq!(
            read_frame(&mut [0, 0, 0, 4, 1, 2].as_slice())
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
