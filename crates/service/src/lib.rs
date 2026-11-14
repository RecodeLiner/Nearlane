//! Application state shared by CLI and Qt, with explicit private-mode gating.

use nearlane_core::{Capabilities, UnavailableVisibility, Visibility};

/// Initial implementation reports unsupported features honestly and stays hidden.
#[derive(Debug, Default)]
pub struct Service {
    visibility: Visibility,
    capabilities: Capabilities,
}

impl Service {
    /// Current effective policy; never a speculative UI selection.
    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// The protocol backend is not connected yet, so all account features are off.
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    /// Change visibility only when the necessary trust has been established.
    pub fn set_visibility(&mut self, requested: Visibility) -> Result<(), UnavailableVisibility> {
        self.capabilities.require(requested)?;
        self.visibility = requested;
        Ok(())
    }

    /// A truthful status suitable for display without exposing private data.
    pub fn status(&self) -> String {
        format!(
            "Platform: {:?}. Hidden. Google certificate exchange is not verified.",
            nearlane_platform::current()
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_selection_does_not_change_effective_visibility() {
        let mut service = Service::default();
        assert!(service.set_visibility(Visibility::Contacts).is_err());
        assert_eq!(service.visibility(), Visibility::Hidden);
    }
}
