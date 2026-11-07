//! Settings API models, aligned with `admin-ui/app/src/lib/api.ts` and `backend/schema.json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// GET /api/settings — the losos.* options parsed out of overrides.nix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsResponse {
    pub sharing_my_storage: bool,
    #[serde(default)]
    pub sharing_locally: bool,
    pub host_name: String,
    #[serde(rename = "gpuEnable")]
    pub gpu_enable: bool,
    #[serde(rename = "proxyEnable")]
    pub proxy_enable: bool,
    pub cluster_enable: bool,
    pub share_compute: bool,
    pub compute_window_start: String,
    pub compute_window_end: String,
    pub hardening_apparmor: bool,
    pub hardening_malloc: bool,
    pub hardening_nosmt: bool,
    pub hardening_usbguard: bool,
}

impl Default for SettingsResponse {
    fn default() -> Self {
        Self {
            sharing_my_storage: false,
            sharing_locally: false,
            host_name: super::DEFAULT_HOST_NAME.to_string(),
            gpu_enable: false,
            proxy_enable: false,
            cluster_enable: false,
            share_compute: false,
            compute_window_start: super::DEFAULT_WINDOW_START.to_string(),
            compute_window_end: super::DEFAULT_WINDOW_END.to_string(),
            hardening_apparmor: false,
            hardening_malloc: false,
            hardening_nosmt: false,
            hardening_usbguard: false,
        }
    }
}

/// GET /api/status — rebuild progress.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub state: String,
    #[serde(default)]
    pub progress: i64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub job: Option<String>,
}

/// POST /api/apply response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChangeResponse {
    pub job: String,
}

/// GET /api/options — option metadata (opaque here; form uses extras/literals).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct OptionsResponse {
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub options: Vec<serde_json::Value>,
}

/// Owned key names (the 13 form fields) — must match React `OWNED_NAMES`.
pub const OWNED_NAMES: &[&str] = &[
    "sharingMyStorage",
    "sharingLocally",
    "hostName",
    "gpu.enable",
    "proxy.enable",
    "cluster.enable",
    "cluster.shareCompute",
    "cluster.computeWindow.start",
    "cluster.computeWindow.end",
    "hardening.apparmor",
    "hardening.malloc",
    "hardening.nosmt",
    "hardening.usbguard",
];

pub fn owned_name_set() -> std::collections::BTreeSet<&'static str> {
    OWNED_NAMES.iter().copied().collect()
}

/// Extras: every other `losos.<name>` line as Nix literals by name.
pub type Extras = BTreeMap<String, String>;
