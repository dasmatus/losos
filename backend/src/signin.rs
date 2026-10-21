//! Checking a password someone typed, and the pieces lososd needs to ask
//! a loopback HTTP service with one.
//!
//! Signing in to the admin pages is LosOS Login's job (`modules/identity.nix`,
//! `crate::people`). What stays here is the shape check for a password
//! being tried, before lososd checks it against the people file for one of
//! the owner's computers (`crate::devices`), and the small HTTP helpers the
//! LosOS Git requests share. Same rule as [`crate::setup`]: **the password
//! must never appear in an argv**, so requests are built as bytes and
//! written to a socket, never handed to `curl -u`.

use crate::setup::Secret;

/// Longest password a sign-in attempt may carry, in bytes. Matches
/// [`crate::setup::MAX_PASSWORD_BYTES`]: nothing longer was ever set.
pub const MAX_CANDIDATE_BYTES: usize = crate::setup::MAX_PASSWORD_BYTES;

/// The password offered to the dialog was not one this box could have set.
///
/// Deliberately **not** [`crate::setup::validate_password`]: that is the rule
/// for a *new* password, and it grew stricter over time. An owner who set a
/// password under an older build must still be able to sign in with it, so
/// the only checks here are the ones that would make the request itself
/// malformed — a control character would end a header line early — plus the
/// length cap nothing was ever set beyond.
pub fn validate_candidate(password: &str) -> Result<Secret, String> {
    if password.is_empty() {
        return Err("password must not be empty".to_string());
    }
    if password.len() > MAX_CANDIDATE_BYTES {
        return Err(format!(
            "password must be at most {MAX_CANDIDATE_BYTES} bytes"
        ));
    }
    if password.chars().any(char::is_control) {
        return Err("password must not contain control characters".to_string());
    }
    Ok(Secret::new(password))
}

/// A loopback service's URL split into the socket address and the request
/// path. Only `http://host:port/path` is accepted: the service is reached
/// over the host's own loopback, and a `https://` here would be a
/// configuration error worth failing loudly on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginEndpoint {
    pub addr: String,
    pub path: String,
}

pub fn parse_login_url(url: &str) -> Result<LoginEndpoint, String> {
    let Some(rest) = url.strip_prefix("http://") else {
        return Err(format!(
            "a loopback URL must start with http:// (got {url:?})"
        ));
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() || !authority.contains(':') {
        return Err(format!("a loopback URL must name host:port (got {url:?})"));
    }
    Ok(LoginEndpoint {
        addr: authority.to_string(),
        path: path.to_string(),
    })
}

/// Read the status code off the first line of an HTTP/1.x response, if it
/// has one.
pub fn status_code(response: &[u8]) -> Option<u16> {
    let line = response.split(|b| *b == b'\n').next()?;
    let line = std::str::from_utf8(line).ok()?.trim_end_matches('\r');
    let mut parts = line.splitn(3, ' ');
    let version = parts.next()?;
    if !version.starts_with("HTTP/1.") {
        return None;
    }
    parts.next()?.parse().ok()
}

/// Standard base64 with padding, as RFC 7617 wants it. Hand-rolled rather
/// than a crate: it is twelve lines, and a new dependency changes the
/// `cargoHash` in nix/packages.nix for every builder of this appliance.
pub fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The password was checked and is wrong. Typed so the HTTP layer answers
/// 401 — the status the page treats as "try again" — and so the daemon's
/// own per-address throttle counts it like a bad token.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("that password was not accepted")]
pub struct WrongPassword;

/// Too many wrong passwords from this address lately; lososd is not
/// checking another one yet. 429, so the page can say "wait" rather than
/// "wrong".
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("too many attempts; this address has to wait a while")]
pub struct Throttled;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"notshared:hunter2"), "bm90c2hhcmVkOmh1bnRlcjI=");
    }

    #[test]
    fn the_login_url_splits_into_address_and_path() {
        let e = parse_login_url("http://127.0.0.1:3000/forgejo/api/v1").unwrap();
        assert_eq!(e.addr, "127.0.0.1:3000");
        assert_eq!(e.path, "/forgejo/api/v1");
        assert_eq!(parse_login_url("http://127.0.0.1:80").unwrap().path, "/");
        assert!(parse_login_url("https://127.0.0.1:443/x").is_err());
        assert!(parse_login_url("http://127.0.0.1/x").is_err());
    }

    #[test]
    fn status_codes_are_read_off_the_first_line() {
        assert_eq!(status_code(b"HTTP/1.1 200 OK\r\nX: y\r\n\r\n{}"), Some(200));
        assert_eq!(status_code(b"HTTP/1.0 401 Unauthorized\r\n"), Some(401));
        assert_eq!(
            status_code(b"HTTP/1.1 429 Too Many Requests\r\n"),
            Some(429)
        );
        assert_eq!(status_code(b"<html>not http"), None);
        assert_eq!(status_code(b""), None);
    }

    #[test]
    fn a_candidate_is_checked_for_shape_only() {
        // Shorter than a new password may be, and without a digit: still a
        // password an older build could have set, so still askable.
        assert!(validate_candidate("short").is_ok());
        assert!(validate_candidate("").is_err());
        assert!(validate_candidate("line\nbreak").is_err());
        assert!(validate_candidate(&"x".repeat(MAX_CANDIDATE_BYTES + 1)).is_err());
    }
}
