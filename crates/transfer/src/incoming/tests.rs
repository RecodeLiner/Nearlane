//! Synthetic owned peers only; no account, LAN discovery or personal files.
#![allow(clippy::unwrap_used)]
use super::*;
use nearlane_certificates::{DeviceMetadata, LocalCertificate};
use nearlane_core::Visibility;
use sha2::{Digest as _, Sha256};

fn identity(name: &str) -> (OwnDeviceIdentity, Arc<RemoteCertificate>) {
    let mut c = LocalCertificate::generate(
        DeviceMetadata::named(name).unwrap(),
        Visibility::OwnDevices,
        now_seconds().unwrap(),
    )
    .unwrap();
    let key = c.advertisement_key().unwrap();
    let public = Arc::new(RemoteCertificate::decode(&c.publication_bytes().unwrap()).unwrap());
    (
        OwnDeviceIdentity {
            certificate: Arc::new(c),
            advertisement: key,
        },
        public,
    )
}

#[derive(Clone, Copy)]
enum Behavior {
    Accept,
    Decline,
    WrongCertificate,
    ExistingFile,
}
fn roundtrip(behavior: Behavior) {
    let directory = tempfile::tempdir().unwrap();
    if matches!(behavior, Behavior::ExistingFile) {
        std::fs::write(directory.path().join("synthetic.bin"), b"existing").unwrap();
    }
    let (receiver, receiver_public) = identity("Synthetic PC");
    let (sender, sender_public) = identity("Synthetic phone");
    let (_, wrong) = identity("Wrong");
    let certificates = vec![if matches!(behavior, Behavior::WrongCertificate) {
        wrong
    } else {
        sender_public
    }];
    let state = Arc::new(Mutex::new(State {
        phase: ReceivePhase::Starting,
        offer: None,
    }));
    let (decide, decisions) = mpsc::sync_channel(1);
    let context = Context {
        identity: receiver,
        certificates,
        directory: directory.path().into(),
        stop: Cancellation::default(),
        state: Arc::clone(&state),
        decisions,
    };
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let receive = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        context.receive(stream)
    });
    let data: Vec<u8> = (0..(128 * 1024 + 13)).map(|i| (i % 251) as u8).collect();
    let sent = data.clone();
    let send = thread::spawn(move || {
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
        crate::outgoing::configure(&stream).unwrap();
        crate::outgoing::send_stream(
            stream,
            &mut sent.as_slice(),
            ("synthetic.bin".into(), sent.len() as u64),
            &sender,
            &receiver_public,
            Cancellation::default(),
            |_| {},
        )
    });
    if !matches!(behavior, Behavior::WrongCertificate) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "No verified offer");
            if state.lock().unwrap().phase == ReceivePhase::AwaitingConsent {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let guard = state.lock().unwrap();
        assert_eq!(guard.offer.as_ref().unwrap().sender, "Synthetic phone");
        assert!(
            !directory.path().join("synthetic.bin").exists()
                || matches!(behavior, Behavior::ExistingFile)
        );
        drop(guard);
        decide.send(!matches!(behavior, Behavior::Decline)).unwrap();
    }
    let saved = receive.join().unwrap();
    let sent = send.join().unwrap();
    match behavior {
        Behavior::Accept => {
            let saved = saved.unwrap();
            assert_eq!(std::fs::read(saved.path).unwrap(), data);
            assert_eq!(saved.summary.sha256, Sha256::digest(&data).as_slice());
            assert_eq!(sent.unwrap(), saved.summary);
        }
        Behavior::Decline => {
            assert_eq!(saved.unwrap_err(), ReceiveFailure::Declined);
            assert!(sent.is_err());
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        }
        Behavior::WrongCertificate => {
            assert_eq!(saved.unwrap_err(), ReceiveFailure::Untrusted);
            assert!(sent.is_err());
            assert!(state.lock().unwrap().offer.is_none());
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        }
        Behavior::ExistingFile => {
            assert_eq!(saved.unwrap_err(), ReceiveFailure::File);
            assert!(sent.is_err());
            assert_eq!(
                std::fs::read(directory.path().join("synthetic.bin")).unwrap(),
                b"existing"
            );
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
}
#[test]
fn verified_consent_streaming_and_persistence_precede_receipt() {
    roundtrip(Behavior::Accept);
}
#[test]
fn decline_saves_nothing() {
    roundtrip(Behavior::Decline);
}
#[test]
fn unmatched_sender_never_reaches_consent() {
    roundtrip(Behavior::WrongCertificate);
}
#[test]
fn existing_destination_is_never_replaced_and_staging_is_removed() {
    roundtrip(Behavior::ExistingFile);
}

#[test]
fn android_sized_image_chunks_save_only_after_verified_consent_and_exact_eof() {
    use nearlane_protocol::wire::{PayloadChunk, PayloadKind};
    // Leave room for both wire envelopes in the near-limit test frame.
    let large_block = nearlane_protocol::MAX_FRAME_BYTES - 1024;
    for (length, block) in [(91_458, 91_458), (large_block + 17, large_block)] {
        let directory = tempfile::tempdir().unwrap();
        let (receiver, receiver_public) = identity("Synthetic PC");
        let (sender, sender_public) = identity("Synthetic phone");
        let state = Arc::new(Mutex::new(State {
            phase: ReceivePhase::Starting,
            offer: None,
        }));
        let (decision, decisions) = mpsc::sync_channel(1);
        let context = Context {
            identity: receiver,
            certificates: vec![sender_public],
            directory: directory.path().into(),
            stop: Cancellation::default(),
            state: Arc::clone(&state),
            decisions,
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let receive = thread::spawn(move || context.receive(listener.accept().unwrap().0));
        let data = (0..length).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        let expected = data.clone();
        let send = thread::spawn(move || {
            let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
            let request = nearlane_protocol::connection::request(
                b"TEST",
                &nearlane_protocol::connection::private_endpoint_info(sender.advertisement),
                7,
            )
            .unwrap();
            let channel =
                SecureChannel::initiate_nearby(DeadlineSocket::for_test(stream), &request).unwrap();
            let mut session = Session::new(channel, Cancellation::default());
            session
                .verify(&sender.certificate, &receiver_public, PeerRole::Sender)
                .unwrap();
            let offer = crate::FileOffer {
                name: "synthetic.jpg".into(),
                size: length as u64,
                payload_id: session.reserve_id().unwrap(),
                attachment_id: session.reserve_id().unwrap(),
                mime_type: "image/jpeg".into(),
            };
            session
                .send_bytes(wire::encode_share(&ShareMessage::Introduction(offer.clone())).unwrap())
                .unwrap();
            assert_eq!(
                wire::decode_share(&session.read_bytes().unwrap()).unwrap(),
                ShareMessage::Accepted
            );
            session.begin_file_transfer();
            let mut offset = 0;
            let mut index = 0;
            for bytes in data.chunks(block) {
                session
                    .send_test_chunk(&PayloadChunk {
                        id: offer.payload_id,
                        kind: PayloadKind::File,
                        total_size: offer.size,
                        offset,
                        index,
                        body: bytes.to_vec(),
                        last: false,
                        file_name: Some(offer.name.clone()),
                        parent_folder: None,
                    })
                    .unwrap();
                offset += bytes.len() as u64;
                index += 1;
            }
            session
                .send_test_chunk(&PayloadChunk {
                    id: offer.payload_id,
                    kind: PayloadKind::File,
                    total_size: offer.size,
                    offset,
                    index,
                    body: Vec::new(),
                    last: true,
                    file_name: Some(offer.name),
                    parent_folder: None,
                })
                .unwrap();
            session.wait_file_ack(offer.payload_id).unwrap();
            session.finish();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.lock().unwrap().phase != ReceivePhase::AwaitingConsent {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        decision.send(true).unwrap();
        let saved = receive.join().unwrap().unwrap();
        send.join().unwrap();
        assert_eq!(saved.summary.bytes, length as u64);
        assert_eq!(saved.summary.sha256, Sha256::digest(&expected).as_slice());
        assert_eq!(std::fs::read(saved.path).unwrap(), expected);
    }
}

#[test]
fn file_bytes_before_local_consent_are_rejected_without_a_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let (receiver, receiver_public) = identity("PC");
    let (sender, sender_public) = identity("Phone");
    let state = Arc::new(Mutex::new(State {
        phase: ReceivePhase::Starting,
        offer: None,
    }));
    let (_decision, decisions) = mpsc::sync_channel(1);
    let context = Context {
        identity: receiver,
        certificates: vec![sender_public],
        directory: directory.path().into(),
        stop: Cancellation::default(),
        state,
        decisions,
    };
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let worker = thread::spawn(move || context.receive(listener.accept().unwrap().0));
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
    let request = nearlane_protocol::connection::request(
        b"TEST",
        &nearlane_protocol::connection::private_endpoint_info(sender.advertisement),
        7,
    )
    .unwrap();
    let channel =
        SecureChannel::initiate_nearby(DeadlineSocket::for_test(stream), &request).unwrap();
    let mut session = Session::new(channel, Cancellation::default());
    session
        .verify(&sender.certificate, &receiver_public, PeerRole::Sender)
        .unwrap();
    let offer = crate::FileOffer {
        name: "not-approved.bin".into(),
        size: 4,
        payload_id: session.reserve_id().unwrap(),
        attachment_id: session.reserve_id().unwrap(),
        mime_type: "application/octet-stream".into(),
    };
    session
        .send_bytes(wire::encode_share(&ShareMessage::Introduction(offer.clone())).unwrap())
        .unwrap();
    let _ = session.send_file(&mut b"data".as_slice(), &offer, |_| {});
    assert_eq!(
        worker.join().unwrap().unwrap_err(),
        ReceiveFailure::Protocol
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn android_sender_acceptance_and_repeated_offer_cannot_replace_local_consent() {
    let directory = tempfile::tempdir().unwrap();
    let (receiver, receiver_public) = identity("PC");
    let (sender, sender_public) = identity("Phone");
    let state = Arc::new(Mutex::new(State {
        phase: ReceivePhase::Starting,
        offer: None,
    }));
    let (decision, decisions) = mpsc::sync_channel(1);
    let context = Context {
        identity: receiver,
        certificates: vec![sender_public],
        directory: directory.path().into(),
        stop: Cancellation::default(),
        state: Arc::clone(&state),
        decisions,
    };
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let receive = thread::spawn(move || context.receive(listener.accept().unwrap().0));
    let (ready, already_sent) = mpsc::channel();
    let send = thread::spawn(move || {
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
        let request = nearlane_protocol::connection::request(
            b"TEST",
            &nearlane_protocol::connection::private_endpoint_info(sender.advertisement),
            7,
        )
        .unwrap();
        let channel =
            SecureChannel::initiate_nearby(DeadlineSocket::for_test(stream), &request).unwrap();
        let mut session = Session::new(channel, Cancellation::default());
        session
            .verify(&sender.certificate, &receiver_public, PeerRole::Sender)
            .unwrap();
        let offer = crate::FileOffer {
            name: "consented.txt".into(),
            size: 4,
            payload_id: session.reserve_id().unwrap(),
            attachment_id: session.reserve_id().unwrap(),
            mime_type: "text/plain".into(),
        };
        session
            .send_bytes(wire::encode_share(&ShareMessage::Introduction(offer.clone())).unwrap())
            .unwrap();
        session
            .send_bytes(wire::encode_share(&ShareMessage::Accepted).unwrap())
            .unwrap();
        session.request_upgrade_capabilities().unwrap();
        session
            .send_bytes(wire::encode_share(&ShareMessage::Introduction(offer.clone())).unwrap())
            .unwrap();
        ready.send(()).unwrap();
        assert_eq!(
            wire::decode_share(&session.read_bytes().unwrap()).unwrap(),
            ShareMessage::Accepted
        );
        session
            .send_file(&mut b"data".as_slice(), &offer, |_| {})
            .unwrap();
        session.wait_file_ack(offer.payload_id).unwrap();
        session.finish();
    });
    already_sent.recv_timeout(Duration::from_secs(3)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while state.lock().unwrap().phase != ReceivePhase::AwaitingConsent {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    decision.send(true).unwrap();
    let saved = receive.join().unwrap().unwrap();
    send.join().unwrap();
    assert_eq!(std::fs::read(saved.path).unwrap(), b"data");
}
