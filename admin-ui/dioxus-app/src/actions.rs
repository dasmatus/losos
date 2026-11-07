//! Action layer: binds frontend actions to the `losos` TUI/backend API surface.
//!
//! Middle layer between UI and backend, mirroring TUI command structure.
//! Actions map to `/api/*` calls via `BoxApi` and can be extended for native
//! `losos` CLI execution on desktop.

use crate::api::{ApiError, BoxApi};
use crate::auth;
use crate::middle_layer::resources as res;
use crate::store::{self, BoxConfig};
use losos_client::Trust;
use serde_json::Value;

/// Core actions available from the admin UI / losos help surface.
/// Incremental mapping; extend as screens are ported.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Overview,
    CloudStatus,
    AppsList,
    MarketInstall(String),
    StorageStatus,
    StorageGrow,
    MeshStatus,
    MeshEnable,
    MeshEnableStorage,
    MeshPin(String),
    MeshUnpin(String),
    MeshGC,
    MeshHistory,
    ShareWith(String),
    BackupStatus,
    About,
    UsersList,
    // Generic escape hatch for not-yet-modeled endpoints.
    Get(String),
    Post(String, Value),
    // --- Credentials (middle-layer extension; do not refactor existing variants) ---
    CredentialsList,
    CredentialsRotate(String),
}

pub(crate) fn make_api(config: &BoxConfig) -> Result<BoxApi, ApiError> {
    let trust = if let Some(pin) = config.tls_pin.as_deref().filter(|s| !s.is_empty()) {
        Trust::Pinned(pin.to_string())
    } else {
        // TOFU: accept and record the certificate on first use; callers can persist
        // the observed pin. Prefer an explicit pin over implicit trust.
        Trust::FirstUse
    };
    BoxApi::new(&config.host, &config.token, trust)
}

/// Dispatch an action and, on native TOFU (no configured pin), persist the
/// observed certificate hash on success so subsequent requests use pinning.
pub async fn dispatch_with_pin(config: &mut BoxConfig, action: Action) -> Result<Value, ApiError> {
    let api = make_api(config)?;
    let res = dispatch_via(&api, action).await;
    if res.is_ok() && config.tls_pin.as_deref().unwrap_or("").is_empty() {
        if let Some(pin) = api.observed_tls_pin() {
            if !pin.is_empty() {
                config.tls_pin = Some(pin);
                store::save(config);
            }
        }
    }
    res
}

async fn dispatch_via(api: &BoxApi, action: Action) -> Result<Value, ApiError> {
    match action {
        Action::Overview => api.get("/api/overview").await,
        Action::CloudStatus => api.get("/api/cloud/status").await,
        // Installed-apps contract (`GET /api/apps`); distinct from market shelf.
        Action::AppsList => api.get(res::apps_installed_path()).await,
        // Legacy/non-canonical: no TS endpoint for install-by-id; kept for caller
        // compatibility and documented in `middle_layer::resources`.
        Action::MarketInstall(id) => {
            api.post_json(res::MARKET_INSTALL_LEGACY_PATH, json_obj(&[("id", &id)]))
                .await
        }
        Action::StorageStatus => api.get(res::storage_status_path()).await,
        Action::StorageGrow => api.post_json(res::storage_grow_path(), serde_json::json!({})).await,
        Action::MeshStatus => api.get("/api/mesh/status").await,
        Action::MeshEnable => api.post_json("/api/mesh/enable", serde_json::json!({})).await,
        Action::MeshEnableStorage => api.post_json("/api/mesh/enable-storage", serde_json::json!({})).await,
        Action::MeshPin(cid) => api.post_json("/api/mesh/pin", json_obj(&[("cid", &cid)])).await,
        Action::MeshUnpin(cid) => api.post_json("/api/mesh/unpin", json_obj(&[("cid", &cid)])).await,
        Action::MeshGC => api.post_json("/api/mesh/gc", serde_json::json!({})).await,
        Action::MeshHistory => api.get("/api/mesh/history").await,
        Action::ShareWith(id) => api.post_json("/api/cloud/share", json_obj(&[("with", &id)])).await,
        Action::BackupStatus => api.get(res::backup_status_path()).await,
        Action::About => api.get("/api/about").await,
        // Canonical people/users contract (`GET /api/people`).
        Action::UsersList => api.get(res::users_list_path()).await,
        Action::Get(path) => api.get(res::normalize_api_path(&path)).await,
        Action::Post(path, body) => api.post_json(res::normalize_api_path(&path), body).await,
        // --- Credentials (middle-layer extension) ---
        Action::CredentialsList => api.get("/api/credentials").await,
        Action::CredentialsRotate(name) => crate::middle_layer::credentials::rotate_via_api(api, &name).await,
    }
}

pub fn is_unauthorized(err: &ApiError) -> bool {
    err.status == 401
}

/// Clear bearer auth on 401 and persist config. Callers should also update
/// UI/auth gate state (e.g. set gate to Signin) as appropriate.
pub fn on_unauthorized(config: &mut BoxConfig) {
    auth::drop_token(config);
    store::save(config);
}

pub async fn dispatch(config: &BoxConfig, action: Action) -> Result<Value, ApiError> {
    let api = make_api(config)?;
    dispatch_via(&api, action).await
}

fn json_obj(pairs: &[(&str, &str)]) -> Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), Value::String((*v).to_string()));
    }
    Value::Object(m)
}
