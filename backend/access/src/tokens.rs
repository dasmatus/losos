//! Personal access tokens: `losos_pat_<id>_<secret>`, made in the web UI,
//! each bound to one seeded key. Only the SHA-256 of a token is kept.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::envelope::Refusal;
use crate::keys::{write_private, Keyring};

pub const MAX_TOKENS: usize = 32;
pub const MAX_NAME_CHARS: usize = 64;
const PREFIX: &str = "losos_pat_";
const ID_HEX: usize = 8;
const SECRET_HEX: usize = 40;
/// `last_used` is written back at most this often, so a busy terminal does
/// not rewrite the file on every call.
const TOUCH_SECS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// GET only.
    Read,
    /// Everything a token may reach.
    Full,
}

impl Scope {
    pub fn allows(self, method: &str) -> bool {
        match self {
            Scope::Full => true,
            Scope::Read => method.eq_ignore_ascii_case("GET"),
        }
    }
}

/// One token as stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRecord {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub scope: Scope,
    pub created: i64,
    pub expires: Option<i64>,
    pub last_used: Option<i64>,
    pub sha256: String,
    /// Who made it: the person signed in to the web UI when it was minted.
    /// A call with it runs as that person, with their groups. `None` for a
    /// token made with the spare key, which runs as the spare key does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// One token as the web UI sees it: everything but the hash.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenView {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub scope: Scope,
    pub created: i64,
    pub expires: Option<i64>,
    pub last_used: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

impl From<&TokenRecord> for TokenView {
    fn from(r: &TokenRecord) -> TokenView {
        TokenView {
            id: r.id.clone(),
            name: r.name.clone(),
            fingerprint: r.fingerprint.clone(),
            scope: r.scope,
            created: r.created,
            expires: r.expires,
            last_used: r.last_used,
            owner: r.owner.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    /// The request is wrong: the web UI shows the sentence (400).
    #[error("{0}")]
    Invalid(String),
    /// [`MAX_TOKENS`] already (409).
    #[error("there are already {MAX_TOKENS} tokens; revoke one first")]
    Full,
    /// No token with that id (404).
    #[error("no token has that id")]
    NotFound,
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: std::io::Error,
    },
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// The id part of a well-formed token.
fn token_id(token: &str) -> Option<&str> {
    let rest = token.strip_prefix(PREFIX)?;
    let (id, secret) = rest.split_once('_')?;
    let hex = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit());
    (hex(id, ID_HEX) && hex(secret, SECRET_HEX)).then_some(id)
}

/// The token file, `tokens.json`. One lock around every read-modify-write.
pub struct Tokens {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Tokens {
    pub fn new(path: impl Into<PathBuf>) -> Tokens {
        Tokens {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> Result<Vec<TokenRecord>, TokenError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| TokenError::Io {
                what: format!("reading {}", self.path.display()),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(source) => Err(TokenError::Io {
                what: format!("reading {}", self.path.display()),
                source,
            }),
        }
    }

    fn write(&self, records: &[TokenRecord]) -> Result<(), TokenError> {
        let bytes = serde_json::to_vec_pretty(records).expect("records serialise");
        write_private(&self.path, &bytes).map_err(|source| TokenError::Io {
            what: format!("writing {}", self.path.display()),
            source,
        })
    }

    /// Every token, oldest first, without hashes.
    pub fn list(&self) -> Result<Vec<TokenView>, TokenError> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read()?.iter().map(TokenView::from).collect())
    }

