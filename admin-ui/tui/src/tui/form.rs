//! The settings form, the web UI's `use-settings-form.ts` in a terminal.
//!
//! `/api/apply` takes the WHOLE of `modules/overrides.nix`, so the draft is
//! a complete settings document and every pane edits a slice of it; Apply
//! writes all thirteen lines plus every other line the file already carries,
//! the way the web UI's `buildOverridesNix` does. Nothing takes effect until
//! Apply, and Apply asks before it starts a rebuild. The shell polls
//! `/api/status` while one runs.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::i18n::t;

/// GET /api/settings, the thirteen settings the panes own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub sharing_my_storage: bool,
    pub forgejo_mode: String,
    pub host_name: String,
    pub gpu_enable: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingKey {
    SharingMyStorage,
    ForgejoMode,
    HostName,
    GpuEnable,
    ProxyEnable,
    ClusterEnable,
    ShareCompute,
    ComputeWindowStart,
    ComputeWindowEnd,
    HardeningApparmor,
    HardeningMalloc,
    HardeningNosmt,
    HardeningUsbguard,
}

impl SettingKey {
    pub const ALL: [SettingKey; 13] = [
        SettingKey::SharingMyStorage,
        SettingKey::ForgejoMode,
        SettingKey::HostName,
        SettingKey::GpuEnable,
        SettingKey::ProxyEnable,
        SettingKey::ClusterEnable,
        SettingKey::ShareCompute,
        SettingKey::ComputeWindowStart,
        SettingKey::ComputeWindowEnd,
        SettingKey::HardeningApparmor,
        SettingKey::HardeningMalloc,
        SettingKey::HardeningNosmt,
        SettingKey::HardeningUsbguard,
    ];
}

/// A value a control writes into the draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingValue {
    Bool(bool),
    Text(String),
}

impl Settings {
    fn bool_mut(&mut self, key: SettingKey) -> Option<&mut bool> {
        Some(match key {
            SettingKey::SharingMyStorage => &mut self.sharing_my_storage,
            SettingKey::GpuEnable => &mut self.gpu_enable,
            SettingKey::ProxyEnable => &mut self.proxy_enable,
            SettingKey::ClusterEnable => &mut self.cluster_enable,
            SettingKey::ShareCompute => &mut self.share_compute,
            SettingKey::HardeningApparmor => &mut self.hardening_apparmor,
            SettingKey::HardeningMalloc => &mut self.hardening_malloc,
            SettingKey::HardeningNosmt => &mut self.hardening_nosmt,
            SettingKey::HardeningUsbguard => &mut self.hardening_usbguard,
            _ => return None,
        })
    }

    fn text_mut(&mut self, key: SettingKey) -> Option<&mut String> {
        Some(match key {
            SettingKey::ForgejoMode => &mut self.forgejo_mode,
            SettingKey::HostName => &mut self.host_name,
            SettingKey::ComputeWindowStart => &mut self.compute_window_start,
            SettingKey::ComputeWindowEnd => &mut self.compute_window_end,
            _ => return None,
        })
    }

    pub fn get_bool(&self, key: SettingKey) -> bool {
        self.clone().bool_mut(key).is_some_and(|b| *b)
    }

    pub fn set(&mut self, key: SettingKey, value: SettingValue) {
        match value {
            SettingValue::Bool(v) => {
                if let Some(b) = self.bool_mut(key) {
                    *b = v;
                }
            }
            SettingValue::Text(v) => {
                if let Some(s) = self.text_mut(key) {
                    *s = v;
                }
            }
        }
    }

    fn differs(&self, other: &Settings, key: SettingKey) -> bool {
        let (mut a, mut b) = (self.clone(), other.clone());
        if let (Some(x), Some(y)) = (a.bool_mut(key), b.bool_mut(key)) {
            return x != y;
        }
        a.text_mut(key) != b.text_mut(key)
    }
}

/// `"…"` with `\`, `"` and `${` escaped: the web UI's `nixString`.
pub fn nix_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace("${", "\\${")
    )
}

