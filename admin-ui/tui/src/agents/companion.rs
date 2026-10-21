//! The companion's sign-in: OAuth with LosOS Login, the box's own sign-in.
//!
//! [`start`] and [`finish`] run once, in `losos agents install`: the device code flow
//! (RFC 8628) as the public client `losos-companion`. The terminal shows a
//! link and a code; the person opens it, signs in to LosOS Login with their
//! password and their second factor when they have one, and approves the
//! companion on its consent page. The box hands back an access token for an
//! hour and a refresh token for 30 days, kept in the Secret Service as one
//! `kind=companion` item per box. The box takes the access token for a few
//! read-only routes as that person and nothing else
//! (`losos_access::COMPANION_ROUTES`, `backend/src/companion.rs`).
//!
//! [`Link`] is what `losos mcp` calls the box with: plain HTTPS, the box's
//! certificate pinned as at login, the access token as a Bearer header,
//! refreshed when it runs out. LosOS Login hands out a new refresh token
//! with every refresh and takes the old one back, and a refresh token used
//! twice ends the whole sign-in, so a refresh happens under a lock file and
//! starts by reading the keychain again: another AI tool's `losos mcp` may
//! have refreshed a moment ago.

use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::envelope::{Answer, Transport};
use crate::http::{Http, HttpError, Response, Trust};
use crate::keychain::Keychain;
use crate::problem::{Problem, Result};
use crate::tr;

const SERVICE: &str = "losos";
const KIND: &str = "companion";
const CLIENT_ID: &str = losos_access::COMPANION_CLIENT_ID;
const SCOPE: &str = "openid profile offline_access";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// An access token this close to running out is refreshed first.
const EARLY_SECS: i64 = 60;

/// The secret of a `kind=companion` item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Companion {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin: Option<String>,
    pub access_token: String,
    /// When the access token runs out, Unix seconds.
    pub expires_at: i64,
    pub refresh_token: String,
}

/// What the person is asked to do in their browser.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    #[serde(default = "five")]
    pub interval: u64,
    #[serde(default = "ten_minutes")]
    pub expires_in: u64,
}

fn five() -> u64 {
    5
}

fn ten_minutes() -> u64 {
    600
}

impl DeviceCode {
    /// The link to open: with the code in it, when the box gives one.
    pub fn link(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .unwrap_or(&self.verification_uri)
    }
}

/// A token endpoint's answer.
#[derive(Debug, Deserialize)]
struct Tokens {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

/// A client for the box: its certificate pinned when login pinned one.
pub fn http_for(tls_pin: Option<&str>) -> Http {
    Http::new(tls_pin.map_or(Trust::Public, |p| Trust::Pinned(p.to_string())))
}

fn form(pairs: &[(&str, &str)]) -> String {
    let mut out = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        out.append_pair(k, v);
    }
    out.finish()
}

fn unreachable(url: &str, e: HttpError) -> Problem {
    match e {
        HttpError::CertificateChanged => {
            Problem::new("losos::pin", tr!("tui.host.certificateChanged", url = url))
        }
        HttpError::Unreachable(detail) => Problem::new(
            "losos::unreachable",
            tr!("tui.agents.loginUnreachable", url = url, detail = detail),
        ),
    }
}

fn post_form(http: &Http, url: &str, base: &str, pairs: &[(&str, &str)]) -> Result<Response> {
    http.post(
        url,
        "application/x-www-form-urlencoded",
        "application/json",
        &form(pairs),
    )
    .map_err(|e| unreachable(base, e))
}

