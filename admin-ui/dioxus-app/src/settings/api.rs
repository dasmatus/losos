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
            let Some(name) = opt.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
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

// ── Own-request pane helpers (Slice 5) ──────────────────────────────────────
// Typed wrappers for pane-owned endpoints. These do NOT participate in the
// draft/apply engine; each pane issues its own requests.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap as Map;

/// A 404/501 means the box does not serve that route yet (older lososd).
pub fn not_present(err: &ApiError) -> bool {
    err.status == 404 || err.status == 501
}

async fn parse_json<T: for<'de> Deserialize<'de>>(v: Value, ctx: &str) -> Result<T, ApiError> {
    serde_json::from_value(v).map_err(|e| ApiError {
        status: 0,
        message: format!("{ctx}: {e}"),
    })
}

fn method_str(m: &str) -> reqwest::Method {
    match m {
        "DELETE" => reqwest::Method::DELETE,
        "PUT" => reqwest::Method::PUT,
        "POST" => reqwest::Method::POST,
        _ => reqwest::Method::GET,
    }
}

/// Low-level request for methods BoxApi does not expose (DELETE/PUT), using
/// config directly for base/token/trust. Adds new capability without rewriting
/// existing helpers.

pub async fn request_json_with_config(
    config: &BoxConfig,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, ApiError> {
    let base = if config.host.starts_with("http://") || config.host.starts_with("https://") {
        config.host.trim_end_matches('/').to_string()
    } else if config.host.is_empty() {
        String::new()
    } else {
        format!("https://{}", config.host.trim_end_matches('/'))
    };
    let url = format!("{base}{path}");
    #[cfg(not(target_arch = "wasm32"))]
    let client = {
        let trust = if let Some(pin) = config.tls_pin.as_deref().filter(|s| !s.is_empty()) {
            Trust::Pinned(pin.to_string())
        } else {
            Trust::FirstUse
        };
        let danger = matches!(trust, Trust::Permissive | Trust::FirstUse);
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(4))
            .timeout(std::time::Duration::from_secs(30))
            .user_agent(format!("losos-frontend/{}", env!("CARGO_PKG_VERSION")))
            .danger_accept_invalid_certs(danger)
            .build()
            .map_err(|e| ApiError {
                status: 0,
                message: e.to_string(),
            })?
    };
    #[cfg(target_arch = "wasm32")]
    let client = reqwest::Client::builder().build().map_err(|e| ApiError {
        status: 0,
        message: e.to_string(),
    })?;
    let mut req = client.request(method_str(method), &url);
    #[cfg(not(target_arch = "wasm32"))]
    {
        req = req.timeout(std::time::Duration::from_secs(30));
    }
    if !config.token.trim().is_empty() {
        req = req.bearer_auth(config.token.trim());
    }
    if let Some(b) = body {
        if !b.is_null() {
            req = req.json(&b);
        }
    }
    send_json(req).await
}

async fn send_json(req: reqwest::RequestBuilder) -> Result<Value, ApiError> {
    let resp = req.send().await.map_err(|e| ApiError {
        status: 0,
        message: format!("No answer: {e}"),
    })?;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();
    if status == 401 {
        return Err(ApiError {
            status,
            message: "The box wants its token again".into(),
        });
    }
    if !(200..300).contains(&status) {
        let msg = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(ApiError { status, message: msg });
    }
    if text.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&text).map_err(|_| ApiError {
        status,
        message: "The box sent a response that was not JSON".into(),
    })
}

// ── Access (terminal tokens) ────────────────────────────────────────────────