    /// Make a token bound to `fingerprint`, which must be in `ring`. Answers
    /// the token itself, the only time it exists outside the caller.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        ring: &Keyring,
        name: &str,
        fingerprint: &str,
        scope: Scope,
        expires_in_days: Option<u32>,
        owner: Option<&str>,
        now: i64,
    ) -> Result<(String, TokenView), TokenError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
            return Err(TokenError::Invalid(format!(
                "a token's name is 1 to {MAX_NAME_CHARS} characters"
            )));
        }
        if name.chars().any(char::is_control) {
            return Err(TokenError::Invalid(
                "a token's name cannot hold control characters".into(),
            ));
        }
        let key = ring
            .get(fingerprint)
            .ok_or_else(|| TokenError::Invalid("that GPG key was not seeded here".into()))?;
        if let Some(days) = expires_in_days {
            if days == 0 || days > 3660 {
                return Err(TokenError::Invalid(
                    "a token expires after 1 to 3660 days, or never".into(),
                ));
            }
        }

        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut records = self.read()?;
        if records.len() >= MAX_TOKENS {
            return Err(TokenError::Full);
        }
        let mut rng = rand::thread_rng();
        let id = loop {
            let mut b = [0u8; ID_HEX / 2];
            rng.fill_bytes(&mut b);
            let id = hex::encode(b);
            if !records.iter().any(|r| r.id == id) {
                break id;
            }
        };
        let mut secret = [0u8; SECRET_HEX / 2];
        rng.fill_bytes(&mut secret);
        let token = format!("{PREFIX}{id}_{}", hex::encode(secret));
        let record = TokenRecord {
            id,
            name: name.to_string(),
            fingerprint: key.fingerprint.clone(),
            scope,
            created: now,
            expires: expires_in_days.map(|d| now + i64::from(d) * 86_400),
            last_used: None,
            sha256: sha256_hex(&token),
            owner: owner.map(str::to_string),
        };
        let view = TokenView::from(&record);
        records.push(record);
        self.write(&records)?;
        Ok((token, view))
    }

    pub fn revoke(&self, id: &str) -> Result<(), TokenError> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut records = self.read()?;
        let before = records.len();
        records.retain(|r| r.id != id);
        if records.len() == before {
            return Err(TokenError::NotFound);
        }
        self.write(&records)
    }

    /// The token's record when it is known, live, bound to `signer` and its
    /// scope allows `method`. Every failure but scope is the same
    /// [`Refusal::Token`], so a caller learns nothing about which part was
    /// wrong.
    pub fn authenticate(
        &self,
        token: &str,
        signer: &str,
        method: &str,
        now: i64,
    ) -> Result<TokenView, Refusal> {
        let id = token_id(token).ok_or(Refusal::Token)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut records = self.read().map_err(|_| Refusal::Token)?;
        let hash = sha256_hex(token);
        let record = records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(Refusal::Token)?;
        if !bool::from(record.sha256.as_bytes().ct_eq(hash.as_bytes())) {
            return Err(Refusal::Token);
        }
        if record.expires.is_some_and(|e| now >= e) {
            return Err(Refusal::Token);
        }
        if !record.fingerprint.eq_ignore_ascii_case(signer) {
            return Err(Refusal::Token);
        }
        if !record.scope.allows(method) {
            return Err(Refusal::Scope);
        }
        let view = TokenView::from(&*record);
        if record.last_used.is_none_or(|t| now - t >= TOUCH_SECS) {
            record.last_used = Some(now);
            // A failed write costs a stale "last used", not the call.
            let _ = self.write(&records);
        }
        Ok(view)
    }
}

/// Is `path` one a token may reach? It must be under one of `allowed`
/// (`/api/` on a box), carry no `..`, no fragment and no whitespace, and not
/// be under one of `forbidden` (`/api/access`, the token routes themselves).
pub fn path_allowed(path: &str, allowed: &[&str], forbidden: &[&str]) -> bool {
    let route = path.split('?').next().unwrap_or(path);
    if path.contains("..")
        || path.contains('#')
        || path.contains('\\')
        || path.chars().any(|c| c.is_whitespace() || c.is_control())
        || route.contains("//")
    {
        return false;
    }
    let under = |prefix: &str| {
        let p = prefix.trim_end_matches('/');
        route == p || route.starts_with(&format!("{p}/")) || route.starts_with(&format!("{p}?"))
    };
    allowed.iter().any(|a| under(a)) && !forbidden.iter().any(|f| under(f))
}

/// The OAuth client the LosOS companion signs in as (`losos agents
/// install`), registered with LosOS Login in `modules/identity.nix`.
pub const COMPANION_CLIENT_ID: &str = "losos-companion";

/// Every route a LosOS companion token reaches on a box, all of them reads:
/// who it acts as, the box's health, and the person's files and
/// repositories. Matched exactly, with any query string. A route an AI tool
/// should reach is added here and to the companion's own tool table
/// (`admin-ui/tui/src/agents/mcp.rs`), whose test holds the two together.
/// `{id}` stands for one path segment of letters, digits, `-` and `_`.
pub const COMPANION_ROUTES: &[&str] = &[
    "/api/me",
    "/api/status",
    "/api/cloud/files",
    "/api/cloud/text",
    "/api/git/repos",
    "/api/git/text",
    "/api/mail",
    "/api/mail/messages",
    "/api/mail/messages/{id}",
];

/// May a companion token make this call? GET only, on one of
/// [`COMPANION_ROUTES`], after the same shape checks as [`path_allowed`].
pub fn companion_route(method: &str, path: &str) -> bool {
    let route = path.split('?').next().unwrap_or(path);
    method.eq_ignore_ascii_case("GET")
        && path_allowed(path, &["/api/"], &[])
        && COMPANION_ROUTES.iter().any(|r| route_matches(r, route))
}

