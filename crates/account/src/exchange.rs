use std::{io::Read, time::Duration};

use nearlane_auth::{TokenGrant, TokenRequest};
use zeroize::Zeroizing;

use crate::LoginError;

pub(crate) fn exchange(request: TokenRequest) -> Result<TokenGrant, LoginError> {
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
    if !response.status().is_success() {
        return Err(LoginError::TokenRejected);
    }
    if response.content_length().is_some_and(|n| n > 64 * 1024) {
        return Err(LoginError::InvalidGrant);
    }
    let mut body = Zeroizing::new(Vec::new());
    response
        .take(64 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(|_| LoginError::Network)?;
    let body = std::str::from_utf8(&body).map_err(|_| LoginError::InvalidGrant)?;
    TokenGrant::from_response(body).map_err(|_| LoginError::InvalidGrant)
}
