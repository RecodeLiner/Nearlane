//! Bounded adapter around Google's Apache-2.0 UKEY2/D2D implementation.
//!
//! This establishes encryption, integrity and message ordering. The caller still
//! has to authenticate the peer using Quick Share certificates before file use.

use std::{
    fmt,
    io::{self, Read, Write},
};

use crypto_provider_rustcrypto::RustCrypto;
use subtle::ConstantTimeEq as _;
use ukey2_connections::{
    D2DConnectionContextV1, D2DHandshakeContext, HandshakeImplementation,
    InitiatorD2DHandshakeContext, NextProtocol, ServerD2DHandshakeContext,
};
use zeroize::Zeroizing;

use crate::{read_frame_bounded, write_frame};

const MAX_HANDSHAKE: usize = 64 * 1024;
const MAX_CIPHERTEXT: usize = 192 * 1024;
/// Maximum decrypted Nearby frame accepted by this adapter.
pub const MAX_PLAINTEXT: usize = 128 * 1024;

/// Safe connection-prelude checkpoints; no peer data or cryptographic material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionStep {
    /// Reading the initiating peer's plain Nearby request.
    IncomingRequestRead,
    /// Validating the initiating peer's bounded request metadata.
    IncomingRequestValidation,
    /// Reading the initiating peer's UKEY2 hello.
    ClientHelloRead,
    /// Validating the initiating peer's UKEY2 hello.
    ClientHelloValidation,
    /// Writing the local UKEY2 hello as receiver.
    ServerHelloWrite,
    /// Reading the initiating peer's UKEY2 completion.
    ClientFinishRead,
    /// Validating the initiating peer's UKEY2 completion.
    ClientFinishValidation,
    /// Writing the initial Nearby request.
    Request,
    /// Reading the server's UKEY2 response.
    ServerHelloRead,
    /// Validating the server's UKEY2 response.
    ServerHelloValidation,
    /// Writing the client's UKEY2 completion.
    ClientFinish,
    /// Deriving the encrypted transport context.
    KeyDerivation,
    /// Writing transport acceptance.
    AcceptanceWrite,
    /// Reading transport acceptance.
    AcceptanceRead,
    /// Peer acceptance or required protocol features were not valid.
    AcceptanceValidation,
}

/// Sanitized transport failures; never contains plaintext, keys or peer messages.
#[derive(Debug)]
pub enum ChannelError {
    /// Stream IO failed.
    Io(io::Error),
    /// A sanitized pre-encryption checkpoint failed.
    Prelude(ConnectionStep),
    /// A UKEY2 handshake message was invalid or out of order.
    Handshake,
    /// The encrypted message failed integrity/sequence checks.
    Authentication,
    /// A stage-specific size limit was exceeded.
    Size,
    /// A failed channel cannot be reused.
    Poisoned,
}
impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Prelude(step) = self {
            return write!(f, "Nearby prelude failed at {step:?}");
        }
        f.write_str(match self {
            Self::Io(_) => "Nearby stream IO failed",
            Self::Prelude(_) => unreachable!(),
            Self::Handshake => "UKEY2 handshake rejected",
            Self::Authentication => "Nearby message integrity or sequence check failed",
            Self::Size => "Nearby frame exceeds the stage limit",
            Self::Poisoned => "Nearby channel is no longer usable",
        })
    }
}
impl std::error::Error for ChannelError {}
impl From<io::Error> for ChannelError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Encrypted, ordered transport. It does not by itself prove contact membership.
pub struct SecureChannel<S> {
    stream: S,
    context: D2DConnectionContextV1,
    authentication: Zeroizing<Vec<u8>>,
    poisoned: bool,
    remote_safe_version: i32,
}

impl<S> fmt::Debug for SecureChannel<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecureChannel")
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

