//! Browser-based OAuth research primitives. No embedded browser or master token.
//!
//! Generating an authorization URL does not prove Google permits the requested
//! scope for a third-party client. No client ID or secret is bundled here.

mod callback;
mod consent;
mod pkce;
mod token;

pub use callback::{AuthError, AuthorizationAttempt, AuthorizationCode, PkceVerifier};
pub use consent::{ConsentSummary, inspect_authorization_url};
pub use token::{TokenGrant, TokenRequest};

/// Scope observed in Google's public Chromium source; third-party access unproven.
pub const NEARBY_SCOPE: &str = "https://www.googleapis.com/auth/nearbysharing-pa";

/// Alternative scope present in the official Windows client; access unproven.
pub const NEARBY_SDK_SCOPE: &str = "https://www.googleapis.com/auth/nearbysharingsdk";
