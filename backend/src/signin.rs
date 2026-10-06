//! `POST /api/sign-in` — unlock the admin pages with the owner's password.
//!
//! Until this existed the admin pages and LosOS cloud had two credentials:
//! the password the owner chose in the wizard, and a 64-character admin key
//! lososd minted for itself and released once, in the claim reply. The key
//! was the only thing that ever opened the admin pages again, and the owner
//! had to copy it somewhere at the one moment it was on screen. One password
//! is what an owner can remember; two is what gets written on the box.
//!
//! So the unlock dialog now takes the password, and lososd asks **Nextcloud**
//! whether it is right. Nextcloud is the only thing on the box that knows the
//! password, and the alternative — lososd keeping a second hash of it — would
//! fork the credential: a password changed inside LosOS cloud's own settings
//! page would stop matching the admin pages, silently, with the old one still
//! opening them. Asking Nextcloud means there is exactly one password and it
//! lives in exactly one place. What that costs is availability: while
//! Nextcloud is not running the admin pages cannot be unlocked by password,
//! which is why the admin key stays, as the spare on the printed sheet, and
//! why the dialog keeps a way to enter it.
//!
//! The question is one HTTP request over loopback to the OCS route that does
//! nothing but authenticate (`/ocs/v2.php/cloud/user`), with the password in
//! a Basic header. Same rule as [`crate::setup`]: **the password must never
//! appear in an argv**, so this is not a `curl -u`. It is a request built as
//! bytes by [`login_request`] and written to a socket by the backend; the
//! builder is pure so a test can read every byte it produces. The real
//! client's address rides along as `X-Forwarded-For`, which Nextcloud honours
//! from `127.0.0.1` (`trusted_proxies`, modules/workloads.nix), so its own
//! brute-force throttle counts the browser, not lososd.
//!
//! What a correct password buys is the same admin token the claim released:
//! the page stores it for the tab exactly as before, every other route stays
//! Bearer-gated, and nothing about the token's lifetime or storage changed.

use crate::setup::Secret;

/// Longest password a sign-in attempt may carry, in bytes. Matches
/// [`crate::setup::MAX_PASSWORD_BYTES`]: nothing longer was ever set.
pub const MAX_CANDIDATE_BYTES: usize = crate::setup::MAX_PASSWORD_BYTES;

/// Default of `$LOSOS_NEXTCLOUD_LOGIN_URL`: the workload pod's Apache on the
/// host's loopback, at the subpath the front vhost preserves
/// (modules/containers.nix, modules/workloads.nix, flake/images.nix). The
/// port is the default of `losos.nextcloud.apachePort`; modules/daemon.nix
/// sets the variable from the option so the two cannot drift.
pub const DEFAULT_LOGIN_URL: &str = "http://127.0.0.1:8080/nextcloud/ocs/v2.php/cloud/user";

/// Default of `$LOSOS_NEXTCLOUD_LOGIN_HOST`, the `Host` header on that
/// request. `localhost` is trusted by Nextcloud unconditionally
/// (`TrustedDomainHelper`, `REGEX_LOCALHOST`), whatever `trusted_domains`
/// says, so the probe cannot be answered "Untrusted domain".
pub const DEFAULT_LOGIN_HOST: &str = "localhost";

/// What Nextcloud said about the password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginOutcome {
    /// 200: the password is the account's.
    Accepted,
    /// 401: it is not.
    Rejected,
    /// 429: Nextcloud's own brute-force protection has closed the door for
    /// this client for a while. Not a wrong password, and not retried.
    Throttled,
    /// Anything else, or no answer at all: Nextcloud could not be asked.
    /// The sentence is for the owner, who can act on it by waiting or by
    /// using the spare key.
    Unavailable(String),
}

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

/// Where the probe goes: `$LOSOS_NEXTCLOUD_LOGIN_URL` split into the socket
/// address and the request path. Only `http://host:port/path` is accepted —
/// the pod is reached over the host's own loopback, and a `https://` here
/// would be a configuration error worth failing loudly on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginEndpoint {
    pub addr: String,
    pub path: String,
}

