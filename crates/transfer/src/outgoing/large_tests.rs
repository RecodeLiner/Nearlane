//! Bounded-memory full Nearby/private-proof transfer, on owned loopback only.
#![allow(clippy::unwrap_used)]
use super::tests::identity;
use super::*;
use crate::{file::FileSink, session::Event};
use sha2::{Digest as _, Sha256};
use std::{
    io::Read as _,
    net::{Ipv4Addr, TcpListener},
    time::Instant,
};

fn streaming_roundtrip(size: u64) {
    let (sender, sender_public) = identity("large test sender");
    let (receiver, receiver_public) = identity("large test receiver");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let start = Instant::now();
    let receive = std::thread::spawn(move || {
        let directory = tempfile::tempdir().unwrap();
        let (stream, _) = listener.accept().unwrap();
        configure(&stream).unwrap();
        let (channel, _) =
            SecureChannel::respond_nearby(DeadlineSocket::handshake(stream)).unwrap();
        let mut session = Session::new(channel, Cancellation::default());
        session
            .verify(&receiver.certificate, &sender_public, PeerRole::Receiver)
            .unwrap();
        let ShareMessage::Introduction(offer) =
            wire::decode_share(&session.read_bytes().unwrap()).unwrap()
        else {
            panic!("missing file offer")
        };
        session
            .send_bytes(wire::encode_share(&ShareMessage::Accepted).unwrap())
            .unwrap();
        session.begin_file_transfer();
        let mut sink = FileSink::new(&offer, directory.path(), MAX_OUTGOING_BYTES).unwrap();
        let mut frames = 0;
        loop {
            match session.next().unwrap() {
                Event::File(chunk) => {
                    frames += 1;
                    let done = sink.apply(&chunk).unwrap();
                    session.file_progress();
                    if done {
                        break;
                    }
                }
                Event::Ack(_) => {}
                _ => panic!("unexpected file control"),
            }
        }
        let saved = sink.finish().unwrap();
        let mut digest = Sha256::new();
        let copied = std::io::copy(
            &mut File::open(&saved.path).unwrap(),
            &mut DigestWriter(&mut digest),
        )
        .unwrap();
        assert_eq!(copied, size);
        assert_eq!(
            saved.summary.sha256.as_slice(),
            digest.finalize().as_slice()
        );
        session.acknowledge_file(offer.payload_id).unwrap();
        let _ = session.next();
        (saved.summary, frames)
    });
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(3)).unwrap();
    configure(&stream).unwrap();
    let mut source = std::io::repeat(0x5a).take(size);
    let sent = send_stream(
        stream,
        &mut source,
        ("large-test.bin".into(), size),
        &sender,
        &receiver_public,
        Cancellation::default(),
        |_| {},
    )
    .unwrap();
    let (saved, frames) = receive.join().unwrap();
    assert_eq!(sent, saved);
    assert_eq!(frames, size.div_ceil(wire::MAX_CHUNK as u64) + 1);
    eprintln!(
        "Owned encrypted streaming test passed: {size} bytes, {frames} frames, {:.2}s",
        start.elapsed().as_secs_f64()
    );
}
struct DigestWriter<'a>(&'a mut Sha256);
impl std::io::Write for DigestWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[test]
fn full_private_stream_exceeds_the_previous_64_mib_limit() {
    streaming_roundtrip(65 * 1024 * 1024 + 17);
}
#[test]
#[ignore = "Opt-in 513 MiB disk/crypto test, owned loopback only, no account or phone"]
fn full_private_stream_exceeds_the_previous_8192_frame_limit() {
    streaming_roundtrip(513 * 1024 * 1024 + 17);
}