fn oauth_error(r: &Response) -> String {
    serde_json::from_str::<Value>(&r.body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

/// Ask the box for a device code.
pub fn start(http: &Http, url: &str) -> Result<DeviceCode> {
    let r = post_form(
        http,
        &format!("{url}/auth/api/oidc/device-authorization"),
        url,
        &[("client_id", CLIENT_ID), ("scope", SCOPE)],
    )?;
    if r.status != 200 {
        return Err(Problem::new(
            "losos::login",
            tr!("tui.agents.loginRefused", status = r.status.to_string()),
        )
        .help(tr!("tui.agents.loginRefusedHelp")));
    }
    serde_json::from_str(&r.body).map_err(|e| Problem::new("losos::login", e.to_string()))
}

/// Wait for the person to approve `code`, asking every `interval` seconds
/// as the box said, and keep the tokens it then hands out.
pub fn finish(
    http: &Http,
    url: &str,
    tls_pin: Option<&str>,
    code: &DeviceCode,
    now: impl Fn() -> i64,
) -> Result<Companion> {
    let mut interval = code.interval;
    let deadline = now() + i64::try_from(code.expires_in).unwrap_or(600);
    loop {
        let r = post_form(
            http,
            &format!("{url}/auth/api/oidc/token"),
            url,
            &[
                ("grant_type", DEVICE_GRANT),
                ("device_code", &code.device_code),
                ("client_id", CLIENT_ID),
            ],
        )?;
        if r.status == 200 {
            let t: Tokens = serde_json::from_str(&r.body)
                .map_err(|e| Problem::new("losos::login", e.to_string()))?;
            let refresh = t
                .refresh_token
                .ok_or_else(|| Problem::new("losos::login", tr!("tui.agents.loginNoRefresh")))?;
            return Ok(Companion {
                url: url.to_string(),
                tls_pin: tls_pin.map(str::to_string),
                access_token: t.access_token,
                expires_at: now() + t.expires_in.unwrap_or(3600),
                refresh_token: refresh,
            });
        }
        match oauth_error(&r).as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "access_denied" => {
                return Err(Problem::new("losos::login", tr!("tui.agents.loginDenied")))
            }
            "expired_token" => {
                return Err(Problem::new("losos::login", tr!("tui.agents.loginExpired"))
                    .help(tr!("tui.agents.loginAgain")))
            }
            other => {
                return Err(Problem::new(
                    "losos::login",
                    tr!(
                        "tui.agents.loginFailed",
                        status = r.status.to_string(),
                        detail = other
                    ),
                ))
            }
        }
        if now() > deadline {
            return Err(Problem::new("losos::login", tr!("tui.agents.loginExpired"))
                .help(tr!("tui.agents.loginAgain")));
        }
        std::thread::sleep(Duration::from_secs(interval));
    }
}

/// A new access token for `c`, from its refresh token.
fn refreshed(http: &Http, c: &Companion, now: i64) -> Result<Companion> {
    let r = post_form(
        http,
        &format!("{}/auth/api/oidc/token", c.url),
        &c.url,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &c.refresh_token),
            ("client_id", CLIENT_ID),
        ],
    )?;
    if r.status != 200 {
        return Err(signed_out(&c.url));
    }
    let t: Tokens =
        serde_json::from_str(&r.body).map_err(|e| Problem::new("losos::login", e.to_string()))?;
    Ok(Companion {
        url: c.url.clone(),
        tls_pin: c.tls_pin.clone(),
        access_token: t.access_token,
        expires_at: now + t.expires_in.unwrap_or(3600),
        refresh_token: t.refresh_token.unwrap_or_else(|| c.refresh_token.clone()),
    })
}

fn signed_out(url: &str) -> Problem {
    Problem::new("losos::agents", tr!("tui.agents.signedOut", url = url))
        .help(tr!("tui.agents.loginAgain"))
}

/// Hand both tokens back to the box (RFC 7009): the box ends the sign-in.
/// False when it did not answer; the keychain item goes either way, and
/// the sign-in then lasts until it runs out.
pub fn sign_out(c: &Companion) -> bool {
    let http = http_for(c.tls_pin.as_deref());
    let mut ended = true;
    for (token, hint) in [
        (&c.refresh_token, "refresh_token"),
        (&c.access_token, "access_token"),
    ] {
        let answer = post_form(
            &http,
            &format!("{}/auth/api/oidc/revocation", c.url),
            &c.url,
            &[
                ("token", token),
                ("token_type_hint", hint),
                ("client_id", CLIENT_ID),
            ],
        );
        ended &= answer.is_ok_and(|r| r.status == 200);
    }
    ended
}

/// The calls of `losos mcp` and `losos agents call`.
pub struct Link {
    keychain: Arc<dyn Keychain + Send + Sync>,
    url: String,
    lock: PathBuf,
    held: Mutex<Option<Companion>>,
    now: fn() -> i64,
}

impl Link {
    pub fn new(
        keychain: Arc<dyn Keychain + Send + Sync>,
        url: &str,
        lock: PathBuf,
        now: fn() -> i64,
    ) -> Link {
        Link {
            keychain,
            url: url.to_string(),
            lock,
            held: Mutex::new(None),
            now,
        }
    }

