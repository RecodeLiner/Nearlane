use std::fmt;

use zeroize::Zeroizing;

use crate::{AuthError, AuthorizationCode, NEARBY_SCOPE, NEARBY_SDK_SCOPE, PkceVerifier};

/// A transport-neutral request for Google's token endpoint.
///
/// The future HTTPS adapter must refuse redirects and cap the response. This
/// type performs no network request and bundles no Google client credentials.
pub struct TokenRequest(Zeroizing<String>);

impl TokenRequest {
    /// The fixed provider endpoint; callers cannot redirect secrets elsewhere.
    pub const ENDPOINT: &str = "https://oauth2.googleapis.com/token";

    /// Standard offline-access renewal. Does not require a master token.
    pub fn refresh(
        client_id: &str,
        client_secret: Option<&str>,
        refresh_token: &str,
    ) -> Result<Self, AuthError> {
        if client_id.is_empty() || !valid_token(refresh_token) {
            return Err(AuthError::InvalidParameters);
        }
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("grant_type", "refresh_token")
            .append_pair("client_id", client_id)
            .append_pair("refresh_token", refresh_token);
        if let Some(secret) = client_secret {
            if secret.is_empty() {
                return Err(AuthError::InvalidParameters);
            }
            form.append_pair("client_secret", secret);
        }
        Ok(Self(Zeroizing::new(form.finish())))
    }

    /// Prepare a standard PKCE authorization-code exchange with caller-owned ID.
    pub fn exchange(
        client_id: &str,
        redirect_uri: &str,
        client_secret: Option<&str>,
        code: AuthorizationCode,
        verifier: PkceVerifier,
    ) -> Result<Self, AuthError> {
        let redirect = url::Url::parse(redirect_uri).map_err(|_| AuthError::InvalidRedirect)?;
        if client_id.is_empty()
            || redirect.scheme() != "http"
            || redirect.host_str() != Some("127.0.0.1")
            || redirect.port().is_none()
            || redirect.path() != "/oauth/callback"
            || !redirect.username().is_empty()
            || redirect.password().is_some()
            || redirect.query().is_some()
            || redirect.fragment().is_some()
        {
            return Err(AuthError::InvalidRedirect);
        }
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("grant_type", "authorization_code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("code", code.expose_secret())
            .append_pair("code_verifier", verifier.expose_secret());
        if let Some(secret) = client_secret {
            if secret.is_empty() {
                return Err(AuthError::InvalidParameters);
            }
            form.append_pair("client_secret", secret);
        }
        Ok(Self(Zeroizing::new(form.finish())))
    }

    /// Expose only to a TLS HTTP request body; do not log it.
    pub fn expose_form(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for TokenRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TokenRequest([REDACTED])")
    }
}

/// Validated bearer grant; it does not by itself prove certificate API access.
pub struct TokenGrant {
    access_token: Zeroizing<String>,
    refresh_token: Option<Zeroizing<String>>,
    expires_in: u64,
    sharing_permission: bool,
}

impl TokenGrant {
    /// Parse a bounded token response and require an explicitly granted Nearby scope.
    /// The original response is never included in an error or Debug output.
    pub fn from_response(response: &str) -> Result<Self, AuthError> {
        Self::parse(response, None)
    }

    /// Parse renewal of a previously validated sharing grant. OAuth refresh may
    /// omit unchanged scope/refresh token; a supplied conflicting scope fails.
    pub fn from_refresh_response(
        response: &str,
        previous_refresh: &str,
    ) -> Result<Self, AuthError> {
        if !valid_token(previous_refresh) {
            return Err(AuthError::InvalidGrant);
        }
        Self::parse(response, Some(previous_refresh))
    }

    fn parse(response: &str, previous_refresh: Option<&str>) -> Result<Self, AuthError> {
        if response.len() > 64 * 1024 {
            return Err(AuthError::InvalidGrant);
        }
        let mut json: serde_json::Value =
            serde_json::from_str(response).map_err(|_| AuthError::InvalidGrant)?;
        if json.get("error").is_some() || json["token_type"].as_str() != Some("Bearer") {
            return Err(AuthError::InvalidGrant);
        }
        let scope = match &json["scope"] {
            serde_json::Value::Null if previous_refresh.is_some() => NEARBY_SCOPE,
            serde_json::Value::String(scope) => scope.as_str(),
            _ => return Err(AuthError::InvalidGrant),
        };
        let sharing_permission = scope.split_whitespace().any(|s| s == NEARBY_SCOPE);
        if !scope
            .split_whitespace()
            .any(|s| matches!(s, NEARBY_SCOPE | NEARBY_SDK_SCOPE))
        {
            return Err(AuthError::InvalidGrant);
        }
        let expires_in = json["expires_in"]
            .as_u64()
            .filter(|&seconds| seconds > 0 && seconds <= 86_400)
            .ok_or(AuthError::InvalidGrant)?;
        let serde_json::Value::String(access_token) = json["access_token"].take() else {
            return Err(AuthError::InvalidGrant);
        };
        if access_token.is_empty()
            || access_token.len() > 16 * 1024
            || access_token.chars().any(char::is_control)
        {
            return Err(AuthError::InvalidGrant);
        }
        let refresh_token = match json["refresh_token"].take() {
            serde_json::Value::Null => {
                previous_refresh.map(|token| Zeroizing::new(token.to_owned()))
            }
            serde_json::Value::String(token)
                if !token.is_empty()
                    && token.len() <= 16 * 1024
                    && !token.chars().any(char::is_control) =>
            {
                Some(Zeroizing::new(token))
            }
            _ => return Err(AuthError::InvalidGrant),
        };
        Ok(Self {
            access_token: Zeroizing::new(access_token),
            refresh_token,
            expires_in,
            sharing_permission,
        })
    }

