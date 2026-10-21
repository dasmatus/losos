//! `POST /api/devices/cloud` and `POST /api/devices/git` — credentials for
//! one computer of the owner's.
//!
//! The terminal client sets up a computer to use LosOS Cloud and LosOS Git
//! without the owner copying anything: it asks the box for a credential of
//! that computer's own, keeps it in the computer's keychain and points the
//! file manager and git at it. The owner's password never leaves the call,
//! and what is stored on the computer can be revoked on its own.
//!
//! Neither credential can be made from lososd's own keys. Nextcloud mints an
//! app password only for a session that signed in with the real password
//! (`GET /ocs/v2.php/core/getapppassword`), and Forgejo creates an access
//! token only under Basic auth with the account's password: its API refuses
//! the bot administrator's token, `Sudo` and a token as a Basic password
//! alike ("auth method not allowed"). So both routes take the owner's
//! password, as the sign-in does, and send it the way the sign-in does: as
//! a header in request bytes built here, written to a loopback socket by the
//! backend, never in an argv (see the header of [`crate::setup`]).
//!
//! The builders and readers are pure so the tests can check every byte.
//! Requests are HTTP/1.0: both servers then answer with a plain body ended
//! by the closed connection, and there is no chunked encoding to undo.

use crate::setup::Secret;

/// Longest computer name, in characters. It ends up in a token name, which
/// Forgejo caps at 255, and in the owner's list of app passwords.
pub const MAX_DEVICE_CHARS: usize = 48;

/// The scopes of a LosOS Git token: clone and push the owner's repositories,
/// and read who the token belongs to (git's credential check). Nothing to
/// administer, nothing about other users.
pub const GIT_SCOPES: [&str; 2] = ["write:repository", "read:user"];

/// Where Nextcloud's WebDAV tree for `user` starts, below the box's address.
pub fn dav_path(user: &str) -> String {
    format!("/nextcloud/remote.php/dav/files/{user}/")
}

/// Where LosOS Git starts, below the box's address.
pub const GIT_PATH: &str = "/forgejo/";

