//! Auth/gate state and setup/sign-in helpers aligned with the React admin UI.
//!
//! The gate decides which screen to show: unknown (checking), setup (unclaimed),
//! signin (claimed, not authenticated), or open (authenticated). Token handling
//! uses sessionStorage on web (`losos-token`, React-compatible key) and the
//! persisted config on desktop.

use crate::api::{ApiError, BoxApi};
use crate::store::{self, BoxConfig};
use losos_client::Trust;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const TOKEN_KEY: &str = "losos-token";

/// Auth gate state, mirroring the React app's gate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gate {
    /// Claim/auth state not known yet.
    #[default]
    Unknown,
    /// Box is unclaimed and needs first-time setup.
    Setup,
    /// Box is claimed but this client is not signed in.
    Signin,
    /// Authenticated and ready.
    Open,
}

/// GET /api/setup/claim response (public).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaimState {
    pub claimed: bool,
    #[serde(default)]
    pub ready: Option<bool>,
    #[serde(default)]
    pub waiting_for: Option<String>,
    #[serde(default)]
    pub people: Option<bool>,
}

/// POST /api/setup/claim response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaimResponse {
    pub claimed: bool,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    pub token: String,
    #[serde(default)]
    pub replayed: Option<bool>,
}

/// Minimal /api/me shape used by the frontend foundation.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub spare_key: bool,
    #[serde(default)]
    pub sections: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub standalone: Option<bool>,
    #[serde(default)]
    pub shared: Option<Vec<String>>,
    /// Login-lock reasons (LosOS Login): when non-empty, the person is held to
    /// their own account settings until they comply.
    #[serde(default)]
    pub locked: Vec<Value>,
}

impl Me {
    pub fn is_held(&self) -> bool {
        !self.locked.is_empty()
    }
}

impl Me {
    pub fn is_authenticated_response(&self) -> bool {
        true
    }

    pub fn label(&self) -> String {
        if self.spare_key {
            "Spare key".to_string()
        } else {
            self.display_name
                .clone()
                .or_else(|| self.name.clone())
                .unwrap_or_else(|| "Signed in".to_string())
        }
    }
}

fn api_for_config(config: &BoxConfig) -> Result<BoxApi, ApiError> {
    let trust = match config.tls_pin.as_deref().filter(|s| !s.is_empty()) {
        Some(pin) => Trust::Pinned(pin.to_string()),
        None => Trust::Permissive,
    };
    BoxApi::new(&config.host, &config.token, trust)
}

pub fn has_token(config: &BoxConfig) -> bool {
    !effective_token(config).trim().is_empty()
}

pub fn effective_token(config: &BoxConfig) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(t) = load_token_web() {
            if !t.trim().is_empty() {
                return t;
            }
        }
    }
    config.token.clone()
}

#[cfg(target_arch = "wasm32")]
pub fn load_token_web() -> Option<String> {
    let win = web_sys::window()?;
    win.session_storage().ok()??.get_item(TOKEN_KEY).ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn save_token_web(token: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = storage.set_item(TOKEN_KEY, token);
    }
}

#[cfg(target_arch = "wasm32")]
pub fn drop_token_web() {
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = storage.remove_item(TOKEN_KEY);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_token_web() -> Option<String> {
    None
}
#[cfg(not(target_arch = "wasm32"))]
pub fn save_token_web(_token: &str) {}
#[cfg(not(target_arch = "wasm32"))]
pub fn drop_token_web() {}

/// Persist token in React-aligned storage (sessionStorage on web) and in config.
pub fn save_token(config: &mut BoxConfig, token: String) {
    config.token = token.clone();
    save_token_web(&token);
    store::save(config);
}

pub fn drop_token(config: &mut BoxConfig) {
    config.token.clear();
    drop_token_web();
    store::save(config);
}

/// GET /api/setup/claim (anonymous/public).
pub async fn get_claim_state(config: &BoxConfig) -> Result<ClaimState, ApiError> {
    let api = api_for_config(&BoxConfig {
        token: String::new(),
        ..config.clone()
    })?;
    let v = api.get_anonymous("/api/setup/claim").await?;
    serde_json::from_value(v).map_err(|_| ApiError {
        status: 0,
        message: "The box sent a setup response that was not understood".into(),
    })
}

/// POST /api/setup/claim (anonymous). Creates/claims first setup on an unclaimed box.
pub async fn claim_box(
    config: &BoxConfig,
    name: &str,
    password: &str,
) -> Result<ClaimResponse, ApiError> {
    let api = api_for_config(&BoxConfig {
        token: String::new(),
        ..config.clone()
    })?;
    let v = api
        .post_json(
            "/api/setup/claim",
            serde_json::json!({ "name": name, "password": password }),
        )
        .await?;
    serde_json::from_value(v).map_err(|_| ApiError {
        status: 0,
        message: "The box sent a claim response that was not understood".into(),
    })
}

/// GET /api/me. Returns None on 401.
pub async fn load_me(config: &BoxConfig) -> Result<Option<Me>, ApiError> {
    let mut cfg = config.clone();
    cfg.token = effective_token(config);
    let api = api_for_config(&cfg)?;
    match api.get("/api/me").await {
        Ok(v) => parse_me(v),
        Err(e) if e.status == 401 => Ok(None),
        Err(e) => Err(e),
    }
}

fn parse_me(v: Value) -> Result<Option<Me>, ApiError> {
    // Require a recognizable /api/me payload.
    let has_sections = v.get("sections").and_then(Value::as_array).is_some();
    let has_admin = v.get("admin").is_some();
    if !has_sections || !has_admin {
        return Ok(None);
    }
    serde_json::from_value(v)
        .map(Some)
        .map_err(|_| ApiError {
            status: 0,
            message: "The box sent an identity response that was not understood".into(),
        })
}

/// Validate a bearer token by probing an authenticated route, then load /api/me.
/// Returns `Ok(Some(me))` on success, `Ok(None)` if the token is rejected.
pub async fn sign_in(config: &BoxConfig, token: &str) -> Result<Option<Me>, ApiError> {
    let mut probe = config.clone();
    probe.token = token.trim().to_string();
    let api = api_for_config(&probe)?;
    match api.get("/api/state").await {
        Ok(_) => {}
        Err(e) if e.status == 401 => return Ok(None),
        Err(e) => return Err(e),
    }
    match load_me(&probe).await {
        Ok(Some(me)) => Ok(Some(me)),
        // `/api/me` returned 401 or an unrecognized payload: do not grant a
        // synthetic admin/spare-key identity.
        Ok(None) => Ok(None),
        // Transport/parse issues after a successful probe: surface the error
        // rather than mislabeling permissions.
        Err(e) => Err(e),
    }
}

pub fn is_unauthorized(err: &ApiError) -> bool {
    err.status == 401
}
