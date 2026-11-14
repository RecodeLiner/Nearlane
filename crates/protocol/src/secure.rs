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

/// Sanitized transport failures; never contains plaintext, keys or peer messages.
#[derive(Debug)]
pub enum ChannelError {
    /// Stream IO failed.
    Io(io::Error),
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
        f.write_str(match self {
            Self::Io(_) => "Nearby stream IO failed",
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
}

impl<S> fmt::Debug for SecureChannel<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecureChannel")
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

impl<S: Read + Write> SecureChannel<S> {
    /// Perform the initiating side of the three-message UKEY2 handshake.
    /// The caller must set IO deadlines on network streams before this call.
    pub fn initiate(mut stream: S) -> Result<Self, ChannelError> {
        let mut handshake = InitiatorD2DHandshakeContext::<RustCrypto>::new(
            HandshakeImplementation::PublicKeyInProtobuf,
            vec![NextProtocol::Aes256CbcHmacSha256],
        );
        send_handshake(&mut stream, &handshake)?;
        let response = read_frame_bounded(&mut stream, MAX_HANDSHAKE)?;
        handshake
            .handle_handshake_message(&response)
            .map_err(|_| ChannelError::Handshake)?;
        send_handshake(&mut stream, &handshake)?;
        Self::complete(stream, &mut handshake)
    }

    /// Perform the receiving side, retaining no partially completed handshake.
    pub fn respond(mut stream: S) -> Result<Self, ChannelError> {
        let mut handshake = ServerD2DHandshakeContext::<RustCrypto>::new(
            HandshakeImplementation::PublicKeyInProtobuf,
            &[NextProtocol::Aes256CbcHmacSha256],
        );
        let init = read_frame_bounded(&mut stream, MAX_HANDSHAKE)?;
        handshake
            .handle_handshake_message(&init)
            .map_err(|_| ChannelError::Handshake)?;
        send_handshake(&mut stream, &handshake)?;
        let finish = read_frame_bounded(&mut stream, MAX_HANDSHAKE)?;
        handshake
            .handle_handshake_message(&finish)
            .map_err(|_| ChannelError::Handshake)?;
        Self::complete(stream, &mut handshake)
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

    /// Encrypt one Nearby frame. Call only after the relevant trust/consent gate.
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
