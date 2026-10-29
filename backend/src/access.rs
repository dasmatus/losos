//! Terminal access: the box's half of `losos`, the terminal client
//! (`docs/terminal-access.md`).
//!
//! Three things live here. The box's own OpenPGP key, made on first start
//! under `$LOSOS_ACCESS_DIR` (`/var/lib/losos/access`, persisted through
//! `/var`), which every request is encrypted to and every answer is signed
//! with. The owner's seeded GPG keys, read from `$LOSOS_ACCESS_KEYS_FILE`,
//! which `modules/daemon.nix` writes from `losos.access.gpgKeys`: the
//! installer put them there, and nothing on the running box can add one,
//! because a web session that could add a key could also make a token for
//! it. And the tokens, made and revoked by the web UI.
//!
//! A call that opens, verifies and authenticates is run against this
//! daemon's own loopback API with the admin token, exactly as the web UI's
//! request would be, so every route the web UI has is a route the terminal
//! has, with the same checks, and nothing here repeats a handler. A token
//! made by a person signed in through Authelia carries their name, and the
//! call says so (`X-Losos-As`), so it gets that person's sections and no
//! more; a token made with the spare key runs as the spare key. What a
//! token may not reach is decided before that: nothing outside `/api/`,
//! and none of `/api/access` (a token cannot mint, list or revoke tokens)
//! or `/api/setup`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use losos_access::{HostKey, Keyring, Opened, Refusal, Replay, Tokens};

/// Where the host key and the token file live.
const DEFAULT_DIR: &str = "/var/lib/losos/access";
/// What `modules/daemon.nix` writes the seeded keys to.
const DEFAULT_KEYS_FILE: &str = "/etc/losos/access-keys.asc";
/// The largest envelope accepted. An apply body or a widget fits; a picture
/// does not, and has no business in a terminal call.
pub const MAX_ENVELOPE_BYTES: usize = 1 << 20;
/// The paths a token reaches, and the ones it never does.
const ALLOWED: [&str; 1] = ["/api/"];
const FORBIDDEN: [&str; 2] = ["/api/access", "/api/setup"];
/// How long an inner call may take. App search reaches out to the internet;
/// everything else answers at once, a rebuild included (it answers a job).
const INNER_TIMEOUT: Duration = Duration::from_secs(120);
/// The largest inner answer passed back.
const MAX_INNER_BYTES: u64 = 8 << 20;

/// The box's terminal-access state, or why there is none.
pub enum Access {
    Ready(Box<Ready>),
    /// The host key could not be read or made, or a seeded key does not
    /// parse. The web UI and the terminal both show the sentence; the rest
    /// of the daemon carries on.
    Broken(String),
}

pub struct Ready {
    pub host: HostKey,
    pub ring: Keyring,
    pub tokens: Tokens,
    pub replay: Replay,
    /// Where the client reaches the box away from its LAN, through the
    /// edge, when the tunnel lets the terminal's routes through
    /// (`LOSOS_ACCESS_EDGE_URL`, losos.access.throughEdge).
    pub edge_url: Option<String>,
}

impl Access {
    pub fn from_env() -> Access {
        let dir = std::env::var_os("LOSOS_ACCESS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DIR));
        let keys = std::env::var_os("LOSOS_ACCESS_KEYS_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_KEYS_FILE));
        let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
        let user_id = format!("LosOS box {}", hostname.trim());
        let host = match HostKey::load_or_create(&dir.join("host-key.asc"), &user_id) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!(error = %e, "terminal access is off: no host key");
                return Access::Broken(format!("this box has no terminal key: {e}"));
            }
        };
        let ring = match Keyring::load(&keys) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "terminal access is off: a seeded GPG key is unusable");
                return Access::Broken(format!("a seeded GPG key is unusable: {e}"));
            }
        };
        tracing::info!(
            fingerprint = host.fingerprint(),
            seeded = ring.keys().len(),
            "terminal access ready"
        );
        Access::Ready(Box::new(Ready {
            host,
            ring,
            tokens: Tokens::new(dir.join("tokens.json")),
            replay: Replay::default(),
            edge_url: std::env::var("LOSOS_ACCESS_EDGE_URL")
                .ok()
                .filter(|url| url.starts_with("https://")),
        }))
    }
}

/// The checks after the envelope opened: the token, its scope, the path.
/// Answers the token, whose id goes on the audit line and whose owner the
/// call runs as.
pub fn authorize(
    ready: &Ready,
    opened: &Opened,
    now: i64,
) -> Result<losos_access::TokenView, Refusal> {
    let r = &opened.request;
    let token = ready
        .tokens
        .authenticate(&r.token, &opened.signer.fingerprint, &r.method, now)?;
    if !losos_access::path_allowed(&r.path, &ALLOWED, &FORBIDDEN) {
        return Err(Refusal::Path);
    }
    if !matches!(r.method.as_str(), "GET" | "POST" | "PUT" | "DELETE") {
        return Err(Refusal::Malformed(format!("method {}", r.method)));
    }
    Ok(token)
}

