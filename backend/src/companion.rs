//! The LosOS companion's sign-in: OAuth access tokens from LosOS Login.
//!
//! `losos agents install` signs the companion in with the device code flow
//! (RFC 8628) as the public client [`COMPANION_CLIENT_ID`]: the person
//! approves it in their browser on LosOS Login, with their password, their
//! second factor when they have one, and a consent page. The AI tools then
//! call the box with `Authorization: Bearer authelia_at_…`. nginx lets a
//! Bearer header past its Authelia check (`modules/identity.nix`), so the
//! token reaches lososd, which asks LosOS Login about it here: token
//! introspection (RFC 7662) as the confidential client `lososd`, over
//! loopback. A token counts only when LosOS Login says it is active and was
//! issued to the companion; it then acts as the person who approved it, and
//! only on [`losos_access::COMPANION_ROUTES`], GET only
//! (`crate::http::guarded_at`).
//!
//! LosOS Login checks that a token is introspected at the address it was
//! issued at, so the question carries the address the request came to.
//! Answers are kept for a minute at most, and never past the token's own
//! expiry, so a busy AI tool does not cost LosOS Login a password hash per
//! call; a token revoked at LosOS Login stops working within that minute.
//!
//! LosOS Desktop's backups sign in the same way as a second public client,
//! [`losos_access::BACKUPS_CLIENT_ID`], whose token reaches one route only
//! ([`losos_access::BACKUPS_ROUTE`]): the grant says which client it was
//! issued to, and the HTTP layer holds each to its own routes.

use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use losos_access::{BACKUPS_CLIENT_ID, COMPANION_CLIENT_ID};

/// LosOS Login's own address, under its `/auth` path.
const DEFAULT_LOGIN: &str = "127.0.0.1:9091";
/// The secret lososd introspects with: the credential `oidc-lososd`, which
/// `modules/identity.nix` declares and systemd opens into lososd's own
/// credential directory.
const DEFAULT_SECRET_FILE: &str = "/run/credentials/lososd.service/oidc-lososd";
/// The client id that secret belongs to.
const CLIENT_ID: &str = "lososd";
/// The longest an answer is kept.
const KEEP: Duration = Duration::from_secs(60);
/// The most answers kept at once; past it the cache starts again.
const MAX_KEPT: usize = 256;

/// What LosOS Login said about one token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// The sign-in name of the person who approved the client.
    pub person: String,
    /// Which client LosOS Login issued it to.
    pub client: Client,
}

/// The public clients of LosOS Login whose tokens lososd takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    /// The LosOS companion in someone's AI tools.
    Companion,
    /// LosOS Desktop's backups.
    Backups,
}

pub struct Companion {
    login: String,
    /// Read once at start. A new secret restarts lososd
    /// (`losos-identity-setup`), so it is never stale, and no request
    /// handler reads a file for it.
    secret: Option<String>,
    kept: Mutex<HashMap<[u8; 32], (Grant, Instant)>>,
}

impl Default for Companion {
    fn default() -> Self {
        Self::from_env()
    }
}

