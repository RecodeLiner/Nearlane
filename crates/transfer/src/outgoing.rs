//! Explicit single-file outgoing operation, bound to a certificate-matched
//! discovery result. No scanning, Google tokens, incoming listener or redirects.
use nearlane_core::Text;
#[cfg(test)]
mod large_tests;
#[cfg(test)]
mod tests;
use crate::{
    Cancellation, FileOffer, FileSummary, TransferError,
    session::{DeadlineSocket, Session},
    validate_filename,
};
use nearlane_certificates::{OwnDeviceIdentity, PeerRole, RemoteCertificate};
use nearlane_discovery::OwnDevice;
use nearlane_protocol::{
    connection,
    secure::SecureChannel,
    wire::{self, ShareMessage},
};
use rand::RngCore as _;
use std::{
    fmt,
    fs::File,
    net::{Shutdown, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

/// Outgoing policy, independent of the protocol's bounded streaming buffers.
pub const MAX_OUTGOING_BYTES: u64 = nearlane_core::MAX_FILE_BYTES;
/// Safe progress phases; never includes network addresses, file paths or keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendPhase {
    /// Opening/validating the explicitly chosen source.
    Preparing,
    /// Connecting only to the selected discovered endpoint.
    Connecting,
    /// Nearby request, UKEY2 and transport acceptance.
    Securing,
    /// Both account-backed role-bound paired-key proofs must succeed.
    Verifying,
    /// Waiting for the receiver to accept this file's introduction.
    AwaitingAcceptance,
    /// Streaming the approved file while handling service messages.
    Sending,
    /// All bytes sent; successful delivery is still unconfirmed.
    AwaitingConfirmation,
    /// A matching payload ACK was received from the verified peer.
    Completed,
}
/// UI-safe progress, containing only phase and byte counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendProgress {
    /// Current operation phase.
    pub phase: SendPhase,
    /// Bytes streamed; never proof of completion by itself.
    pub bytes_sent: u64,
    /// Chosen source size after validation.
    pub total_bytes: u64,
}
impl Default for SendProgress {
    fn default() -> Self {
        Self {
            phase: SendPhase::Preparing,
            bytes_sent: 0,
            total_bytes: 0,
        }
    }
}
impl SendProgress {
    /// Plain progress text, independent of credentials or file/device identifiers.
    pub fn message(self) -> String {
        self.text().to_string()
    }

