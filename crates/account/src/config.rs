use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};

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
        Self::read(path, true)
    }

    fn read(path: &Path, require_private: bool) -> Result<Self, LoginError> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            // A chosen FIFO/device must not block the GUI before metadata checks.
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = options.open(path).map_err(|_| LoginError::Configuration)?;
        let metadata = file.metadata().map_err(|_| LoginError::Configuration)?;
        if !metadata.is_file() || metadata.len() > 16 * 1024 {
            return Err(LoginError::Configuration);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if require_private && metadata.permissions().mode() & 0o077 != 0 {
                return Err(LoginError::Configuration);
            }
        }
        #[cfg(not(unix))]
        let _ = require_private;
        let mut bytes = Zeroizing::new(Vec::new());
        file.take(16 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| LoginError::Configuration)?;
        Self::parse(&bytes)
    }

    /// Validate a selected JSON and atomically install normalized application
    /// parameters. The original file is untouched; unknown fields are not copied.
    /// No account grants are imported or written by this operation.
    pub fn import(source: &Path, destination: &Path) -> Result<Self, LoginError> {
        let config = Self::read(source, false)?;
        config.save_private(destination)?;
        Ok(config)
    }

    fn save_private(&self, destination: &Path) -> Result<(), LoginError> {
        let save = || -> Result<(), Box<dyn std::error::Error>> {
            if !destination.is_absolute() || destination.starts_with("/nix/store") {
                return Err("invalid local configuration destination".into());
            }
            let directory = destination
                .parent()
                .ok_or("missing configuration directory")?;
            if let Ok(metadata) = fs::symlink_metadata(directory) {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("invalid configuration directory".into());
                }
            }
            let new_directory = !directory.exists();
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt as _;
                builder.mode(0o700);
            }
            builder.create(directory)?;
            // Resolve parent links before changing permissions or writing secrets.
            if directory.canonicalize()?.starts_with("/nix/store") {
                return Err("configuration directory resolves into the store".into());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                // Do not chmod an unrelated existing parent of an explicit override.
                if new_directory
                    || directory
                        .file_name()
                        .is_some_and(|n| n == "nearlane" || n == "Nearlane")
                {
                    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
                }
                if fs::metadata(directory)?.permissions().mode() & 0o022 != 0 {
                    return Err("configuration directory is writable by another account".into());
                }
            }
            #[cfg(not(unix))]
            let _ = new_directory;
            if let Ok(metadata) = fs::symlink_metadata(destination) {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err("invalid configuration file".into());
                }
            }
            let mut file = tempfile::Builder::new()
                .prefix(".oauth-client-")
                .tempfile_in(directory)?;
            file.write_all(b"{\"client_id\":")?;
            serde_json::to_writer(&mut file, self.client_id.as_str())?;
            if let Some(secret) = &self.client_secret {
                file.write_all(b",\"client_secret\":")?;
                serde_json::to_writer(&mut file, secret.as_str())?;
            }
            file.write_all(b",\"scope\":")?;
            serde_json::to_writer(&mut file, nearlane_auth::NEARBY_SCOPE)?;
            file.write_all(b"}\n")?;
            file.as_file().sync_all()?;
            file.persist(destination)?;
            Ok(())
        };
        save().map_err(|_| LoginError::ConfigurationStorage)
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

    #[test]
    fn explicit_import_normalizes_and_survives_source_removal() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected.json");
        fs::write(&source, br#"{"client_id":"synthetic-client","client_secret":"synthetic-secret","refresh_token":"must-not-be-copied","token_uri":"https://invalid.example"}"#).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(ClientConfig::load(&source).is_err());
        }
        let destination = root.path().join("config/nearlane/oauth-client.json");
        ClientConfig::import(&source, &destination).unwrap();
        fs::remove_file(&source).unwrap();
        let config = ClientConfig::load(&destination).unwrap();
        assert_eq!(config.client_id.as_str(), "synthetic-client");
        let saved = fs::read_to_string(&destination).unwrap();
        assert!(!saved.contains("must-not-be-copied") && !saved.contains("token_uri"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(destination.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn invalid_import_keeps_the_previous_valid_configuration() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected.json");
        let destination = root.path().join("config/nearlane/oauth-client.json");
        fs::write(&source, br#"{"client_id":"synthetic-first"}"#).unwrap();
        ClientConfig::import(&source, &destination).unwrap();
        let previous = fs::read(&destination).unwrap();
        fs::write(&source, b"{}").unwrap();
        assert_eq!(
            ClientConfig::import(&source, &destination).err(),
            Some(LoginError::Configuration)
        );
        assert_eq!(fs::read(&destination).unwrap(), previous);
        fs::write(&source, br#"{"client_id":"synthetic-replacement"}"#).unwrap();
        ClientConfig::import(&source, &destination).unwrap();
        assert_eq!(
            ClientConfig::load(&destination).unwrap().client_id.as_str(),
            "synthetic-replacement"
        );
        assert_eq!(
            fs::read_dir(destination.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn import_refuses_symlink_destinations_and_the_nix_store() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("selected.json");
        fs::write(&source, br#"{"client_id":"synthetic-client"}"#).unwrap();
        let other = root.path().join("other");
        fs::create_dir(&other).unwrap();
        let directory = root.path().join("linked");
        symlink(&other, &directory).unwrap();
        assert_eq!(
            ClientConfig::import(&source, &directory.join("oauth-client.json")).err(),
            Some(LoginError::ConfigurationStorage)
        );
        assert_eq!(fs::read_dir(&other).unwrap().count(), 0);
        let target = other.join("oauth-client.json");
        symlink(&source, &target).unwrap();
        assert_eq!(
            ClientConfig::import(&source, &target).err(),
            Some(LoginError::ConfigurationStorage)
        );
        assert_eq!(
            ClientConfig::import(
                &source,
                Path::new("/nix/store/nearlane-test/oauth-client.json")
            )
            .err(),
            Some(LoginError::ConfigurationStorage)
        );
    }
}
