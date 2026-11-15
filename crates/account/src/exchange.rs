use std::{io::Read, time::Duration};

use nearlane_auth::{TokenGrant, TokenRequest};
use zeroize::Zeroizing;

use crate::{ClientConfig, LoginError};

pub(crate) fn exchange(request: TokenRequest) -> Result<TokenGrant, LoginError> {
    send(request, None)
}

pub(crate) fn refresh(config: &ClientConfig, token: &str) -> Result<TokenGrant, LoginError> {
    let request = TokenRequest::refresh(
        &config.client_id,
        config.client_secret.as_deref().map(String::as_str),
        token,
    )
    .map_err(|_| LoginError::InvalidGrant)?;
    send(request, Some(token))
}

fn send(request: TokenRequest, previous_refresh: Option<&str>) -> Result<TokenGrant, LoginError> {
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| LoginError::Network)?;
    // Deliberately do not log reqwest errors, request bodies or provider replies.
    let response = client
        .post(TokenRequest::ENDPOINT)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(request.expose_form().to_owned())
        .send()
        .map_err(|_| LoginError::Network)?;
    let success = response.status().is_success();
    if response.content_length().is_some_and(|n| n > 64 * 1024) {
        return Err(LoginError::InvalidGrant);
    }
    let mut body = Zeroizing::new(Vec::new());
    response
        .take(64 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(|_| LoginError::Network)?;
    let body = std::str::from_utf8(&body).map_err(|_| LoginError::InvalidGrant)?;
    if !success {
        if previous_refresh.is_some()
            && serde_json::from_str::<serde_json::Value>(body)
                .ok()
                .is_some_and(|json| json["error"].as_str() == Some("invalid_grant"))
        {
            return Err(LoginError::RefreshRejected);
        }
        return Err(LoginError::TokenRejected);
    }
    if let Some(previous) = previous_refresh {
        TokenGrant::from_refresh_response(body, previous).map_err(|_| LoginError::InvalidGrant)
    } else {
        TokenGrant::from_response(body).map_err(|_| LoginError::InvalidGrant)
    }
}