    /// Deferred text for a localized presentation layer.
    pub fn text(self) -> Text {
        match self.phase {
            SendPhase::Preparing => Text::new("Preparing the selected file…"),
            SendPhase::Connecting => Text::new("Connecting to the selected own device…"),
            SendPhase::Securing => Text::new("Establishing the encrypted Nearby connection…"),
            SendPhase::Verifying => Text::new("Checking private device keys in both directions…"),
            SendPhase::AwaitingAcceptance => Text::new("Waiting for the phone to accept the file…"),
            SendPhase::Sending => Text::with_args(
                "Sending %1 of %2 bytes…",
                [
                    Text::literal(self.bytes_sent),
                    Text::literal(self.total_bytes),
                ],
            ),
            SendPhase::AwaitingConfirmation => {
                Text::new("File bytes sent. Waiting for the phone's acknowledgement…")
            }
            SendPhase::Completed => Text::with_args(
                "The verified device acknowledged %1 bytes.",
                [Text::literal(self.total_bytes)],
            ),
        }
    }
}
/// Sanitized outgoing errors, never raw IO/provider messages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendFailureKind {
    /// Invalid, changed, non-regular or oversized selected file.
    File,
    /// No current own-device private credential is available.
    Identity,
    /// The discovered endpoint is too old; search again before connecting.
    StaleDiscovery,
    /// Network/framing/UKEY2/transport acceptance failed.
    Connection,
    /// Safe pre-encryption checkpoint; never includes received frames.
    Prelude(nearlane_protocol::secure::ConnectionStep),
    /// The required mutual private paired-key proof did not succeed.
    PrivateVerification,
    /// The receiver declined or cancelled this offer.
    Rejected,
    /// A message was unsupported, conflicting or out of sequence.
    Protocol,
    /// Bytes may have arrived, but receiver delivery could not be confirmed.
    ConfirmationMissing,
    /// This process cancelled and shut down its socket.
    Cancelled,
    /// Background operation stopped or could not start.
    Worker,
}
/// Phase plus bounded failure category, suitable for UI and issue reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendFailure {
    /// Phase reached before failure.
    pub phase: SendPhase,
    /// Sanitized category.
    pub kind: SendFailureKind,
}
impl SendFailure {
    /// Deferred, sanitized explanation with a separately translated stage.
    pub fn text(&self) -> Text {
        let message = match self.kind {
            SendFailureKind::File => Text::new(
                "Choose an unchanged regular file up to 64 GiB with a supported filename.",
            ),
            SendFailureKind::Identity => {
                Text::new("Current own-device certificates are unavailable. Retry device setup.")
            }
            SendFailureKind::StaleDiscovery => {
                Text::new("This discovery result expired. Find your devices again.")
            }
            SendFailureKind::Connection => {
                Text::new("The Nearby connection could not be established securely.")
            }
            SendFailureKind::Prelude(step) => Text::with_args(
                "Nearby connection prelude failed at %1. No file content was sent.",
                [Text::literal(format!("{step:?}"))],
            ),
            SendFailureKind::PrivateVerification => Text::new(
                "Mutual private-device verification failed. The phone may not yet have this computer's current certificate. No file content was sent.",
            ),
            SendFailureKind::Rejected => {
                Text::new("The receiving device declined or cancelled the transfer.")
            }
            SendFailureKind::Protocol => {
                Text::new("The peer sent an unsupported or out-of-sequence message.")
            }
            SendFailureKind::ConfirmationMissing => Text::new(
                "File bytes were sent, but the phone's acknowledgement was not received. Check the phone before retrying.",
            ),
            SendFailureKind::Cancelled => {
                Text::new("Transfer cancelled. A file already sent may still be on the phone.")
            }
            SendFailureKind::Worker => Text::new("The transfer worker stopped unexpectedly."),
        };

        Text::with_args("%1 (stage: %2)", [message, self.phase.text()])
    }
}
impl SendPhase {
    /// Localizable stage name, without progress values or private metadata.
    pub fn text(self) -> Text {
        Text::new(match self {
            Self::Preparing => "Preparing",
            Self::Connecting => "Connecting",
            Self::Securing => "Securing",
            Self::Verifying => "Verifying",
            Self::AwaitingAcceptance => "Awaiting acceptance",
            Self::Sending => "Sending",
            Self::AwaitingConfirmation => "Awaiting confirmation",
            Self::Completed => "Completed",
        })
    }
}
impl fmt::Display for SendFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text().fmt(f)
    }
}
impl std::error::Error for SendFailure {}