    /// The companion with a live access token: the one in hand, else the
    /// one in the keychain, refreshed when it is about to run out (or
    /// `force`, after the box refused it).
    fn current(&self, force: bool) -> Result<Companion> {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = |c: &Companion| c.expires_at - EARLY_SECS > (self.now)();
        if let Some(c) = held.as_ref() {
            if !force && fresh(c) {
                return Ok(c.clone());
            }
        }
        // One refresh at a time across this computer.
        if let Some(dir) = self.lock.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let file =
            File::create(&self.lock).map_err(|e| Problem::new("losos::agents", e.to_string()))?;
        file.lock()
            .map_err(|e| Problem::new("losos::agents", e.to_string()))?;
        let kept = get(self.keychain.as_ref(), &self.url)?.ok_or_else(|| signed_out(&self.url))?;
        // Another `losos mcp` may have refreshed since this one last looked.
        let stale = held
            .as_ref()
            .is_some_and(|c| c.access_token == kept.access_token);
        let c = if fresh(&kept) && !(force && stale) {
            kept
        } else {
            let http = http_for(kept.tls_pin.as_deref());
            let new = refreshed(&http, &kept, (self.now)())?;
            put(self.keychain.as_ref(), &new)?;
            new
        };
        drop(file);
        *held = Some(c.clone());
        Ok(c)
    }

    fn send(&self, c: &Companion, method: &str, path: &str) -> Result<Response> {
        if !method.eq_ignore_ascii_case("GET") {
            return Err(Problem::new(
                "losos::agents",
                tr!("tui.agents.readOnly", method = method),
            ));
        }
        http_for(c.tls_pin.as_deref())
            .get_bearer(
                &format!("{}{path}", c.url),
                "application/json",
                &c.access_token,
            )
            .map_err(|e| unreachable(&c.url, e))
    }
}

impl Transport for Link {
    fn request(&self, method: &str, path: &str, _body: Value) -> Result<Answer> {
        let c = self.current(false)?;
        let mut r = self.send(&c, method, path)?;
        if r.status == 401 {
            let c = self.current(true)?;
            r = self.send(&c, method, path)?;
        }
        let body = serde_json::from_str(&r.body).unwrap_or(Value::String(r.body));
        Ok(Answer {
            status: r.status,
            body,
            sealed: false,
        })
    }
}

pub fn put(keychain: &dyn Keychain, companion: &Companion) -> Result<()> {
    let json = serde_json::to_string(companion).expect("a companion serialises");
    keychain.store(
        &[
            ("service", SERVICE),
            ("kind", KIND),
            ("url", &companion.url),
        ],
        &tr!("tui.agents.keychainLabel", url = &companion.url),
        &json,
    )
}

pub fn get(keychain: &dyn Keychain, url: &str) -> Result<Option<Companion>> {
    Ok(keychain
        .lookup(&[("service", SERVICE), ("kind", KIND), ("url", url)])?
        .and_then(|s| serde_json::from_str(&s).ok()))
}

/// Every companion this computer keeps, by URL.
pub fn list(keychain: &dyn Keychain) -> Result<Vec<Companion>> {
    let mut all: Vec<Companion> = keychain
        .search(&[("service", SERVICE), ("kind", KIND)])?
        .into_iter()
        .filter_map(|i| serde_json::from_str(&i.secret).ok())
        .collect();
    all.sort_by(|a, b| a.url.cmp(&b.url));
    Ok(all)
}

pub fn forget(keychain: &dyn Keychain, url: &str) -> Result<()> {
    keychain.clear(&[("service", SERVICE), ("kind", KIND), ("url", url)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_with_the_code_in_it_comes_first() {
        let mut code: DeviceCode = serde_json::from_value(serde_json::json!({
            "device_code": "authelia_dc_x",
            "user_code": "RDQXQRJZ",
            "verification_uri": "https://box.local/auth/consent/openid/device-authorization",
            "verification_uri_complete": "https://box.local/auth/consent/openid/device-authorization?user_code=RDQXQRJZ",
            "interval": 10,
            "expires_in": 599,
        }))
        .unwrap();
        assert!(code.link().ends_with("?user_code=RDQXQRJZ"));
        code.verification_uri_complete = None;
        assert!(code.link().ends_with("/device-authorization"));
        let bare: DeviceCode = serde_json::from_value(serde_json::json!({
            "device_code": "d", "user_code": "u", "verification_uri": "https://b/x",
        }))
        .unwrap();
        assert_eq!((bare.interval, bare.expires_in), (5, 600));
    }

    #[test]
    fn a_form_is_escaped() {
        assert_eq!(
            form(&[("scope", SCOPE), ("client_id", CLIENT_ID)]),
            "scope=openid+profile+offline_access&client_id=losos-companion"
        );
    }
}