impl<S: Read + Write> SecureChannel<S> {
    /// Full initiating Nearby transport prelude: plain request, UKEY2, then
    /// mutual plain acceptance before encrypted paired-key/application traffic.
    pub fn initiate_nearby(mut stream: S, request: &[u8]) -> Result<Self, ChannelError> {
        write_frame(&mut stream, request)
            .and_then(|_| stream.flush())
            .map_err(|_| ChannelError::Prelude(ConnectionStep::Request))?;
        let mut channel = Self::initiate(stream)?;
        channel.accept_nearby()?;
        Ok(channel)
    }
    /// Receiving prelude. Returned endpoint metadata is unverified; the caller
    /// must match an own-device certificate and require live proofs before offers.
    pub fn respond_nearby(mut stream: S) -> Result<(Self, Vec<u8>), ChannelError> {
        let bytes = read_frame_bounded(&mut stream, 4096)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::IncomingRequestRead))?;
        let info = crate::connection::request_info(&bytes)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::IncomingRequestValidation))?;
        let mut channel = Self::respond(stream)?;
        channel.accept_nearby()?;
        Ok((channel, info))
    }
    fn accept_nearby(&mut self) -> Result<(), ChannelError> {
        write_frame(&mut self.stream, &crate::connection::acceptance())
            .and_then(|_| self.stream.flush())
            .map_err(|_| ChannelError::Prelude(ConnectionStep::AcceptanceWrite))?;
        for _ in 0..16 {
            let bytes = read_frame_bounded(&mut self.stream, 4096)
                .map_err(|_| ChannelError::Prelude(ConnectionStep::AcceptanceRead))?;
            if let Ok(version) = crate::connection::require_acceptance(&bytes) {
                self.remote_safe_version = version;
                return Ok(());
            }
            match crate::connection::control(&bytes) {
                Ok(crate::connection::Control::KeepAlive { ack, sequence }) => {
                    if !ack {
                        write_frame(
                            &mut self.stream,
                            &crate::connection::keep_alive(true, sequence),
                        )?;
                        self.stream.flush()?;
                    }
                }
                _ => return Err(ChannelError::Prelude(ConnectionStep::AcceptanceValidation)),
            }
        }
        Err(ChannelError::Prelude(ConnectionStep::AcceptanceValidation))
    }
    /// Socket readiness/deadline access only. Never read or write outside the
    /// encrypted methods after the connection prelude.
    pub fn io_ref(&self) -> &S {
        &self.stream
    }
    /// Whether mutual feature negotiation allows received-payload ACK frames.
    /// A legacy peer may still accept files; socket writes are not receipts.
    pub fn supports_payload_ack(&self) -> bool {
        self.remote_safe_version >= 6
    }
    /// Perform the initiating side of the three-message UKEY2 handshake.
    /// The caller must set IO deadlines on network streams before this call.
    pub fn initiate(mut stream: S) -> Result<Self, ChannelError> {
        let mut handshake = InitiatorD2DHandshakeContext::<RustCrypto>::new(
            HandshakeImplementation::PublicKeyInProtobuf,
            vec![NextProtocol::Aes256CbcHmacSha256],
        );
        send_handshake(&mut stream, &handshake)?;
        let response = read_frame_bounded(&mut stream, MAX_HANDSHAKE)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::ServerHelloRead))?;
        handshake
            .handle_handshake_message(&response)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::ServerHelloValidation))?;
        send_handshake(&mut stream, &handshake)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::ClientFinish))?;
        Self::complete(stream, &mut handshake)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::KeyDerivation))
    }

    /// Perform the receiving side, retaining no partially completed handshake.
    pub fn respond(mut stream: S) -> Result<Self, ChannelError> {
        let mut handshake = ServerD2DHandshakeContext::<RustCrypto>::new(
            HandshakeImplementation::PublicKeyInProtobuf,
            &[NextProtocol::Aes256CbcHmacSha256],
        );
        let init = read_frame_bounded(&mut stream, MAX_HANDSHAKE)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::ClientHelloRead))?;
        handshake
            .handle_handshake_message(&init)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::ClientHelloValidation))?;
        send_handshake(&mut stream, &handshake)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::ServerHelloWrite))?;
        let finish = read_frame_bounded(&mut stream, MAX_HANDSHAKE)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::ClientFinishRead))?;
        handshake
            .handle_handshake_message(&finish)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::ClientFinishValidation))?;
        Self::complete(stream, &mut handshake)
            .map_err(|_| ChannelError::Prelude(ConnectionStep::KeyDerivation))
    }

    fn complete(stream: S, handshake: &mut impl D2DHandshakeContext) -> Result<Self, ChannelError> {
        let authentication = Zeroizing::new(
            handshake
                .completed_handshake()
                .map_err(|_| ChannelError::Handshake)?
                .auth_string::<RustCrypto>()
                .derive_vec(32)
                .ok_or(ChannelError::Handshake)?,
        );
        let context = handshake
            .to_connection_context()
            .map_err(|_| ChannelError::Handshake)?;
        Ok(Self {
            stream,
            context,
            authentication,
            poisoned: false,
            remote_safe_version: 0,
        })
    }

    /// Input to the Quick Share certificate/signature verifier. Keep out of UI,
    /// logs and disk. Knowing this value does not identify a Google contact.
    pub fn authentication_token(&self) -> &[u8] {
        &self.authentication
    }

    /// Compare an independently obtained peer authentication value. Used by the
    /// local harness; production requires certificate-backed verification.
    pub fn matches_authentication(&self, expected: &[u8]) -> bool {
        bool::from(self.authentication.as_slice().ct_eq(expected))
    }

    /// Encrypt one Nearby frame. Authentication control frames precede trust;
    /// file introductions/content require the relevant trust and consent gates.
    pub fn send(&mut self, plaintext: &[u8]) -> Result<(), ChannelError> {
        if self.poisoned {
            return Err(ChannelError::Poisoned);
        }
        if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT {
            return Err(ChannelError::Size);
        }
        if self.context.encode_seq_num() == i32::MAX {
            self.poisoned = true;
            return Err(ChannelError::Poisoned);
        }
        let ciphertext = self
            .context
            .encode_message_to_peer::<RustCrypto>(plaintext, None::<&[u8]>);
        if let Err(error) =
            write_frame(&mut self.stream, &ciphertext).and_then(|_| self.stream.flush())
        {
            self.poisoned = true;
            return Err(error.into());
        }
        Ok(())
    }

    /// Decrypt one frame. Any IO, integrity, ordering or bound failure poisons the
    /// channel so callers cannot continue with ambiguous stream/crypto state.
    pub fn receive(&mut self) -> Result<Vec<u8>, ChannelError> {
        if self.poisoned {
            return Err(ChannelError::Poisoned);
        }
        let result = (|| {
            let ciphertext = read_frame_bounded(&mut self.stream, MAX_CIPHERTEXT)?;
            let plaintext = self
                .context
                .decode_message_from_peer::<RustCrypto>(&ciphertext, None::<&[u8]>)
                .map_err(|_| ChannelError::Authentication)?;
            if plaintext.is_empty() || plaintext.len() > MAX_PLAINTEXT {
                return Err(ChannelError::Size);
            }
            Ok(plaintext)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
}

fn send_handshake<S: Write>(
    stream: &mut S,
    handshake: &impl D2DHandshakeContext,
) -> Result<(), ChannelError> {
    let message = handshake
        .get_next_handshake_message()
        .ok_or(ChannelError::Handshake)?;
    if message.len() > MAX_HANDSHAKE {
        return Err(ChannelError::Size);
    }
    write_frame(stream, &message)?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::{
        net::{TcpListener, TcpStream},
        time::Duration,
    };

    fn pair() -> (SecureChannel<TcpStream>, SecureChannel<TcpStream>) {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            SecureChannel::respond(stream).unwrap()
        });
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        (
            SecureChannel::initiate(stream).unwrap(),
            server.join().unwrap(),
        )
    }

    #[test]
    fn legacy_transport_acceptance_keeps_encryption_without_claiming_payload_ack() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let request = read_frame_bounded(&mut stream, 4096).unwrap();
            crate::connection::request_info(&request).unwrap();
            let mut channel = SecureChannel::respond(stream).unwrap();
            let mut response = crate::connection::acceptance();
            // Public wire fixture: final field 7 is safe-disconnect version.
            assert_eq!(response.pop(), Some(6));
            response.push(4);
            write_frame(&mut channel.stream, &response).unwrap();
            let request = read_frame_bounded(&mut channel.stream, 4096).unwrap();
            assert_eq!(crate::connection::require_acceptance(&request).unwrap(), 6);
            channel.send(b"legacy encrypted traffic").unwrap();
            assert_eq!(channel.receive().unwrap(), b"still encrypted");
        });
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let request = crate::connection::request(b"TEST", &[0x10; 17], 7).unwrap();
        let mut channel = SecureChannel::initiate_nearby(stream, &request).unwrap();
        assert!(!channel.supports_payload_ack());
        assert_eq!(channel.receive().unwrap(), b"legacy encrypted traffic");
        channel.send(b"still encrypted").unwrap();
        server.join().unwrap();
    }

    #[test]
    fn reference_handshake_matches_and_exchanges_encrypted_messages_both_ways() {
        let (mut client, mut server) = pair();
        assert!(client.matches_authentication(server.authentication_token()));
        assert!(!client.matches_authentication(b"wrong-peer-value"));
        client.send(b"first Nearby frame").unwrap();
        assert_eq!(server.receive().unwrap(), b"first Nearby frame");
        server.send(b"response frame").unwrap();
        assert_eq!(client.receive().unwrap(), b"response frame");
        assert!(!format!("{client:?}").contains("authentication"));
    }

    #[test]
    fn replay_and_tampering_poison_the_channel() {
        for tamper in [false, true] {
            let (mut client, mut server) = pair();
            let mut packet = client
                .context
                .encode_message_to_peer::<RustCrypto>(b"synthetic", None::<&[u8]>);
            if tamper {
                let last = packet.len() - 1;
                packet[last] ^= 0x01;
            } else {
                write_frame(&mut client.stream, &packet).unwrap();
                assert_eq!(server.receive().unwrap(), b"synthetic");
            }
            write_frame(&mut client.stream, &packet).unwrap();
            assert!(matches!(
                server.receive(),
                Err(ChannelError::Authentication)
            ));
            assert!(matches!(server.receive(), Err(ChannelError::Poisoned)));
        }
    }
}
