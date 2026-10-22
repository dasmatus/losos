//! The WOPI access token: who may do what to which file, until when.
//!
//! The token is the only thing coolwsd shows the WOPI host, so it carries
//! everything the host needs and is signed so nobody can make one: the
//! payload as JSON, hex-encoded, then a dot, then HMAC-SHA256 of the hex
//! payload under the box's own key. Hex rather than base64 keeps it to
//! characters no URL, header or log line needs to escape.
//!
//! The key is 32 random bytes in a file the service makes on its first start
//! ([`TokenKey::load_or_create`]). A new key ends every open editor session,
//! which is the right outcome after a restore or a reset.

use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// What one token grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessToken {
    #[serde(rename = "f")]
    pub file_id: String,
    /// The LosOS Login name the store checks every call against.
    #[serde(rename = "u")]
    pub login: String,
    /// The name Collabora shows beside this person's cursor.
    #[serde(rename = "n")]
    pub display_name: String,
    #[serde(rename = "w")]
    pub writable: bool,
    /// The editor page's origin, for Collabora's `PostMessageOrigin`.
    #[serde(rename = "o")]
    pub origin: String,
    /// Milliseconds since 1970.
    #[serde(rename = "e")]
    pub expires_ms: u64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("the access token is malformed")]
    Malformed,
    #[error("the access token's signature does not match")]
    BadSignature,
    #[error("the access token has expired")]
    Expired,
}

/// The box's signing key for access tokens.
pub struct TokenKey(hmac::Key);

impl std::fmt::Debug for TokenKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenKey(..)")
    }
}

const KEY_LEN: usize = 32;

impl TokenKey {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(hmac::Key::new(hmac::HMAC_SHA256, bytes))
    }

    /// Read the key at `path`, or make one there (mode 0600) if there is none.
    pub fn load_or_create(path: &Path) -> std::io::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) if bytes.len() == KEY_LEN => return Ok(Self::from_bytes(&bytes)),
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("{} is not a {KEY_LEN}-byte key", path.display()),
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut bytes = [0u8; KEY_LEN];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| std::io::Error::other("no randomness for the token key"))?;
        let tmp = path.with_extension("tmp");
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(Self::from_bytes(&bytes))
    }

    pub fn sign(&self, token: &AccessToken) -> String {
        let payload = hex(&serde_json::to_vec(token).expect("a token serialises"));
        let tag = hmac::sign(&self.0, payload.as_bytes());
        format!("{payload}.{}", hex(tag.as_ref()))
    }

    /// The token's grant, if this key signed it and it has not run out at
    /// `now_ms`.
    pub fn verify(&self, text: &str, now_ms: u64) -> Result<AccessToken, TokenError> {
        let (payload, tag) = text.split_once('.').ok_or(TokenError::Malformed)?;
        let tag = unhex(tag).ok_or(TokenError::Malformed)?;
        hmac::verify(&self.0, payload.as_bytes(), &tag).map_err(|_| TokenError::BadSignature)?;
        let json = unhex(payload).ok_or(TokenError::Malformed)?;
        let token: AccessToken =
            serde_json::from_slice(&json).map_err(|_| TokenError::Malformed)?;
        if now_ms >= token.expires_ms {
            return Err(TokenError::Expired);
        }
        Ok(token)
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 15) as usize] as char);
    }
    out
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    text.as_bytes()
        .chunks(2)
        .map(|p| Some(digit(p[0])? << 4 | digit(p[1])?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> AccessToken {
        AccessToken {
            file_id: "42".into(),
            login: "matus".into(),
            display_name: "Matus".into(),
            writable: true,
            origin: "https://box.local".into(),
            expires_ms: 5_000,
        }
    }

    #[test]
    fn a_signed_token_verifies_until_it_expires() {
        let key = TokenKey::from_bytes(&[1; 32]);
        let text = key.sign(&grant());
        assert!(text.bytes().all(|c| c.is_ascii_hexdigit() || c == b'.'));
        assert_eq!(key.verify(&text, 4_999).unwrap(), grant());
        assert_eq!(key.verify(&text, 5_000), Err(TokenError::Expired));
    }

    #[test]
    fn another_key_or_a_changed_payload_is_refused() {
        let key = TokenKey::from_bytes(&[1; 32]);
        let text = key.sign(&grant());
        let other = TokenKey::from_bytes(&[2; 32]);
        assert_eq!(other.verify(&text, 0), Err(TokenError::BadSignature));

        // Make the token read-write for a different file: same signature.
        let mut forged = grant();
        forged.file_id = "43".into();
        let forged_payload = key.sign(&forged);
        let (payload, _) = forged_payload.split_once('.').unwrap();
        let (_, tag) = text.split_once('.').unwrap();
        assert_eq!(
            key.verify(&format!("{payload}.{tag}"), 0),
            Err(TokenError::BadSignature)
        );
    }

    #[test]
    fn garbage_is_malformed() {
        let key = TokenKey::from_bytes(&[1; 32]);
        for text in ["", "abc", "zz.zz", "0.0", "ab.c"] {
            assert!(key.verify(text, 0).is_err(), "{text}");
        }
    }

    #[test]
    fn the_key_file_is_made_once_and_kept() {
        let dir = assert_fs::TempDir::new().unwrap();
        let path = dir.path().join("office-token.key");
        let first = TokenKey::load_or_create(&path).unwrap();
        let text = first.sign(&grant());
        let again = TokenKey::load_or_create(&path).unwrap();
        assert_eq!(again.verify(&text, 0).unwrap(), grant());
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_key_file_of_the_wrong_length_is_an_error_not_a_new_key() {
        let dir = assert_fs::TempDir::new().unwrap();
        let path = dir.path().join("office-token.key");
        std::fs::write(&path, b"short").unwrap();
        assert!(TokenKey::load_or_create(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"short");
    }
}