    /// Expose only for an authenticated API request. Keep out of UI state and logs.
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
    /// Expose only to trusted OS storage or refresh/revocation code.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_ref().map(|token| token.as_str())
    }
    /// Lifetime reported by the provider; the service must enforce expiry.
    pub fn expires_in(&self) -> u64 {
        self.expires_in
    }

    /// Whether the exact sharing permission was granted (SDK-only is distinct).
    pub fn has_sharing_permission(&self) -> bool {
        self.sharing_permission
    }
}

fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 16 * 1024 && !token.chars().any(char::is_control)
}

impl fmt::Debug for TokenGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenGrant")
            .field("credentials", &"[REDACTED]")
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn renewal_preserves_omitted_values_but_rejects_explicit_scope_loss() {
        let reply = r#"{"access_token":"renewed","token_type":"Bearer","expires_in":3600}"#;
        let grant = TokenGrant::from_refresh_response(reply, "synthetic-refresh").unwrap();
        assert!(grant.has_sharing_permission());
        assert_eq!(grant.refresh_token(), Some("synthetic-refresh"));
        assert!(TokenGrant::from_response(reply).is_err());
        assert!(
            TokenGrant::from_refresh_response(
                r#"{"access_token":"x","token_type":"Bearer","expires_in":3600,"scope":"openid"}"#,
                "synthetic-refresh"
            )
            .is_err()
        );
        let request = TokenRequest::refresh(
            "synthetic-client",
            Some("synthetic-secret"),
            "synthetic&refresh",
        )
        .unwrap();
        let form: std::collections::BTreeMap<_, _> =
            url::form_urlencoded::parse(request.expose_form().as_bytes())
                .into_owned()
                .collect();
        assert_eq!(form["grant_type"], "refresh_token");
        assert_eq!(form["refresh_token"], "synthetic&refresh");
        assert!(!format!("{request:?}").contains("synthetic"));
    }
    #[test]
    fn token_form_encodes_secrets_without_parameter_injection() {
        let attempt = crate::AuthorizationAttempt::new(43119).unwrap();
        let redirect = attempt.redirect_uri().to_owned();
        let consent = attempt.authorization_url("owned-client").unwrap();
        let state = consent
            .query_pairs()
            .find(|(key, _)| key == "state")
            .unwrap()
            .1
            .into_owned();
        let (code, verifier) = attempt
            .accept(&format!("{redirect}?state={state}&code=a%2Bb%26c"))
            .unwrap();
        let request = TokenRequest::exchange(
            "owned-client",
            &redirect,
            Some("private&secret"),
            code,
            verifier,
        )
        .unwrap();
        let form: std::collections::BTreeMap<_, _> =
            url::form_urlencoded::parse(request.expose_form().as_bytes())
                .into_owned()
                .collect();
        assert_eq!(form["code"], "a+b&c");
        assert_eq!(form["client_secret"], "private&secret");
        assert_eq!(form.len(), 6);
        assert!(!format!("{request:?}").contains("private&secret"));
    }
    #[test]
    fn refuses_successful_login_without_nearby_permission() {
        let response = r#"{"access_token":"sensitive","token_type":"Bearer","expires_in":3600,"scope":"openid email"}"#;
        assert_eq!(
            TokenGrant::from_response(response).unwrap_err(),
            AuthError::InvalidGrant
        );
    }
    #[test]
    fn accepts_narrow_grant_and_redacts_both_tokens() {
        let response = format!(
            r#"{{"access_token":"private-access","refresh_token":"private-refresh","token_type":"Bearer","expires_in":3600,"scope":"{NEARBY_SCOPE}"}}"#
        );
        let grant = TokenGrant::from_response(&response).unwrap();
        assert_eq!(grant.access_token(), "private-access");
        assert_eq!(grant.refresh_token(), Some("private-refresh"));
        assert!(!format!("{grant:?}").contains("private-"));
    }
    #[test]
    fn rejects_expired_wrong_type_and_oversized_responses() {
        for response in [
            "{}".to_owned(),
            "x".repeat(65 * 1024),
            format!(
                r#"{{"access_token":"a","token_type":"Bearer","expires_in":0,"scope":"{NEARBY_SCOPE}"}}"#
            ),
            format!(
                r#"{{"access_token":"a","token_type":"MAC","expires_in":3600,"scope":"{NEARBY_SCOPE}"}}"#
            ),
        ] {
            assert!(TokenGrant::from_response(&response).is_err());
        }
    }
}
