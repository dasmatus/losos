//! Middle layer for box credentials (`GET /api/credentials`, rotate).
//!
//! Typed models, name validation, API calls, and a small RSX helper for the
//! Settings pane. A credential value is never exposed by the API.

use crate::api::{ApiError, BoxApi};
use crate::store::BoxConfig;
use losos_client::Trust;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "gui")]
use dioxus::prelude::*;

/// Credential name shape accepted by the box (`[a-z0-9-]+`, non-empty).
pub fn validate_credential_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(ApiError {
            status: 400,
            message: format!("Invalid credential name: {name}"),
        });
    }
    Ok(())
}

/// `GET /api/credentials` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CredentialsResponse {
    /// What the credentials are sealed to (`tpm2` or `host`).
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub credentials: Vec<CredentialStatus>,
}

/// One credential row (everything but its value).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub description: String,
    /// `box` for one the box makes, `owner` for one a person gives.
    #[serde(default)]
    pub source: String,
    /// Whether the sealed file is present.
    #[serde(default)]
    pub set: bool,
    /// Unix seconds when it last changed.
    #[serde(default)]
    pub updated: Option<u64>,
    /// `tpm2` or `host`.
    #[serde(default, alias = "sealedWith")]
    pub sealed_with: String,
    #[serde(default)]
    pub services: Vec<String>,
    /// Whether a person may make a new value.
    #[serde(default)]
    pub rotate: bool,
    /// Pane where the owner sets it, when owner-provided.
    #[serde(default)]
    pub pane: Option<String>,
    #[serde(default)]
    pub remade: bool,
    #[serde(default)]
    pub adopted: bool,
}

/// `POST /api/credentials/{name}/rotate` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RotateResponse {
    #[serde(default)]
    pub credential: CredentialStatus,
}

fn api_for(config: &BoxConfig) -> Result<BoxApi, ApiError> {
    let trust = match config.tls_pin.as_deref().filter(|s| !s.is_empty()) {
        Some(pin) => Trust::Pinned(pin.to_string()),
        None => Trust::FirstUse,
    };
    BoxApi::new(&config.host, &config.token, trust)
}

fn clear_auth_on_401(config: &mut BoxConfig, err: &ApiError) {
    if err.status == 401 {
        crate::auth::drop_token(config);
    }
}

fn parse_credentials(v: Value) -> Result<CredentialsResponse, ApiError> {
    serde_json::from_value(v).map_err(|e| ApiError {
        status: 0,
        message: format!("The box sent a credentials response that was not understood: {e}"),
    })
}

/// List credentials (raw API call). 401 is returned as an error; caller may clear auth.
pub async fn list(api: &BoxApi) -> Result<CredentialsResponse, ApiError> {
    let v = api.get("/api/credentials").await?;
    parse_credentials(v)
}

/// List credentials and clear auth on 401.
pub async fn list_with_auth(config: &mut BoxConfig) -> Result<CredentialsResponse, ApiError> {
    let api = api_for(config)?;
    match list(&api).await {
        Ok(v) => {
            maybe_persist_tofu_pin(config, &api);
            Ok(v)
        }
        Err(e) => {
            clear_auth_on_401(config, &e);
            Err(e)
        }
    }
}

/// Internal rotate call used by `actions` dispatch and direct callers.
pub async fn rotate_via_api(api: &BoxApi, name: &str) -> Result<Value, ApiError> {
    validate_credential_name(name)?;
    let path = format!("/api/credentials/{}/rotate", name);
    api.post(&path).await
}

/// Rotate a credential (validates name). 401 is returned as an error.
pub async fn rotate(api: &BoxApi, name: &str) -> Result<CredentialStatus, ApiError> {
    let v = rotate_via_api(api, name).await?;
    let resp: RotateResponse = serde_json::from_value(v).map_err(|e| ApiError {
        status: 0,
        message: format!("The box sent a rotate response that was not understood: {e}"),
    })?;
    Ok(resp.credential)
}

