use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use nearlane_auth::{AuthError, AuthorizationAttempt, TokenGrant, TokenRequest};
use zeroize::Zeroizing;

use crate::{ClientConfig, LoginError};

pub(crate) fn run(
    listener: TcpListener,
    attempt: AuthorizationAttempt,
    config: ClientConfig,
    cancelled: Arc<AtomicBool>,
    exchanging: Arc<AtomicBool>,
    timeout: Duration,
    exchange: impl FnOnce(TokenRequest) -> Result<TokenGrant, LoginError>,
) -> Result<TokenGrant, LoginError> {
    let authority = listener
        .local_addr()
        .map_err(|_| LoginError::Listener)?
        .to_string();
    let deadline = Instant::now() + timeout;
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(LoginError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(LoginError::TimedOut);
        }
        let (mut stream, _) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
                continue;
            }
            Err(_) => return Err(LoginError::Listener),
        };
        let Some(target) = read_target(&mut stream, &authority, &cancelled, deadline) else {
            respond(&mut stream, false);
            continue;
        };
        let callback = Zeroizing::new(format!("http://{authority}{}", target.as_str()));
        match attempt.validate_callback(&callback) {
            Ok(()) => {}
            Err(AuthError::ProviderDenied) => {
                respond(&mut stream, false);
                return Err(LoginError::Denied);
            }
            Err(_) => {
                respond(&mut stream, false);
                continue;
            }
        }
        if cancelled.load(Ordering::Acquire) {
            return Err(LoginError::Cancelled);
        }
        let redirect = attempt.redirect_uri().to_owned();
        let (code, verifier) = attempt
            .accept(&callback)
            .map_err(|_| LoginError::Callback)?;
        let request = TokenRequest::exchange(
            &config.client_id,
            &redirect,
            config.client_secret.as_deref().map(|s| s.as_str()),
            code,
            verifier,
        )
        .map_err(|_| LoginError::Configuration)?;
        respond(&mut stream, true);
        // Stop accepting requests once the one-use code has been validated.
        drop(stream);
        drop(listener);
        exchanging.store(true, Ordering::Release);
        if cancelled.load(Ordering::Acquire) {
            return Err(LoginError::Cancelled);
        }
        let result = exchange(request);
        return if cancelled.load(Ordering::Acquire) {
            Err(LoginError::Cancelled)
        } else {
            result
        };
    }
}

fn read_target(
    stream: &mut TcpStream,
    authority: &str,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Option<Zeroizing<String>> {
    stream
        .set_read_timeout(Some(Duration::from_millis(150)))
        .ok()?;
    let request_deadline = std::cmp::min(deadline, Instant::now() + Duration::from_secs(2));
    let mut request = Zeroizing::new(Vec::new());
    let mut buffer = Zeroizing::new([0_u8; 1024]);
    loop {
        if cancelled.load(Ordering::Acquire) || Instant::now() >= request_deadline {
            return None;
        }
        let n = match stream.read(buffer.as_mut()) {
            Ok(0) => return None,
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                continue;
            }
            Err(_) => return None,
        };
        if request.len() + n > 16 * 1024 {
            return None;
        }
        request.extend_from_slice(&buffer[..n]);
        if request.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    parse_request(&request, authority)
}

fn parse_request(request: &[u8], authority: &str) -> Option<Zeroizing<String>> {
    if request.len() > 16 * 1024 {
        return None;
    }
    let text = std::str::from_utf8(request).ok()?;
    let (headers, body) = text.split_once("\r\n\r\n")?;
    if !body.is_empty() {
        return None;
    }
    let mut lines = headers.split("\r\n");
    let mut first = lines.next()?.split(' ');
    if first.next()? != "GET" {
        return None;
    }
    let target = first.next()?;
    if first.next()? != "HTTP/1.1"
        || first.next().is_some()
        || !target.starts_with("/oauth/callback?")
        || target.chars().any(char::is_control)
    {
        return None;
    }
    let mut host = None;
    for line in lines {
        let (key, value) = line.split_once(':')?;
        if key.eq_ignore_ascii_case("host") {
            if host.replace(value.trim()).is_some() {
                return None;
            }
        } else if key.eq_ignore_ascii_case("origin")
            || key.eq_ignore_ascii_case("transfer-encoding")
            || (key.eq_ignore_ascii_case("content-length") && value.trim() != "0")
        {
            return None;
        }
    }
    if host != Some(authority) {
        return None;
    }
    Some(Zeroizing::new(target.to_owned()))
}

fn respond(stream: &mut TcpStream, accepted: bool) {
    let _ = stream.set_write_timeout(Some(Duration::from_millis(150)));
    let (status, body) = if accepted {
        (
            "200 OK",
            "<!doctype html><title>Nearlane</title><p>Return to Nearlane to check the sign-in result.</p>",
        )
    } else {
        (
            "400 Bad Request",
            "<!doctype html><title>Nearlane</title><p>This sign-in response was not accepted. Return to Nearlane.</p>",
        )
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_foreign_hosts_scripts_duplicate_hosts_and_bodies() {
        for request in [
            "GET /oauth/callback?code=x HTTP/1.1\r\nHost: evil.test\r\n\r\n",
            "GET /oauth/callback?code=x HTTP/1.1\r\nHost: 127.0.0.1:1\r\nOrigin: https://evil.test\r\n\r\n",
            "GET /oauth/callback?code=x HTTP/1.1\r\nHost: 127.0.0.1:1\r\nHost: 127.0.0.1:1\r\n\r\n",
            "GET /oauth/callback?code=x HTTP/1.1\r\nHost: 127.0.0.1:1\r\n\r\nx",
            "POST /oauth/callback?code=x HTTP/1.1\r\nHost: 127.0.0.1:1\r\n\r\n",
        ] {
            assert!(parse_request(request.as_bytes(), "127.0.0.1:1").is_none());
        }
        assert!(
            parse_request(
                b"GET /oauth/callback?code=x HTTP/1.1\r\nHost: 127.0.0.1:1\r\n\r\n",
                "127.0.0.1:1"
            )
            .is_some()
        );
    }
}
