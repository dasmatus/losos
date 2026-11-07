//! Client-side config/state persistence.
//!
//! The companion stored config in Android SharedPreferences; the web uses
//! localStorage; desktop can use a config file. We abstract reads/writes and
//! provide best-effort platform implementations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxConfig {
    #[serde(default)]
    pub host: String,
    /// Bearer token. On web this is kept in sessionStorage (`losos-token`, React
    /// parity) and not persisted in localStorage config to reduce exposure.
    #[serde(default)]
    #[cfg_attr(target_arch = "wasm32", serde(skip_serializing))]
    pub token: String,
    #[serde(default = "default_true")]
    pub demo: bool,
    #[serde(default)]
    pub theme_mode: ThemeMode,
    /// SHA-256 of the pinned TLS cert (hex), when using TOFU/pinning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for BoxConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            token: String::new(),
            demo: true,
            theme_mode: ThemeMode::System,
            tls_pin: None,
        }
    }
}

impl BoxConfig {
    pub fn base(&self) -> String {
        if self.host.starts_with("http://") || self.host.starts_with("https://") {
            self.host.trim_end_matches('/').to_string()
        } else if self.host.is_empty() {
            String::new()
        } else {
            format!("https://{}", self.host.trim_end_matches('/'))
        }
    }

    pub fn needs_connect(&self) -> bool {
        if self.demo {
            self.host.trim().is_empty()
        } else {
            self.host.trim().is_empty() || self.token.trim().is_empty()
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const KEY: &str = "losos-frontend-config";

pub fn load() -> BoxConfig {
    let mut cfg = load_raw()
        .and_then(|s| serde_json::from_str::<BoxConfig>(&s).ok())
        .unwrap_or_default();
    #[cfg(target_arch = "wasm32")]
    {
        // Migrate away from persisting bearer tokens in localStorage (React parity:
        // sessionStorage `losos-token` is the web session source).
        if !cfg.token.is_empty() {
            cfg.token.clear();
            // Persist sanitized config without token.
            let _ = (|| -> Option<()> {
                let s = serde_json::to_string(&cfg).ok()?;
                save_raw(&s);
                Some(())
            })();
        }
    }
    cfg
}

pub fn save(config: &BoxConfig) {
    if let Ok(s) = serde_json::to_string(config) {
        save_raw(&s);
    }
}

#[cfg(target_arch = "wasm32")]
fn load_raw() -> Option<String> {
    let win = web_sys::window()?;
    win.local_storage().ok()??.get_item(KEY).ok()?
}

#[cfg(target_arch = "wasm32")]
fn save_raw(value: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(KEY, value);
    }
}

/// On web, the auth token is also stored in sessionStorage under `losos-token`
/// (React admin parity). This helper syncs the loaded config from that source
/// when present so bearer `BoxApi` and the gate see the active session token.
pub fn load_with_session_token() -> BoxConfig {
    let mut cfg = load();
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(token) = crate::auth::load_token_web() {
            if !token.trim().is_empty() {
                cfg.token = token;
            }
        }
    }
    cfg
}

#[cfg(not(target_arch = "wasm32"))]
fn config_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("us", "dasmat", "losos-frontend")
        .map(|d| d.config_dir().join("config.json"))
}

#[cfg(not(target_arch = "wasm32"))]
fn load_raw() -> Option<String> {
    std::fs::read_to_string(config_path()?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn save_raw(value: &str) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, value);
    }
}
