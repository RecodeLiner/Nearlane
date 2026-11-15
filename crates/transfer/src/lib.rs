//! Streaming file payloads using Nearby's published wire format.
//!
//! Production callers must establish certificate-backed peer trust and explicit
//! acceptance before file content. The desktop wires an explicit outgoing
//! operation to a recently certificate-matched candidate. This crate does not
//! discover or advertise devices and does not open a production file receiver.

mod file;
mod loopback;
mod outgoing;
mod session;
pub use outgoing::{SendFailure, SendFailureKind, SendJob, SendPhase, SendProgress};

use std::{
    fmt, io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub use file::{receive_file, send_file, validate_filename};
pub use loopback::{SelfTestReport, run_loopback_self_test};
pub use nearlane_protocol::wire::FileMetadata as FileOffer;
use nearlane_protocol::{secure::ChannelError, wire::WireError};

/// Conservative default per-file limit; configurable by a future consent screen.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;

/// Explicit cancellation, checked between bounded chunks. The socket owner must
/// also use IO deadlines so a stalled peer cannot block cancellation indefinitely.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    /// Request cancellation of this operation only.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Streaming digest and byte count; does not imply Google account/device trust.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSummary {
    /// Number of file bytes processed.
    pub bytes: u64,
    /// SHA-256 of those bytes (diagnostic; not an extra wire protocol field).
    pub sha256: [u8; 32],
}

/// A fully received file, published without overwriting an existing destination.
#[derive(Debug)]
pub struct ReceivedFile {
    /// Local destination chosen by the receiver, never a remote folder path.
    pub path: PathBuf,
    /// Observed length and digest.
    pub summary: FileSummary,
}

/// Transfer failures contain no file content, keys or raw peer frames.
#[derive(Debug)]
pub enum TransferError {
    /// Filesystem or input stream failure.
    Io(io::Error),
    /// Encrypted transport failed.
    Channel(ChannelError),
    /// Nearby wire validation failed.
    Wire(WireError),
    /// A remote filename is unsafe or unsupported on supported platforms.
    Filename,
    /// An advertised size exceeds receiver policy.
    SizeLimit,
    /// Payload ID, type, size, offset or terminal marker changed unexpectedly.
    Sequence,
    /// Source length changed while it was being read.
    SourceChanged,
    /// The peer or a local action cancelled the transfer.
    Cancelled,
    /// The receiver declined the offer or could not save it.
    Rejected,
    /// The required peer authentication proof did not succeed.
    PeerVerification,
    /// An owned self-test worker stopped or timed out.
    Worker,
}
impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io(_) => "File transfer IO failed",
            Self::Channel(_) => "Encrypted Nearby transport failed",
            Self::Wire(_) => "Nearby payload frame rejected",
            Self::Filename => "Unsafe or unsupported file name",
            Self::SizeLimit => "File exceeds the receiver's size limit",
            Self::Sequence => "File payload sequence or length is invalid",
            Self::SourceChanged => "Source file length changed during transfer",
            Self::Cancelled => "File transfer cancelled",
            Self::Rejected => "File offer rejected",
            Self::PeerVerification => "Private peer verification failed",
            Self::Worker => "Local transfer test worker failed",
        })
    }
}
impl std::error::Error for TransferError {}
impl From<io::Error> for TransferError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<ChannelError> for TransferError {
    fn from(e: ChannelError) -> Self {
        Self::Channel(e)
    }
}
impl From<WireError> for TransferError {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}