impl Companion {
    pub fn from_env() -> Companion {
        let var = |name: &str, default: &str| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default.to_string())
        };
        Companion {
            login: var("LOSOS_LOGIN_ADDR", DEFAULT_LOGIN),
            secret: std::fs::read_to_string(var("LOSOS_OIDC_SECRET_FILE", DEFAULT_SECRET_FILE))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            kept: Mutex::new(HashMap::new()),
        }
    }

    /// Whether `token` looks like an access token of LosOS Login at all;
    /// anything else is never sent to it.
    pub fn is_login_token(token: &str) -> bool {
        token.starts_with("authelia_at_")
            && token.len() <= 512
            && token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    }

    /// The person `token` acts for, when LosOS Login says it is a live
    /// token of the companion's or the backups' issued at `host`.
    pub fn check(&self, token: &str, host: &str) -> Option<Grant> {
        if !Self::is_login_token(token) || !valid_host(host) {
            return None;
        }
        let key = crate::boxid::sha256(format!("{host}\n{token}").as_bytes());
        let now = Instant::now();
        {
            let mut kept = self.kept.lock().unwrap_or_else(|e| e.into_inner());
            match kept.get(&key) {
                Some((grant, until)) if *until > now => return Some(grant.clone()),
                Some(_) => {
                    kept.remove(&key);
                }
                None => {}
            }
        }
        let answer = match self.introspect(token, host) {
            Ok(answer) => answer,
            Err(e) => {
                tracing::warn!(error = %e, "LosOS Login could not be asked about a companion token");
                return None;
            }
        };
        let now_secs = chrono::Utc::now().timestamp();
        let (grant, left) = judge(&answer, now_secs)?;
        let until = now + KEEP.min(Duration::from_secs(left));
        let mut kept = self.kept.lock().unwrap_or_else(|e| e.into_inner());
        if kept.len() >= MAX_KEPT {
            kept.clear();
        }
        kept.insert(key, (grant.clone(), until));
        Some(grant)
    }

    /// One introspection request to LosOS Login, as `lososd`.
    fn introspect(&self, token: &str, host: &str) -> std::io::Result<serde_json::Value> {
        let secret = self.secret.as_deref().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "lososd has no OIDC client secret",
            )
        })?;
        let basic =
            base64::engine::general_purpose::STANDARD.encode(format!("{CLIENT_ID}:{secret}"));
        // The token is letters, digits and `_-.` (checked above): nothing
        // in it needs escaping in a form body.
        let body = format!("token={token}&token_type_hint=access_token");
        let mut stream = TcpStream::connect(&self.login)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        let head = format!(
            "POST /auth/api/oidc/introspection HTTP/1.1\r\nHost: {host}\r\n\
             X-Forwarded-Proto: https\r\nX-Forwarded-Host: {host}\r\n\
             Authorization: Basic {basic}\r\n\
             Content-Type: application/x-www-form-urlencoded\r\n\
             Accept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes())?;
        stream.write_all(body.as_bytes())?;
        stream.flush()?;
        let inner = crate::access::read_response(BufReader::new(stream))?;
        if inner.status != 200 {
            return Err(std::io::Error::other(format!(
                "introspection answered {}",
                inner.status
            )));
        }
        Ok(inner.body)
    }
}

/// The grant in an introspection answer, and how many seconds it has
/// left, when it is an active token of the companion's or the backups'.
pub fn judge(answer: &serde_json::Value, now_secs: i64) -> Option<(Grant, u64)> {
    let field = |name: &str| answer.get(name).and_then(serde_json::Value::as_str);
    if answer.get("active").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    let client = match field("client_id")? {
        COMPANION_CLIENT_ID => Client::Companion,
        BACKUPS_CLIENT_ID => Client::Backups,
        _ => return None,
    };
    let person = field("username")?;
    crate::people::validate_name(person).ok()?;
    let exp = answer.get("exp").and_then(serde_json::Value::as_i64)?;
    let left = u64::try_from(exp - now_secs).ok().filter(|s| *s > 0)?;
    Some((
        Grant {
            person: person.to_string(),
            client,
        },
        left,
    ))
}