/// The name the client gave its computer, checked. Letters, digits, spaces,
/// dots, dashes and underscores: it is shown in two lists the owner reads
/// and goes into a URL path when a stale token is removed.
pub fn validate_device(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("device must name this computer".to_string());
    }
    if name.chars().count() > MAX_DEVICE_CHARS {
        return Err(format!(
            "device must be at most {MAX_DEVICE_CHARS} characters"
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_'))
    {
        return Err(
            "device may hold letters, digits, spaces, dots, dashes and underscores".to_string(),
        );
    }
    Ok(name.to_string())
}

/// What both credentials are called, so the owner recognises them in LosOS
/// cloud's security settings and in LosOS Git's applications.
pub fn credential_name(device: &str) -> String {
    format!("LosOS on {device}")
}

/// The app password route beside the sign-in probe's: same Apache, same
/// subpath, so it is derived from `$LOSOS_NEXTCLOUD_LOGIN_URL` rather than
/// configured a second time.
pub fn app_password_path(login_path: &str) -> String {
    let base = login_path
        .split_once("/ocs/")
        .map(|(base, _)| base)
        .unwrap_or("");
    format!("{base}/ocs/v2.php/core/getapppassword")
}

/// `GET …/core/getapppassword` as `user` with the real password. Nextcloud
/// names the new app password after the `User-Agent`, so that header carries
/// [`credential_name`]. `client` is forwarded as in
/// [`crate::signin::login_request`], so Nextcloud's throttle counts the
/// caller and not lososd.
pub fn app_password_request(
    path: &str,
    host: &str,
    user: &str,
    secret: &Secret,
    device: &str,
    client: &str,
) -> Vec<u8> {
    let mut extra = vec![
        "OCS-APIRequest: true".to_string(),
        format!("User-Agent: {}", credential_name(device)),
    ];
    if client.parse::<std::net::IpAddr>().is_ok() {
        extra.push(format!("X-Forwarded-For: {client}"));
    }
    basic_request("GET", path, host, user, secret, &extra, None)
}

/// One Forgejo API call as `owner` with the real password.
pub fn forgejo_request(
    method: &str,
    path: &str,
    host: &str,
    owner: &str,
    secret: &Secret,
    body: Option<&str>,
) -> Vec<u8> {
    basic_request(method, path, host, owner, secret, &[], body)
}

/// `DELETE` of the token a previous setup of the same computer left, so the
/// name is free again. Forgejo refuses a second token of one name.
pub fn token_delete_path(owner: &str, name: &str) -> String {
    format!(
        "/api/v1/users/{owner}/tokens/{}",
        percent_encode_segment(name)
    )
}

/// `POST` that creates the token, and its body.
pub fn token_create(owner: &str, name: &str) -> (String, String) {
    (
        format!("/api/v1/users/{owner}/tokens"),
        serde_json::json!({ "name": name, "scopes": GIT_SCOPES }).to_string(),
    )
}

fn basic_request(
    method: &str,
    path: &str,
    host: &str,
    user: &str,
    secret: &Secret,
    extra: &[String],
    body: Option<&str>,
) -> Vec<u8> {
    let credentials = crate::signin::base64(format!("{user}:{}", secret.expose()).as_bytes());
    let mut out = String::new();
    out.push_str(&format!("{method} {path} HTTP/1.0\r\n"));
    out.push_str(&format!("Host: {}\r\n", header_safe(host)));
    out.push_str(&format!("Authorization: Basic {credentials}\r\n"));
    out.push_str("Accept: application/json\r\n");
    for line in extra {
        out.push_str(&header_safe(line));
        out.push_str("\r\n");
    }
    if let Some(body) = body {
        out.push_str("Content-Type: application/json\r\n");
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    out.push_str("Connection: close\r\n\r\n");
    if let Some(body) = body {
        out.push_str(body);
    }
    out.into_bytes()
}

fn header_safe(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
}

/// A path segment with everything but unreserved characters escaped.
fn percent_encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A whole HTTP/1.x answer split into its status and its body.
pub fn split_response(raw: &[u8]) -> Option<(u16, String)> {
    let status = crate::signin::status_code(raw)?;
    let at = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let body = String::from_utf8_lossy(&raw[at + 4..]).into_owned();
    Some((status, body))
}

/// What a mint came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Minted {
    /// The new credential.
    Credential(String),
    /// The password is not the account's.
    Rejected,
    /// The server's brute-force protection is making the caller wait.
    Throttled,
    /// The server answered something that is neither; the sentence says what.
    Unavailable(String),
}

/// Nextcloud's answer to [`app_password_request`].
pub fn interpret_app_password(status: u16, body: &str) -> Minted {
    match status {
        200 => {
            let doc: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
            match doc
                .pointer("/ocs/data/apppassword")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(p) => Minted::Credential(p.to_string()),
                None => {
                    Minted::Unavailable("LosOS Cloud answered without an app password.".to_string())
                }
            }
        }
        401 => Minted::Rejected,
        429 => Minted::Throttled,
        code => Minted::Unavailable(format!(
            "LosOS Cloud answered {code} instead of an app password. It may still be starting."
        )),
    }
}

/// Forgejo's answer to the `POST` of [`token_create`], or to the `DELETE`
/// before it when that was neither 204 nor 404.
pub fn interpret_token(status: u16, body: &str) -> Minted {
    match status {
        201 => {
            let doc: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
            match doc
                .get("sha1")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(t) => Minted::Credential(t.to_string()),
                None => Minted::Unavailable("LosOS Git answered without a token.".to_string()),
            }
        }
        // LosOS Cloud has already judged the password by now, and LosOS Git
        // was just given it, so a refusal here means the two are not in step
        // yet, not that the owner typed it wrong.
        401 | 403 => Minted::Unavailable(
            "LosOS Git did not take the password LosOS Cloud accepted. Try again in a minute."
                .to_string(),
        ),
        429 => Minted::Throttled,
        code => Minted::Unavailable(format!(
            "LosOS Git answered {code} instead of a token. It may still be starting."
        )),
    }
}

