//! Process-owned simulated peer only. No LAN discovery or Google credentials.
#![allow(clippy::unwrap_used)]
use super::*;
use crate::{
    file::FileSink,
    session::{Event, Session},
};

#[cfg(unix)]
#[test]
fn source_links_are_rejected_without_reading_their_target() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.bin");
    std::fs::write(&source, b"synthetic").unwrap();
    let link = directory.path().join("link.bin");
    std::os::unix::fs::symlink(&source, &link).unwrap();
    assert!(open_source(&link).is_err());
    assert!(open_source(&source).is_ok());
}
use nearlane_certificates::{DeviceMetadata, EncryptedMetadataKey, LocalCertificate, now_seconds};
use nearlane_core::Visibility;
use sha2::{Digest as _, Sha256};
use std::{
    io::Read,
    net::{Ipv4Addr, TcpListener},
    sync::mpsc,
};

pub(super) fn identity(name: &str) -> (OwnDeviceIdentity, RemoteCertificate) {
    let mut certificate = LocalCertificate::generate(
        DeviceMetadata::named(name).unwrap(),
        Visibility::OwnDevices,
        now_seconds().unwrap(),
    )
    .unwrap();
    let advertisement = certificate.advertisement_key().unwrap();
    let remote = RemoteCertificate::decode(&certificate.publication_bytes().unwrap()).unwrap();
    (
        OwnDeviceIdentity {
            certificate: Arc::new(certificate),
            advertisement,
        },
        remote,
    )
}
#[derive(Clone, Copy)]
enum Behavior {
    Accept,
    Reject,
    WrongKey,
    NoConfirmation,
}
fn roundtrip(
    size: usize,
    behavior: Behavior,
) -> (Result<FileSummary, SendFailure>, Option<FileSummary>) {
    let (sender, sender_public) = identity("synthetic sender");
    let (receiver, receiver_public) = identity("synthetic receiver");
    let (wrong, _) = identity("untrusted different key");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let directory = tempfile::tempdir().unwrap();
        let (stream, _) = listener.accept().unwrap();
        configure(&stream).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let (channel, info) =
            SecureChannel::respond_nearby(DeadlineSocket::for_test(stream)).unwrap();
        let metadata = sender_public
            .identify(
                EncryptedMetadataKey::from_bytes(&info[1..17]).unwrap(),
                now_seconds().unwrap(),
            )
            .unwrap();
        assert_eq!(metadata.device_name(), "synthetic sender");
        let mut session = Session::new(channel, Cancellation::default());
        session.ping().unwrap();
        let local = if matches!(behavior, Behavior::WrongKey) {
            &wrong.certificate
        } else {
            &receiver.certificate
        };
        if session
            .verify(local, &sender_public, PeerRole::Receiver)
            .is_err()
        {
            assert!(matches!(behavior, Behavior::WrongKey));
            return None;
        }
        let ShareMessage::Introduction(offer) =
            wire::decode_share(&session.read_bytes().unwrap()).unwrap()
        else {
            panic!("expected introduction")
        };
        if matches!(behavior, Behavior::Reject) {
            session
                .send_bytes(wire::encode_share(&ShareMessage::Rejected).unwrap())
                .unwrap();
            // A rejection must not be followed by file data, even when its ACK
            // and the peer's closing frame are interleaved.
            for _ in 0..8 {
                match session.next() {
                    Ok(Event::Ack(_)) => {}
                    Err(_) => return None,
                    _ => panic!("file data sent after rejection"),
                }
            }
            panic!("connection not closed after rejection");
        }
        session.ping().unwrap();
        session
            .send_bytes(wire::encode_share(&ShareMessage::Accepted).unwrap())
            .unwrap();
        let mut sink = FileSink::new(&offer, directory.path(), MAX_OUTGOING_BYTES).unwrap();
        let mut chunks = 0;
        loop {
            match session.next().unwrap() {
                Event::Ack(_) => {}
                Event::File(chunk) => {
                    assert_eq!(chunk.index, chunks);
                    chunks += 1;
                    if chunks % 3 == 0 {
                        session.ping().unwrap();
                    }
                    if sink.apply(&chunk).unwrap() {
                        break;
                    }
                }
                _ => panic!("unexpected control while receiving file"),
            }
        }
        let file = sink.finish().unwrap();
        let mut saved = File::open(&file.path).unwrap();
        let mut data = Vec::new();
        saved.read_to_end(&mut data).unwrap();
        assert_eq!(data.len(), size);
        assert!(data.iter().enumerate().all(|(i, b)| *b == (i % 251) as u8));
        assert_eq!(file.summary.sha256, Sha256::digest(&data).as_slice());
        if matches!(behavior, Behavior::NoConfirmation) {
            return Some(file.summary);
        }
        session.acknowledge_file(offer.payload_id).unwrap();
        // Process and acknowledge the sender's safe-disconnect request.
        let _ = session.next();
        Some(file.summary)
    });
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
    configure(&stream).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
    let mut progress = Vec::new();
    let result = send_stream(
        stream,
        &mut data.as_slice(),
        ("received.bin".into(), size as u64),
        &sender,
        &receiver_public,
        Cancellation::default(),
        |p| progress.push(p),
    );
    let received = server.join().unwrap();
    if result.is_ok() {
        assert_eq!(progress.last().unwrap().phase, SendPhase::Completed);
    } else {
        assert!(progress.iter().all(|p| p.phase != SendPhase::Completed));
    }
    (result, received)
}

