//! Settings form engine state (draft/apply/rebuild), aligned with React `use-settings-form`.
//!
//! Critical invariant: `/api/apply` replaces the entire `modules/overrides.nix`.
//! `build_overrides_nix` must emit all 13 owned keys plus preserved/edited extras.

use super::api;
use super::model::{OptionsResponse, SettingsResponse, StatusResponse};
use super::nix::{build_overrides_nix, is_valid_host_name, is_valid_time};
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

    fn extra_problems_of(extra: &Extras) -> BTreeMap<String, String> {
        // Minimal: ban Nix interpolation (React/lososd refuse it). Stray/opaque type
        // checks require full options metadata; keep Apply blocked on interpolation.
        let mut p = BTreeMap::new();
        for (k, v) in extra {
            if v.contains("${") {
                p.insert(k.clone(), "Nix interpolation is not allowed here.".into());
            }
        }
        p
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
        self.extra_problems = Self::extra_problems_of(&self.extra);
        self.load_error = None;
    }

    pub fn set_extra(&mut self, name: String, raw: Option<String>) {
        match raw {
            Some(v) => { self.extra.insert(name, v); }
            None => { self.extra.remove(&name); }
        }
        self.extra_problems = Self::extra_problems_of(&self.extra);
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
        if let Some(ref raw) = opts_raw {
            if raw.get("error").is_some() && raw.get("options").is_none() {
                // treat as unavailable but continue with settings
            }
        }
        match api::get_settings(config).await {
            Ok(s) => {
                let s = Self::normalize(s);
                let mut extra = Extras::new();
                if let Some(ref raw) = opts_raw {
                    api::extras_from_options_value(raw, &mut extra);
                }
                self.saved = Some(s.clone());
                self.draft = Some(s);
                self.saved_extra = extra.clone();
                self.extra = extra;
                self.extra_problems = Self::extra_problems_of(&self.extra);
                self.options_raw = opts_raw;
                self.ready = true;
                self.locked = false;
                // If box is building, reflect it (basic resume)
                if let Ok(st) = api::get_status(config).await {
                    self.on_status(st, now_ms());
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

    pub fn on_status(&mut self, status: StatusResponse, now: f64) {
        let state = status.state.as_str();
        if state == "building" {
            self.applying = true;
            self.saw_building = true;
            self.job = status.job.clone().or(self.job.clone());
            self.rebuild = Some(RebuildBanner {
                phase: "building".into(),
                title: "Applying changes".into(),
                message: if status.message.is_empty() { "Rebuilding the box...".into() } else { status.message.clone() },
            });
            return;
        }
        let terminal = state == "done" || state == "failed" || state == "idle";
        if !terminal { return; }
        // stale-terminal guard: if we started a job and haven't seen building and
        // we're within settle window, ignore a terminal status for a different/old job.
        if let Some(ref job) = self.job {
            if let Some(ref sj) = status.job {
                if sj != job && !self.saw_building && now < self.settle_until_ms {
                    return;
                }
            } else if !self.saw_building && now < self.settle_until_ms {
                return;
            }
        }
        if self.applying || state == "failed" {
            if state == "failed" {
                self.rebuild = Some(RebuildBanner {
                    phase: "failed".into(),
                    title: "Rebuild failed".into(),
                    message: if status.message.is_empty() { "The box could not apply the changes.".into() } else { status.message.clone() },
                });
            } else if state == "done" && self.applying {
                self.rebuild = None;
                self.load_error = None;
            } else if state == "idle" {
                self.rebuild = None;
            }
            self.applying = false;
            self.saw_building = false;
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
                // Mark draft as saved baseline for the apply intent? React keeps draft
                // but saved becomes the applied values on success path; easier to update
                // saved to draft now and let status confirm.
                self.saved = Some(draft.clone());
                self.draft = Some(draft);
                self.saved_extra = extra.clone();
                self.extra = extra;
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
