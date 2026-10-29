//! The edge as its boxes' Tang server (`losos.edge.tang`, the box side in
//! modules/network-unlock.nix).
//!
//! A box installed with `losos-install --tang-edge` seals its disk key to its
//! TPM2 AND a Tang key of its own on this edge. Every boot its initrd asks
//! the edge; the edge's answer is one half of a blinded exchange (McCallum-Relyea),
//! so it learns that the box booted and never sees the key. What the edge
//! adds over a Tang server on the home network is the switch below: an owner
//! whose box was stolen says so here with the box's recovery code, and from
//! then on the edge refuses to answer for that box. The box does not open
//! anywhere, at home included, until the owner says they have it back.
//!
//! Each box gets its own key pair, made by Tang's own `tangd-keygen`, in
//! `<dir>/<id>/keys`, and every request runs Tang's own `tangd` on that
//! directory, the way its socket unit runs it: one process per connection,
//! the HTTP request on stdin, the answer on stdout. No Tang cryptography is
//! written here.
//!
//! The files beside the keys:
//!
//! * `claim`: the SHA-256 of a secret the installer made and left inside the
//!   box's encrypted volume. Only that box can name its owner.
//! * `lost-hash`: the SHA-256 of [`lost_secret`] of the box's recovery code,
//!   set by the box (with the claim secret) once it has a code, and replaced
//!   when the code changes. The edge never sees the code.
//! * `lost`: present while the owner has reported the box stolen.
//!
//! Anyone may ask for a key, as anyone may use a public Tang server: a key
//! is a few hundred bytes and opens nothing on its own. The count is capped
//! ([`MAX_BOXES`]) and so is the rate ([`MAX_NEW_PER_MINUTE`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use base64::Engine as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

/// `serve --tang-dir DIR --tangd PATH --tangd-keygen PATH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TangOpts {
    pub dir: PathBuf,
    /// Tang's `libexec/tangd`.
    pub tangd: PathBuf,
    /// Tang's `libexec/tangd-keygen`.
    pub keygen: PathBuf,
}

/// Keys this edge holds at most.
pub const MAX_BOXES: usize = 100_000;
/// New keys per minute, for everyone together.
pub const MAX_NEW_PER_MINUTE: u32 = 30;
/// A `rec` body: one P-521 public key as a JWK.
pub const MAX_REC_BYTES: usize = 4096;
/// How long one `tangd` run may take.
const TANGD_TIMEOUT: Duration = Duration::from_secs(5);
/// The largest answer `tangd` gives: an advertisement of a handful of keys.
const MAX_ANSWER_BYTES: u64 = 64 * 1024;

/// The secret the edge's lost-box switch takes, from the box's recovery
/// code. The owner's browser computes it on the edge's page
/// (`server::tang`), and the box computes the same to tell the edge its
/// hash; the code itself never leaves either.
pub fn lost_secret(code: &str) -> String {
    sha256_hex(format!("losos-tang-lost\n{}", code.trim().to_ascii_lowercase()).as_bytes())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, bytes);
    d.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A box's id on this edge: 16 random bytes in hex.
