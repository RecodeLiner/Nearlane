use std::fmt;

use subtle::ConstantTimeEq as _;
use url::Url;
use zeroize::Zeroizing;

use crate::{NEARBY_SCOPE, pkce};

/// Errors never contain authorization codes, tokens, or raw provider responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthError {
    /// A URL is invalid or too large.
    InvalidUrl,
    /// Only the known Google authorization origin is accepted.
    UntrustedOrigin,
    /// The configured redirect is not an explicit IPv4 loopback callback.
    InvalidRedirect,
    /// A required parameter is absent or duplicated.
    InvalidParameters,
    /// The callback does not belong to this authorization attempt.
    StateMismatch,
    /// The provider denied authorization.
    ProviderDenied,
    /// The operating system could not provide secure entropy.
    Entropy,
    /// The token response is malformed or does not grant the required permission.
    InvalidGrant,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "authorization rejected: {self:?}")
    }
}
impl std::error::Error for AuthError {}

/// A short-lived authorization code. Formatting is deliberately redacted.
pub struct AuthorizationCode(Zeroizing<String>);

impl AuthorizationCode {
    /// Expose only to the token exchanger; never persist or log this value.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AuthorizationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthorizationCode([REDACTED])")
    }
}

/// One browser authorization attempt with independent CSRF state and PKCE.
///
/// Keep this object in the trusted service, not the QML UI. Consuming it when
/// accepting a callback prevents accidental reuse of the verifier.
pub struct AuthorizationAttempt {
    redirect: Url,
    verifier: Zeroizing<String>,
    state: Zeroizing<String>,
}

impl fmt::Debug for AuthorizationAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthorizationAttempt([REDACTED])")
    }
}

impl AuthorizationAttempt {
    /// Exact redirect to supply to the token exchanger after browser completion.
    pub fn redirect_uri(&self) -> &str {
        self.redirect.as_str()
    }

    /// Prepare an attempt after binding a listener to an OS-assigned loopback port.
    pub fn new(port: u16) -> Result<Self, AuthError> {
        if port == 0 {
            return Err(AuthError::InvalidRedirect);
        }
        let redirect = Url::parse(&format!("http://127.0.0.1:{port}/oauth/callback"))
            .map_err(|_| AuthError::InvalidRedirect)?;
        Ok(Self {
            redirect,
            verifier: Zeroizing::new(pkce::random_secret()?),
            state: Zeroizing::new(pkce::random_secret()?),
        })
    }

    /// Generate a consent URL for a caller-supplied, authorized OAuth client ID.
    /// This is a research plan, not a claim that this client can access Nearby.
    pub fn authorization_url(&self, client_id: &str) -> Result<Url, AuthError> {
        if client_id.is_empty() || client_id.len() > 1024 || client_id.chars().any(char::is_control)
        {
            return Err(AuthError::InvalidParameters);
        }
        let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
            .map_err(|_| AuthError::InvalidUrl)?;
        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", self.redirect.as_str())
            .append_pair("response_type", "code")
            .append_pair("scope", NEARBY_SCOPE)
            .append_pair("state", &self.state)
            .append_pair("code_challenge", &pkce::challenge(&self.verifier))
            .append_pair("code_challenge_method", "S256")
            .append_pair("prompt", "select_account consent")
            .append_pair("access_type", "offline");
        Ok(url)
    }

    /// Check origin, callback path, unique parameters and CSRF state.
    /// Returns code and verifier only after all checks pass. Both redact Debug.
    pub fn accept(self, callback: &str) -> Result<(AuthorizationCode, PkceVerifier), AuthError> {
        let code = self.parse_callback(callback)?;
        Ok((code, PkceVerifier(self.verifier)))
    }

    /// Validate a callback without consuming this attempt. A stray loopback
    /// request must not destroy a legitimate browser login in progress.
    pub fn validate_callback(&self, callback: &str) -> Result<(), AuthError> {
        self.parse_callback(callback).map(|_| ())
    }