/// Rotate a credential and clear auth on 401.
pub async fn rotate_with_auth(
    config: &mut BoxConfig,
    name: &str,
) -> Result<CredentialStatus, ApiError> {
    validate_credential_name(name)?;
    let api = api_for(config)?;
    match rotate(&api, name).await {
        Ok(v) => {
            maybe_persist_tofu_pin(config, &api);
            Ok(v)
        }
        Err(e) => {
            clear_auth_on_401(config, &e);
            Err(e)
        }
    }
}

fn maybe_persist_tofu_pin(config: &mut BoxConfig, api: &BoxApi) {
    if config.tls_pin.as_deref().unwrap_or("").is_empty() {
        if let Some(pin) = api.observed_tls_pin() {
            if !pin.is_empty() {
                config.tls_pin = Some(pin);
                crate::store::save(config);
            }
        }
    }
}

#[cfg(feature = "gui")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CredentialsPaneState {
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
    pub view: Option<CredentialsResponse>,
    pub busy: Option<String>,
    pub notice: Option<String>,
}

/// Small RSX helper for settings Credentials pane integration.
///
/// Renders the credentials list and rotate controls. Callers own config/auth
/// state updates; this helper focuses on presentation and local fetch/rotate
/// flow given a config snapshot and a setter.
#[cfg(feature = "gui")]
#[component]
pub fn CredentialsSection(
    config: BoxConfig,
    on_config: EventHandler<BoxConfig>,
    on_unauthorized: EventHandler<()>,
) -> Element {
    let mut state = use_signal(CredentialsPaneState::default);

    let load = {
        let config = config.clone();
        move |mut state: Signal<CredentialsPaneState>,
              on_config: EventHandler<BoxConfig>,
              on_unauthorized: EventHandler<()>| {
            let config = config.clone();
            spawn(async move {
                {
                    let mut s = state.write();
                    s.loading = true;
                    s.error = None;
                }
                let mut cfg = config;
                match list_with_auth(&mut cfg).await {
                    Ok(view) => {
                        on_config.call(cfg.clone());
                        let mut s = state.write();
                        s.loading = false;
                        s.loaded = true;
                        s.view = Some(view);
                    }
                    Err(e) => {
                        if e.status == 401 {
                            on_config.call(cfg);
                            on_unauthorized.call(());
                        }
                        let mut s = state.write();
                        s.loading = false;
                        s.loaded = true;
                        s.error = Some(e.message);
                    }
                }
            });
        }
    };

    {
        let load = load.clone();
        let on_config = on_config;
        let on_unauthorized = on_unauthorized;
        use_effect(move || {
            if !state().loaded && !state().loading {
                load(state, on_config, on_unauthorized);
            }
        });
    }

    let view = state().view.clone();
    let err = state().error.clone();
    let busy = state().busy.clone();
    let notice = state().notice.clone();

    rsx! {
        div { class: "credentials-section",
            div { class: "row",
                h4 { "Credentials" }
                button {
                    class: "btn",
                    disabled: state().loading,
                    onclick: {
                        let load = load.clone();
                        move |_| load(state, on_config, on_unauthorized)
                    },
                    if state().loading { "Loading…" } else { "Refresh" }
                }
            }
            if let Some(msg) = err {
                p { class: "status error", "{msg}" }
            }
            if let Some(msg) = notice {
                p { class: "status ok", "{msg}" }
            }
            if let Some(v) = view {
                p { class: "muted",
                    if v.key == "tpm2" { "Sealed to this box's TPM." } else { "Sealed to this box's host key." }
                }
                if v.credentials.is_empty() {
                    p { class: "muted", "No credentials on this box." }
                } else {
                    ul { class: "credentials-list",
                        for cred in v.credentials.iter() {
                            {
                                let name = cred.name.clone();
                                let can_rotate = cred.rotate;
                                let is_busy = busy.as_deref() == Some(name.as_str());
                                let set_label = if !cred.set {
                                    "not set"
                                } else if cred.source == "owner" {
                                    "set by owner"
                                } else {
                                    "set by box"
                                };
                                let cfg = config.clone();
                                rsx! {
                                    li { key: "{cred.name}", class: "credential-row",
                                        div { class: "credential-main",
                                            strong { "{cred.name}" }
                                            span { class: "muted", " — {set_label}" }
                                            if !cred.services.is_empty() {
                                                span { class: "muted", " · read by {cred.services.join(\", \")}" }
                                            }
                                            if cred.remade {
                                                span { class: "muted", " · remade" }
                                            }
                                            if cred.adopted {
                                                span { class: "muted", " · adopted" }
                                            }
                                        }
                                        if can_rotate {
                                            button {
                                                class: "btn",
                                                disabled: is_busy || busy.is_some(),
                                                onclick: move |_| {
                                                    let name = name.clone();
                                                    let mut cfg = cfg.clone();
                                                    spawn(async move {
                                                        state.write().busy = Some(name.clone());
                                                        state.write().notice = None;
                                                        state.write().error = None;
                                                        match rotate_with_auth(&mut cfg, &name).await {
                                                            Ok(row) => {
                                                                on_config.call(cfg.clone());
                                                                let mut s = state.write();
                                                                s.busy = None;
                                                                if row.services.is_empty() {
                                                                    s.notice = Some(format!("Rotated {name}."));
                                                                } else {
                                                                    s.notice = Some(format!(
                                                                        "Rotated {name}; restarted {}.",
                                                                        row.services.join(", ")
                                                                    ));
                                                                }
                                                                // Refresh list to reflect new state.
                                                                drop(s);
                                                                let mut cfg2 = cfg;
                                                                if let Ok(view) = list_with_auth(&mut cfg2).await {
                                                                    state.write().view = Some(view);
                                                                    on_config.call(cfg2);
                                                                }
                                                            }
                                                            Err(e) => {
                                                                if e.status == 401 {
                                                                    on_config.call(cfg);
                                                                    on_unauthorized.call(());
                                                                }
                                                                let mut s = state.write();
                                                                s.busy = None;
                                                                s.error = Some(e.message);
                                                            }
                                                        }
                                                    });
                                                },
                                                if is_busy { "Rotating…" } else { "Rotate" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if !state().loading && !state().loaded {
                p { class: "muted", "Credentials not loaded." }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_credential_name() {
        assert!(validate_credential_name("admin-token").is_ok());
        assert!(validate_credential_name("box42").is_ok());
        assert!(validate_credential_name("").is_err());
        assert!(validate_credential_name("Admin").is_err());
        assert!(validate_credential_name("bad_name").is_err());
        assert!(validate_credential_name("bad/name").is_err());
        assert!(validate_credential_name("../etc").is_err());
    }

    #[test]
    fn parses_credentials_response() {
        let v = serde_json::json!({
            "key": "tpm2",
            "credentials": [{
                "name": "admin-token",
                "app": "admin",
                "description": "Admin token",
                "source": "box",
                "set": true,
                "updated": 123,
                "sealedWith": "tpm2",
                "services": ["lososd.service"],
                "rotate": true,
                "pane": null,
                "remade": false,
                "adopted": false
            }]
        });
        let parsed = parse_credentials(v).unwrap();
        assert_eq!(parsed.key, "tpm2");
        assert_eq!(parsed.credentials.len(), 1);
        assert_eq!(parsed.credentials[0].name, "admin-token");
        assert!(parsed.credentials[0].rotate);
    }

    #[test]
    fn parses_rotate_response() {
        let v = serde_json::json!({
            "credential": {
                "name": "admin-token",
                "app": "admin",
                "description": "Admin token",
                "source": "box",
                "set": true,
                "updated": 456,
                "sealedWith": "tpm2",
                "services": ["lososd.service"],
                "rotate": true,
                "pane": null,
                "remade": false,
                "adopted": false
            }
        });
        let resp: RotateResponse = serde_json::from_value(v).unwrap();
        assert_eq!(resp.credential.name, "admin-token");
    }
}
