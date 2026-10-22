//! `POST /api/devices/cloud` and `POST /api/devices/git` — credentials for
//! one computer of the owner's.
//!
//! The terminal client sets up a computer to use LosOS Cloud and LosOS Git
//! without the owner copying anything: it asks the box for a credential of
//! that computer's own, keeps it in the computer's keychain and points the
//! file manager and git at it. The owner's password never leaves the call,
//! and what is stored on the computer can be revoked on its own.
//!
//! Both routes take the owner's password and check it against LosOS Login's
//! people file first. LosOS Cloud then makes the app password for the owner
//! on its socket (`crate::pins::CloudRequest::MintKey`). LosOS Git holds no
//! passwords: lososd asks it, as its administrator, for a token
//! (`crate::config_repo::GitOp::CreateToken`).
//!
//! The readers are pure so the tests can check every byte.

/// Longest computer name, in characters. It ends up in a token name, which
/// LosOS Git caps at 100, and in the owner's list of app passwords.
pub const MAX_DEVICE_CHARS: usize = 48;

/// The scopes of a LosOS Git token: clone and push the owner's repositories,
/// and read who the token belongs to (git's credential check). Nothing to
/// administer, nothing about other users.
pub const GIT_SCOPES: [&str; 2] = ["write:repository", "read:user"];

/// Where LosOS Cloud's WebDAV tree for `user` starts, below the box's
/// address (`modules/cloud.nix`).
pub fn dav_path(user: &str) -> String {
    format!("/remote.php/dav/files/{user}/")
}

/// Where LosOS Git starts, below the box's address.
pub const GIT_PATH: &str = "/git/";

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
/// Cloud's Devices and in LosOS Git's applications.
pub fn credential_name(device: &str) -> String {
    format!("LosOS on {device}")
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

/// LosOS Cloud's answer to `getapppassword`.
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

/// LosOS Git's answer to [`crate::config_repo::GitOp::CreateToken`].
pub fn interpret_token(status: u16, body: &str) -> Minted {
    match status {
        201 => {
            let doc: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
            match doc
                .get("token")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(t) => Minted::Credential(t.to_string()),
                None => Minted::Unavailable("LosOS Git answered without a token.".to_string()),
            }
        }
        code => Minted::Unavailable(format!(
            "LosOS Git answered {code} instead of a token. It may still be starting."
        )),
    }
}

/// LosOS Git is off on this box (`losos.git.enable = false`, or the
/// configuration repository points elsewhere), so there is no account to
/// make a token for. 409: the request is fine, the box is not set up for it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("LosOS Git is off on this box")]
pub struct GitOff;

#[cfg(test)]
mod tests {
    use super::*;

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
    fn cloud_answers_mean_what_the_client_needs() {
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
    fn git_answers_mean_what_the_client_needs() {
        let ok = r#"{"id":2,"name":"LosOS on laptop","token":"e617fd66","last_eight":"e617fd66","scopes":["read:user","write:repository"]}"#;
        assert_eq!(
            interpret_token(201, ok),
            Minted::Credential("e617fd66".into())
        );
        assert!(matches!(interpret_token(201, "{}"), Minted::Unavailable(_)));
        assert!(matches!(interpret_token(403, ""), Minted::Unavailable(_)));
    }
}
