//! Setups as files (files.js): the document, its checks, and the rebuild
//! through `newDevice` and `connect`, so a hand-edited or foreign file can
//! only produce what the tray could have produced.

use crate::catalog::{default_cfg, default_name, is_link_kind, is_site, port_kind, type_info};
use crate::js::{prop, round, slice16, string};
use crate::lab::LabCore;
use crate::scenarios::device_name;
use crate::world::{Connected, NewOpts};
use serde_json::{json, Value};
use std::collections::HashMap;

pub const SETUP_FORMAT: &str = "losos-lab-setup";
pub const MAX_BYTES: usize = 1 << 20;

/// `num(v, lo, hi)`: a finite number, rounded and clamped, else nothing.
fn num(v: Option<&Value>, lo: f64, hi: f64) -> Option<f64> {
    let f = v?.as_f64().filter(|f| f.is_finite())?;
    Some(round(f).clamp(lo, hi))
}

/// `docProblem`: why a parsed document is not a setup, if it is not.
pub fn doc_problem(doc: &Value) -> Option<String> {
    let d = Some(doc);
    let ok = matches!(doc, Value::Object(_))
        && prop(d, "format").and_then(Value::as_str) == Some(SETUP_FORMAT)
        && matches!(prop(d, "devices"), Some(Value::Array(_)))
        && matches!(prop(d, "links"), Some(Value::Array(_)));
    if !ok {
        return Some("This is not a LosOS Lab setup file.".into());
    }
    let v = prop(d, "version");
    if v.and_then(Value::as_f64) != Some(1.0) {
        return Some(format!(
            "This setup file is version {}; this Lab reads version 1.",
            slice16(&string(v), 10)
        ));
    }
    None
}

