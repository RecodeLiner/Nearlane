//! Owned local peers only. This exercises the encrypted file engine without
//! pretending that Google account/certificate interoperability is already solved.

use std::{
    fs::File,
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    time::{Duration, Instant},
};

use nearlane_certificates::{
    DeviceMetadata, LocalCertificate, PeerRole, RemoteCertificate, now_seconds,
};
use nearlane_core::Visibility;
use nearlane_protocol::{
    paired,
    secure::SecureChannel,
    wire::{self, MAX_CHUNK, PayloadChunk, PayloadEvent, PayloadKind, ShareMessage},
};
use sha2::{Digest as _, Sha256};

use crate::{Cancellation, FileOffer, FileSummary, TransferError, receive_file, send_file};

/// Result of a local transfer between two endpoints owned by this process.
#[derive(Debug)]
pub struct SelfTestReport {
    /// Observed bytes and digest matched after rereading the received file.
    pub summary: FileSummary,
    /// Elapsed time including handshake and disk IO.
    pub elapsed: Duration,
}

/// Transfer a synthetic 3 MiB file over loopback using Google's UKEY2/D2D,
/// Quick Share introduction/acceptance and Nearby file payload/ack frames.
/// No LAN discovery, Google requests or real account credentials are involved.
/// This validates a protocol subset, not Android interoperability.
pub fn run_loopback_self_test() -> Result<SelfTestReport, TransferError> {
    run_with_size(3 * 1024 * 1024 + 123)
}

fn run_with_size(size: u64) -> Result<SelfTestReport, TransferError> {
    let start = Instant::now();
    // Synthetic identities owned by this harness. Distribution here is local
    // setup, not a Google account API or a claim of Android interoperability.
    let now = now_seconds().map_err(|_| TransferError::PeerVerification)?;
    let (sender_private, sender_public) = synthetic_identity("sender", now)?;
    let (receiver_private, receiver_public) = synthetic_identity("receiver", now)?;
    let root = tempfile::tempdir()?;
    let source_path = root.path().join("source.bin");
    let mut source = File::create(&source_path)?;
    let pattern: Vec<u8> = (0..MAX_CHUNK).map(|i| (i % 251) as u8).collect();
    let mut expected_hash = Sha256::new();
    let mut remaining = size;
    while remaining > 0 {
        let n = remaining.min(pattern.len() as u64) as usize;
        source.write_all(&pattern[..n])?;
        expected_hash.update(&pattern[..n]);
        remaining -= n as u64;
    }
    source.flush()?;
    drop(source);
    let expected = FileSummary {
        bytes: size,
        sha256: expected_hash.finalize().into(),
    };
    let directory = root.path().join("received");
    std::fs::create_dir(&directory)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let server = std::thread::spawn(move || -> Result<FileSummary, TransferError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => return Err(e.into()),
            }
        };
        configure(&stream)?;
        let mut channel = SecureChannel::respond(stream)?;
        let proof = receive_control(&mut channel)?;
        paired::verify_proof(
            &proof,
            &receiver_private,
            &sender_public,
            channel.authentication_token(),
            PeerRole::Receiver,
            now,
        )
        .map_err(|_| TransferError::PeerVerification)?;
        let proof = paired::encode_proof(
            &receiver_private,
            &sender_public,
            channel.authentication_token(),
            PeerRole::Receiver,
            now,
        )
        .map_err(|_| TransferError::PeerVerification)?;
        send_bytes(&mut channel, 102, proof)?;
        paired::require_success(&receive_control(&mut channel)?)
            .map_err(|_| TransferError::PeerVerification)?;
        send_bytes(&mut channel, 104, paired::encode_result(true))?;
        // Synthetic consent is limited to these two process-owned peers.
        let ShareMessage::Introduction(offer) =
            wire::decode_share(&receive_control(&mut channel)?)?
        else {
            return Err(TransferError::Sequence);
        };
        send_control(&mut channel, 2, &ShareMessage::Accepted)?;
        let received = receive_file(
            &mut channel,
            &offer,
            &directory,
            size,
            &Cancellation::default(),
        )?;
        let mut saved = File::open(&received.path)?;
        let mut digest = Sha256::new();
        let mut buffer = vec![0_u8; MAX_CHUNK];
        let mut bytes = 0;
        loop {
            let n = saved.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
            bytes += n as u64;
        }
        let reread = FileSummary {
            bytes,
            sha256: digest.finalize().into(),
        };
        if reread != received.summary {
            return Err(TransferError::Sequence);
        }
        Ok(reread)
    });
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    configure(&stream)?;
    let mut channel = SecureChannel::initiate(stream)?;
    let proof = paired::encode_proof(
        &sender_private,
        &receiver_public,
        channel.authentication_token(),
        PeerRole::Sender,
        now,
    )
    .map_err(|_| TransferError::PeerVerification)?;
    send_bytes(&mut channel, 101, proof)?;
    let proof = receive_control(&mut channel)?;
    paired::verify_proof(
        &proof,
        &sender_private,
        &receiver_public,
        channel.authentication_token(),
        PeerRole::Sender,
        now,
    )
    .map_err(|_| TransferError::PeerVerification)?;
    send_bytes(&mut channel, 103, paired::encode_result(true))?;
    paired::require_success(&receive_control(&mut channel)?)
        .map_err(|_| TransferError::PeerVerification)?;
    let offer = FileOffer {
        name: "received.bin".into(),
        size,
        payload_id: 7,
        attachment_id: 8,
        mime_type: "application/octet-stream".into(),
    };
    send_control(&mut channel, 1, &ShareMessage::Introduction(offer.clone()))?;
    if wire::decode_share(&receive_control(&mut channel)?)? != ShareMessage::Accepted {
        return Err(TransferError::Rejected);
    }
    let sent = send_file(
        &mut channel,
        &mut File::open(source_path)?,
        &offer,
        size,
        &Cancellation::default(),
    )?;
    let received = server.join().map_err(|_| TransferError::Worker)??;
    if sent != expected || received != expected {
        return Err(TransferError::Sequence);
    }
    Ok(SelfTestReport {
        summary: received,
        elapsed: start.elapsed(),
    })
}