pub fn valid_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A Tang key id: a JWK thumbprint, base64url.
pub fn valid_kid(s: &str) -> bool {
    (1..=128).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[derive(Debug, thiserror::Error)]
pub enum TangError {
    #[error("this edge is no Tang server")]
    Unconfigured,
    #[error("{0}")]
    Invalid(&'static str),
    #[error("no such box")]
    NotFound,
    /// The claim secret did not match.
    #[error("not this box's claim")]
    Forbidden,
    /// The owner reported the box stolen.
    #[error("this box was reported stolen")]
    Lost,
    #[error("{0}")]
    Full(&'static str),
    #[error("tangd: {0}")]
    Tangd(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// What a Tang client asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Adv(Option<String>),
    Rec(String, Vec<u8>),
}

/// `tangd`'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub struct Tang {
    opts: TangOpts,
    state: Mutex<Index>,
}

struct Index {
    count: usize,
    /// `lost-hash` to box id, for the owner's switch, which names no box.
    lost: HashMap<String, String>,
    /// The start of the current minute and the keys made in it.
    window: (Instant, u32),
}

impl Tang {
    /// Open the directory, making it on the first start, and read every
    /// box's `lost-hash` into the index.
    pub async fn open(opts: TangOpts) -> std::io::Result<Self> {
        tokio::fs::create_dir_all(&opts.dir).await?;
        let mut count = 0;
        let mut lost = HashMap::new();
        let mut entries = tokio::fs::read_dir(&opts.dir).await?;
        while let Some(e) = entries.next_entry().await? {
            let name = e.file_name().to_string_lossy().into_owned();
            if !valid_id(&name) {
                continue;
            }
            count += 1;
            if let Ok(h) = tokio::fs::read_to_string(e.path().join("lost-hash")).await {
                let h = h.trim();
                if is_hex64(h) {
                    lost.insert(h.to_string(), name);
                }
            }
        }
        Ok(Self {
            opts,
            state: Mutex::new(Index {
                count,
                lost,
                window: (Instant::now(), 0),
            }),
        })
    }

    fn box_dir(&self, id: &str) -> Result<PathBuf, TangError> {
        if !valid_id(id) {
            return Err(TangError::Invalid("not a box id"));
        }
        Ok(self.opts.dir.join(id))
    }

    /// A key pair for a new box. `claim` is the SHA-256 (hex) of the secret
    /// the box keeps. Returns the box's id and the signing key's thumbprint.
    pub async fn create(&self, claim: &str) -> Result<(String, String), TangError> {
        if !is_hex64(claim) {
            return Err(TangError::Invalid("claim must be a SHA-256 in hex"));
        }
        let mut st = self.state.lock().await;
        if st.count >= MAX_BOXES {
            return Err(TangError::Full("this edge holds as many keys as it will"));
        }
        if st.window.0.elapsed() >= Duration::from_secs(60) {
            st.window = (Instant::now(), 0);
        }
        if st.window.1 >= MAX_NEW_PER_MINUTE {
            return Err(TangError::Full(
                "too many new keys this minute; retry shortly",
            ));
        }
        let id: String = rand::random::<[u8; 16]>()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let dir = self.opts.dir.join(&id);
        let keys = dir.join("keys");
        tokio::fs::create_dir_all(&keys).await?;
        let made = tokio::process::Command::new(&self.opts.keygen)
            .arg(&keys)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        let thumbprint = match made {
            Ok(s) if s.success() => signing_thumbprint(&keys).await,
            Ok(s) => Err(TangError::Tangd(format!("tangd-keygen exited {s}"))),
            Err(e) => Err(TangError::Io(e)),
        };
        let thumbprint = match thumbprint {
            Ok(t) => t,
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&dir).await;
                return Err(e);
            }
        };
        crate::fsutil::atomic_write(&dir.join("claim"), format!("{claim}\n").as_bytes(), 0o600)
            .await?;
        st.count += 1;
        st.window.1 += 1;
        Ok((id, thumbprint))
    }

    /// The box names the hash of its owner's lost-box secret, proving it is
    /// the box with the claim secret.
    pub async fn set_owner(&self, id: &str, claim: &str, lost_hash: &str) -> Result<(), TangError> {
        let dir = self.box_dir(id)?;
        if !is_hex64(lost_hash) {
            return Err(TangError::Invalid("lost must be a SHA-256 in hex"));
        }
        let stored = match tokio::fs::read_to_string(dir.join("claim")).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(TangError::NotFound),
            Err(e) => return Err(e.into()),
        };
        let offered = sha256_hex(claim.as_bytes());
        if !bool::from(subtle::ConstantTimeEq::ct_eq(
            offered.as_bytes(),
            stored.trim().as_bytes(),
        )) {
            return Err(TangError::Forbidden);
        }
        let mut st = self.state.lock().await;
        st.lost.retain(|_, v| v != id);
        crate::fsutil::atomic_write(
            &dir.join("lost-hash"),
            format!("{lost_hash}\n").as_bytes(),
            0o600,
        )
        .await?;
        st.lost.insert(lost_hash.to_string(), id.to_string());
        Ok(())
    }

    /// The owner's switch: `lost` true reports the box stolen, false takes
    /// the report back. Found by the secret alone.
    pub async fn mark(&self, secret: &str, lost: bool) -> Result<String, TangError> {
        if !is_hex64(secret) {
            return Err(TangError::Invalid("not a lost-box secret"));
        }
        let id = {
            let st = self.state.lock().await;
            st.lost.get(&sha256_hex(secret.as_bytes())).cloned()
        };
        let Some(id) = id else {
            return Err(TangError::NotFound);
        };
        let marker = self.opts.dir.join(&id).join("lost");
        if lost {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            crate::fsutil::atomic_write(&marker, format!("{now}\n").as_bytes(), 0o600).await?;
        } else {
            match tokio::fs::remove_file(&marker).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(id)
    }

    /// Answer a Tang client for one box with `tangd` on that box's keys.
    pub async fn serve(&self, id: &str, req: Request) -> Result<Answer, TangError> {
        let dir = self.box_dir(id)?;
        let raw = match &req {
            Request::Adv(None) => {
                b"GET /adv HTTP/1.1\r\nHost: tang\r\nConnection: close\r\n\r\n".to_vec()
            }
            Request::Adv(Some(kid)) => {
                if !valid_kid(kid) {
                    return Err(TangError::Invalid("not a key id"));
                }
                format!("GET /adv/{kid} HTTP/1.1\r\nHost: tang\r\nConnection: close\r\n\r\n")
                    .into_bytes()
            }
            Request::Rec(kid, body) => {
                if !valid_kid(kid) {
                    return Err(TangError::Invalid("not a key id"));
                }
                if body.len() > MAX_REC_BYTES {
                    return Err(TangError::Invalid("body too large"));
                }
                let mut r = format!(
                    "POST /rec/{kid} HTTP/1.1\r\nHost: tang\r\nContent-Type: application/jwk+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                r.extend_from_slice(body);
                r
            }
        };
        let keys = dir.join("keys");
        if !tokio::fs::try_exists(&keys).await.unwrap_or(false) {
            return Err(TangError::NotFound);
        }
        if tokio::fs::try_exists(dir.join("lost"))
            .await
            .unwrap_or(true)
        {
            return Err(TangError::Lost);
        }
        let run = async {
            let mut child = tokio::process::Command::new(&self.opts.tangd)
                .arg(&keys)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&raw).await?;
                stdin.shutdown().await?;
            }
            let mut out = Vec::new();
            if let Some(stdout) = child.stdout.take() {
                stdout.take(MAX_ANSWER_BYTES).read_to_end(&mut out).await?;
            }
            let _ = child.wait().await;
            Ok::<_, std::io::Error>(out)
        };
        let out = tokio::time::timeout(TANGD_TIMEOUT, run)
            .await
            .map_err(|_| TangError::Tangd("timed out".into()))??;
        parse_answer(&out)
    }
}

