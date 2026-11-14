use std::{fmt, fs::File, io::Read, path::Path};

use zeroize::Zeroizing;

use crate::LoginError;

/// Local desktop application parameters; these are never compiled into Nearlane.
/// Debug deliberately omits both fields. Account tokens are not configuration.
#[derive(Clone)]
pub struct ClientConfig {
    pub(crate) client_id: Zeroizing<String>,
    pub(crate) client_secret: Option<Zeroizing<String>>,
}

impl ClientConfig {
    /// Read a bounded private JSON file. Never includes file contents in errors.
    pub fn load(path: &Path) -> Result<Self, LoginError> {
        let file = File::open(path).map_err(|_| LoginError::Configuration)?;
        let metadata = file.metadata().map_err(|_| LoginError::Configuration)?;
        if !metadata.is_file() || metadata.len() > 16 * 1024 {
            return Err(LoginError::Configuration);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(LoginError::Configuration);
            }
        }
        let mut bytes = Zeroizing::new(Vec::new());
        file.take(16 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| LoginError::Configuration)?;
        Self::parse(&bytes)
    }

    fn parse(bytes: &[u8]) -> Result<Self, LoginError> {
        if bytes.len() > 16 * 1024 {
            return Err(LoginError::Configuration);
        }
        let mut value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| LoginError::Configuration)?;
        let serde_json::Value::String(client_id) = value["client_id"].take() else {
            return Err(LoginError::Configuration);
        };
        let client_id = Zeroizing::new(client_id);
        if client_id.is_empty()
            || client_id.len() > 1024
            || !client_id.is_ascii()
            || client_id
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(LoginError::Configuration);
        }
        // Endpoints and permission are fixed in the implementation. A local
        // JSON file cannot redirect an authorization code to another provider.
        if value
            .get("scope")
            .is_some_and(|v| v.as_str() != Some(nearlane_auth::NEARBY_SCOPE))
        {
            return Err(LoginError::Configuration);
        }
        let client_secret = match value["client_secret"].take() {
            serde_json::Value::Null => None,
            serde_json::Value::String(secret) => {
                let secret = Zeroizing::new(secret);
                if secret.is_empty() || secret.len() > 1024 || secret.chars().any(char::is_control)
                {
                    return Err(LoginError::Configuration);
                }
                Some(secret)
            }
            _ => return Err(LoginError::Configuration),
        };
        Ok(Self {
            client_id,
            client_secret,
        })
    }
}

impl fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClientConfig([REDACTED])")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_bounded_config_without_printing_values() {
        let config = ClientConfig::parse(
            br#"{"client_id":"synthetic-client","client_secret":"synthetic-secret"}"#,
        )
        .unwrap();
        assert!(!format!("{config:?}").contains("synthetic"));
        for bytes in [
            b"{}".as_slice(),
            br#"{"client_id":"x","scope":"openid"}"#,
            br#"{"client_id":"x","client_secret":false}"#,
            &vec![b'x'; 16 * 1024 + 1],
        ] {
            assert!(ClientConfig::parse(bytes).is_err());
        }
    }
}