fn nix_bool(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

/// The thirteen `losos.*` names the lines below always write.
pub const OWNED_NAMES: [&str; 13] = [
    "sharingMyStorage",
    "forgejo.mode",
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

/// The full `modules/overrides.nix` body, as the web UI's
/// `buildOverridesNix` writes it: the thirteen, then every other line the
/// file carries, sorted, so the commit lososd makes shows only what changed.
pub fn build_overrides_nix(s: &Settings, extra: &[(String, String)]) -> String {
    let mut extra: Vec<&(String, String)> = extra
        .iter()
        .filter(|(name, _)| !OWNED_NAMES.contains(&name.as_str()))
        .collect();
    extra.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lines = vec![
        "{ ... }:".to_string(),
        "{".to_string(),
        format!(
            "  losos.sharingMyStorage = {};",
            nix_bool(s.sharing_my_storage)
        ),
        format!("  losos.forgejo.mode = {};", nix_string(&s.forgejo_mode)),
        format!("  losos.hostName = {};", nix_string(&s.host_name)),
        format!("  losos.gpu.enable = {};", nix_bool(s.gpu_enable)),
        format!("  losos.proxy.enable = {};", nix_bool(s.proxy_enable)),
        format!("  losos.cluster.enable = {};", nix_bool(s.cluster_enable)),
        format!(
            "  losos.cluster.shareCompute = {};",
            nix_bool(s.share_compute)
        ),
        format!(
            "  losos.cluster.computeWindow.start = {};",
            nix_string(&s.compute_window_start)
        ),
        format!(
            "  losos.cluster.computeWindow.end = {};",
            nix_string(&s.compute_window_end)
        ),
        format!(
            "  losos.hardening.apparmor = {};",
            nix_bool(s.hardening_apparmor)
        ),
        format!(
            "  losos.hardening.malloc = {};",
            nix_bool(s.hardening_malloc)
        ),
        format!("  losos.hardening.nosmt = {};", nix_bool(s.hardening_nosmt)),
        format!(
            "  losos.hardening.usbguard = {};",
            nix_bool(s.hardening_usbguard)
        ),
    ];
    for (name, value) in extra {
        lines.push(format!("  losos.{name} = {value};"));
    }
    lines.push("}".to_string());
    lines.push(String::new());
    lines.join("\n")
}

/// Every other `losos.<name>` the box's file carries now: the options the
/// Advanced pane reports as set, and the stray lines.
pub fn extras_of(options: &Value) -> Vec<(String, String)> {
    let mut extra = Vec::new();
    for option in options["options"].as_array().into_iter().flatten() {
        if let (Some(name), Some(set)) = (option["name"].as_str(), option["set"].as_str()) {
            if !OWNED_NAMES.contains(&name) {
                extra.push((name.to_string(), set.to_string()));
            }
        }
    }
    for stray in options["stray"].as_array().into_iter().flatten() {
        if let (Some(key), Some(value)) = (stray["key"].as_str(), stray["value"].as_str()) {
            extra.push((key.to_string(), value.to_string()));
        }
    }
    extra
}

pub fn is_valid_time(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && ((b[0] == b'0' || b[0] == b'1') || (b[0] == b'2' && b[1] <= b'3'))
        && b[3] <= b'5'
}

/// An RFC 1123 label: what the web UI's HOSTNAME_PATTERN accepts.
pub fn is_valid_host_name(value: &str) -> bool {
    let b = value.as_bytes();
    let alnum = |c: &u8| c.is_ascii_alphanumeric();
    !b.is_empty()
        && b.len() <= 63
        && alnum(&b[0])
        && alnum(&b[b.len() - 1])
        && b.iter().all(|c| alnum(c) || *c == b'-')
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Problems {
    pub host_name: Option<String>,
    pub compute_window: Option<String>,
}

pub fn problems_of(draft: Option<&Settings>) -> Problems {
    let Some(d) = draft else {
        return Problems::default();
    };
    let host_name = if d.host_name.is_empty() {
        Some("settings.form.hostEmpty")
    } else if d.host_name.len() > 63 {
        Some("settings.form.hostTooLong")
    } else if !is_valid_host_name(&d.host_name) {
        Some("settings.form.hostChars")
    } else {
        None
    };
    Problems {
        host_name: host_name.map(t),
        compute_window: (!(is_valid_time(&d.compute_window_start)
            && is_valid_time(&d.compute_window_end)))
        .then(|| t("settings.form.window")),
    }
}

/// A rebuild in flight: its title and the box's last line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuild {
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Form {
    pub saved: Option<Settings>,
    pub draft: Option<Settings>,
    /// GET /api/options, or `{available: false}` when the box has none.
    pub options: Option<Value>,
    pub load_error: Option<String>,
    pub rebuild: Option<Rebuild>,
    /// The rebuild job being watched.
    pub job: Option<String>,
}

impl Form {
    pub fn ready(&self) -> bool {
        self.draft.is_some()
    }

    pub fn loaded(&mut self, settings: Settings, options: Value) {
        self.saved = Some(settings.clone());
        self.draft = Some(settings);
        self.options = Some(options);
        self.load_error = None;
    }

    pub fn change_count(&self) -> usize {
        match (&self.saved, &self.draft) {
            (Some(s), Some(d)) => SettingKey::ALL.iter().filter(|k| s.differs(d, **k)).count(),
            _ => 0,
        }
    }

    pub fn dirty(&self) -> bool {
        self.change_count() > 0
    }

    pub fn problems(&self) -> Problems {
        problems_of(self.draft.as_ref())
    }

    pub fn valid(&self) -> bool {
        self.problems() == Problems::default()
    }

    pub fn set(&mut self, key: SettingKey, value: SettingValue) {
        if let Some(d) = self.draft.as_mut() {
            d.set(key, value);
        }
    }

    pub fn discard(&mut self) {
        self.draft = self.saved.clone();
    }

    /// The body for POST /api/apply.
    pub fn apply_body(&self) -> Option<String> {
        let draft = self.draft.as_ref().filter(|_| self.valid())?;
        let extra = self.options.as_ref().map(extras_of).unwrap_or_default();
        Some(build_overrides_nix(draft, &extra))
    }

    /// The installer fixed this box to keyfile unlock: no TPM.
    pub fn no_tpm(&self) -> bool {
        self.options
            .as_ref()
            .and_then(|o| o["options"].as_array())
            .and_then(|list| list.iter().find(|o| o["name"] == "tpm.enable"))
            .is_some_and(|o| o["current"] == Value::Bool(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Settings {
        Settings {
            sharing_my_storage: true,
            forgejo_mode: "container".into(),
            host_name: "mattbox".into(),
            gpu_enable: false,
            proxy_enable: false,
            cluster_enable: true,
            share_compute: true,
            compute_window_start: "23:00".into(),
            compute_window_end: "07:00".into(),
            hardening_apparmor: false,
            hardening_malloc: false,
            hardening_nosmt: false,
            hardening_usbguard: true,
        }
    }

    #[test]
    fn overrides_match_the_web_uis_lines() {
        let mut s = sample();
        s.host_name = "a\"b${c}".into();
        let body = build_overrides_nix(
            &s,
            &[
                ("upgradeFlakeUri".into(), "\"github:x/y\"".into()),
                ("hostName".into(), "\"dropped\"".into()),
                ("apps.ports".into(), "[ ]".into()),
            ],
        );
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines[0], "{ ... }:");
        assert_eq!(lines[4], "  losos.hostName = \"a\\\"b\\${c}\";");
        assert_eq!(lines[5], "  losos.gpu.enable = false;");
        assert_eq!(lines[15], "  losos.apps.ports = [ ];");
        assert_eq!(lines[16], "  losos.upgradeFlakeUri = \"github:x/y\";");
        assert_eq!(lines[17], "}");
        assert!(body.ends_with("}\n"));
        assert!(!body.contains("dropped"));
    }

    #[test]
    fn validators_follow_the_web_ui() {
        assert!(is_valid_time("23:59") && is_valid_time("00:00"));
        assert!(!is_valid_time("24:00") && !is_valid_time("7:00") && !is_valid_time("23:60"));
        assert!(is_valid_host_name("mattbox-2"));
        assert!(!is_valid_host_name("-x") && !is_valid_host_name("a b") && !is_valid_host_name(""));
    }

    #[test]
    fn the_form_counts_changes_and_refuses_to_apply_a_bad_draft() {
        let mut form = Form::default();
        form.loaded(
            sample(),
            serde_json::json!({"available": false, "options": [], "stray": []}),
        );
        assert!(!form.dirty());
        form.set(SettingKey::GpuEnable, SettingValue::Bool(true));
        form.set(SettingKey::HostName, SettingValue::Text("box2".into()));
        assert_eq!(form.change_count(), 2);
        assert!(form.apply_body().is_some());
        form.set(SettingKey::HostName, SettingValue::Text("-box".into()));
        assert!(!form.valid());
        assert!(form.apply_body().is_none());
        form.discard();
        assert!(!form.dirty());
    }
}