/// One cancellable outgoing operation. Drop discards late results and shuts down
/// an established socket; connection establishment itself has a five-second bound.
pub struct SendJob {
    cancellation: Cancellation,
    socket: Arc<Mutex<Option<TcpStream>>>,
    progress: Arc<Mutex<SendProgress>>,
    result: mpsc::Receiver<Result<FileSummary, SendFailure>>,
}
impl fmt::Debug for SendJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendJob")
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}
impl Drop for SendJob {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl SendJob {
    /// Explicitly approved selected file and selected own-device candidate only.
    /// No file content is transmitted before private proof and receiver consent.
    pub fn start(
        peer: OwnDevice,
        identity: OwnDeviceIdentity,
        path: PathBuf,
    ) -> Result<Self, SendFailure> {
        if !peer.is_recent() {
            return Err(failure(
                SendPhase::Preparing,
                SendFailureKind::StaleDiscovery,
            ));
        }
        let cancellation = Cancellation::default();
        let socket = Arc::new(Mutex::new(None));
        let progress = Arc::new(Mutex::new(SendProgress::default()));
        let stop = cancellation.clone();
        let worker_socket = Arc::clone(&socket);
        let worker_progress = Arc::clone(&progress);
        let (sender, result) = mpsc::channel();
        std::thread::Builder::new()
            .name("nearlane-send".into())
            .spawn(move || {
                let mut phase = SendPhase::Preparing;
                let outcome = (|| {
                    let mut file =
                        open_source(&path).map_err(|_| failure(phase, SendFailureKind::File))?;
                    let (name, size) = source_info(&file, &path)
                        .map_err(|_| failure(phase, SendFailureKind::File))?;
                    set(&worker_progress, phase, 0, size);
                    if stop.is_cancelled() {
                        return Err(failure(phase, SendFailureKind::Cancelled));
                    }
                    phase = SendPhase::Connecting;
                    set(&worker_progress, phase, 0, size);
                    let stream = TcpStream::connect_timeout(
                        &SocketAddr::V4(peer.socket_address()),
                        Duration::from_secs(5),
                    )
                    .map_err(|_| failure(phase, SendFailureKind::Connection))?;
                    configure(&stream).map_err(|_| failure(phase, SendFailureKind::Connection))?;
                    let copy = stream
                        .try_clone()
                        .map_err(|_| failure(phase, SendFailureKind::Connection))?;
                    if let Ok(mut slot) = worker_socket.lock() {
                        *slot = Some(copy);
                    }
                    if stop.is_cancelled() {
                        return Err(failure(phase, SendFailureKind::Cancelled));
                    }
                    send_stream(
                        stream,
                        &mut file,
                        (name, size),
                        &identity,
                        peer.certificate(),
                        stop.clone(),
                        |p| {
                            phase = p.phase;
                            set(&worker_progress, p.phase, p.bytes_sent, p.total_bytes)
                        },
                    )
                })();
                let outcome = if stop.is_cancelled() {
                    Err(failure(phase, SendFailureKind::Cancelled))
                } else {
                    outcome
                };
                if let Ok(mut slot) = worker_socket.lock() {
                    *slot = None;
                }
                let _ = sender.send(outcome);
            })
            .map_err(|_| failure(SendPhase::Preparing, SendFailureKind::Worker))?;
        Ok(Self {
            cancellation,
            socket,
            progress,
            result,
        })
    }
    /// Request cancellation and interrupt blocked established-socket IO.
    pub fn cancel(&self) {
        self.cancellation.cancel();
        if let Ok(slot) = self.socket.lock()
            && let Some(stream) = slot.as_ref()
        {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
    /// Read only safe scalar progress on the GUI thread.
    pub fn progress(&self) -> SendProgress {
        self.progress.lock().map(|p| *p).unwrap_or_default()
    }
    /// Nonblocking terminal result, discarded if the owning service signed out.
    pub fn poll(&self) -> Option<Result<FileSummary, SendFailure>> {
        match self.result.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err(failure(self.progress().phase, SendFailureKind::Worker))),
        }
    }
}
fn set(progress: &Mutex<SendProgress>, phase: SendPhase, bytes_sent: u64, total_bytes: u64) {
    if let Ok(mut p) = progress.lock() {
        *p = SendProgress {
            phase,
            bytes_sent,
            total_bytes,
        };
    }
}
fn failure(phase: SendPhase, kind: SendFailureKind) -> SendFailure {
    SendFailure { phase, kind }
}
pub(crate) fn configure(stream: &TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(20)))?;
    stream.set_nodelay(true)
}
fn source_info(file: &File, path: &Path) -> Result<(String, u64), TransferError> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_OUTGOING_BYTES {
        return Err(TransferError::SizeLimit);
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(TransferError::Filename)?
        .to_owned();
    validate_filename(&name)?;
    Ok((name, metadata.len()))
}
fn open_source(path: &Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        // Do not block opening a FIFO or follow a swapped source symlink while
        // a background worker holds a private certificate handle.
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    options.open(path)
}
pub(crate) fn send_stream(
    stream: TcpStream,
    source: &mut impl std::io::Read,
    source_info: (String, u64),
    identity: &OwnDeviceIdentity,
    remote: &RemoteCertificate,
    cancellation: Cancellation,
    mut update: impl FnMut(SendProgress),
) -> Result<FileSummary, SendFailure> {
    let (name, size) = source_info;
    let stop = cancellation.clone();
    let mut phase = SendPhase::Securing;
    let run = || -> Result<FileSummary, TransferError> {
        update(SendProgress {
            phase,
            bytes_sent: 0,
            total_bytes: size,
        });
        let mut random = [0; 8];
        rand::rngs::OsRng
            .try_fill_bytes(&mut random)
            .map_err(|_| TransferError::Worker)?;
        let alphabet = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        let mut endpoint = [0; 4];
        for (i, b) in endpoint.iter_mut().enumerate() {
            *b = alphabet[(random[i] as usize) % alphabet.len()];
        }
        let nonce = i32::from_le_bytes(random[4..].try_into().map_err(|_| TransferError::Worker)?);
        let request = connection::request(
            &endpoint,
            &connection::private_endpoint_info(identity.advertisement),
            nonce,
        )?;
        let channel = SecureChannel::initiate_nearby(DeadlineSocket::new(stream), &request)?;
        let mut session = Session::new(channel, cancellation);
        phase = SendPhase::Verifying;
        update(SendProgress {
            phase,
            bytes_sent: 0,
            total_bytes: size,
        });
        session.verify(&identity.certificate, remote, PeerRole::Sender)?;
        let offer = FileOffer {
            name,
            size,
            payload_id: session.reserve_id()?,
            attachment_id: session.reserve_id()?,
            mime_type: "application/octet-stream".into(),
        };
        phase = SendPhase::AwaitingAcceptance;
        update(SendProgress {
            phase,
            bytes_sent: 0,
            total_bytes: size,
        });
        session.send_bytes(wire::encode_share(&ShareMessage::Introduction(
            offer.clone(),
        ))?)?;
        match wire::decode_share(&session.read_bytes()?)? {
            ShareMessage::Accepted => {}
            ShareMessage::Rejected => return Err(TransferError::Rejected),
            ShareMessage::Cancel => return Err(TransferError::Cancelled),
            _ => return Err(TransferError::Sequence),
        }
        phase = SendPhase::Sending;
        update(SendProgress {
            phase,
            bytes_sent: 0,
            total_bytes: size,
        });
        let summary = session.send_file(source, &offer, |bytes_sent| {
            update(SendProgress {
                phase: SendPhase::Sending,
                bytes_sent,
                total_bytes: size,
            })
        })?;
        phase = SendPhase::AwaitingConfirmation;
        update(SendProgress {
            phase,
            bytes_sent: size,
            total_bytes: size,
        });
        let confirmation = session.wait_file_ack(offer.payload_id);
        session.finish();
        confirmation?;
        phase = SendPhase::Completed;
        update(SendProgress {
            phase,
            bytes_sent: size,
            total_bytes: size,
        });
        Ok(summary)
    };
    run().map_err(|error| {
        if stop.is_cancelled() {
            return failure(phase, SendFailureKind::Cancelled);
        }
        failure(
            phase,
            match error {
                _ if phase == SendPhase::AwaitingConfirmation => {
                    SendFailureKind::ConfirmationMissing
                }
                TransferError::Cancelled => SendFailureKind::Cancelled,
                TransferError::PeerVerification => SendFailureKind::PrivateVerification,
                TransferError::Channel(nearlane_protocol::secure::ChannelError::Prelude(step)) => {
                    SendFailureKind::Prelude(step)
                }
                TransferError::Rejected => SendFailureKind::Rejected,
                TransferError::SourceChanged
                | TransferError::Filename
                | TransferError::SizeLimit => SendFailureKind::File,
                TransferError::Sequence | TransferError::Wire(_) => SendFailureKind::Protocol,
                _ => SendFailureKind::Connection,
            },
        )
    })
}
