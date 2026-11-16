//! OS integration boundary. No fake Bluetooth discovery or secret storage.
pub mod bluetooth;
pub mod configuration;
pub mod credentials;
pub mod notifications;
pub mod preferences;

/// Open an authorization URL in the OS browser without logging it or passing
/// it through a shell. Only the application controller calls this function.
pub fn open_browser(url: &str) -> std::io::Result<()> {
    open::that_detached(url)
}

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