impl LabCore {
    pub(crate) fn setup_doc(&self) -> Value {
        let name = if self.scenario == "file" || self.scenario == "last" {
            Some(self.file_name.clone())
        } else {
            self.scenario_name(&self.scenario)
        };
        let name = name
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "My setup".into());
        let devices: Vec<Value> = self
            .world
            .devices
            .iter()
            .map(|d| {
                json!({
                    "id": d.id, "type": d.kind, "name": d.name, "site": d.site,
                    "x": crate::js::num_value(d.x), "y": crate::js::num_value(d.y),
                    "px": d.px.map(crate::js::num_value), "py": d.py.map(crate::js::num_value),
                    "power": d.power, "cfg": d.cfg,
                })
            })
            .collect();
        let links: Vec<Value> = self
            .world
            .links
            .iter()
            .map(|l| json!({ "a": l.a, "b": l.b, "kind": l.kind }))
            .collect();
        json!({ "format": SETUP_FORMAT, "version": 1, "name": name, "devices": devices, "links": links })
    }

    /// `buildFromDoc` into the (already emptied) world; returns what it left out.
    pub(crate) fn build_from_doc(&mut self, doc: &Value) -> usize {
        let empty = Vec::new();
        let devices = prop(Some(doc), "devices")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        let links = prop(Some(doc), "links")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        let mut ids: HashMap<String, String> = HashMap::new();
        let mut skipped = 0;
        for s in devices.iter().take(200) {
            let kind = match prop(Some(s), "type") {
                Some(Value::String(t)) if type_info(t).is_some() => t.clone(),
                _ => {
                    skipped += 1;
                    continue;
                }
            };
            let base = default_cfg(&kind);
            let mut cfg = Vec::new();
            if let Some(Value::Object(m)) = prop(Some(s), "cfg") {
                for (k, v) in m {
                    let same = matches!(
                        (base.get(k), v),
                        (Some(Value::Bool(_)), Value::Bool(_))
                            | (Some(Value::String(_)), Value::String(_))
                    );
                    if same {
                        cfg.push((k.clone(), v.clone()));
                    }
                }
            }
            let site = match prop(Some(s), "site") {
                Some(Value::String(x)) if is_site(x) => Some(x.clone()),
                _ => None,
            };
            let opts = NewOpts {
                name: Some(slice16(
                    &device_name(prop(Some(s), "name"), default_name(&kind)),
                    40,
                )),
                site,
                px: num(prop(Some(s), "px"), 0.0, 2000.0),
                py: num(prop(Some(s), "py"), 0.0, 2000.0),
                power: Some(prop(Some(s), "power") != Some(&Value::Bool(false))),
                cfg,
            };
            let x = num(prop(Some(s), "x"), -5000.0, 5000.0).unwrap_or(400.0);
            let y = num(prop(Some(s), "y"), -5000.0, 5000.0).unwrap_or(300.0);
            let id = self.world.new_device(&kind, x, y, opts, &mut self.rng);
            ids.insert(string(prop(Some(s), "id")), id);
        }
        for s in links.iter().take(400) {
            let end = |k: &str| {
                let e = prop(Some(s), k)?;
                if !crate::js::truthy(Some(e)) {
                    return None;
                }
                let id = ids.get(&string(prop(Some(e), "dev")))?;
                let port = prop(Some(e), "port");
                Some((id.clone(), port.cloned()))
            };
            let (Some((a, ap)), Some((b, bp))) = (end("a"), end("b")) else {
                skipped += 1;
                continue;
            };
            let kind = prop(Some(s), "kind").and_then(Value::as_str).unwrap_or("");
            let r = if kind == "wifi" {
                self.world.connect(&a, &b, "wifi", None, None)
            } else {
                let ok = |d: &str, p: &Option<Value>| match p {
                    Some(Value::String(p)) => {
                        let kind = self
                            .world
                            .dev(d)
                            .map(|x| x.kind.clone())
                            .unwrap_or_default();
                        port_kind(&kind, p) == Some("eth") && !self.world.port_used(d, p)
                    }
                    _ => false,
                };
                if !ok(&a, &ap) || !ok(&b, &bp) {
                    skipped += 1;
                    continue;
                }
                let k = if is_link_kind(kind) { kind } else { "copper" };
                let (ap, bp) = (
                    ap.as_ref().and_then(Value::as_str).map(str::to_string),
                    bp.as_ref().and_then(Value::as_str).map(str::to_string),
                );
                self.world.connect(&a, &b, k, ap.as_deref(), bp.as_deref())
            };
            if let Connected::Error(_) = r {
                skipped += 1;
            }
        }
        skipped
    }

    /// `openDoc`: checked before anything is cleared.
    pub(crate) fn open_doc(
        &mut self,
        doc: &Value,
        key: &str,
        label: &str,
    ) -> Result<usize, String> {
        if let Some(p) = doc_problem(doc) {
            return Err(p);
        }
        let n = prop(Some(doc), "name");
        let name = if crate::js::truthy(n) {
            string(n)
        } else {
            label.to_string()
        };
        self.file_name = slice16(&name, 60);
        let mut skipped = 0;
        self.start_world(key, |lab| {
            skipped = lab.build_from_doc(doc);
            None
        });
        self.dirty = false;
        Ok(skipped)
    }

    pub(crate) fn setup_filename(&self, doc: &Value) -> String {
        let name = device_name(prop(Some(doc), "name"), "setup");
        let short = slice16(&name, 32);
        format!("{}.llf", short.trim_end_matches('-'))
    }
}

/// The label of an opened file: its name without `.llf`, `.losos-lab.json` or `.json`.
pub fn file_label(file_name: &str) -> String {
    let lower = file_name.to_ascii_lowercase();
    for ext in [".llf", ".losos-lab.json", ".json"] {
        if lower.ends_with(ext) {
            return file_name[..file_name.len() - ext.len()].to_string();
        }
    }
    file_name.to_string()
}

pub fn parse_doc(text: &str) -> Result<Value, String> {
    if text.len() > MAX_BYTES {
        return Err("That file is too big to be a setup.".into());
    }
    serde_json::from_str(text).map_err(|_| "This is not a LosOS Lab setup file.".to_string())
}

pub fn keep_name(mut doc: Value, builtin: bool) -> Value {
    if builtin {
        if let Some(Value::String(n)) = doc.get("name").cloned() {
            doc["name"] = Value::String(format!("Edited {n}"));
        }
    }
    doc
}

pub fn last_label(doc: &Value) -> Option<String> {
    if prop(Some(doc), "format").and_then(Value::as_str) != Some(SETUP_FORMAT) {
        return None;
    }
    let n = prop(Some(doc), "name");
    let name = if crate::js::truthy(n) {
        string(n)
    } else {
        "My setup".into()
    };
    Some(slice16(&name, 60))
}
