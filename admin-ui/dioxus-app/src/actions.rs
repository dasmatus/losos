//! Action layer: binds frontend actions to the `losos` TUI/backend API surface.
//!
//! Middle layer between UI and backend, mirroring TUI command structure.
//! Actions map to `/api/*` calls via `BoxApi` and can be extended for native
//! `losos` CLI execution on desktop.

use crate::api::{ApiError, BoxApi};
use crate::store::BoxConfig;
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
}

fn make_api(config: &BoxConfig) -> Result<BoxApi, ApiError> {
    let trust = if let Some(pin) = config.tls_pin.as_deref().filter(|s| !s.is_empty()) {
        Trust::Pinned(pin.to_string())
    } else {
        Trust::Permissive
    };
    BoxApi::new(&config.host, &config.token, trust)
}

pub async fn dispatch(config: &BoxConfig, action: Action) -> Result<Value, ApiError> {
    let api = make_api(config)?;
    match action {
        Action::Overview => api.get("/api/overview").await,
        Action::CloudStatus => api.get("/api/cloud/status").await,
        Action::AppsList => api.get("/api/market").await,
        Action::MarketInstall(id) => api.post_json("/api/market/install", json_obj(&[("id", &id)])).await,
        Action::StorageStatus => api.get("/api/storage/status").await,
        Action::StorageGrow => api.post_json("/api/grow", serde_json::json!({})).await,
        Action::MeshStatus => api.get("/api/mesh/status").await,
        Action::MeshEnable => api.post_json("/api/mesh/enable", serde_json::json!({})).await,
        Action::MeshEnableStorage => api.post_json("/api/mesh/enable-storage", serde_json::json!({})).await,
        Action::MeshPin(cid) => api.post_json("/api/mesh/pin", json_obj(&[("cid", &cid)])).await,
        Action::MeshUnpin(cid) => api.post_json("/api/mesh/unpin", json_obj(&[("cid", &cid)])).await,
        Action::MeshGC => api.post_json("/api/mesh/gc", serde_json::json!({})).await,
        Action::MeshHistory => api.get("/api/mesh/history").await,
        Action::ShareWith(id) => api.post_json("/api/cloud/share", json_obj(&[("with", &id)])).await,
        Action::BackupStatus => api.get("/api/backup/status").await,
        Action::About => api.get("/api/about").await,
        Action::UsersList => api.get("/api/users").await,
        Action::Get(path) => api.get(&path).await,
        Action::Post(path, body) => api.post_json(&path, body).await,
    }
}

fn json_obj(pairs: &[(&str, &str)]) -> Value {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), Value::String((*v).to_string()));
    }
    Value::Object(m)
}
