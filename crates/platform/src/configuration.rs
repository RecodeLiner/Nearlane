//! Per-account application configuration paths, resolved at runtime only.
use crate::Platform;
use std::{ffi::OsString, path::PathBuf};

/// OAuth application parameters, independent of the process working directory.
/// An explicit override must be absolute; an invalid override fails closed.
/// No directory or file is created here, and no account token is read.
pub fn oauth_config_path() -> Option<PathBuf> {
    resolve(crate::current(), |name| std::env::var_os(name))
}

fn resolve(platform: Platform, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |value: OsString| {
        let path = PathBuf::from(value);
        path.is_absolute().then_some(path)
    };
    if let Some(value) = env("NEARLANE_OAUTH_CONFIG") {
        return absolute(value);
    }
    let base = match platform {
        Platform::Linux => env("XDG_CONFIG_HOME")
            .and_then(absolute)
            .or_else(|| env("HOME").and_then(absolute).map(|p| p.join(".config")))?
            .join("nearlane"),
        Platform::MacOs => env("HOME")
            .and_then(absolute)?
            .join("Library/Application Support/Nearlane"),
        Platform::Windows => env("APPDATA").and_then(absolute)?.join("Nearlane"),
        Platform::Unsupported => return None,
    };
    Some(base.join("oauth-client.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        std::env::temp_dir().join("nearlane-path-fixture")
    }
    #[test]
    fn platform_defaults_and_override_are_absolute_and_cwd_independent() {
        let home = root();
        let env = |name: &str| match name {
            "HOME" | "APPDATA" => Some(home.clone().into_os_string()),
            _ => None,
        };
        assert_eq!(
            resolve(Platform::Linux, env),
            Some(home.join(".config/nearlane/oauth-client.json"))
        );
        assert_eq!(
            resolve(Platform::MacOs, env),
            Some(home.join("Library/Application Support/Nearlane/oauth-client.json"))
        );
        assert_eq!(
            resolve(Platform::Windows, env),
            Some(home.join("Nearlane/oauth-client.json"))
        );
        let external = home.join("external.json");
        assert_eq!(
            resolve(Platform::Linux, |key| (key == "NEARLANE_OAUTH_CONFIG")
                .then(|| external.clone().into_os_string())),
            Some(external)
        );
    }
    #[test]
    fn relative_xdg_falls_back_but_invalid_explicit_override_never_does() {
        let home = root();
        assert_eq!(
            resolve(Platform::Linux, |key| match key {
                "XDG_CONFIG_HOME" => Some("relative".into()),
                "HOME" => Some(home.clone().into_os_string()),
                _ => None,
            }),
            Some(home.join(".config/nearlane/oauth-client.json"))
        );
        let xdg = home.join("custom-config");
        assert_eq!(
            resolve(Platform::Linux, |key| (key == "XDG_CONFIG_HOME")
                .then(|| xdg.clone().into_os_string())),
            Some(xdg.join("nearlane/oauth-client.json"))
        );
        for value in ["", "relative.json"] {
            assert!(
                resolve(Platform::Linux, |key| match key {
                    "NEARLANE_OAUTH_CONFIG" => Some(value.into()),
                    "HOME" => Some(home.clone().into_os_string()),
                    _ => None,
                })
                .is_none()
            );
        }
        assert!(resolve(Platform::Linux, |_| None).is_none());
    }
}