/// Split `tangd`'s HTTP/1.1 answer into status, type and body.
pub fn parse_answer(out: &[u8]) -> Result<Answer, TangError> {
    let split = out
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| TangError::Tangd("no answer".into()))?;
    let head = std::str::from_utf8(&out[..split])
        .map_err(|_| TangError::Tangd("answer is not text".into()))?;
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or_else(|| TangError::Tangd("no status".into()))?;
    let content_type = lines.find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case("content-type")
            .then(|| v.trim().to_string())
    });
    Ok(Answer {
        status,
        content_type,
        body: out[split + 4..].to_vec(),
    })
}

/// The RFC 7638 SHA-256 thumbprint of the signing key in a `tangd-keygen`
/// directory: what `tang-show-keys` prints and Clevis checks.
pub async fn signing_thumbprint(keys: &Path) -> Result<String, TangError> {
    let mut entries = tokio::fs::read_dir(keys).await?;
    while let Some(e) = entries.next_entry().await? {
        let Ok(text) = tokio::fs::read_to_string(e.path()).await else {
            continue;
        };
        let Ok(jwk) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let signs = jwk["key_ops"]
            .as_array()
            .is_some_and(|ops| ops.iter().any(|o| o == "sign"));
        if signs {
            return ec_thumbprint(&jwk)
                .ok_or_else(|| TangError::Tangd("signing key is not EC".into()));
        }
    }
    Err(TangError::Tangd("no signing key".into()))
}

