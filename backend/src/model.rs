//! Domain types and their JSON encodings.
//!
//! Every shape here is a wire contract consumed by the admin SPA, the VM tests
//! and `losos-ctl`'s stdout; `backend/schema.json` is the specification and
//! field order is significant. The decoding side is deliberately lenient in two
//! places (see [`State`]) because a corrupt state file must degrade to a fresh
//! appliance rather than take the admin UI down with it.

use serde::{Deserialize, Serialize};

/// Sharing posture. `local` keeps storage private; `mesh` contributes it to the
/// Tahoe-LAFS grid (and is the sole source of truth for `sharingMyStorage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Local,
    Mesh,
}

impl Mode {
    /// The wire spelling (`"local"` / `"mesh"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Local => "local",
            Mode::Mesh => "mesh",
        }
    }

    /// Exact-match parse; anything else is `None`. Callers at every transport
    /// boundary reject `None` with the message
    /// `mode must be 'local' or 'mesh'`.
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "local" => Some(Mode::Local),
            "mesh" => Some(Mode::Mesh),
            _ => None,
        }
    }
}

/// Lifecycle of the last or current `nixos-rebuild`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RebuildState {
    Idle,
    Building,
    Done,
    Failed,
}

impl RebuildState {
    pub fn as_str(self) -> &'static str {
        match self {
            RebuildState::Idle => "idle",
            RebuildState::Building => "building",
            RebuildState::Done => "done",
            RebuildState::Failed => "failed",
        }
    }
}

/// One tracked rebuild. Persisted inside [`State`]; never returned on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rebuild {
    pub job: String,
    pub state: RebuildState,
    /// Absent in an older state file; decodes as 0.
    #[serde(default)]
    pub progress: i64,
    /// Absent in an older state file; decodes as "".
    #[serde(default)]
    pub message: String,
}

/// The persisted state at `$LOSOS_STATE_DIR/state.json`.
///
/// Two decode leniencies are deliberate and load-bearing:
///
///   * an unparseable `mode` falls back to [`Mode::Local`] instead of failing
///     the whole document (see [`de_mode_lenient`]), and
///   * a document that fails to parse outright is treated as [`State::default`]
///     by the reader in `io_backend`, not surfaced as an error.
///
/// Between them, a corrupt state file degrades to "fresh appliance" rather than
/// wedging the admin UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(deserialize_with = "de_mode_lenient")]
    pub mode: Mode,
    pub sharing: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<Rebuild>,
}

impl Default for State {
    /// `{"mode":"local","sharing":false,"rebuild":null}` — what a fresh or
    /// unreadable appliance reports, and what `factory-reset` writes.
    fn default() -> Self {
        State {
            mode: Mode::Local,
            sharing: false,
            rebuild: None,
        }
    }
}

/// Accept any string for `mode`, falling back to `local` when it isn't one of
/// the two known spellings.
fn de_mode_lenient<'de, D>(d: D) -> Result<Mode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = String::deserialize(d)?;
    Ok(Mode::parse(&raw).unwrap_or(Mode::Local))
}

/// The user-tunable `losos.*` options parsed out of `modules/overrides.nix`.
///
/// Field order matters: this is serialized straight to the settings response
/// and the admin SPA reads these exact key names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub sharing_my_storage: bool,
    pub nextcloud_mode: String,
    pub forgejo_mode: String,
    pub host_name: String,
    pub https: bool,
    pub gpu_enable: bool,
    pub apache_port: i64,
    pub proxy_enable: bool,
}

impl Default for Settings {
    /// Mirrors the committed `overrides.nix` body, and supplies the per-field
    /// fallback whenever a key is missing from the file being parsed.
    fn default() -> Self {
        Settings {
            sharing_my_storage: true,
            nextcloud_mode: "container".to_string(),
            forgejo_mode: "container".to_string(),
            host_name: "mattbox".to_string(),
            https: false,
            gpu_enable: true,
            apache_port: 11000,
            proxy_enable: false,
        }
    }
}

impl Settings {
    /// The settings response, with the camelCase key names and the exact field
    /// order `schema.json` fixes.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "sharingMyStorage": self.sharing_my_storage,
            "nextcloudMode": self.nextcloud_mode,
            "forgejoMode": self.forgejo_mode,
            "hostName": self.host_name,
            "https": self.https,
            "gpuEnable": self.gpu_enable,
            "apachePort": self.apache_port,
            "proxyEnable": self.proxy_enable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_round_trips_and_rejects_junk() {
        assert_eq!(Mode::parse("local"), Some(Mode::Local));
        assert_eq!(Mode::parse("mesh"), Some(Mode::Mesh));
        assert_eq!(Mode::parse("Mesh"), None);
        assert_eq!(Mode::parse(""), None);
        assert_eq!(Mode::Mesh.as_str(), "mesh");
    }

    #[test]
    fn default_state_is_local_not_sharing_no_rebuild() {
        let s = State::default();
        assert_eq!(s.mode, Mode::Local);
        assert!(!s.sharing);
        assert!(s.rebuild.is_none());
    }

    #[test]
    fn state_decode_falls_back_to_local_on_unknown_mode() {
        let s: State = serde_json::from_str(r#"{"mode":"wat","sharing":true}"#).unwrap();
        assert_eq!(s.mode, Mode::Local);
        assert!(s.sharing);
    }

    #[test]
    fn rebuild_decode_defaults_progress_and_message() {
        let r: Rebuild = serde_json::from_str(r#"{"job":"j","state":"building"}"#).unwrap();
        assert_eq!(r.progress, 0);
        assert_eq!(r.message, "");
    }

    #[test]
    fn settings_json_uses_camel_case_keys() {
        let out = Settings::default().to_json().to_string();
        for key in [
            "sharingMyStorage",
            "nextcloudMode",
            "forgejoMode",
            "hostName",
            "https",
            "gpuEnable",
            "apachePort",
            "proxyEnable",
        ] {
            assert!(out.contains(key), "missing {key} in {out}");
        }
        // Legacy internal spellings must never leak onto the wire.
        assert!(!out.contains("aioApachePort"));
        assert!(!out.contains("aioInterfacePort"));
    }
}
