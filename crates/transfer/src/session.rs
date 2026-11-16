//! Bounded duplex control handling on the selected encrypted LAN socket.
use crate::limits::{Clock, ControlBudget};
use crate::{Cancellation, FileOffer, FileSummary, TransferError};
use nearlane_certificates::{LocalCertificate, PeerRole, RemoteCertificate, now_seconds};
use nearlane_protocol::{
    connection::{self, Control},
    paired,
    secure::SecureChannel,
    wire::{self, MAX_CHUNK, PayloadChunk, PayloadEvent, PayloadKind},
};
use rand::RngCore as _;
use sha2::{Digest as _, Sha256};
use std::{
    cell::Cell,
    collections::HashSet,
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};

/// An absolute deadline also bounds partial headers and slow trickle bodies.
pub(crate) struct DeadlineSocket {
    socket: TcpStream,
    deadline: Cell<Instant>,
}
impl DeadlineSocket {
    pub(crate) fn handshake(socket: TcpStream) -> Self {
        Self {
            socket,
            deadline: Cell::new(Instant::now() + Duration::from_secs(15)),
        }
    }
    pub(crate) fn new(socket: TcpStream) -> Self {
        Self {
            socket,
            deadline: Cell::new(Instant::now() + Duration::from_secs(15)),
        }
    }
    fn remaining(&self, cap: Duration) -> std::io::Result<Duration> {
        self.deadline
            .get()
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .map(|d| d.min(cap))
            .ok_or_else(|| std::io::Error::new(ErrorKind::TimedOut, "session deadline"))
    }
    #[cfg(test)]
    pub(crate) fn for_test(socket: TcpStream) -> Self {
        Self {
            socket,
            deadline: Cell::new(Instant::now() + Duration::from_secs(3)),
        }
    }
}
impl Read for DeadlineSocket {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.socket
            .set_read_timeout(Some(self.remaining(Duration::from_secs(30))?))?;
        self.socket.read(buf)
    }
}
impl Write for DeadlineSocket {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.socket
            .set_write_timeout(Some(self.remaining(Duration::from_secs(20))?))?;
        self.socket.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.socket.flush()
    }
}
pub(crate) enum Event {
    Bytes(Vec<u8>),
    File(PayloadChunk),
    Ack(i64),
}
struct Assembly {
    id: i64,
    size: u64,
    bytes: Vec<u8>,
}
pub(crate) struct Session {
    channel: SecureChannel<DeadlineSocket>,
    sent: HashSet<i64>,
    received: HashSet<i64>,
    assembly: Option<Assembly>,
    cancellation: Cancellation,
    clock: Clock,
    controls: ControlBudget,
}
impl Session {
    pub(crate) fn new(channel: SecureChannel<DeadlineSocket>, cancellation: Cancellation) -> Self {
        Self {
            channel,
            sent: HashSet::new(),
            received: HashSet::new(),
            assembly: None,
            cancellation,
            clock: Clock::bounded(Instant::now(), Duration::from_secs(180)),
            controls: ControlBudget::new(Instant::now()),
        }
    }
    fn check(&self) -> Result<(), TransferError> {
        if self.cancellation.is_cancelled() {
            return Err(TransferError::Cancelled);
        }
        if self.clock.expired(Instant::now()) {
            return Err(std::io::Error::new(ErrorKind::TimedOut, "session deadline").into());
        }
        Ok(())
    }
    fn prepare_frame(&self) -> Result<(), TransferError> {
        self.check()?;
        self.channel
            .io_ref()
            .deadline
            .set(self.clock.frame_deadline(Instant::now()));
        Ok(())
    }
    fn send_frame(&mut self, bytes: &[u8]) -> Result<(), TransferError> {
        self.prepare_frame()?;
        self.channel.send(bytes)?;
        Ok(())
    }
    fn receive_frame(&mut self) -> Result<Vec<u8>, TransferError> {
        self.prepare_frame()?;
        Ok(self.channel.receive()?)
    }
    fn control_frame(&mut self) -> Result<(), TransferError> {
        if self.controls.record(Instant::now()) {
            Ok(())
        } else {
            Err(TransferError::Sequence)
        }
    }
    pub(crate) fn begin_file_transfer(&mut self) {
        self.clock = Clock::data(Instant::now());
    }
    pub(crate) fn file_progress(&mut self) {
        self.clock.progress(Instant::now());
    }
    pub(crate) fn reserve_id(&mut self) -> Result<i64, TransferError> {
        if self.sent.len() >= 32 {
            return Err(TransferError::Sequence);
        }
        for _ in 0..16 {
            let mut bytes = [0; 8];
            rand::rngs::OsRng
                .try_fill_bytes(&mut bytes)
                .map_err(|_| TransferError::Worker)?;
            let id = i64::from_le_bytes(bytes) & i64::MAX;
            if id != 0 && self.sent.insert(id) {
                return Ok(id);
            }
        }
        Err(TransferError::Worker)
    }
    pub(crate) fn begin_receive_consent(&mut self) {
        self.clock = Clock::bounded(Instant::now(), Duration::from_secs(180));
    }
    pub(crate) fn poll_consent_control(&mut self, offer: &FileOffer) -> Result<(), TransferError> {
        if self.ready()? {
            match self.pump()? {
                None | Some(Event::Ack(_)) => {}
                Some(Event::Bytes(bytes)) => {
                    if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                        eprintln!(
                            "receiver: while awaiting consent {}",
                            wire::diagnostic_share_shape(&bytes)
                        );
                    }
                    match wire::decode_share(&bytes) {
                        Ok(wire::ShareMessage::Accepted) => {}
                        Ok(wire::ShareMessage::Rejected) => return Err(TransferError::Rejected),
                        Ok(wire::ShareMessage::Introduction(repeated)) if repeated == *offer => {}
                        Ok(wire::ShareMessage::Cancel) => return Err(TransferError::Cancelled),
                        _ if wire::is_progress(&bytes) => {}
                        _ => return Err(TransferError::Sequence),
                    }
                }
                Some(Event::File(_)) => {
                    if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                        eprintln!("receiver: file content arrived before local consent");
                    }
                    return Err(TransferError::Sequence);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn send_bytes(&mut self, bytes: Vec<u8>) -> Result<(), TransferError> {
        self.check()?;
        if bytes.is_empty() || bytes.len() > MAX_CHUNK {
            return Err(TransferError::Sequence);
        }
        let id = self.reserve_id()?;
        let size = bytes.len() as u64;
        self.send_frame(&wire::encode_data(&PayloadChunk {
            id,
            kind: PayloadKind::Bytes,
            total_size: size,
            offset: 0,
            index: 0,
            body: bytes,
            last: false,
            file_name: None,
            parent_folder: None,
        })?)?;
        self.send_frame(&wire::encode_data(&PayloadChunk {
            id,
            kind: PayloadKind::Bytes,
            total_size: size,
            offset: size,
            index: 1,
            body: Vec::new(),
            last: true,
            file_name: None,
            parent_folder: None,
        })?)?;
        Ok(())
    }
    pub(crate) fn verify(
        &mut self,
        local: &LocalCertificate,
        remote: &RemoteCertificate,
        role: PeerRole,
    ) -> Result<(), TransferError> {
        let now = now_seconds().map_err(|_| TransferError::PeerVerification)?;
        let proof = paired::encode_proof(
            local,
            remote,
            self.channel.authentication_token(),
            role,
            now,
        )
        .map_err(|_| TransferError::PeerVerification)?;
        // Both peers send first; never wait for an ACK before reading the peer.
        self.send_bytes(proof)?;
        let peer = self.read_bytes()?;
        if paired::verify_proof(
            &peer,
            local,
            remote,
            self.channel.authentication_token(),
            role,
            now,
        )
        .is_err()
        {
            let _ = self.send_bytes(paired::encode_result(false));
            return Err(TransferError::PeerVerification);
        }
        self.send_bytes(paired::encode_result(true))?;
        paired::require_success(&self.read_bytes()?).map_err(|_| TransferError::PeerVerification)
    }
    pub(crate) fn read_bytes(&mut self) -> Result<Vec<u8>, TransferError> {
        for _ in 0..256 {
            match self.next()? {
                Event::Bytes(bytes) => return Ok(bytes),
                Event::Ack(_) => {}
                Event::File(chunk) => {
                    drop(chunk);
                    return Err(TransferError::Sequence);
                }
            }
        }
        Err(TransferError::Sequence)
    }
    pub(crate) fn next(&mut self) -> Result<Event, TransferError> {
        for _ in 0..256 {
            if let Some(event) = self.pump()? {
                return Ok(event);
            }
        }
        Err(TransferError::Sequence)
    }
    fn pump(&mut self) -> Result<Option<Event>, TransferError> {
        self.check()?;
        let bytes = self.receive_frame()?;
        let control = connection::control(&bytes).inspect_err(|_| {
            if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                eprintln!(
                    "receiver/transport: unsupported frame type {:?}",
                    connection::diagnostic_frame_type(&bytes)
                );
            }
        })?;
        if !matches!(control, Control::Payload) {
            self.control_frame()?;
        }
        match control {
            Control::KeepAlive { ack, sequence } => {
                if !ack {
                    self.send_frame(&connection::keep_alive(true, sequence))?;
                }
                Ok(None)
            }
            Control::Upgrade { medium } => {
                self.send_frame(&connection::decline_upgrade(medium))?;
                Ok(None)
            }
            Control::UpgradeCapabilities { request } => {
                if request {
                    self.send_frame(&connection::upgrade_capabilities(false))?;
                }
                Ok(None)
            }
            Control::Disconnect { request, ack } => {
                if request && !ack {
                    let _ = self.send_frame(&connection::disconnect(true, true));
                }
                Err(TransferError::Cancelled)
            }
            Control::Payload => {
                let payload = wire::decode_payload(&bytes).inspect_err(|error| {
                    if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                        eprintln!(
                            "receiver/transport: payload decoder {error:?}; {}",
                            wire::diagnostic_payload_shape(&bytes)
                        );
                    }
                })?;
                if !matches!(&payload, PayloadEvent::Data(chunk) if chunk.kind == PayloadKind::File)
                {
                    self.control_frame()?;
                }
                match payload {
                    PayloadEvent::Acknowledged(id) => {
                        if !self.sent.contains(&id) {
                            if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                                eprintln!(
                                    "receiver/transport: acknowledgement of an unknown outgoing payload"
                                );
                            }
                            return Err(TransferError::Sequence);
                        }
                        Ok(Some(Event::Ack(id)))
                    }
                    PayloadEvent::Cancelled(_) => Err(TransferError::Cancelled),
                    PayloadEvent::Failed(_) => Err(TransferError::Rejected),
                    PayloadEvent::Data(chunk) if chunk.kind == PayloadKind::File => {
                        Ok(Some(Event::File(chunk)))
                    }
                    PayloadEvent::Data(chunk) => self.assemble(chunk),
                }
            }
        }
    }
    fn assemble(&mut self, chunk: PayloadChunk) -> Result<Option<Event>, TransferError> {
        if chunk.total_size > MAX_CHUNK as u64
            || chunk.file_name.is_some()
            || chunk.parent_folder.is_some()
            || self.received.contains(&chunk.id)
        {
            if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                eprintln!(
                    "receiver/transport: invalid control assembly size_limit={} file_metadata={} reused_id={} last={} body_bytes={}",
                    chunk.total_size > MAX_CHUNK as u64,
                    chunk.file_name.is_some() || chunk.parent_folder.is_some(),
                    self.received.contains(&chunk.id),
                    chunk.last,
                    chunk.body.len()
                );
            }
            return Err(TransferError::Sequence);
        }
        if self.assembly.is_none() {
            if chunk.offset != 0 || self.received.len() >= 32 {
                return Err(TransferError::Sequence);
            }
            self.assembly = Some(Assembly {
                id: chunk.id,
                size: chunk.total_size,
                bytes: Vec::new(),
            });
        }
        let a = self.assembly.as_mut().ok_or(TransferError::Sequence)?;
        if a.id != chunk.id || a.size != chunk.total_size || a.bytes.len() as u64 != chunk.offset {
            if std::env::var_os("NEARLANE_DIAGNOSTICS").is_some() {
                eprintln!(
                    "receiver/transport: mismatched control fragment id={} size={} offset={}",
                    a.id != chunk.id,
                    a.size != chunk.total_size,
                    a.bytes.len() as u64 != chunk.offset
                );
            }
            return Err(TransferError::Sequence);
        }
        a.bytes.extend_from_slice(&chunk.body);
        if chunk.last {
            let a = self.assembly.take().ok_or(TransferError::Sequence)?;
            if a.bytes.len() as u64 != a.size {
                return Err(TransferError::Sequence);
            }
            self.received.insert(a.id);
            if self.channel.supports_payload_ack() {
                self.send_frame(&wire::encode_ack(a.id))?;
            }
            return Ok(Some(Event::Bytes(a.bytes)));
        }
        Ok(None)
    }
    // Peek never consumes ciphertext. Restore blocking mode before crypto reads.
    fn ready(&self) -> Result<bool, TransferError> {
        let stream = &self.channel.io_ref().socket;
        stream.set_nonblocking(true)?;
        let mut byte = [0];
        let result = stream.peek(&mut byte);
        stream.set_nonblocking(false)?;
        match result {
            Ok(0) => Err(std::io::Error::from(ErrorKind::UnexpectedEof).into()),
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    fn upload_control(event: Event, file_id: i64) -> Result<(), TransferError> {
        match event {
            Event::Ack(id) if id != file_id => Ok(()),
            Event::Bytes(bytes) if wire::is_progress(&bytes) => Ok(()),
            Event::Bytes(bytes)
                if matches!(wire::decode_share(&bytes), Ok(wire::ShareMessage::Cancel)) =>
            {
                Err(TransferError::Cancelled)
            }
            _ => Err(TransferError::Sequence),
        }
    }
    pub(crate) fn send_file(
        &mut self,
        source: &mut impl Read,
        offer: &FileOffer,
        mut progress: impl FnMut(u64),
    ) -> Result<FileSummary, TransferError> {
        if !self.sent.contains(&offer.payload_id) {
            return Err(TransferError::Sequence);
        }
        self.begin_file_transfer();
        let mut offset = 0;
        let mut index = 0;
        let mut digest = Sha256::new();
        let mut buffer = vec![0; MAX_CHUNK];
        while offset < offer.size {
            self.check()?;
            let length = (offer.size - offset).min(MAX_CHUNK as u64) as usize;
            source.read_exact(&mut buffer[..length]).map_err(|e| {
                if e.kind() == ErrorKind::UnexpectedEof {
                    TransferError::SourceChanged
                } else {
                    e.into()
                }
            })?;
            digest.update(&buffer[..length]);
            self.send_frame(&wire::encode_data(&PayloadChunk {
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
            self.file_progress();
            progress(offset);
            for _ in 0..8 {
                if !self.ready()? {
                    break;
                }
                if let Some(event) = self.pump()? {
                    Self::upload_control(event, offer.payload_id)?;
                }
            }
        }
        self.check()?;
        let mut extra = [0];
        if source.read(&mut extra)? != 0 {
            return Err(TransferError::SourceChanged);
        }
        self.send_frame(&wire::encode_data(&PayloadChunk {
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
        Ok(FileSummary {
            bytes: offset,
            sha256: digest.finalize().into(),
        })
    }
    pub(crate) fn wait_file_ack(&mut self, file_id: i64) -> Result<(), TransferError> {
        let wait = if !self.channel.supports_payload_ack() {
            // Legacy Android does not negotiate file receipts. Give its final
            // messages a bounded grace period; keep-alives cannot prolong it.
            Duration::from_secs(5)
        } else {
            Duration::from_secs(30)
        };
        self.clock = Clock::bounded(Instant::now(), wait);
        for _ in 0..256 {
            let event = self.next()?;
            if matches!(event,Event::Ack(id) if id==file_id) {
                return Ok(());
            }
            Self::upload_control(event, file_id)?;
        }
        Err(TransferError::Sequence)
    }
    pub(crate) fn finish(&mut self) {
        // Try safe close after EOF whether or not a receipt was available.
        // Closing never upgrades an unconfirmed result to acknowledged delivery.
        self.clock = Clock::bounded(Instant::now(), Duration::from_secs(2));
        if self
            .send_frame(&connection::disconnect(true, false))
            .is_err()
        {
            return;
        }
        for _ in 0..16 {
            let Ok(bytes) = self.receive_frame() else {
                return;
            };
            match connection::control(&bytes) {
                Ok(Control::Disconnect { request, ack }) => {
                    if request && !ack {
                        let _ = self.send_frame(&connection::disconnect(true, true));
                    }
                    return;
                }
                Ok(Control::KeepAlive {
                    ack: false,
                    sequence,
                }) => {
                    let sent = self.send_frame(&connection::keep_alive(true, sequence));
                    if sent.is_err() {
                        return;
                    }
                }
                _ => {}
            }
        }
    }
    pub(crate) fn acknowledge_file(&mut self, id: i64) -> Result<(), TransferError> {
        if self.channel.supports_payload_ack() {
            self.send_frame(&wire::encode_ack(id))?;
        }
        Ok(())
    }
    pub(crate) fn ping(&mut self) -> Result<(), TransferError> {
        self.send_frame(&connection::keep_alive(false, 17))?;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn request_upgrade_capabilities(&mut self) -> Result<(), TransferError> {
        self.send_frame(&connection::upgrade_capabilities(true))?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod deadline_tests {
    use super::*;
    #[test]
    fn absolute_deadline_stops_partial_frame_trickle() {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            started_rx.recv().unwrap();
            for _ in 0..20 {
                if stream.write_all(&[0]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        });
        let socket = TcpStream::connect_timeout(&addr, Duration::from_secs(1)).unwrap();
        let mut bounded = DeadlineSocket {
            socket,
            deadline: Cell::new(Instant::now() + Duration::from_millis(110)),
        };
        started_tx.send(()).unwrap();
        let mut bytes = [0; 20];
        assert!(bounded.read_exact(&mut bytes).is_err());
        drop(bounded);
        server.join().unwrap();
    }
}
