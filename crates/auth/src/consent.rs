use crate::callback::{AuthError, bounded_url, unique_parameters};

/// Safe-to-display metadata; intentionally excludes IDs, state, codes and tokens.
#[derive(Debug, Eq, PartialEq)]
pub struct ConsentSummary {
    /// The validated Google authorization path.
    pub path: String,
    /// Requested OAuth permission names.
    pub scopes: Vec<String>,
    /// A client identifier was present, but is not retained.
    pub has_client_id: bool,
    /// Whether S256 PKCE is advertised.
    pub uses_pkce_s256: bool,
    /// Whether a state value was present.
    pub has_state: bool,
    /// Redirect kind, without any query string or account information.
    pub redirect_kind: &'static str,
}

/// Inspect a captured browser URL offline, retaining no credentials or client ID.
pub fn inspect_authorization_url(input: &str) -> Result<ConsentSummary, AuthError> {
    let url = bounded_url(input)?;
    if url.scheme() != "https"
        || url.host_str() != Some("accounts.google.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(AuthError::UntrustedOrigin);
    }
    if !matches!(url.path(), "/o/oauth2/v2/auth" | "/o/oauth2/auth") {
        return Err(AuthError::InvalidUrl);
    }
    let params = unique_parameters(&url)?;
    let redirect_kind = match params
        .get("redirect_uri")
        .and_then(|s| url::Url::parse(s).ok())
    {
        Some(redirect)
            if redirect.scheme() == "http"
                && matches!(
                    redirect.host_str(),
                    Some("127.0.0.1" | "[::1]" | "localhost")
                ) =>
        {
            "loopback"
        }
        Some(redirect) if redirect.scheme() == "https" => "https",
        Some(_) => "custom-scheme",
        None => "missing-or-invalid",
    };
    Ok(ConsentSummary {
        path: url.path().to_owned(),
        scopes: params
            .get("scope")
            .map(|s| s.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default(),
        has_client_id: params.get("client_id").is_some_and(|s| !s.is_empty()),
        uses_pkce_s256: params
            .get("code_challenge_method")
            .is_some_and(|s| s == "S256")
            && params.get("code_challenge").is_some_and(|s| !s.is_empty()),
        has_state: params.get("state").is_some_and(|s| !s.is_empty()),
        redirect_kind,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn inspection_does_not_retain_private_parameters() {
        let summary = inspect_authorization_url("https://accounts.google.com/o/oauth2/v2/auth?client_id=private-client&state=private-state&scope=openid%20email&redirect_uri=http%3A%2F%2F127.0.0.1%3A1234%2Fcallback").unwrap();
        let debug = format!("{summary:?}");
        assert!(!debug.contains("private-client"));
        assert!(!debug.contains("private-state"));
        assert_eq!(summary.redirect_kind, "loopback");
        assert_eq!(summary.scopes, ["openid", "email"]);
    }
    #[test]
    fn rejects_lookalike_google_domains() {
        for origin in [
            "https://accounts.google.com.evil.test",
            "http://accounts.google.com",
            "https://accounts.google.com@evil.test",
        ] {
            assert!(inspect_authorization_url(&format!("{origin}/o/oauth2/v2/auth")).is_err());
        }
    }
}
