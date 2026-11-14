use aes::cipher::{KeyIvInit as _, StreamCipher as _};
use aes_gcm::{Aes256Gcm, KeyInit as _, aead::Aead as _};
use hkdf::Hkdf;
use hmac::{Hmac, Mac as _};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::CertificateError;

pub(crate) fn random<const N: usize>() -> Result<Zeroizing<[u8; N]>, CertificateError> {
    use rand::RngCore as _;
    let mut bytes = Zeroizing::new([0_u8; N]);
    rand::rngs::OsRng
        .try_fill_bytes(bytes.as_mut())
        .map_err(|_| CertificateError::Entropy)?;
    Ok(bytes)
}

pub(crate) fn derive<const N: usize>(key: &[u8]) -> Result<Zeroizing<[u8; N]>, CertificateError> {
    let mut bytes = Zeroizing::new([0_u8; N]);
    Hkdf::<Sha256>::new(None, key)
        .expand(&[], bytes.as_mut())
        .map_err(|_| CertificateError::Key)?;
    Ok(bytes)
}

pub(crate) fn metadata_tag(key: &[u8]) -> Result<[u8; 32], CertificateError> {
    let mut hmac =
        <Hmac<Sha256> as hmac::Mac>::new_from_slice(&[0; 32]).map_err(|_| CertificateError::Key)?;
    hmac.update(key);
    Ok(hmac.finalize().into_bytes().into())
}

pub(crate) fn authentication_hash(
    token: &[u8],
    secret: &[u8],
) -> Result<[u8; 6], CertificateError> {
    // HKDF salt = certificate secret, input key material = UKEY2 auth token.
    let mut result = [0; 6];
    Hkdf::<Sha256>::new(Some(secret), token)
        .expand(&[], &mut result)
        .map_err(|_| CertificateError::Key)?;
    Ok(result)
}

pub(crate) fn crypt_metadata_key(
    secret: &[u8; 32],
    salt: &[u8; 2],
    key: &[u8; 14],
) -> Result<Zeroizing<[u8; 14]>, CertificateError> {
    let counter = derive::<16>(salt)?;
    let mut cipher = ctr::Ctr128BE::<aes::Aes256>::new(secret.into(), counter.as_ref().into());
    let mut output = Zeroizing::new(*key);
    cipher.apply_keystream(output.as_mut());
    Ok(output)
}

pub(crate) fn encrypt_metadata(
    metadata: &[u8],
    key: &[u8],
    secret: &[u8],
) -> Result<Vec<u8>, CertificateError> {
    let key = derive::<32>(key)?;
    let nonce = derive::<12>(secret)?;
    Aes256Gcm::new_from_slice(key.as_ref())
        .map_err(|_| CertificateError::Key)?
        .encrypt(aes_gcm::Nonce::from_slice(nonce.as_ref()), metadata)
        .map_err(|_| CertificateError::Integrity)
}

pub(crate) fn decrypt_metadata(
    metadata: &[u8],
    key: &[u8],
    secret: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CertificateError> {
    let key = derive::<32>(key)?;
    let nonce = derive::<12>(secret)?;
    Aes256Gcm::new_from_slice(key.as_ref())
        .map_err(|_| CertificateError::Key)?
        .decrypt(aes_gcm::Nonce::from_slice(nonce.as_ref()), metadata)
        .map(Zeroizing::new)
        .map_err(|_| CertificateError::Integrity)
}
