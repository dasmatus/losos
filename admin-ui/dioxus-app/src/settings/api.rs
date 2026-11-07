//! Settings API helpers.

use super::model::{ChangeResponse, OptionsResponse, SettingsResponse, StatusResponse};
use super::nix::build_overrides_nix;
use super::Extras;
use crate::api::{ApiError, BoxApi};
use crate::store::BoxConfig;
use losos_client::Trust;

pub fn api_for_config(config: &BoxConfig) -> Result<BoxApi, ApiError> {
    let trust = if let Some(pin) = config.tls_pin.as_deref().filter(|s| !s.is_empty()) {
        Trust::Pinned(pin.to_string())
    } else {
        Trust::FirstUse
    };
    BoxApi::new(&config.host, &config.token, trust)
}

pub async fn get_settings(config: &BoxConfig) -> Result<SettingsResponse, ApiError> {
    let api = api_for_config(config)?;
    let v = api.get("/api/settings").await?;
    let s: SettingsResponse = serde_json::from_value(v).map_err(|e| ApiError {
        status: 0,
        message: format!("settings: {e}"),
    })?;
    // normalize like React: missing sharingLocally is false (serde default), times validated at use
    Ok(s)
}

pub async fn get_options(config: &BoxConfig) -> Result<OptionsResponse, ApiError> {
    let api = api_for_config(config)?;
    let v = api.get("/api/options").await?;
    serde_json::from_value(v).map_err(|e| ApiError {
        status: 0,
        message: format!("options: {e}"),
    })
}

pub async fn get_status(config: &BoxConfig) -> Result<StatusResponse, ApiError> {
    let api = api_for_config(config)?;
    let v = api.get("/api/status").await?;
    serde_json::from_value(v).map_err(|e| ApiError {
        status: 0,
        message: format!("status: {e}"),
    })
}

pub async fn post_apply(
    config: &BoxConfig,
    settings: &SettingsResponse,
    extra: &Extras,
) -> Result<ChangeResponse, ApiError> {
    let api = api_for_config(config)?;
    let nix = build_overrides_nix(settings, extra);
    let v = api
        .post_raw("/api/apply", nix, "text/plain; charset=utf-8")
        .await?;
    serde_json::from_value(v).map_err(|e| ApiError {
        status: 0,
        message: format!("apply: {e}"),
    })
}

pub async fn post_factory_reset(config: &BoxConfig) -> Result<serde_json::Value, ApiError> {
    let api = api_for_config(config)?;
    api.post("/api/factory-reset").await
}

/// Extract extra Nix literals from /api/options (set overrides not owned by the 13 keys, plus strays).
pub fn extras_from_options(doc: &OptionsResponse) -> Extras {
    let owned = super::model::owned_name_set();
    let mut extra = Extras::new();
    for opt in &doc.options {
        let Some(name) = opt.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        if owned.contains(name) {
            continue;
        }
        if let Some(set) = opt.get("set").and_then(|v| v.as_str()) {
            if !set.is_empty() {
                extra.insert(name.to_string(), set.to_string());
            }
        } else if let Some(lit) = opt.get("literal").and_then(|v| v.as_str()) {
            if !lit.is_empty() {
                extra.insert(name.to_string(), lit.to_string());
            }
        }
    }
    extra
}

/// Best-effort stray extraction from a raw options JSON value.
pub fn extras_from_options_value(doc: &serde_json::Value, base: &mut Extras) {
    let owned = super::model::owned_name_set();
    if let Some(strays) = doc.get("stray").and_then(|v| v.as_array()) {
        for s in strays {
            let key = s.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let val = s.get("value").and_then(|v| v.as_str()).unwrap_or("");
            if !key.is_empty() && !owned.contains(key) && !val.is_empty() {
                base.insert(key.to_string(), val.to_string());
            }
        }
    }
    if let Some(opts) = doc.get("options").and_then(|v| v.as_array()) {
        for opt in opts {
            let Some(name) = opt.get("name").and_then(|v| v.as_str()) else { continue };
            if owned.contains(name) {
                continue;
            }
            if let Some(set) = opt.get("set").and_then(|v| v.as_str()) {
                if !set.is_empty() {
                    base.insert(name.to_string(), set.to_string());
                }
            } else if let Some(lit) = opt.get("literal").and_then(|v| v.as_str()) {
                if !lit.is_empty() {
                    base.insert(name.to_string(), lit.to_string());
                }
            }
        }
    }
}