pub type AccessScope = String;
pub type AccessExpiry = Option<i64>;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessKey {
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub user_ids: Vec<String>,
    pub created: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessTokenRow {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub created: i64,
    pub expires: Option<i64>,
    pub last_used: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessHostKey {
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessResponse {
    pub host_key: Option<AccessHostKey>,
    #[serde(default)]
    pub keys: Vec<AccessKey>,
    #[serde(default)]
    pub tokens: Vec<AccessTokenRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessTokenCreated {
    #[serde(default)]
    pub token: String,
    pub record: AccessTokenRow,
}

pub async fn get_access(config: &BoxConfig) -> Result<AccessResponse, ApiError> {
    let api = api_for_config(config)?;
    parse_json(api.get("/api/access").await?, "access").await
}

pub async fn post_access_token(
    config: &BoxConfig,
    name: &str,
    fingerprint: &str,
    scope: &str,
    expires_in_days: AccessExpiry,
) -> Result<AccessTokenCreated, ApiError> {
    let api = api_for_config(config)?;
    let body = serde_json::json!({
        "name": name,
        "fingerprint": fingerprint,
        "scope": scope,
        "expiresInDays": expires_in_days,
    });
    parse_json(api.post_json("/api/access/tokens", body).await?, "access token").await
}

pub async fn post_access_revoke(config: &BoxConfig, id: &str) -> Result<Value, ApiError> {
    let api = api_for_config(config)?;
    api.post_json("/api/access/tokens/revoke", serde_json::json!({ "id": id }))
        .await
}

// ── Backup ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoughtStorage {
    #[serde(default)]
    pub order: String,
    #[serde(default)]
    pub edge: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupTarget {
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default)]
    pub has_secret: bool,
    pub storage_class: Option<String>,
    pub bought: Option<BoughtStorage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupReport {
    #[serde(default)]
    pub time: i64,
    #[serde(default)]
    pub snapshot: String,
    #[serde(default)]
    pub files: i64,
    #[serde(default)]
    pub bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupJob {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub job: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub started_at: i64,
    pub finished_at: Option<i64>,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EraseOutside {
    #[serde(default)]
    pub had_edge: bool,
    #[serde(default)]
    pub domains_removed: i64,
    #[serde(default)]
    pub listings_closed: i64,
    #[serde(default)]
    pub left_edge: bool,
    #[serde(default)]
    pub problems: Vec<String>,
    #[serde(default)]
    pub erased_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EraseState {
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub backup: bool,
    #[serde(default)]
    pub started_at: i64,
    pub deadline: Option<i64>,
    pub seconds_left: Option<i64>,
    #[serde(default)]
    pub cancellable: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub outside: EraseOutside,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupResponse {
    pub target: Option<BackupTarget>,
    pub last: Option<BackupReport>,
    pub job: Option<BackupJob>,
    pub erase: Option<EraseState>,
    pub last_erase: Option<EraseOutside>,
    #[serde(default)]
    pub grace_seconds: i64,
}

pub async fn get_backup(config: &BoxConfig) -> Result<BackupResponse, ApiError> {
    let api = api_for_config(config)?;
    parse_json(api.get("/api/backup").await?, "backup").await
}

pub async fn post_backup_target(config: &BoxConfig, target: Value) -> Result<Value, ApiError> {
    let api = api_for_config(config)?;
    api.post_json("/api/backup/target", target).await
}

pub async fn post_backup_target_bought(config: &BoxConfig, order: &str) -> Result<Value, ApiError> {
    let api = api_for_config(config)?;
    api.post_json("/api/backup/target/bought", serde_json::json!({ "order": order }))
        .await
}

pub async fn delete_backup_target(config: &BoxConfig) -> Result<Value, ApiError> {
    request_json_with_config(config, "DELETE", "/api/backup/target", None).await
}

pub async fn post_backup_run(config: &BoxConfig) -> Result<Value, ApiError> {
    api_for_config(config)?.post("/api/backup/run").await
}

pub async fn post_backup_restore(config: &BoxConfig, code: &str) -> Result<Value, ApiError> {
    let api = api_for_config(config)?;
    api.post_json("/api/backup/restore", serde_json::json!({ "code": code }))
        .await
}

pub async fn get_recovery_code(config: &BoxConfig) -> Result<String, ApiError> {
    let api = api_for_config(config)?;
    let v = api.get("/api/recovery").await?;
    Ok(v.get("code").and_then(Value::as_str).unwrap_or("").to_string())
}

// ── People / Users ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoginRules {
    #[serde(default)]
    pub two_factor: bool,
    #[serde(default)]
    pub password_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomRole {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hold {
    #[serde(default)]
    pub at: i64,
    #[serde(default)]
    pub by: String,
    pub until: Option<i64>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgeMark {
    #[serde(default)]
    pub age: String,
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub disabled: bool,
    pub two_factor: Option<bool>,
    pub password_set_at: Option<i64>,
    pub locked: Option<Vec<String>>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub accent: Option<String>,
    pub role: Option<String>,
    pub role_name: Option<String>,
    pub muted: Option<Hold>,
    pub suspended: Option<Hold>,
    pub age: Option<AgeMark>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeopleResponse {
    #[serde(default)]
    pub standalone: bool,
    pub you: Option<String>,
    #[serde(default)]
    pub sections: Vec<String>,
    pub rules: Option<LoginRules>,
    pub roles: Option<Vec<CustomRole>>,
    #[serde(default)]
    pub people: Vec<PersonEntry>,
}

pub async fn get_people(config: &BoxConfig) -> Result<PeopleResponse, ApiError> {
    let api = api_for_config(config)?;
    parse_json(api.get("/api/people").await?, "people").await
}

// ── Apps / Catalogue ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSource {
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub kind: String,
    pub chart: Option<AppSource>,
    pub title: Option<String>,
    #[serde(default, rename = "runAs")]
    pub run_as: String,
    #[serde(default)]
    pub runtime: String,
    #[serde(default)]
    pub values: Map<String, Value>,
    pub values_yaml: Option<String>,
    #[serde(default, rename = "frontPort")]
    pub front_port: i64,
    pub app_port: Option<i64>,
    #[serde(default)]
    pub phase: String,
    pub message: Option<String>,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppsResponse {
    #[serde(default)]
    pub available: bool,
    pub reason: Option<String>,
    #[serde(default)]
    pub shared_available: bool,
    pub kindashared_available: Option<bool>,
    #[serde(default)]
    pub apps: Vec<InstalledApp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueApp {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub homepage: Option<String>,
    pub chart: Option<AppSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueResults {
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub results: Vec<CatalogueApp>,
}

pub async fn get_apps(config: &BoxConfig) -> Result<AppsResponse, ApiError> {
    let api = api_for_config(config)?;
    parse_json(api.get("/api/apps").await?, "apps").await
}

pub async fn post_app_remove(config: &BoxConfig, release: &str) -> Result<InstalledApp, ApiError> {
    let api = api_for_config(config)?;
    parse_json(
        api.post_json("/api/apps/remove", serde_json::json!({ "release": release }))
            .await?,
        "app remove",
    )
    .await
}

pub async fn post_app_install(config: &BoxConfig, request: Value) -> Result<InstalledApp, ApiError> {
    let api = api_for_config(config)?;
    parse_json(
        api.post_json("/api/apps/install", request).await?,
        "app install",
    )
    .await
}

pub async fn search_catalogue(config: &BoxConfig, query: &str) -> Result<CatalogueResults, ApiError> {
    let path = format!("/api/apps/search?q={}", urlencoding::encode(query));
    let api = api_for_config(config)?;
    parse_json(api.get(&path).await?, "catalogue").await
}

// ── Look ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LookBackground {
    #[serde(default)]
    pub kind: String,
    pub name: Option<String>,
    pub version: Option<i64>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LookLimits {
    #[serde(default = "default_widgets")]
    pub widgets: i64,
    #[serde(default = "default_veil_min")]
    pub veil_min: i64,
    #[serde(default = "default_veil_max")]
    pub veil_max: i64,
}
fn default_widgets() -> i64 {
    24
}
fn default_veil_min() -> i64 {
    20
}
fn default_veil_max() -> i64 {
    90
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LookResponse {
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub background: LookBackground,
    #[serde(default = "default_veil")]
    pub veil: i64,
    #[serde(default)]
    pub personal: bool,
    #[serde(default)]
    pub widgets: Vec<Value>,
    #[serde(default)]
    pub shipped: Vec<String>,
    pub limits: Option<Value>,
}
fn default_veil() -> i64 {
    60
}

pub async fn get_look(config: &BoxConfig) -> Result<LookResponse, ApiError> {
    let api = api_for_config(config)?;
    parse_json(api.get("/api/look").await?, "look").await
}

pub async fn post_look(config: &BoxConfig, patch: Value) -> Result<LookResponse, ApiError> {
    let api = api_for_config(config)?;
    parse_json(api.post_json("/api/look", patch).await?, "look").await
}

pub async fn delete_look_background(config: &BoxConfig) -> Result<LookResponse, ApiError> {
    parse_json(
        request_json_with_config(config, "DELETE", "/api/look/background", None).await?,
        "look background",
    )
    .await
}
