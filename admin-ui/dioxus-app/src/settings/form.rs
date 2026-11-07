//! Settings form engine state (draft/apply/rebuild), aligned with React `use-settings-form`.
//!
//! Critical invariant: `/api/apply` replaces the entire `modules/overrides.nix`.
//! `build_overrides_nix` must emit all 13 owned keys plus preserved/edited extras.

use super::api;
use super::model::{OptionsResponse, SettingsResponse, StatusResponse};
use super::nix::{build_overrides_nix, fits_editor, is_valid_host_name, is_valid_time};
use super::{Extras, DEFAULT_HOST_NAME, DEFAULT_WINDOW_END, DEFAULT_WINDOW_START, SETTLE_MS};
use crate::store::BoxConfig;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct RebuildBanner {
    pub phase: String,
    pub title: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FormProblems {
    pub host_name: Option<String>,
    pub compute_window: Option<String>,
}

impl FormProblems {
    pub fn has_any(&self) -> bool {
        self.host_name.is_some() || self.compute_window.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SettingsFormState {
    pub locked: bool,
    pub ready: bool,
    pub load_error: Option<String>,
    pub saved: Option<SettingsResponse>,
    pub draft: Option<SettingsResponse>,
    pub options: Option<OptionsResponse>,
    pub options_raw: Option<Value>,
    pub saved_extra: Extras,
    pub extra: Extras,
    pub extra_problems: BTreeMap<String, String>,
    pub applying: bool,
    pub rebuild: Option<RebuildBanner>,
    pub job: Option<String>,
    saw_building: bool,
    settle_until_ms: f64,
    last_job: Option<String>,
}

impl SettingsFormState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dirty(&self) -> bool {
        match (&self.saved, &self.draft) {
            (Some(s), Some(d)) if s != d => return true,
            _ => {}
        }
        self.saved_extra != self.extra
    }

    pub fn change_count(&self) -> usize {
        let mut n = 0usize;
        if let (Some(s), Some(d)) = (&self.saved, &self.draft) {
            macro_rules! c {
                ($f:ident) => { if s.$f != d.$f { n += 1; } };
            }
            c!(sharing_my_storage); c!(sharing_locally); c!(host_name); c!(gpu_enable);
            c!(proxy_enable); c!(cluster_enable); c!(share_compute);
            c!(compute_window_start); c!(compute_window_end);
            c!(hardening_apparmor); c!(hardening_malloc); c!(hardening_nosmt); c!(hardening_usbguard);
        }
        // count changed extras (added/removed/edited)
        let keys: BTreeSet<&String> = self.saved_extra.keys().chain(self.extra.keys()).collect();
        for k in keys {
            if self.saved_extra.get(k) != self.extra.get(k) {
                n += 1;
            }
        }
        n
    }

    pub fn problems(&self) -> FormProblems {
        let Some(d) = &self.draft else { return FormProblems::default(); };
        let host_name = if d.host_name.is_empty() {
            Some("Host name cannot be empty.".into())
        } else if d.host_name.len() > 63 {
            Some("Host name is too long.".into())
        } else if !is_valid_host_name(&d.host_name) {
            Some("Host name has invalid characters.".into())
        } else {
            None
        };
        let compute_window = if is_valid_time(&d.compute_window_start) && is_valid_time(&d.compute_window_end) {
            None
        } else {
            Some("Compute window must use HH:MM (24-hour).".into())
        };
        FormProblems { host_name, compute_window }
    }

    fn options_metadata_usable(options_raw: Option<&Value>) -> bool {
        let Some(doc) = options_raw else { return false };
        if doc.get("available").and_then(Value::as_bool) == Some(false)
            && doc.get("options").and_then(Value::as_array).is_none_or(|a| a.is_empty())
        {
            return false;
        }
        doc.get("options").is_some() || doc.get("stray").is_some()
    }

    fn extra_problems_of(
        extra: &Extras,
        saved_extra: &Extras,
        options_raw: Option<&Value>,
    ) -> BTreeMap<String, String> {
        let mut p = BTreeMap::new();
        // Interpolation is always refused by the box; never allow it.
        for (k, v) in extra {
            if v.contains("${") {
                p.insert(k.clone(), "Nix interpolation is not allowed here.".into());
            }
        }
        if !Self::options_metadata_usable(options_raw) {
            return p;
        }
        let doc = options_raw.unwrap();
        let options = doc.get("options").and_then(Value::as_array);
        let stray_keys: BTreeSet<&str> = doc
            .get("stray")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.get("key").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default();

        for (name, raw) in extra {
            if p.contains_key(name) {
                continue;
            }
            let unchanged = saved_extra.get(name).is_some_and(|s| s == raw);
            let opt = options.and_then(|arr| {
                arr.iter()
                    .find(|o| o.get("name").and_then(Value::as_str) == Some(name.as_str()))
            });
            if stray_keys.contains(name.as_str()) {
                p.insert(
                    name.clone(),
                    "This override names no option the box declares (stray). Remove it before applying.".into(),
                );
                continue;
            }
            let Some(opt) = opt else {
                // Not a declared option: treat as stray when metadata is present.
                p.insert(
                    name.clone(),
                    "This override names no option the box declares. Remove it before applying.".into(),
                );
                continue;
            };
            let read_only = opt.get("readOnly").and_then(Value::as_bool).unwrap_or(false);
            let Some(editor) = opt.get("editor") else {
                // No editor metadata: avoid false blocks.
                if !unchanged && read_only {
                    p.insert(name.clone(), "This option is read-only.".into());
                }
                continue;
            };
            let is_opaque = editor.get("kind").and_then(Value::as_str) == Some("opaque");
            if read_only || is_opaque {
                // Without breaking: unchanged read-only/opaque values were already present;
                // blocking apply on them would prevent unrelated settings changes.
                if !unchanged {
                    p.insert(
                        name.clone(),
                        if read_only {
                            "This option is read-only.".into()
                        } else {
                            "This value has no form editor; changing it may be refused.".into()
                        },
                    );
                }
                continue;
            }
            if let Some(misfit) = fits_editor(editor, raw) {
                p.insert(name.clone(), misfit.message);
            }
        }
        p
    }

    fn recompute_extra_problems(&mut self) {
        self.extra_problems =
            Self::extra_problems_of(&self.extra, &self.saved_extra, self.options_raw.as_ref());
    }

    pub fn valid(&self) -> bool {
        self.ready && self.draft.is_some() && !self.problems().has_any() && self.extra_problems.is_empty() && !self.locked
    }

    pub fn can_apply(&self) -> bool {
        self.valid() && self.dirty() && !self.applying
    }

    pub fn discard(&mut self) {
        self.draft = self.saved.clone();
        self.extra = self.saved_extra.clone();
        self.recompute_extra_problems();
        self.load_error = None;
    }

    pub fn set_extra(&mut self, name: String, raw: Option<String>) {
        match raw {
            Some(v) => { self.extra.insert(name, v); }
            None => { self.extra.remove(&name); }
        }
        self.recompute_extra_problems();
    }

    fn normalize(mut s: SettingsResponse) -> SettingsResponse {
        if !is_valid_time(&s.compute_window_start) {
            s.compute_window_start = DEFAULT_WINDOW_START.to_string();
        }
        if !is_valid_time(&s.compute_window_end) {
            s.compute_window_end = DEFAULT_WINDOW_END.to_string();
        }
        s
    }

    fn sanitize(mut s: SettingsResponse) -> SettingsResponse {
        if !is_valid_host_name(&s.host_name) {
            s.host_name = DEFAULT_HOST_NAME.to_string();
        }
        if !is_valid_time(&s.compute_window_start) {
            s.compute_window_start = DEFAULT_WINDOW_START.to_string();
        }
        if !is_valid_time(&s.compute_window_end) {
            s.compute_window_end = DEFAULT_WINDOW_END.to_string();
        }
        s
    }

    pub async fn load(&mut self, config: &BoxConfig) {
        self.load_error = None;
        let opts_raw = match api::api_for_config(config) {
            Ok(api) => api.get("/api/options").await.ok(),
            Err(e) => {
                if e.status == 401 {
                    self.locked = true;
                }
                self.load_error = Some(e.message);
                return;
            }
        };
        // An error-shaped or proxy-shaped answer is not a usable options document.
        let opts_raw = match opts_raw {
            Some(ref raw) if raw.get("error").is_some() && raw.get("options").is_none() => None,
            Some(raw) if raw.get("options").is_none() && raw.get("stray").is_none() => None,
            other => other,
        };
        match api::get_settings(config).await {
            Ok(s) => {
                let s = Self::normalize(s);
                let mut extra = Extras::new();
                if let Some(ref raw) = opts_raw {
                    api::extras_from_options_value(raw, &mut extra);
                }
                self.options_raw = opts_raw;
                self.options = self.options_raw.as_ref().and_then(|raw| {
                    serde_json::from_value(raw.clone()).ok()
                });
                self.saved = Some(s.clone());
                self.draft = Some(s);
                self.saved_extra = extra.clone();
                self.extra = extra;
                self.recompute_extra_problems();
                self.ready = true;
                self.locked = false;
                // If box is building, reflect it (resume in-flight rebuild).
                if let Ok(st) = api::get_status(config).await {
                    self.on_status(st, now_ms());
                } else {
                    // ignore status fetch errors on load
                }
            }
            Err(e) => {
                if e.status == 401 {
                    self.locked = true;
                }
                self.load_error = Some(e.message);
            }
        }
    }

    fn settle_active(now: f64, until: f64) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (now, until);
            false
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            now < until
        }
    }

    pub fn on_status(&mut self, status: StatusResponse, now: f64) {
        let state = status.state.as_str();
        let in_settle = self.applying && !self.saw_building && Self::settle_active(now, self.settle_until_ms);
        // Job-id comparison: ignore statuses that belong to a different rebuild than ours.
        if let (Some(ref ours), Some(ref theirs)) = (&self.job, &status.job) {
            if ours != theirs {
                // Another job entirely; never let it drive our UI.
                return;
            }
        }

        if state == "building" {
            // If we know our job and this building status names a different one, ignore.
            if let (Some(ref ours), Some(ref theirs)) = (&self.job, &status.job) {
                if ours != theirs {
                    return;
                }
            }
            self.applying = true;
            self.saw_building = true;
            if self.job.is_none() {
                self.job = status.job.clone();
            }
            self.rebuild = Some(RebuildBanner {
                phase: "building".into(),
                title: "Applying changes".into(),
                message: if status.message.is_empty() {
                    "Rebuilding the box...".into()
                } else {
                    status.message.clone()
                },
            });
            return;
        }

        let terminal = state == "done" || state == "failed" || state == "idle";
        if !terminal {
            return;
        }
        // Stale-terminal guard: before the daemon has reported our new build as
        // building, a terminal status (often with no/old job) may describe the
        // previous rebuild. Rely on job-id when present; otherwise use settle window.
        if in_settle {
            match (&self.job, &status.job) {
                (Some(ours), Some(theirs)) if ours == theirs => {}
                (Some(_), Some(_)) => return,
                (Some(_), None) => return,
                (None, _) => return,
            }
        }

        if self.applying || state == "failed" || (self.saw_building && terminal) {
            if state == "failed" {
                self.rebuild = Some(RebuildBanner {
                    phase: "failed".into(),
                    title: "Rebuild failed".into(),
                    message: if status.message.is_empty() {
                        "The box could not apply the changes.".into()
                    } else {
                        status.message.clone()
                    },
                });
                self.applying = false;
                self.saw_building = false;
            } else if state == "done" {
                if self.applying || self.saw_building {
                    self.rebuild = None;
                    self.load_error = None;
                }
                self.applying = false;
                self.saw_building = false;
                self.settle_until_ms = 0.0;
            } else if state == "idle" {
                if self.applying {
                    self.rebuild = None;
                }
                self.applying = false;
                self.saw_building = false;
                self.settle_until_ms = 0.0;
            }
        }
    }

    pub async fn apply(&mut self, config: &BoxConfig) -> Result<(), String> {
        if !self.can_apply() {
            return Err("Nothing to apply or form is not valid.".into());
        }
        let Some(draft) = self.draft.clone() else { return Err("Not ready.".into()); };
        let draft = Self::sanitize(draft);
        let extra = self.extra.clone();
        self.applying = true;
        self.saw_building = false;
        self.settle_until_ms = now_ms() + SETTLE_MS as f64;
        self.rebuild = Some(RebuildBanner {
            phase: "building".into(),
            title: "Applying changes".into(),
            message: "Sending configuration to the box...".into(),
        });
        match api::post_apply(config, &draft, &extra).await {
            Ok(resp) => {
                self.job = Some(resp.job.clone());
                self.last_job = Some(resp.job);
                // Mark draft as saved baseline for the apply intent; status confirms outcome.
                self.saved = Some(draft.clone());
                self.draft = Some(draft);
                self.saved_extra = extra.clone();
                self.extra = extra;
                self.recompute_extra_problems();
                Ok(())
            }
            Err(e) => {
                if e.status == 401 {
                    self.locked = true;
                }
                self.applying = false;
                self.rebuild = Some(RebuildBanner {
                    phase: "failed".into(),
                    title: "Apply failed".into(),
                    message: e.message.clone(),
                });
                Err(e.message)
            }
        }
    }

    pub async fn factory_reset(&mut self, config: &BoxConfig) -> Result<(), String> {
        self.applying = true;
        match api::post_factory_reset(config).await {
            Ok(_) => {
                self.rebuild = Some(RebuildBanner {
                    phase: "building".into(),
                    title: "Resetting".into(),
                    message: "Factory reset started.".into(),
                });
                Ok(())
            }
            Err(e) => {
                if e.status == 401 {
                    self.locked = true;
                }
                self.applying = false;
                Err(e.message)
            }
        }
    }

    pub fn build_nix_preview(&self) -> Option<String> {
        let draft = self.draft.as_ref()?;
        Some(build_overrides_nix(&Self::sanitize(draft.clone()), &self.extra))
    }
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    // Real clock not wired (avoids adding js-sys dep in constrained env).
    // Stale-guard still uses job-id + saw_building; settle time is best-effort.
    0.0
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}