    fn parse_callback(&self, callback: &str) -> Result<AuthorizationCode, AuthError> {
        let url = bounded_url(callback)?;
        if url.scheme() != self.redirect.scheme()
            || url.host_str() != self.redirect.host_str()
            || url.port_or_known_default() != self.redirect.port_or_known_default()
            || url.path() != self.redirect.path()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(AuthError::InvalidRedirect);
        }
        let params = unique_parameters(&url)?;
        let state = params.get("state").ok_or(AuthError::InvalidParameters)?;
        if !bool::from(state.as_bytes().ct_eq(self.state.as_bytes())) {
            return Err(AuthError::StateMismatch);
        }
        if params.contains_key("error") {
            return Err(AuthError::ProviderDenied);
        }
        let code = params
            .get("code")
            .filter(|s| !s.is_empty())
            .ok_or(AuthError::InvalidParameters)?;
        Ok(AuthorizationCode(Zeroizing::new(code.clone())))
    }
}

/// PKCE verifier that may be exposed only to a token request.
pub struct PkceVerifier(Zeroizing<String>);
impl PkceVerifier {
    /// Expose to a token exchanger; never log this value.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for PkceVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PkceVerifier([REDACTED])")
    }
}

pub(crate) fn bounded_url(input: &str) -> Result<Url, AuthError> {
    if input.len() > 16 * 1024 {
        return Err(AuthError::InvalidUrl);
    }
    Url::parse(input).map_err(|_| AuthError::InvalidUrl)
}

pub(crate) fn unique_parameters(
    url: &Url,
) -> Result<std::collections::BTreeMap<String, String>, AuthError> {
    let mut params = std::collections::BTreeMap::new();
    for (key, value) in url.query_pairs() {
        if params
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Err(AuthError::InvalidParameters);
        }
    }
    Ok(params)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn fixture() -> AuthorizationAttempt {
        AuthorizationAttempt {
            redirect: Url::parse("http://127.0.0.1:43119/oauth/callback").unwrap(),
            verifier: Zeroizing::new("verifier-secret".into()),
            state: Zeroizing::new("expected-state".into()),
        }
    }

    #[test]
    fn accepts_only_matching_code_and_state() {
        let (code, verifier) = fixture()
            .accept("http://127.0.0.1:43119/oauth/callback?code=private-code&state=expected-state")
            .unwrap();
        assert_eq!(code.expose_secret(), "private-code");
        assert_eq!(verifier.expose_secret(), "verifier-secret");
        assert!(!format!("{code:?} {verifier:?}").contains("private-code"));
    }

    #[test]
    fn rejects_wrong_state_duplicate_params_and_remote_callbacks() {
        for url in [
            "http://127.0.0.1:43119/oauth/callback?code=x&state=wrong",
            "http://127.0.0.1:43119/oauth/callback?code=x&state=expected-state&state=expected-state",
            "http://example.com:43119/oauth/callback?code=x&state=expected-state",
            "http://127.0.0.1:43120/oauth/callback?code=x&state=expected-state",
            "http://127.0.0.1:43119/other?code=x&state=expected-state",
            "http://127.0.0.1:43119/oauth/callback?code=x&state=expected-state#fragment",
        ] {
            assert!(fixture().accept(url).is_err(), "accepted invalid callback");
        }
    }

    #[test]
    fn reports_denial_without_echoing_provider_content() {
        let result = fixture().accept(
            "http://127.0.0.1:43119/oauth/callback?error=access_denied&state=expected-state",
        );
        assert_eq!(result.unwrap_err(), AuthError::ProviderDenied);
    }

    #[test]
    fn consent_has_pkce_but_never_the_verifier() {
        let url = fixture().authorization_url("test-client").unwrap();
        let params = unique_parameters(&url).unwrap();
        assert_eq!(params["code_challenge_method"], "S256");
        assert_eq!(params["scope"], NEARBY_SCOPE);
        assert!(!url.as_str().contains("verifier-secret"));
    }
}