#[test]
fn complete_connection_proof_acceptance_and_saved_file_survive_duplex_controls() {
    for size in [0, wire::MAX_CHUNK + 3, 3 * 1024 * 1024 + 123] {
        let (result, received) = roundtrip(size, Behavior::Accept);
        assert_eq!(result.unwrap(), received.unwrap());
    }
}
#[test]
fn wrong_private_key_and_rejection_prevent_file_content() {
    let (result, received) = roundtrip(1024, Behavior::WrongKey);
    let error = result.unwrap_err();
    assert_eq!(error.phase, SendPhase::Verifying);
    assert_eq!(error.kind, SendFailureKind::PrivateVerification);
    assert!(received.is_none());
    let (result, received) = roundtrip(1024, Behavior::Reject);
    assert_eq!(result.unwrap_err().kind, SendFailureKind::Rejected);
    assert!(received.is_none());
}
#[test]
fn missing_ack_is_not_reported_as_delivery_even_when_receiver_saved_file() {
    let (result, received) = roundtrip(1024, Behavior::NoConfirmation);
    assert!(received.is_some());
    let error = result.unwrap_err();
    assert_eq!(error.phase, SendPhase::AwaitingConfirmation);
    assert_eq!(error.kind, SendFailureKind::ConfirmationMissing);
}
#[test]
fn cancellation_interrupts_waiting_acceptance_without_file_payload() {
    let (sender, sender_public) = identity("synthetic sender");
    let (receiver, receiver_public) = identity("synthetic receiver");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        configure(&stream).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let (channel, _) = SecureChannel::respond_nearby(DeadlineSocket::for_test(stream)).unwrap();
        let mut session = Session::new(channel, Cancellation::default());
        session
            .verify(&receiver.certificate, &sender_public, PeerRole::Receiver)
            .unwrap();
        assert!(matches!(
            wire::decode_share(&session.read_bytes().unwrap()),
            Ok(ShareMessage::Introduction(_))
        ));
        started_tx.send(()).unwrap();
        for _ in 0..8 {
            match session.next() {
                Ok(Event::Ack(_)) => {}
                Err(_) => return,
                _ => panic!("file data sent without acceptance"),
            }
        }
        panic!("socket remained open");
    });
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
    configure(&stream).unwrap();
    let stop = Cancellation::default();
    let cancellation = stop.clone();
    let socket = Arc::new(Mutex::new(Some(stream.try_clone().unwrap())));
    let cancel_thread = std::thread::spawn(move || {
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let (_, result) = mpsc::channel();
        let job = SendJob {
            cancellation,
            socket,
            progress: Arc::new(Mutex::new(SendProgress::default())),
            result,
        };
        job.cancel();
    });
    let result = send_stream(
        stream,
        &mut [0u8; 1024].as_slice(),
        ("data.bin".into(), 1024),
        &sender,
        &receiver_public,
        stop,
        |_| {},
    );
    assert_eq!(result.unwrap_err().kind, SendFailureKind::Cancelled);
    cancel_thread.join().unwrap();
    server.join().unwrap();
}