/// RFC 7638 over an EC public key: the required members in lexical order,
/// no whitespace, SHA-256, base64url without padding.
pub fn ec_thumbprint(jwk: &serde_json::Value) -> Option<String> {
    let member = |k: &str| jwk[k].as_str();
    if member("kty")? != "EC" {
        return None;
    }
    let canonical = format!(
        r#"{{"crv":"{}","kty":"EC","x":"{}","y":"{}"}}"#,
        member("crv")?,
        member("x")?,
        member("y")?
    );
    let d = ring::digest::digest(&ring::digest::SHA256, canonical.as_bytes());
    Some(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(d.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thumbprint_is_rfc_7638() {
        // RFC 7638's own example is RSA; this one is checked against
        // `jose jwk thp -a S256` (jose 14) on a key tangd-keygen made.
        let jwk = serde_json::json!({
            "alg": "ES512", "crv": "P-521", "key_ops": ["sign", "verify"], "kty": "EC",
            "x": "AReszPc8FGJszptYKTZIbVfSA1ldFUEYQZDSXzYcsXmr_G6JZgJp3Bs_vBDejcpcllZtlqTQkhnu6js-se559Ex5",
            "y": "ARnhS4nryb1uKUSw6sCjpnypGctlfkc-OjA1Op0m05bRs64QiXvci4fxkmnQNYVkFkZ366mrzOr7vWnmanTuhcQw",
        });
        assert_eq!(
            ec_thumbprint(&jwk).as_deref(),
            Some("0_JX_0smhrUKXWw8gE19kBbQBY-Nk7-c6Qssa5xtbGs")
        );
        assert_eq!(ec_thumbprint(&serde_json::json!({ "kty": "RSA" })), None);
    }

    #[test]
    fn the_lost_secret_ignores_case_and_spaces() {
        let code = "6f1c2a8e-4b3d-4f0a-9c1e-2d7b5a9e8f10";
        assert_eq!(
            lost_secret(code),
            lost_secret(&format!("  {}\n", code.to_uppercase()))
        );
        assert_ne!(lost_secret(code), sha256_hex(code.as_bytes()));
        assert!(is_hex64(&lost_secret(code)));
    }

    #[test]
    fn ids_and_key_ids_keep_to_their_alphabets() {
        assert!(valid_id("0123456789abcdef0123456789abcdef"));
        assert!(!valid_id("0123456789ABCDEF0123456789abcdef"));
        assert!(!valid_id("../../etc/passwd"));
        assert!(valid_kid("0_JX_0smhrUKXWw8gE19kBbQBY-Nk7-c6Qssa5xtbGs"));
        assert!(!valid_kid("a/b"));
        assert!(!valid_kid(""));
        assert!(!valid_kid(&"a".repeat(129)));
    }

    #[test]
    fn an_answer_splits_into_status_type_and_body() {
        let a = parse_answer(b"HTTP/1.1 200 OK\r\nContent-Type: application/jose+json\r\nContent-Length: 2\r\n\r\n{}")
            .unwrap();
        assert_eq!(a.status, 200);
        assert_eq!(a.content_type.as_deref(), Some("application/jose+json"));
        assert_eq!(a.body, b"{}");
        assert!(parse_answer(b"garbage").is_err());
    }
}