/// LosOS Git is off on this box (`losos.forgejo.enable = false`, or the
/// configuration repository points elsewhere), so there is no account to
/// make a token for. 409: the request is fine, the box is not set up for it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("LosOS Git is off on this box")]
pub struct GitOff;

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> Secret {
        Secret::new("hunter2:with colon")
    }

    #[test]
    fn device_names_are_short_and_plain() {
        assert_eq!(validate_device("  laptop ").unwrap(), "laptop");
        assert!(validate_device("Jana's").is_err());
        assert!(validate_device("work-pc_2.lan").is_ok());
        assert!(validate_device("").is_err());
        assert!(validate_device("a\r\nX-Evil: 1").is_err());
        assert!(validate_device("a/b").is_err());
        assert!(validate_device(&"x".repeat(MAX_DEVICE_CHARS + 1)).is_err());
    }

    #[test]
    fn the_app_password_route_sits_beside_the_sign_in_probe() {
        assert_eq!(
            app_password_path("/nextcloud/ocs/v2.php/cloud/user"),
            "/nextcloud/ocs/v2.php/core/getapppassword"
        );
        assert_eq!(
            app_password_path("/ocs/v2.php/cloud/user"),
            "/ocs/v2.php/core/getapppassword"
        );
    }

    #[test]
    fn the_app_password_request_names_the_computer_and_carries_the_password_once() {
        let raw = app_password_request(
            "/nextcloud/ocs/v2.php/core/getapppassword",
            "localhost",
            "notshared",
            &secret(),
            "laptop",
            "192.168.1.20",
        );
        let text = String::from_utf8(raw).unwrap();
        assert!(text.starts_with("GET /nextcloud/ocs/v2.php/core/getapppassword HTTP/1.0\r\n"));
        assert!(text.contains("\r\nUser-Agent: LosOS on laptop\r\n"));
        assert!(text.contains("\r\nOCS-APIRequest: true\r\n"));
        assert!(text.contains("\r\nX-Forwarded-For: 192.168.1.20\r\n"));
        let basic = crate::signin::base64(b"notshared:hunter2:with colon");
        assert!(text.contains(&format!("\r\nAuthorization: Basic {basic}\r\n")));
        assert!(
            !text.contains("hunter2"),
            "the password is only in the header"
        );
        assert!(text.ends_with("\r\n\r\n"));
    }

    #[test]
    fn a_client_that_is_not_an_address_is_not_forwarded() {
        let raw = app_password_request("/x", "h", "u", &secret(), "pc", "1.2.3.4\r\nX: y");
        assert!(!String::from_utf8(raw).unwrap().contains("X-Forwarded-For"));
    }

    #[test]
    fn the_token_requests_free_the_name_then_create_it() {
        let name = credential_name("work pc");
        assert_eq!(
            token_delete_path("notshared", &name),
            "/api/v1/users/notshared/tokens/LosOS%20on%20work%20pc"
        );
        let (path, body) = token_create("notshared", &name);
        assert_eq!(path, "/api/v1/users/notshared/tokens");
        let doc: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(doc["name"], "LosOS on work pc");
        assert_eq!(doc["scopes"], serde_json::json!(GIT_SCOPES));

        let raw = forgejo_request(
            "POST",
            &path,
            "localhost",
            "notshared",
            &secret(),
            Some(&body),
        );
        let text = String::from_utf8(raw).unwrap();
        assert!(text.starts_with("POST /api/v1/users/notshared/tokens HTTP/1.0\r\n"));
        assert!(text.contains(&format!("\r\nContent-Length: {}\r\n", body.len())));
        assert!(text.ends_with(&format!("\r\n\r\n{body}")));
    }

    #[test]
    fn answers_split_into_status_and_body() {
        let raw =
            b"HTTP/1.0 201 Created\r\nContent-Type: application/json\r\n\r\n{\"sha1\":\"abc\"}";
        assert_eq!(
            split_response(raw),
            Some((201, "{\"sha1\":\"abc\"}".to_string()))
        );
        assert_eq!(
            split_response(b"HTTP/1.1 204 No Content\r\n\r\n"),
            Some((204, String::new()))
        );
        assert_eq!(split_response(b"garbage"), None);
    }

    #[test]
    fn nextcloud_answers_mean_what_the_client_needs() {
        let ok = r#"{"ocs":{"meta":{"status":"ok"},"data":{"apppassword":"Abc-123"}}}"#;
        assert_eq!(
            interpret_app_password(200, ok),
            Minted::Credential("Abc-123".into())
        );
        assert_eq!(interpret_app_password(401, ""), Minted::Rejected);
        assert_eq!(interpret_app_password(429, ""), Minted::Throttled);
        assert!(matches!(
            interpret_app_password(200, "{}"),
            Minted::Unavailable(_)
        ));
        assert!(matches!(
            interpret_app_password(503, ""),
            Minted::Unavailable(_)
        ));
    }

    #[test]
    fn forgejo_answers_mean_what_the_client_needs() {
        // The shape Forgejo 16 answers with.
        let ok = r#"{"id":2,"name":"LosOS on laptop","sha1":"e617fd66","token_last_eight":"e617fd66","scopes":["write:repository","read:user"]}"#;
        assert_eq!(
            interpret_token(201, ok),
            Minted::Credential("e617fd66".into())
        );
        assert!(matches!(interpret_token(401, ""), Minted::Unavailable(_)));
        assert!(matches!(
            interpret_token(
                400,
                r#"{"message":"access token name has been used already"}"#
            ),
            Minted::Unavailable(_)
        ));
    }
}
