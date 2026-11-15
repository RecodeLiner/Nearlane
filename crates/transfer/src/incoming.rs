//! Explicit private receiver. The worker owns trust, consent, IO and staging.
#[cfg(test)]
mod tests;
use crate::{
    Cancellation, ReceivedFile, TransferError,
    file::FileSink,
    session::{DeadlineSocket, Event, Session},
};
use nearlane_certificates::{OwnDeviceIdentity, PeerRole, RemoteCertificate, now_seconds};
use nearlane_discovery::{PrivateAdvertisement, match_incoming};
use nearlane_protocol::{
    secure::SecureChannel,
    wire::{self, ShareMessage},
};
use rand::RngCore as _;
use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, Shutdown, TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

/// Stable Wi-Fi receiver port, matching the existing local Quick Share rule.
pub const RECEIVE_PORT: u16 = 7364;
const MAX_RECEIVE_BYTES: u64 = 64 * 1024 * 1024;

/// Details exposed only after account-backed live mutual private verification.
#[derive(Clone)]
pub struct IncomingOffer {
    /// Authenticated decrypted certificate name, never a plaintext advertisement.
    pub sender: String,
    /// Validated single basename, rendered as plain text in the UI.
    pub name: String,
    /// Proposed size, bounded to 64 MiB.
    pub bytes: u64,
}
impl fmt::Debug for IncomingOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IncomingOffer([REDACTED])")
    }
}
/// Safe receiver progress, independent of private identifiers and file contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceivePhase {
    /// Registering an encrypted LAN advertisement.
    Starting,
    /// Waiting for an own device, for at most ten minutes.
    Listening,
    /// Exchanging the plain request, UKEY2 and transport acceptance.
    Negotiating,
    /// Checking an incoming connection's account-backed keys.
    Verifying,
    /// A verified offer is waiting for explicit local approval.
    AwaitingConsent,
    /// Streaming an approved file into private temporary storage.
    Receiving {
        /// Bytes staged so far.
        bytes: u64,
        /// Approved total.
        total: u64,
    },
    /// Flushing/publishing the complete file without replacement.
    Saving,
}
impl ReceivePhase {
    /// Plain status text without names, credentials, paths or addresses.
    pub fn message(self) -> String {
        match self {
            Self::Starting => "Starting private Wi-Fi reception…".into(),
            Self::Listening => "Ready to receive from your own devices.".into(),
            Self::Negotiating => "Establishing the incoming encrypted Nearby connection…".into(),
            Self::Verifying => "Verifying the incoming device's private keys…".into(),
            Self::AwaitingConsent => {
                "A verified own device wants to send a file. Accept or decline below.".into()
            }
            Self::Receiving { bytes, total } => format!("Receiving {bytes} of {total} bytes…"),
            Self::Saving => "Saving the complete file…".into(),
        }
    }
}
/// Sanitized receiver failures; never provider text, keys or remote IO details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveFailure {
    /// A safe transport checkpoint failed before any file offer.
    Handshake(nearlane_protocol::secure::ConnectionStep),
    /// Port unavailable, possibly occupied by another Quick Share client.
    Listen,
    /// No private IPv4 interface or mDNS announcement could be established.
    Advertisement,
    /// Current own-device credentials are unavailable or invalid.
    Identity,
    /// Unmatched or unverified peer; no offer or content is accepted.
    Untrusted,
    /// Unsupported, malformed or out-of-sequence input.
    Protocol,
    /// The incoming offer was explicitly declined.
    Declined,
    /// Explicit cancellation or sign-out.
    Cancelled,
    /// A receiver/consent deadline or attempt bound was reached.
    TimedOut,
    /// Destination unavailable, file exists, or staging/saving failed.
    File,
    /// IO stopped before successful persistence.
    Connection,
    /// Background worker could not start or stopped unexpectedly.
    Worker,
}
impl fmt::Display for ReceiveFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Handshake(step) = self {
            return write!(
                f,
                "Incoming connection failed at {step:?}. No file was accepted."
            );
        }
        f.write_str(match self {
            Self::Handshake(_)=>unreachable!(),
            Self::Listen => "Could not open the receiving port. Close another Quick Share receiver and retry.",
            Self::Advertisement => "Private Wi-Fi advertisement could not start. Check the local network and mDNS.",
            Self::Identity => "Current own-device keys are unavailable. Prepare your devices again.",
            Self::Untrusted => "The incoming device could not be privately verified. No file was accepted.",
            Self::Protocol => "An unsupported incoming transfer was rejected.",
            Self::Declined => "Incoming file declined. No file was saved.",
            Self::Cancelled => "Reception stopped. Incomplete temporary files are removed; completed files are kept.",
            Self::TimedOut => "Private receiving expired. Start receiving again when ready.",
            Self::File => "Could not save the file without replacing an existing file. Choose a writable destination.",
            Self::Connection => "The incoming connection stopped before the file was saved.",
            Self::Worker => "The receiver worker stopped unexpectedly.",
        })
    }
}
impl std::error::Error for ReceiveFailure {}