fn route_matches(pattern: &str, route: &str) -> bool {
    let (mut p, mut r) = (pattern.split('/'), route.split('/'));
    loop {
        match (p.next(), r.next()) {
            (None, None) => return true,
            (Some("{id}"), Some(seg)) => {
                let fits = !seg.is_empty()
                    && seg
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                if !fits {
                    return false;
                }
            }
            (Some(a), Some(b)) if a == b => {}
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::HostKey;

    #[test]
    fn a_companion_reads_its_routes_and_nothing_else() {
        assert!(companion_route("GET", "/api/me"));
        assert!(companion_route("get", "/api/cloud/text?q=salmon"));
        assert!(!companion_route("POST", "/api/me"));
        assert!(!companion_route("GET", "/api/settings"));
        assert!(!companion_route("GET", "/api/cloud/files/../settings"));
        assert!(!companion_route("GET", "/api/cloud/filesx"));
        assert!(!companion_route("GET", "/api/access/tokens"));
        assert!(companion_route("GET", "/api/mail/messages?mailbox=a&q=x"));
        assert!(companion_route("GET", "/api/mail/messages/Ma1b-2_c"));
        assert!(!companion_route("GET", "/api/mail/messages/m1/html"));
        assert!(!companion_route("GET", "/api/mail/messages/"));
        assert!(!companion_route("GET", "/api/mail/messages/m%2F1"));
        assert!(!companion_route("GET", "/api/mail/attachments/b1"));
        assert!(!companion_route("POST", "/api/mail/messages/m1"));
        assert!(!companion_route("GET", "/api/mail/send"));
    }

    fn ring() -> (Keyring, String) {
        let k = HostKey::generate("Owner").unwrap();
        (
            Keyring::parse(k.armored_public()).unwrap(),
            k.fingerprint().to_string(),
        )
    }

    fn store(tag: &str) -> Tokens {
        let dir = std::env::temp_dir().join(format!("losos-tokens-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Tokens::new(dir.join("tokens.json"))
    }

    #[test]
    fn a_token_authenticates_only_with_its_own_key() {
        let (ring, fp) = ring();
        let t = store("own");
        let (token, view) = t
            .create(&ring, "laptop", &fp, Scope::Full, Some(30), None, 1000)
            .unwrap();
        assert!(token.starts_with("losos_pat_"));
        assert_eq!(token.len(), PREFIX.len() + ID_HEX + 1 + SECRET_HEX);
        assert_eq!(view.expires, Some(1000 + 30 * 86_400));
        assert!(t.authenticate(&token, &fp, "POST", 2000).is_ok());
        assert_eq!(
            t.authenticate(&token, "00", "GET", 2000).unwrap_err(),
            Refusal::Token
        );
        // Last used was recorded; the hash never leaves the file.
        let listed = t.list().unwrap();
        assert_eq!(listed[0].last_used, Some(2000));
        let raw = std::fs::read_to_string(t.path()).unwrap();
        assert!(!raw.contains(&token));
    }

    #[test]
    fn a_wrong_secret_an_expired_or_revoked_token_is_refused() {
        let (ring, fp) = ring();
        let t = store("dead");
        let (token, view) = t
            .create(&ring, "laptop", &fp, Scope::Read, Some(1), None, 1000)
            .unwrap();
        let mut wrong = token.clone();
        wrong.pop();
        wrong.push(if token.ends_with('0') { '1' } else { '0' });
        assert_eq!(
            t.authenticate(&wrong, &fp, "GET", 1000).unwrap_err(),
            Refusal::Token
        );
        assert_eq!(
            t.authenticate(&token, &fp, "GET", 1000 + 86_400)
                .unwrap_err(),
            Refusal::Token
        );
        assert_eq!(
            t.authenticate(&token, &fp, "POST", 1000).unwrap_err(),
            Refusal::Scope
        );
        t.revoke(&view.id).unwrap();
        assert_eq!(
            t.authenticate(&token, &fp, "GET", 1000).unwrap_err(),
            Refusal::Token
        );
        assert!(matches!(t.revoke(&view.id), Err(TokenError::NotFound)));
    }

    #[test]
    fn creation_checks_its_input() {
        let (ring, fp) = ring();
        let t = store("input");
        assert!(matches!(
            t.create(&ring, "  ", &fp, Scope::Read, None, None, 0),
            Err(TokenError::Invalid(_))
        ));
        assert!(matches!(
            t.create(&ring, "x", "ABCD", Scope::Read, None, None, 0),
            Err(TokenError::Invalid(_))
        ));
        assert!(matches!(
            t.create(&ring, "x", &fp, Scope::Read, Some(0), None, 0),
            Err(TokenError::Invalid(_))
        ));
        for i in 0..MAX_TOKENS {
            t.create(&ring, &format!("t{i}"), &fp, Scope::Read, None, None, 0)
                .unwrap();
        }
        assert!(matches!(
            t.create(&ring, "one more", &fp, Scope::Read, None, None, 0),
            Err(TokenError::Full)
        ));
    }

    #[test]
    fn paths_stay_inside_the_api_and_away_from_tokens() {
        let allowed = ["/api/"];
        let forbidden = ["/api/access", "/api/setup", "/api/sign-in"];
        let ok = |p| path_allowed(p, &allowed, &forbidden);
        assert!(ok("/api/state"));
        assert!(ok("/api/apps/search?q=nextcloud"));
        assert!(!ok("/api/access/tokens"));
        assert!(!ok("/api/access"));
        assert!(!ok("/api/accessories/../access/tokens"));
        assert!(!ok("/api/setup/claim"));
        assert!(!ok("/api/sign-in"));
        assert!(!ok("/apifoo"));
        assert!(!ok("/handbook/"));
        assert!(!ok("/api//access"));
        assert!(!ok("/api/state #"));
        assert!(ok("/api/accessibility"));
    }
}