/// What the inner call answered.
pub struct Inner {
    pub status: u16,
    pub body: serde_json::Value,
}

/// Run one call against this daemon's own API on `port`, as the web UI
/// would: the admin token in the header, `remote` as the address the
/// throttle and the audit log see, and `person`, when the token has an
/// owner, as the one the call runs as.
pub fn forward(
    port: u16,
    admin_token: &str,
    remote: &str,
    person: Option<&str>,
    method: &str,
    path: &str,
    body: &serde_json::Value,
) -> std::io::Result<Inner> {
    // A string body goes as raw text: `/api/apply` takes Nix, not JSON, and
    // the web UI sends it as text/plain. Everything else is JSON.
    let (payload, content_type) = match body {
        serde_json::Value::Null => (Vec::new(), None),
        serde_json::Value::String(text) => (text.as_bytes().to_vec(), Some("text/plain")),
        other => (serde_json::to_vec(other)?, Some("application/json")),
    };
    let raw = forward_raw(
        port,
        Some(admin_token),
        remote,
        person,
        method,
        path,
        content_type,
        &payload,
    )?;
    Ok(Inner {
        status: raw.status,
        body: json_or_text(&raw.body),
    })
}

/// What [`forward_raw`] got back: the status, the content type and the
/// body as bytes.
pub struct RawAnswer {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

/// [`forward`] with the body as bytes, and with no token at all for a
/// caller who has none (a public route). A plugin's `dispatch.call` to one
/// of this daemon's own routes goes this way (`crate::plugin`), so it is
/// held to the same sign-in and section checks as a call from the page.
#[allow(clippy::too_many_arguments)]
pub fn forward_raw(
    port: u16,
    admin_token: Option<&str>,
    remote: &str,
    person: Option<&str>,
    method: &str,
    path: &str,
    content_type: Option<&str>,
    payload: &[u8],
) -> std::io::Result<RawAnswer> {
    // The request line and every header below are written by hand: refuse
    // anything that could end a line.
    let clean = |v: &str| !v.bytes().any(|b| b == b'\r' || b == b'\n');
    if ![method, path, remote, content_type.unwrap_or("")]
        .iter()
        .all(|v| clean(v))
        || !path.starts_with('/')
        || path.contains(' ')
    {
        return Err(invalid("a request line or header with a line break"));
    }
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(INNER_TIMEOUT))?;
    stream.set_write_timeout(Some(INNER_TIMEOUT))?;
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         X-Real-IP: {remote}\r\nConnection: close\r\nContent-Length: {}\r\n",
        payload.len()
    );
    if let Some(token) = admin_token {
        head.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    if let Some(content_type) = content_type {
        head.push_str(&format!("Content-Type: {content_type}\r\n"));
    }
    if let Some(person) = person {
        // A sign-in name is lower-case letters, digits and `.-_`
        // (`crate::people::validate_name`), so it cannot break the header.
        if crate::people::validate_name(person).is_err() {
            return Err(invalid("a token's owner is not a sign-in name"));
        }
        head.push_str(&format!("X-Losos-As: {person}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(payload)?;
    stream.flush()?;
    read_raw(BufReader::new(stream))
}

fn invalid(what: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, what.to_string())
}

/// Parse an HTTP/1.1 answer: status, headers, then a body by length, by
/// chunks, or to the end of the stream.
pub(crate) fn read_response(r: impl BufRead) -> std::io::Result<Inner> {
    let raw = read_raw(r)?;
    Ok(Inner {
        status: raw.status,
        body: json_or_text(&raw.body),
    })
}

/// A body as JSON, else as a string of its text, else null when empty.
fn json_or_text(bytes: &[u8]) -> serde_json::Value {
    if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(bytes).unwrap_or_else(|_| {
            serde_json::Value::String(String::from_utf8_lossy(bytes).into_owned())
        })
    }
}

/// [`read_response`] without reading the body as JSON.
fn read_raw(mut r: impl BufRead) -> std::io::Result<RawAnswer> {
    let mut line = String::new();
    r.read_line(&mut line)?;
    let status: u16 = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| invalid("no status line"))?;
    let mut length: Option<u64> = None;
    let mut content_type: Option<String> = None;
    let mut chunked = false;
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            return Err(invalid("the headers ended early"));
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-type") {
                content_type = Some(value.to_string());
            } else if name.eq_ignore_ascii_case("content-length") {
                length = value.parse().ok();
            } else if name.eq_ignore_ascii_case("transfer-encoding")
                && value.eq_ignore_ascii_case("chunked")
            {
                chunked = true;
            }
        }
    }
    let mut bytes = Vec::new();
    if chunked {
        loop {
            line.clear();
            r.read_line(&mut line)?;
            let size = u64::from_str_radix(line.trim().split(';').next().unwrap_or(""), 16)
                .map_err(|_| invalid("a chunk size"))?;
            if size == 0 {
                break;
            }
            if bytes.len() as u64 + size > MAX_INNER_BYTES {
                return Err(invalid("the answer is too large"));
            }
            let mut chunk = vec![0u8; usize::try_from(size).map_err(|_| invalid("size"))?];
            r.read_exact(&mut chunk)?;
            bytes.extend_from_slice(&chunk);
            line.clear();
            r.read_line(&mut line)?;
        }
    } else {
        let limit = length.unwrap_or(MAX_INNER_BYTES).min(MAX_INNER_BYTES);
        r.take(limit).read_to_end(&mut bytes)?;
    }
    Ok(RawAnswer {
        status,
        content_type,
        body: bytes,
    })
}

