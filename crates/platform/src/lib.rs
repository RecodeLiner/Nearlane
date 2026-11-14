//! OS integration boundary. No fake Bluetooth discovery or secret storage.

/// Identifies where a future OS-specific adapter must be implemented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    /// BlueZ / NetworkManager / Secret Service.
    Linux,
    /// Windows Bluetooth / networking / credential APIs.
    Windows,
    /// CoreBluetooth / networking / Keychain.
    MacOs,
    /// This operating system has not been targeted.
    Unsupported,
}

/// Return the build target, not the host on which cross-compilation happened.
pub const fn current() -> Platform {
    if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else {
        Platform::Unsupported
    }
}
