//! App-scoped OS credential storage. No plaintext-file fallback or global vault enumeration.
use sha2::{Digest as _, Sha256};
use std::fmt;
use zeroize::Zeroizing;

/// Sanitized OS-store failure. Provider errors can contain paths or identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreError;
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("System credential storage is unavailable or locked.")
    }
}
impl std::error::Error for StoreError {}

/// Serial worker-owned store contract. Tests use isolated synthetic implementations.
pub trait SecretStore: Send + 'static {
    /// Read only this application's entry, with zeroizing ownership.
    fn load(&mut self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError>;
    /// Replace this application's entry in the OS vault only.
    fn save(&mut self, bytes: &[u8]) -> Result<(), StoreError>;
    /// Remove this application's entry; absence counts as success.
    fn delete(&mut self) -> Result<(), StoreError>;
}

/// Keychain / Credential Manager / Secret Service, scoped by the OAuth client hash.
/// Creation performs no vault operation. Use only on a background worker.
pub struct SystemStore {
    entry_key: String,
}
impl SystemStore {
    /// Namespace the entry without using account names or raw client parameters.
    pub fn for_client(client_id: &str) -> Self {
        let hash = Sha256::digest(client_id.as_bytes());
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        Self {
            entry_key: format!("oauth-v1-{hex}"),
        }
    }
    fn entry(&self) -> Result<keyring::Entry, StoreError> {
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        {
            keyring::Entry::new("org.nearlane.Nearlane", &self.entry_key).map_err(|_| StoreError)
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Err(StoreError)
        }
    }
}
impl fmt::Debug for SystemStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SystemStore([REDACTED])")
    }
}
impl SecretStore for SystemStore {
    fn load(&mut self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError> {
        match self.entry()?.get_secret() {
            Ok(bytes) if bytes.len() <= 32 * 1024 => Ok(Some(Zeroizing::new(bytes))),
            Ok(bytes) => {
                drop(Zeroizing::new(bytes));
                Err(StoreError)
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(StoreError),
        }
    }
    fn save(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
        if bytes.len() > 32 * 1024 {
            return Err(StoreError);
        }
        self.entry()?.set_secret(bytes).map_err(|_| StoreError)
    }
    fn delete(&mut self) -> Result<(), StoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(StoreError),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "explicit OS-vault check; creates and deletes a unique synthetic entry"]
    fn system_store_roundtrip() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut store = SystemStore::for_client(&format!("Nearlane synthetic vault test {unique}"));
        let result = (|| {
            assert!(store.load()?.is_none());
            store.save(b"synthetic-no-account-secret")?;
            assert_eq!(
                store.load()?.unwrap().as_slice(),
                b"synthetic-no-account-secret"
            );
            Ok::<_, StoreError>(())
        })();
        let deleted = store.delete();
        assert!(result.is_ok(), "synthetic OS-vault roundtrip failed");
        assert!(deleted.is_ok(), "synthetic OS-vault cleanup failed");
        assert!(store.load().unwrap().is_none());
    }
}