/// `GET /api/access` for the web UI.
pub fn overview(ready: &Ready) -> anyhow::Result<serde_json::Value> {
    let keys: Vec<_> = ready
        .ring
        .keys()
        .iter()
        .map(|k| {
            serde_json::json!({
                "fingerprint": k.fingerprint,
                "userIds": k.user_ids,
                "created": k.created,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "hostKey": { "fingerprint": ready.host.fingerprint() },
        "keys": keys,
        "tokens": ready.tokens.list()?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_answer_is_read_by_length() {
        let raw = b"HTTP/1.1 200 OK\r\ncontent-length: 11\r\ncontent-type: application/json\r\n\r\n{\"ok\":true}";
        let got = read_response(&raw[..]).unwrap();
        assert_eq!(got.status, 200);
        assert_eq!(got.body, serde_json::json!({"ok": true}));
    }

    #[test]
    fn a_chunked_answer_is_put_back_together() {
        let raw = b"HTTP/1.1 409 Conflict\r\nTransfer-Encoding: chunked\r\n\r\n6\r\n{\"a\":1\r\n1\r\n}\r\n0\r\n\r\n";
        let got = read_response(&raw[..]).unwrap();
        assert_eq!(got.status, 409);
        assert_eq!(got.body, serde_json::json!({"a": 1}));
    }

    #[test]
    fn a_body_that_is_not_json_comes_back_as_text() {
        let raw = b"HTTP/1.1 404 Not Found\r\n\r\nnot here";
        let got = read_response(&raw[..]).unwrap();
        assert_eq!(got.body, serde_json::json!("not here"));
    }

    fn ready(tag: &str) -> (Ready, HostKey) {
        let dir =
            std::env::temp_dir().join(format!("losos-access-box-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let owner = HostKey::generate("Owner").unwrap();
        (
            Ready {
                host: HostKey::generate("LosOS box test").unwrap(),
                ring: Keyring::parse(owner.armored_public()).unwrap(),
                tokens: Tokens::new(dir.join("tokens.json")),
                replay: Replay::default(),
                edge_url: None,
            },
            owner,
        )
    }

    fn opened(ready: &Ready, owner: &HostKey, token: &str, method: &str, path: &str) -> Opened {
        let req = losos_access::Request {
            v: 1,
            host: ready.host.fingerprint().into(),
            token: token.into(),
            method: method.into(),
            path: path.into(),
            ts: 1000,
            nonce: format!("{:032x}", rand_nonce(path, method)),
            body: serde_json::Value::Null,
        };
        let sealed =
            losos_access::seal_request(owner, &ready.host.as_seeded().unwrap(), &req).unwrap();
        losos_access::open(
            &ready.host,
            &ready.ring,
            sealed.as_bytes(),
            &ready.replay,
            1000,
        )
        .unwrap()
    }

    fn rand_nonce(a: &str, b: &str) -> u128 {
        a.bytes()
            .chain(b.bytes())
            .fold(7u128, |h, x| h.wrapping_mul(31).wrapping_add(u128::from(x)))
    }

    #[test]
    fn a_token_never_reaches_the_token_routes() {
        let (ready, owner) = ready("routes");
        let (token, _) = ready
            .tokens
            .create(
                &ready.ring,
                "laptop",
                owner.fingerprint(),
                losos_access::Scope::Full,
                None,
                None,
                1000,
            )
            .unwrap();
        for path in [
            "/api/access/tokens",
            "/api/setup/claim",
            "/api/access",
            "/handbook/",
        ] {
            let o = opened(&ready, &owner, &token, "POST", path);
            assert_eq!(
                authorize(&ready, &o, 1000).unwrap_err(),
                Refusal::Path,
                "{path}"
            );
        }
        let o = opened(&ready, &owner, &token, "POST", "/api/change");
        assert!(authorize(&ready, &o, 1000).is_ok());
    }

    #[test]
    fn a_read_token_cannot_change_anything() {
        let (ready, owner) = ready("read");
        let (token, _) = ready
            .tokens
            .create(
                &ready.ring,
                "phone",
                owner.fingerprint(),
                losos_access::Scope::Read,
                None,
                None,
                1000,
            )
            .unwrap();
        let o = opened(&ready, &owner, &token, "GET", "/api/state");
        assert!(authorize(&ready, &o, 1000).is_ok());
        let o = opened(&ready, &owner, &token, "POST", "/api/apply");
        assert_eq!(authorize(&ready, &o, 1000).unwrap_err(), Refusal::Scope);
    }
}