/// A host as nginx passes it: a name or an address, with a port perhaps.
/// Anything else is not put in a header.
fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 255
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn active(client: &str) -> serde_json::Value {
        json!({
            "active": true,
            "client_id": client,
            "exp": 2_000,
            "iat": 1_000,
            "iss": "https://mattbox.local/auth",
            "scope": "openid profile offline_access",
            "sub": "591cb911-c903-418e-8695-1661716b8f70",
            "username": "matus",
        })
    }

    #[test]
    fn an_active_companion_token_names_its_person() {
        let (grant, left) = judge(&active("losos-companion"), 1_500).unwrap();
        assert_eq!(grant.person, "matus");
        assert_eq!(grant.client, Client::Companion);
        assert_eq!(left, 500);
    }

    #[test]
    fn a_backups_token_says_it_is_the_backups() {
        let (grant, _) = judge(&active("losos-backups"), 1_500).unwrap();
        assert_eq!(grant.person, "matus");
        assert_eq!(grant.client, Client::Backups);
    }

    #[test]
    fn another_clients_token_an_old_one_or_an_inactive_one_is_nobody() {
        assert_eq!(judge(&active("forgejo"), 1_500), None);
        assert_eq!(judge(&active("losos-companion"), 2_000), None);
        assert_eq!(judge(&json!({ "active": false }), 1_500), None);
        let mut odd = active("losos-companion");
        odd["username"] = json!("Matus Smith");
        assert_eq!(judge(&odd, 1_500), None);
        let mut nameless = active("losos-companion");
        nameless.as_object_mut().unwrap().remove("username");
        assert_eq!(judge(&nameless, 1_500), None);
    }

    #[test]
    fn only_a_login_access_token_is_ever_asked_about() {
        assert!(Companion::is_login_token(
            "authelia_at_VxmzpW5WKS3iq-QTcc7P2MK.RgODOyLEyqXPaAgfh-odoi2"
        ));
        assert!(!Companion::is_login_token("authelia_rt_abc.def"));
        assert!(!Companion::is_login_token("losos_pat_0a1b2c3d_x"));
        assert!(!Companion::is_login_token("authelia_at_a&token=b"));
        assert!(!valid_host("box.local\r\nX-Evil: 1"));
        assert!(valid_host("192.168.1.20"));
        assert!(valid_host("mattbox.local:8443"));
    }

    /// A LosOS Login that answers introspection from a table, counting the
    /// questions, to see the minute's memory at work.
    #[test]
    fn an_answer_is_kept_and_asked_again_once_it_runs_out() {
        use std::io::{BufRead, Read};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let asked = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = asked.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    head.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                seen.lock()
                    .unwrap()
                    .push(format!("{head}{}", String::from_utf8(body).unwrap()));
                let exp = chrono::Utc::now().timestamp() + 3600;
                let answer = json!({
                    "active": true, "client_id": "losos-companion",
                    "exp": exp, "username": "matus",
                })
                .to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{answer}",
                    answer.len()
                )
                .unwrap();
            }
        });
        let c = Companion {
            login: addr,
            secret: Some("s3cret".into()),
            kept: Mutex::new(HashMap::new()),
        };
        let token = "authelia_at_abc.def";
        assert_eq!(c.check(token, "mattbox.local").unwrap().person, "matus");
        assert_eq!(c.check(token, "mattbox.local").unwrap().person, "matus");
        assert_eq!(asked.lock().unwrap().len(), 1, "the second call is kept");
        let first = asked.lock().unwrap()[0].clone();
        assert!(first.starts_with("POST /auth/api/oidc/introspection HTTP/1.1\r\n"));
        assert!(first.contains("X-Forwarded-Host: mattbox.local\r\n"));
        assert!(first.contains("X-Forwarded-Proto: https\r\n"));
        // lososd:s3cret
        assert!(first.contains("Authorization: Basic bG9zb3NkOnMzY3JldA==\r\n"));
        assert!(first.ends_with("token=authelia_at_abc.def&token_type_hint=access_token"));
        // Another address is another question: the token is bound to where
        // it was issued.
        c.check(token, "192.168.1.20").unwrap();
        assert_eq!(asked.lock().unwrap().len(), 2);
        // Run out the minute.
        for (_, until) in c.kept.lock().unwrap().values_mut() {
            *until = Instant::now();
        }
        c.check(token, "mattbox.local").unwrap();
        assert_eq!(asked.lock().unwrap().len(), 3);
    }
}