pub fn parse_login_url(url: &str) -> Result<LoginEndpoint, String> {
    let Some(rest) = url.strip_prefix("http://") else {
        return Err(format!(
            "LOSOS_NEXTCLOUD_LOGIN_URL must start with http:// (got {url:?})"
        ));
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() || !authority.contains(':') {
        return Err(format!(
            "LOSOS_NEXTCLOUD_LOGIN_URL must name host:port (got {url:?})"
        ));
    }
    Ok(LoginEndpoint {
        addr: authority.to_string(),
        path: path.to_string(),
    })
}

/// The request, as the bytes that go on the wire.
///
/// Pure, so the test suite can check what every call site relies on: the
/// password is in the Basic header and nowhere else, `Connection: close` is
/// set (the reader stops at EOF), and nothing the caller supplied can start
/// a new header line. `client` is the browser's address for
/// `X-Forwarded-For`; anything that does not parse as an IP is dropped
/// rather than forwarded, since the header would otherwise be writable by
/// whatever set `X-Real-IP`.
///
/// `user` must have passed [`crate::setup::validate_user`], which admits no
/// `:` and no control character, so the `user:password` pair splits where
/// Basic auth expects it to.
pub fn login_request(
    endpoint: &LoginEndpoint,
    host: &str,
    user: &str,
    secret: &Secret,
    client: &str,
) -> Vec<u8> {
    let credentials = base64(format!("{user}:{}", secret.expose()).as_bytes());
    let mut out = String::new();
    out.push_str(&format!("GET {} HTTP/1.1\r\n", endpoint.path));
    out.push_str(&format!("Host: {}\r\n", header_safe(host)));
    out.push_str(&format!("Authorization: Basic {credentials}\r\n"));
    // Without this the OCS routes answer 401 with a WWW-Authenticate that is
    // meant for a browser, before they look at the credentials at all.
    out.push_str("OCS-APIRequest: true\r\n");
    out.push_str("Accept: application/json\r\n");
    if client.parse::<std::net::IpAddr>().is_ok() {
        out.push_str(&format!("X-Forwarded-For: {client}\r\n"));
    }
    out.push_str("Connection: close\r\n\r\n");
    out.into_bytes()
}

/// A header value with anything that could end the line removed.
fn header_safe(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
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

/// What a status means for the sign-in.
pub fn interpret_login(status: Option<u16>) -> LoginOutcome {
    match status {
        Some(200) => LoginOutcome::Accepted,
        Some(401) => LoginOutcome::Rejected,
        Some(429) => LoginOutcome::Throttled,
        // Apache answers 503 while the pod's entrypoint is still installing,
        // and 502/504 come from nothing on this box (there is no proxy on
        // the loopback hop), so every other status reads as "not now".
        Some(code) => LoginOutcome::Unavailable(format!(
            "LosOS cloud answered {code} instead of checking the password. It may still be starting."
        )),
        None => LoginOutcome::Unavailable(
            "LosOS cloud did not answer, so the password could not be checked. It may still be starting."
                .to_string(),
        ),
    }
}

/// Standard base64 with padding, as RFC 7617 wants it. Hand-rolled rather
/// than a crate: it is twelve lines, and a new dependency changes the
/// `cargoHash` in flake/packages.nix for every builder of this appliance.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrongPassword;

impl std::fmt::Display for WrongPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("that password was not accepted")
    }
}

impl std::error::Error for WrongPassword {}

/// Nextcloud's brute-force protection refused to look. 429, so the page can
/// say "wait" rather than "wrong".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Throttled;

impl std::fmt::Display for Throttled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("too many attempts; LosOS cloud is making this address wait")
    }
}

impl std::error::Error for Throttled {}

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
        let e = parse_login_url(DEFAULT_LOGIN_URL).unwrap();
        assert_eq!(e.addr, "127.0.0.1:8080");
        assert_eq!(e.path, "/nextcloud/ocs/v2.php/cloud/user");
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
    fn statuses_mean_what_the_dialog_needs() {
        assert_eq!(interpret_login(Some(200)), LoginOutcome::Accepted);
        assert_eq!(interpret_login(Some(401)), LoginOutcome::Rejected);
        assert_eq!(interpret_login(Some(429)), LoginOutcome::Throttled);
        assert!(matches!(
            interpret_login(Some(503)),
            LoginOutcome::Unavailable(_)
        ));
        assert!(matches!(
            interpret_login(None),
            LoginOutcome::Unavailable(_)
        ));
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