fn configure(stream: &TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    stream.set_nodelay(true)
}

fn synthetic_identity(
    name: &str,
    now: i64,
) -> Result<(LocalCertificate, RemoteCertificate), TransferError> {
    let local = LocalCertificate::generate(
        DeviceMetadata::named(name).map_err(|_| TransferError::PeerVerification)?,
        Visibility::OwnDevices,
        now,
    )
    .map_err(|_| TransferError::PeerVerification)?;
    let public = RemoteCertificate::decode(
        &local
            .publication_bytes()
            .map_err(|_| TransferError::PeerVerification)?,
    )
    .map_err(|_| TransferError::PeerVerification)?;
    Ok((local, public))
}

fn send_control(
    channel: &mut SecureChannel<TcpStream>,
    id: i64,
    message: &ShareMessage,
) -> Result<(), TransferError> {
    let body = wire::encode_share(message)?;
    send_bytes(channel, id, body)
}

fn send_bytes(
    channel: &mut SecureChannel<TcpStream>,
    id: i64,
    body: Vec<u8>,
) -> Result<(), TransferError> {
    channel.send(&wire::encode_data(&PayloadChunk {
        index: 0,
        id,
        kind: PayloadKind::Bytes,
        total_size: body.len() as u64,
        offset: 0,
        body,
        last: true,
        file_name: None,
        parent_folder: None,
    })?)?;
    match wire::decode_payload(&channel.receive()?)? {
        PayloadEvent::Acknowledged(received) if received == id => Ok(()),
        _ => Err(TransferError::Sequence),
    }
}

fn receive_control(channel: &mut SecureChannel<TcpStream>) -> Result<Vec<u8>, TransferError> {
    let PayloadEvent::Data(data) = wire::decode_payload(&channel.receive()?)? else {
        return Err(TransferError::Sequence);
    };
    if data.kind != PayloadKind::Bytes
        || data.offset != 0
        || !data.last
        || data.total_size != data.body.len() as u64
    {
        return Err(TransferError::Sequence);
    }
    channel.send(&wire::encode_ack(data.id))?;
    Ok(data.body)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_file_roundtrip_handles_empty_and_multichunk_files() {
        for size in [0, MAX_CHUNK as u64 + 3, 3 * 1024 * 1024 + 123] {
            assert_eq!(run_with_size(size).unwrap().summary.bytes, size);
        }
    }
}
