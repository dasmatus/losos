//! A box signed in through LosOS Login.
//!
//! `losos login` keeps the access token and the refresh token in the host
//! record. Calls go out as `Authorization: Bearer`, over the certificate
//! pinned at login. An access token close to running out, or one the box
//! refuses, is exchanged for a new pair. LosOS Login ends the whole sign-in
//! when a used refresh token is shown twice, so a refresh takes a lock and
//! reads the keychain again first.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::agents::companion::http_for;
use crate::envelope::{unreachable, Answer, Transport};
use crate::keychain::Keychain;
use crate::oidc::{self, Granted};
use crate::problem::{Problem, Result};
use crate::tr;
use crate::vault::{HostRecord, Vault};

const CLIENT_ID: &str = losos_access::TUI_CLIENT_ID;
/// An access token this close to running out is refreshed first.
const EARLY_SECS: i64 = 60;

struct Held {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    tls_pin: Option<String>,
}

/// The calls `losos` makes to a box it signed in to with LosOS Login.
pub struct BoxLink {
    keychain: Arc<dyn Keychain + Send + Sync>,
    url: String,
    lock: PathBuf,
    held: Mutex<Option<Held>>,
    now: fn() -> i64,
}

impl BoxLink {
    pub fn new(
        keychain: Arc<dyn Keychain + Send + Sync>,
        url: &str,
        lock: PathBuf,
        now: fn() -> i64,
    ) -> BoxLink {
        BoxLink {
            keychain,
            url: url.to_string(),
            lock,
            held: Mutex::new(None),
            now,
        }
    }

    fn ended(&self) -> Problem {
        Problem::new("losos::login", tr!("tui.login.ended", url = &self.url))
            .help(tr!("tui.login.endedHelp", url = &self.url))
    }

    fn current(&self, force: bool) -> Result<Held> {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = |h: &Held| h.expires_at - EARLY_SECS > (self.now)();
        if let Some(h) = held.as_ref() {
            if !force && fresh(h) {
                return Ok(clone_held(h));
            }
        }
        let _guard = Lock::acquire(&self.lock)?;
        let vault = Vault::new(self.keychain.clone());
        let record = vault
            .get_host(&self.url)?
            .filter(|h| h.login_session())
            .ok_or_else(|| self.ended())?;
        let kept = held_of(&record).ok_or_else(|| self.ended())?;
        let stale = held
            .as_ref()
            .is_some_and(|h| h.access_token == kept.access_token);
        let next = if fresh(&kept) && !(force && stale) {
            kept
        } else {
            let http = http_for(kept.tls_pin.as_deref());
            let granted = oidc::refresh(
                &http,
                &self.url,
                CLIENT_ID,
                &kept.refresh_token,
                (self.now)(),
            )
            .map_err(|_| self.ended())?;
            store(&vault, &self.url, &granted)?;
            Held {
                access_token: granted.access_token,
                refresh_token: granted.refresh_token,
                expires_at: granted.expires_at,
                tls_pin: kept.tls_pin,
            }
        };
        *held = Some(clone_held(&next));
        Ok(next)
    }

    fn send(
        &self,
        held: &Held,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<crate::http::Response> {
        let http = http_for(held.tls_pin.as_deref());
        let sending = method.eq_ignore_ascii_case("POST")
            || method.eq_ignore_ascii_case("PUT")
            || method.eq_ignore_ascii_case("PATCH");
        let encoded =
            sending.then(|| serde_json::to_string(body).unwrap_or_else(|_| "null".into()));
        http.exchange(
            &method.to_ascii_uppercase(),
            &format!("{}{path}", self.url),
            &held.access_token,
            encoded.as_deref(),
        )
        .map_err(|e| unreachable(&self.url, e))
    }
}

fn clone_held(h: &Held) -> Held {
    Held {
        access_token: h.access_token.clone(),
        refresh_token: h.refresh_token.clone(),
        expires_at: h.expires_at,
        tls_pin: h.tls_pin.clone(),
    }
}

fn held_of(record: &HostRecord) -> Option<Held> {
    Some(Held {
        access_token: record.access_token.clone()?,
        refresh_token: record.refresh_token.clone()?,
        expires_at: record.expires_at?,
        tls_pin: record.tls_pin.clone(),
    })
}

fn store(vault: &Vault, url: &str, granted: &Granted) -> Result<()> {
    let mut record = vault
        .get_host(url)?
        .ok_or_else(|| Problem::new("losos::host", tr!("tui.host.unknown", url = url)))?;
    record.access_token = Some(granted.access_token.clone());
    record.refresh_token = Some(granted.refresh_token.clone());
    record.expires_at = Some(granted.expires_at);
    vault.put_host(&record)
}

impl Transport for BoxLink {
    fn request(&self, method: &str, path: &str, body: Value) -> Result<Answer> {
        let held = self.current(false)?;
        let mut response = self.send(&held, method, path, &body)?;
        if response.status == 401 {
            let held = self.current(true)?;
            response = self.send(&held, method, path, &body)?;
        }
        let parsed = serde_json::from_str(&response.body).unwrap_or(Value::String(response.body));
        Ok(Answer {
            status: response.status,
            body: parsed,
            // The certificate pin checked the peer. There is no envelope.
            sealed: true,
        })
    }
}

/// Where two `losos` processes wait their turn to refresh one box.
pub fn refresh_lock(env: &std::collections::HashMap<String, String>, url: &str) -> PathBuf {
    let base = env
        .get("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env.get("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".local/share"))
        })
        .unwrap_or_else(std::env::temp_dir);
    let name = hex::encode(Sha256::digest(url.as_bytes()));
    base.join("losos").join(format!("login-{name}.lock"))
}

/// A lock file. Removed when dropped, including after a panic in the refresh.
struct Lock {
    path: PathBuf,
}

impl Lock {
    fn acquire(path: &Path) -> Result<Lock> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| Problem::new("losos::login", e.to_string()))?;
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(_) => {
                    return Ok(Lock {
                        path: path.to_path_buf(),
                    })
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if stale(path) {
                        let _ = std::fs::remove_file(path);
                        continue;
                    }
                    if Instant::now() > deadline {
                        return Err(Problem::new("losos::login", tr!("tui.login.refreshBusy")));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => return Err(Problem::new("losos::login", e.to_string())),
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A lock left by a crashed process, older than a minute, is not a live one.
fn stale(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age > Duration::from_secs(60))
}