struct State {
    phase: ReceivePhase,
    offer: Option<IncomingOffer>,
}
/// One explicitly enabled single-file receiving session. Drop cancels, shuts
/// down established IO and withdraws the advertisement as the worker finishes.
pub struct ReceiveJob {
    stop: Cancellation,
    socket: Arc<Mutex<Option<TcpStream>>>,
    state: Arc<Mutex<State>>,
    decisions: mpsc::SyncSender<bool>,
    result: mpsc::Receiver<Result<ReceivedFile, ReceiveFailure>>,
}
impl fmt::Debug for ReceiveJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReceiveJob([REDACTED])")
    }
}
impl Drop for ReceiveJob {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl ReceiveJob {
    /// Bind the LAN listener and start one private receiver worker. Never pass
    /// account tokens here; only current own-device certificate handles are used.
    pub fn start(
        identity: OwnDeviceIdentity,
        certificates: Vec<Arc<RemoteCertificate>>,
        directory: PathBuf,
        port: u16,
    ) -> Result<Self, ReceiveFailure> {
        validate_identity(&identity, &certificates)?;
        let listener =
            TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).map_err(|_| ReceiveFailure::Listen)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| ReceiveFailure::Listen)?;
        let port = listener
            .local_addr()
            .map_err(|_| ReceiveFailure::Listen)?
            .port();
        let stop = Cancellation::default();
        let state = Arc::new(Mutex::new(State {
            phase: ReceivePhase::Starting,
            offer: None,
        }));
        let socket = Arc::new(Mutex::new(None));
        let (decisions, input) = mpsc::sync_channel(1);
        let (tx, result) = mpsc::channel();
        let context = Context {
            identity,
            certificates,
            directory,
            stop: stop.clone(),
            state: Arc::clone(&state),
            decisions: input,
        };
        let worker_socket = Arc::clone(&socket);
        thread::Builder::new()
            .name("nearlane-receiver".into())
            .spawn(move || {
                let outcome = context.listen(listener, port, &worker_socket);
                let _ = tx.send(outcome);
            })
            .map_err(|_| ReceiveFailure::Worker)?;
        Ok(Self {
            stop,
            socket,
            state,
            decisions,
            result,
        })
    }
    /// Read only safe progress on the caller's thread.
    pub fn phase(&self) -> ReceivePhase {
        self.state
            .lock()
            .map(|s| s.phase)
            .unwrap_or(ReceivePhase::Starting)
    }
    /// Verified metadata awaiting explicit consent; never an unverified offer.
    pub fn offer(&self) -> Option<IncomingOffer> {
        self.state.lock().ok().and_then(|s| s.offer.clone())
    }
    /// First decision wins, and decisions outside a verified pending offer fail.
    pub fn decide(&self, accept: bool) -> bool {
        self.state
            .lock()
            .ok()
            .is_some_and(|s| s.phase == ReceivePhase::AwaitingConsent && s.offer.is_some())
            && self.decisions.try_send(accept).is_ok()
    }
    /// Cancel both listener lifetime and established blocking IO.
    pub fn cancel(&self) {
        self.stop.cancel();
        if let Ok(s) = self.socket.lock() {
            if let Some(s) = s.as_ref() {
                let _ = s.shutdown(Shutdown::Both);
            }
        }
    }
    /// Terminal saved-file result; a lost outgoing ACK cannot undo persistence.
    pub fn poll(&self) -> Option<Result<ReceivedFile, ReceiveFailure>> {
        match self.result.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err(ReceiveFailure::Worker)),
        }
    }
}
fn validate_identity(
    identity: &OwnDeviceIdentity,
    certificates: &[Arc<RemoteCertificate>],
) -> Result<(), ReceiveFailure> {
    let now = now_seconds().map_err(|_| ReceiveFailure::Identity)?;
    if !identity.certificate.active_at(now)
        || certificates.is_empty()
        || certificates.len() > 256
        || !certificates
            .iter()
            .any(|c| c.for_own_device() && c.valid_at(now))
    {
        return Err(ReceiveFailure::Identity);
    }
    Ok(())
}
struct Context {
    identity: OwnDeviceIdentity,
    certificates: Vec<Arc<RemoteCertificate>>,
    directory: PathBuf,
    stop: Cancellation,
    state: Arc<Mutex<State>>,
    decisions: mpsc::Receiver<bool>,
}
impl Context {
    fn phase(&self, phase: ReceivePhase) {
        if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
            eprintln!("receiver: phase {phase:?}");
        }
        if let Ok(mut s) = self.state.lock() {
            s.phase = phase;
        }
    }
    fn listen(
        &self,
        listener: TcpListener,
        port: u16,
        socket: &Mutex<Option<TcpStream>>,
    ) -> Result<ReceivedFile, ReceiveFailure> {
        std::fs::create_dir_all(&self.directory).map_err(|_| ReceiveFailure::File)?;
        let mut bytes = [0; 4];
        rand::rngs::OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| ReceiveFailure::Worker)?;
        let alphabet = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        let endpoint = bytes.map(|b| alphabet[b as usize % alphabet.len()]);
        let advertisement =
            PrivateAdvertisement::start(endpoint, self.identity.advertisement, port)
                .map_err(|_| ReceiveFailure::Advertisement)?;
        let announcement = Instant::now() + Duration::from_secs(5);
        loop {
            if self.stop.is_cancelled() {
                return Err(ReceiveFailure::Cancelled);
            }
            if advertisement
                .announced(Duration::from_millis(100))
                .map_err(|_| ReceiveFailure::Advertisement)?
            {
                break;
            }
            if Instant::now() >= announcement {
                return Err(ReceiveFailure::Advertisement);
            }
        }
        let deadline = Instant::now() + Duration::from_secs(600);
        let mut attempts = 0;
        self.phase(ReceivePhase::Listening);
        while Instant::now() < deadline && attempts < 16 {
            if self.stop.is_cancelled() {
                return Err(ReceiveFailure::Cancelled);
            }
            validate_identity(&self.identity, &self.certificates)?;
            match listener.accept() {
                Ok((stream, addr)) => {
                    if !matches!(addr.ip(), IpAddr::V4(v) if v.is_private() || v.is_link_local()) {
                        continue;
                    }
                    attempts += 1;
                    let copy = stream.try_clone().map_err(|_| ReceiveFailure::Connection)?;
                    *socket.lock().map_err(|_| ReceiveFailure::Worker)? = Some(copy);
                    let outcome = self.receive(stream);
                    if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                        if let Err(error) = &outcome {
                            eprintln!("receiver: rejected {error:?}");
                        }
                    }
                    *socket.lock().map_err(|_| ReceiveFailure::Worker)? = None;
                    if self.stop.is_cancelled() {
                        return Err(ReceiveFailure::Cancelled);
                    }
                    match outcome {
                        Err(
                            ReceiveFailure::Untrusted
                            | ReceiveFailure::Protocol
                            | ReceiveFailure::Connection
                            | ReceiveFailure::Handshake(_),
                        ) if self.state.lock().is_ok_and(|s| s.offer.is_none()) => {
                            self.phase(ReceivePhase::Listening);
                        }
                        result => return result,
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100))
                }
                Err(_) => return Err(ReceiveFailure::Listen),
            }
        }
        Err(ReceiveFailure::TimedOut)
    }
    fn receive(&self, stream: TcpStream) -> Result<ReceivedFile, ReceiveFailure> {
        self.phase(ReceivePhase::Negotiating);
        stream
            .set_nodelay(true)
            .map_err(|_| ReceiveFailure::Connection)?;
        let (channel, info) = SecureChannel::respond_nearby(DeadlineSocket::handshake(stream))
            .map_err(|e| match e {
                nearlane_protocol::secure::ChannelError::Prelude(step) => {
                    ReceiveFailure::Handshake(step)
                }
                _ => ReceiveFailure::Connection,
            })?;
        self.phase(ReceivePhase::Verifying);
        let (remote, sender) =
            match_incoming(&info, &self.certificates).ok_or(ReceiveFailure::Untrusted)?;
        let mut session = Session::new(channel, self.stop.clone());
        session
            .verify(&self.identity.certificate, &remote, PeerRole::Receiver)
            .map_err(|_| ReceiveFailure::Untrusted)?;
        if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
            eprintln!("receiver: mutual private verification passed");
        }
        session.begin_receive_consent();
        let introduction = session.read_bytes().map_err(protocol)?;
        if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
            eprintln!("receiver: {}", wire::diagnostic_share_shape(&introduction));
        }
        let ShareMessage::Introduction(offer) =
            wire::decode_share(&introduction).map_err(|error| {
                if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                    eprintln!("receiver: introduction decoder rejected {error:?}");
                }
                ReceiveFailure::Protocol
            })?
        else {
            return Err(ReceiveFailure::Protocol);
        };
        if offer.size > MAX_RECEIVE_BYTES
            || offer.payload_id == 0
            || crate::validate_filename(&offer.name).is_err()
        {
            if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                eprintln!(
                    "receiver: offer policy size_limit={} zero_payload_id={} invalid_filename={}",
                    offer.size > MAX_RECEIVE_BYTES,
                    offer.payload_id == 0,
                    crate::validate_filename(&offer.name).is_err()
                );
            }
            let _ = session.send_bytes(
                wire::encode_share(&ShareMessage::Rejected)
                    .map_err(|_| ReceiveFailure::Protocol)?,
            );
            return Err(ReceiveFailure::Protocol);
        }
        {
            let mut s = self.state.lock().map_err(|_| ReceiveFailure::Worker)?;
            s.offer = Some(IncomingOffer {
                sender,
                name: offer.name.clone(),
                bytes: offer.size,
            });
            s.phase = ReceivePhase::AwaitingConsent;
        }
        self.phase(ReceivePhase::AwaitingConsent);
        let accepted = self.consent(&mut session, &offer)?;
        if !accepted {
            let _ = session.send_bytes(
                wire::encode_share(&ShareMessage::Rejected)
                    .map_err(|_| ReceiveFailure::Protocol)?,
            );
            return Err(ReceiveFailure::Declined);
        }
        if self.stop.is_cancelled() {
            return Err(ReceiveFailure::Cancelled);
        }
        let mut sink = FileSink::new(&offer, &self.directory, MAX_RECEIVE_BYTES)
            .map_err(|_| ReceiveFailure::File)?;
        session
            .send_bytes(
                wire::encode_share(&ShareMessage::Accepted)
                    .map_err(|_| ReceiveFailure::Protocol)?,
            )
            .map_err(protocol)?;
        self.phase(ReceivePhase::Receiving {
            bytes: 0,
            total: offer.size,
        });
        loop {
            match session.next().map_err(protocol)? {
                Event::Ack(_) => {}
                Event::Bytes(bytes) => match wire::decode_share(&bytes) {
                    Ok(ShareMessage::Accepted) => {}
                    Ok(ShareMessage::Introduction(repeated)) if repeated == offer => {}
                    Ok(ShareMessage::Cancel | ShareMessage::Rejected) => {
                        return Err(ReceiveFailure::Cancelled);
                    }
                    _ if wire::is_progress(&bytes) => {}
                    _ => return Err(ReceiveFailure::Protocol),
                },
                Event::File(chunk) => {
                    let done = sink.apply(&chunk).map_err(|_| ReceiveFailure::Protocol)?;
                    self.phase(ReceivePhase::Receiving {
                        bytes: chunk.offset + chunk.body.len() as u64,
                        total: offer.size,
                    });
                    if done {
                        break;
                    }
                }
            }
        }
        if self.stop.is_cancelled() {
            return Err(ReceiveFailure::Cancelled);
        }
        self.phase(ReceivePhase::Saving);
        let saved = sink.finish().map_err(|_| ReceiveFailure::File)?;
        let _ = session.acknowledge_file(offer.payload_id);
        session.finish();
        Ok(saved)
    }
    fn consent(
        &self,
        session: &mut Session,
        offer: &crate::FileOffer,
    ) -> Result<bool, ReceiveFailure> {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut ping = Instant::now();
        while Instant::now() < deadline {
            if self.stop.is_cancelled() {
                return Err(ReceiveFailure::Cancelled);
            }
            if let Ok(accepted) = self.decisions.try_recv() {
                return Ok(accepted);
            }
            session.poll_consent_control(offer).map_err(protocol)?;
            if ping.elapsed() >= Duration::from_secs(5) {
                session.ping().map_err(protocol)?;
                ping = Instant::now();
            }
            thread::sleep(Duration::from_millis(50));
        }
        Ok(false)
    }
}
fn protocol(error: TransferError) -> ReceiveFailure {
    match error {
        TransferError::Cancelled => ReceiveFailure::Cancelled,
        TransferError::Io(_) | TransferError::Channel(_) => ReceiveFailure::Connection,
        _ => ReceiveFailure::Protocol,
    }
}
