//! Bounded framing for Nearby's length-prefixed transport.
//!
//! Framing, Google's published UKEY2/D2D adapter and a bounded payload subset.
//! Encrypted transport alone does not establish contact/device identity.

pub mod connection;
pub mod paired;
pub mod secure;
pub mod wire;

use std::io::{self, Read, Write};

/// Google Nearby's default channel read limit. The four-byte length prefix
/// counts the entire wire frame, including encryption and protobuf overhead.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Read one frame, refusing empty or oversized data before allocation.
pub fn read_frame(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    read_frame_bounded(reader, MAX_FRAME_BYTES)
}

/// Read with a stricter stage-specific bound, checked before allocation.
pub fn read_frame_bounded(reader: &mut impl Read, max: usize) -> io::Result<Vec<u8>> {
    if max == 0 || max > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid frame bound",
        ));
    }
    let mut header = [0_u8; 4];
    reader.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > max {
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
    fn one_mib_fits_exactly_after_the_length_prefix() {
        let frame = vec![0xab; 1024 * 1024];
        let mut wire = Vec::new();
        write_frame(&mut wire, &frame).unwrap();
        assert_eq!(wire.len(), frame.len() + 4);
        assert_eq!(read_frame(&mut wire.as_slice()).unwrap(), frame);
        assert!(write_frame(&mut Vec::new(), &vec![0; frame.len() + 1]).is_err());
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
