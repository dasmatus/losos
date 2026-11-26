//! LosOS Login's device-code sign-in (RFC 8628), the way the companion and
//! the terminal both use it: a public client, a link and a code, then an
//! access token and a refresh token.
//!
//! The words of a failure stay with the caller. This module only says which
//! kind of failure it was.

use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use crate::http::{Http, HttpError, Response};

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

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

/// The two tokens LosOS Login handed out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Granted {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

#[derive(Debug)]
pub enum OauthError {
    Transport(HttpError),
    /// The device-code request itself was refused.
    Refused(u16),
    NoRefresh,
    Denied,
    Expired,
    Failed {
        status: u16,
        detail: String,
    },
    BadJson(String),
    /// A refresh was refused. The sign-in is over.
    Refresh,
}

#[derive(Debug, Deserialize)]
struct Tokens {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

fn form(pairs: &[(&str, &str)]) -> String {
    let mut out = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        out.append_pair(k, v);
    }
    out.finish()
}

fn post_form(http: &Http, url: &str, pairs: &[(&str, &str)]) -> Result<Response, OauthError> {
    http.post(
        url,
        "application/x-www-form-urlencoded",
        "application/json",
        &form(pairs),
    )
    .map_err(OauthError::Transport)
}

fn oauth_error(r: &Response) -> String {
    serde_json::from_str::<Value>(&r.body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

fn granted(body: &str, now: i64, previous: Option<&str>) -> Result<Granted, OauthError> {
    let t: Tokens = serde_json::from_str(body).map_err(|e| OauthError::BadJson(e.to_string()))?;
    let refresh = t
        .refresh_token
        .or_else(|| previous.map(str::to_string))
        .ok_or(OauthError::NoRefresh)?;
    Ok(Granted {
        access_token: t.access_token,
        refresh_token: refresh,
        expires_at: now + t.expires_in.unwrap_or(3600),
    })
}

/// Ask LosOS Login for a device code for `client_id`.
pub fn start(
    http: &Http,
    url: &str,
    client_id: &str,
    scope: &str,
) -> Result<DeviceCode, OauthError> {
    let r = post_form(
        http,
        &format!("{url}/auth/api/oidc/device-authorization"),
        &[("client_id", client_id), ("scope", scope)],
    )?;
    if r.status != 200 {
        return Err(OauthError::Refused(r.status));
    }
    serde_json::from_str(&r.body).map_err(|e| OauthError::BadJson(e.to_string()))
}

/// Wait until the person approves `code`. `now` is Unix seconds.
pub fn finish(
    http: &Http,
    url: &str,
    client_id: &str,
    code: &DeviceCode,
    now: impl Fn() -> i64,
) -> Result<Granted, OauthError> {
    let mut interval = code.interval;
    let deadline = now() + i64::try_from(code.expires_in).unwrap_or(600);
    loop {
        let r = post_form(
            http,
            &format!("{url}/auth/api/oidc/token"),
            &[
                ("grant_type", DEVICE_GRANT),
                ("device_code", &code.device_code),
                ("client_id", client_id),
            ],
        )?;
        if r.status == 200 {
            return granted(&r.body, now(), None);
        }
        match oauth_error(&r).as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "access_denied" => return Err(OauthError::Denied),
            "expired_token" => return Err(OauthError::Expired),
            other => {
                return Err(OauthError::Failed {
                    status: r.status,
                    detail: other.to_string(),
                })
            }
        }
        if now() > deadline {
            return Err(OauthError::Expired);
        }
        std::thread::sleep(Duration::from_secs(interval));
    }
}

/// A new pair from `refresh_token`. LosOS Login rotates it.
pub fn refresh(
    http: &Http,
    url: &str,
    client_id: &str,
    refresh_token: &str,
    now: i64,
) -> Result<Granted, OauthError> {
    let r = post_form(
        http,
        &format!("{url}/auth/api/oidc/token"),
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ],
    )?;
    if r.status != 200 {
        return Err(OauthError::Refresh);
    }
    granted(&r.body, now, Some(refresh_token))
}

/// Hand both tokens back (RFC 7009). False when LosOS Login did not answer.
pub fn revoke(http: &Http, url: &str, client_id: &str, access: &str, refresh_token: &str) -> bool {
    let mut ended = true;
    for (token, hint) in [(refresh_token, "refresh_token"), (access, "access_token")] {
        let answer = post_form(
            http,
            &format!("{url}/auth/api/oidc/revocation"),
            &[
                ("token", token),
                ("token_type_hint", hint),
                ("client_id", client_id),
            ],
        );
        ended &= answer.is_ok_and(|r| r.status == 200);
    }
    ended
}
