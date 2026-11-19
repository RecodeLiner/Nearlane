//! Toolkit-independent desktop preferences. Account grants never belong here.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 64 * 1024;

/// A persisted language choice shared by all desktop frontends.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Follow the operating system, with English as the fallback.
    #[default]
    System,
    /// English.
    En,
    /// Russian.
    Ru,
}
impl Language {
    /// Stable identifier used by UI controls and catalogs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::En => "en",
            Self::Ru => "ru",
        }
    }
    /// Validate a UI language without changing the saved choice on bad input.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "en" => Some(Self::En),
            "ru" => Some(Self::Ru),
            _ => None,
        }
    }
}

/// Local intent only; none of these values authorizes another device.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Version of the on-disk schema.
    pub version: u32,
    /// Interface language.
    pub language: Language,
    /// Resume receiving after private authentication succeeds.
    pub background_receiving: bool,
    /// Ask before saving a file from a verified own device.
    pub confirm_each_file: bool,
    /// Keep private discovery running while the window is hidden.
    pub continuous_discovery: bool,
    /// Explicit destination, or the OS Downloads/Nearlane default.
    pub receive_directory: Option<PathBuf>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            language: Language::System,
            background_receiving: true,
            confirm_each_file: false,
            continuous_discovery: true,
            receive_directory: None,
        }
    }
}
impl Preferences {
    /// Resolve the destination without creating it or receiving any data.
    pub fn destination(&self) -> Option<PathBuf> {
        self.receive_directory
            .clone()
            .or_else(|| dirs::download_dir().map(|p| p.join("Nearlane")))
    }
}

/// Persist shared desktop preferences atomically.
#[derive(Clone, Debug)]
pub struct PreferenceStore {
    path: PathBuf,
    legacy: Option<PathBuf>,
}
impl PreferenceStore {
    /// Explicit path for embedded integrations and isolated tests.
    pub fn at(path: PathBuf) -> Self {
        Self { path, legacy: None }
    }
    /// Runtime OS configuration directory. OAuth path overrides do not redirect
    /// preferences; the opt-in smoke directory applies only to smoke launches.
    pub fn discover() -> Option<Self> {
        if std::env::var_os("NEARLANE_SMOKE_TEST").is_some() {
            let root = PathBuf::from(std::env::var_os("NEARLANE_PREFERENCES_SMOKE_DIR")?);
            return root
                .is_absolute()
                .then(|| Self::at(root.join("preferences.json")));
        }
        let config = dirs::config_dir()?;
        let legacy = cfg!(target_os = "linux").then(|| config.join("Nearlane/Nearlane.conf"));
        Some(Self {
            path: config
                .join(if cfg!(target_os = "linux") {
                    "nearlane"
                } else {
                    "Nearlane"
                })
                .join("preferences.json"),
            legacy,
        })
    }
    /// Path for diagnostics, without reading account configuration.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Read bounded data. Missing preferences use defaults. A legacy Linux Qt
    /// INI file is migrated once, without modifying that original file.
    pub fn load(&self) -> io::Result<Preferences> {
        match read_bounded(&self.path) {
            Ok(data) => {
                let value: Preferences = serde_json::from_slice(&data).map_err(invalid)?;
                if value.version != 1 {
                    return Err(invalid("Unsupported preferences version"));
                }
                Ok(value)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if let Some(legacy) = &self.legacy {
                    match read_bounded(legacy) {
                        Ok(data) => {
                            let value =
                                legacy_preferences(std::str::from_utf8(&data).map_err(invalid)?)?;
                            self.save(&value)?;
                            return Ok(value);
                        }
                        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e),
                    }
                }
                Ok(Preferences::default())
            }
            Err(e) => Err(e),
        }
    }
    /// Replace settings atomically; never include tokens, keys or OAuth data.
    pub fn save(&self, value: &Preferences) -> io::Result<()> {
        if !self.path.is_absolute() || self.path.starts_with("/nix/store") {
            return Err(invalid("Invalid settings destination"));
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| invalid("Missing parent"))?;
        if fs::symlink_metadata(&self.path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(invalid("Settings destination is a symlink"));
        }
        fs::create_dir_all(parent)?;
        if fs::symlink_metadata(parent)?.file_type().is_symlink() {
            return Err(invalid("Settings directory is a symlink"));
        }
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, value).map_err(invalid)?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(&self.path).map_err(|e| e.error)?;
        Ok(())
    }
}
fn invalid(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
fn read_bounded(path: &Path) -> io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Not a regular settings file"));
    }
    let mut data = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_BYTES {
        return Err(invalid("Settings exceed limit"));
    }
    Ok(data)
}
fn legacy_preferences(text: &str) -> io::Result<Preferences> {
    let mut result = Preferences::default();
    let mut group = "";
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            group = &line[1..line.len() - 1];
            continue;
        }
        let Some((key, raw)) = line.split_once('=') else {
            continue;
        };
        match (group, key.trim()) {
            ("interface", "language") => {
                result.language = Language::parse(raw.trim()).unwrap_or_default()
            }
            ("receiving", "ownDevices") => result.background_receiving = parse_bool(raw)?,
            ("receiving", "confirmEachFile") => result.confirm_each_file = parse_bool(raw)?,
            ("receiving", "directory") => {
                let path = legacy_string(raw.trim())?;
                if !path.is_empty() {
                    result.receive_directory = Some(path.into());
                }
            }
            _ => {}
        }
    }
    Ok(result)
}
fn parse_bool(value: &str) -> io::Result<bool> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid("Invalid legacy boolean")),
    }
}
fn legacy_string(value: &str) -> io::Result<String> {
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    if value.starts_with('@') {
        return Err(invalid("Unsupported legacy settings encoding"));
    }
    let mut result = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        result.push(match chars.next() {
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('\\') => '\\',
            Some('"') => '"',
            Some('x') => {
                let mut hex = String::new();
                while hex.len() < 4 && chars.peek().is_some_and(char::is_ascii_hexdigit) {
                    if let Some(c) = chars.next() {
                        hex.push(c);
                    }
                }
                u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| invalid("Invalid legacy escape"))?
            }
            _ => return Err(invalid("Unsupported legacy escape")),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_private_receiving_intent_and_unicode_paths() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let legacy = root.path().join("Nearlane.conf");
        let content = "[interface]\nlanguage=ru\n[receiving]\nownDevices=false\nconfirmEachFile=true\ndirectory=\"/home/demo/Загрузки/файл, папка\"\n";
        fs::write(&legacy, content)?;
        let store = PreferenceStore {
            path: root.path().join("new/preferences.json"),
            legacy: Some(legacy.clone()),
        };
        let first = store.load()?;
        assert_eq!(first.language, Language::Ru);
        assert!(!first.background_receiving);
        assert!(first.confirm_each_file);
        assert_eq!(
            first.receive_directory,
            Some(PathBuf::from("/home/demo/Загрузки/файл, папка"))
        );
        fs::remove_file(&legacy)?;
        assert_eq!(first, store.load()?);
        Ok(())
    }
    #[test]
    fn future_or_oversized_settings_are_not_overwritten() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("preferences.json");
        let store = PreferenceStore::at(path.clone());
        fs::write(&path, br#"{"version":99}"#)?;
        assert!(store.load().is_err());
        assert_eq!(fs::read(&path)?, br#"{"version":99}"#);
        fs::write(&path, vec![b' '; MAX_BYTES as usize + 1])?;
        assert!(store.load().is_err());
        Ok(())
    }
}
