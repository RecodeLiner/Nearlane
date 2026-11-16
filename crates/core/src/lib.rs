//! UI-independent domain types. This crate intentionally has no dependencies.
mod text;
pub use text::Text;

/// Per-file product policy shared by transfer validation and notifications.
/// Streaming buffers and encrypted frame bounds remain independent of this size.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024 * 1024;

/// The privacy policy enforced by the prototype.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Visibility {
    /// Do not advertise or receive; outgoing discovery has a separate switch.
    #[default]
    Hidden,
    /// Permit only peers authenticated as contacts.
    Contacts,
    /// Permit only peers authenticated as the same account.
    OwnDevices,
}

/// What the running backend has actually verified, not what its UI promises.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Capabilities {
    /// Certificate exchange has been successfully established with Google.
    pub account_certificates: bool,
    /// Contact certificate distribution has been verified separately.
    pub contacts: bool,
}

/// An unavailable privacy mode must never silently become public visibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnavailableVisibility(pub Visibility);

impl std::fmt::Display for UnavailableVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} visibility is not available on this backend",
            self.0
        )
    }
}

impl std::error::Error for UnavailableVisibility {}

impl Capabilities {
    /// Validate a requested policy without downgrading it to public access.
    pub fn require(self, mode: Visibility) -> Result<(), UnavailableVisibility> {
        let supported = match mode {
            Visibility::Hidden => true,
            Visibility::Contacts => self.account_certificates && self.contacts,
            Visibility::OwnDevices => self.account_certificates,
        };
        supported.then_some(()).ok_or(UnavailableVisibility(mode))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn signed_out_backend_cannot_advertise_private_modes() {
        let capabilities = Capabilities::default();
        assert!(capabilities.require(Visibility::Hidden).is_ok());
        assert!(capabilities.require(Visibility::Contacts).is_err());
        assert!(capabilities.require(Visibility::OwnDevices).is_err());
    }

    #[test]
    fn self_share_is_not_proof_of_contacts() {
        let capabilities = Capabilities {
            account_certificates: true,
            contacts: false,
        };
        assert!(capabilities.require(Visibility::OwnDevices).is_ok());
        assert!(capabilities.require(Visibility::Contacts).is_err());
    }
}
