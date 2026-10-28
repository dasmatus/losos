//! The settings the panes change, written the way the script pages write
//! them (lib/api.ts's buildOverridesNix): the whole of overrides.nix, the
//! twelve lines every apply carries and then every other line the box has,
//! sent to `POST /api/apply`, which starts a rebuild. A pane's form sends
//! only its own fields, each with the value it showed, so a field nobody
//! touched keeps whatever the box has now.

use crate::host::Host;
use serde_json::Value;

/// The twelve lines: the name in overrides.nix, the key in
/// `GET /api/settings`, and whether it is a string.
pub const OWNED: &[(&str, &str, bool)] = &[
    ("sharingMyStorage", "sharingMyStorage", false),
    ("hostName", "hostName", true),
    ("gpu.enable", "gpuEnable", false),
    ("proxy.enable", "proxyEnable", false),
    ("cluster.enable", "clusterEnable", false),
    ("cluster.shareCompute", "shareCompute", false),
    ("cluster.computeWindow.start", "computeWindowStart", true),
    ("cluster.computeWindow.end", "computeWindowEnd", true),
    ("hardening.apparmor", "hardeningApparmor", false),
    ("hardening.malloc", "hardeningMalloc", false),
    ("hardening.nosmt", "hardeningNosmt", false),
    ("hardening.usbguard", "hardeningUsbguard", false),
];

/// lib/api.ts's nixString.
#[must_use]
pub fn nix_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace("${", "\\${")
    )
}

/// Every line beyond the twelve the box's file has now, as the options
/// document lists them (use-settings-form.ts's extrasOf).
#[must_use]
pub fn extras_of(options: &Value) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let owned = |name: &str| OWNED.iter().any(|(n, _, _)| *n == name);
    for o in options["options"].as_array().into_iter().flatten() {
        if let (Some(name), Some(set)) = (o["name"].as_str(), o["set"].as_str()) {
            if !owned(name) {
                out.push((name.to_string(), set.to_string()));
            }
        }
    }
    for s in options["stray"].as_array().into_iter().flatten() {
        if let (Some(key), Some(value)) = (s["key"].as_str(), s["value"].as_str()) {
            out.retain(|(k, _)| k != key);
            out.push((key.to_string(), value.to_string()));
        }
    }
    out.retain(|(k, _)| !owned(k));
    out.sort();
    out
}

/// The file: `settings` is `GET /api/settings` with the form's changes in.
#[must_use]
pub fn overrides_nix(settings: &Value, extras: &[(String, String)]) -> String {
    let mut lines = vec!["{ ... }:".to_string(), "{".to_string()];
    for (name, key, string) in OWNED {
        let value = if *string {
            nix_string(settings[*key].as_str().unwrap_or_default())
        } else {
            settings[*key].as_bool().unwrap_or(false).to_string()
        };
        lines.push(format!("  losos.{name} = {value};"));
    }
    for (name, value) in extras {
        lines.push(format!("  losos.{name} = {value};"));
    }
    lines.push("}".to_string());
    lines.push(String::new());
    lines.join("\n")
}

/// lib/api.ts's HOSTNAME_PATTERN.
#[must_use]
pub fn host_name_ok(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

/// What one field of a pane's form says: the setting's key, the value the
/// page showed and the one sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub key: &'static str,
    pub value: Value,
}

/// The fields of `form` that differ from what the page showed (`was.<key>`),
/// for the `keys` a pane draws. A switch left off is absent from a form,
/// so a key with a `was` and no value is a switch turned off.
#[must_use]
pub fn changes(
    form: &[(String, String)],
    switches: &[&'static str],
    texts: &[&'static str],
) -> Vec<Change> {
    let field = |name: &str| crate::web::field(form, name);
    let mut out = Vec::new();
    for key in switches {
        let Some(was) = field(&format!("was.{key}")) else {
            continue;
        };
        let now = field(key) == Some("on");
        if now != (was == "on") {
            out.push(Change {
                key,
                value: Value::Bool(now),
            });
        }
    }
    for key in texts {
        let (Some(was), Some(now)) = (field(&format!("was.{key}")), field(key)) else {
            continue;
        };
        let now = now.trim();
        if now != was {
            out.push(Change {
                key,
                value: Value::String(now.to_string()),
            });
        }
    }
    out
}

/// Why an apply did not start, in the box's words when it gave any.
pub struct NotStarted(pub Option<String>);

/// Write the changes over what the box has now and start the rebuild;
/// the rebuild's job when it started.
pub fn apply(host: &mut dyn Host, changes: &[Change]) -> Result<String, NotStarted> {
    let mut settings = host.get("/api/settings").ok_or(NotStarted(None))?;
    for c in changes {
        settings[c.key] = c.value.clone();
    }
    let extras = host
        .get("/api/options")
        .map(|o| extras_of(&o))
        .unwrap_or_default();
    let body = overrides_nix(&settings, &extras);
    let answer = host.call("POST", "/api/apply", Some("text/plain"), body.as_bytes());
    if answer.ok() {
        Ok(answer.json["job"].as_str().unwrap_or_default().to_string())
    } else {
        Err(NotStarted(
            answer.json["error"].as_str().map(str::to_string),
        ))
    }
}

/// `GET /api/status`, as the panes read it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub state: String,
    pub message: String,
    pub job: Option<String>,
}

impl Status {
    #[must_use]
    pub fn from_json(v: &Value) -> Status {
        Status {
            state: v["state"].as_str().unwrap_or("idle").to_string(),
            message: v["message"].as_str().unwrap_or_default().to_string(),
            job: v["job"]
                .as_str()
                .filter(|j| !j.is_empty())
                .map(str::to_string),
        }
    }

    #[must_use]
    pub fn building(&self) -> bool {
        self.state == "building"
    }
}

/// The rebuild this browser started: its job and when, kept in a cookie so
/// the page that comes back after Apply knows which outcome is its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mine {
    pub job: String,
    pub since: u64,
}

impl Mine {
    #[must_use]
    pub fn parse(cookie: &str) -> Option<Mine> {
        let (job, since) = cookie.split_once('.')?;
        let ok = !job.is_empty()
            && job.len() <= 64
            && job
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        let since = since.parse().ok()?;
        ok.then(|| Mine {
            job: job.to_string(),
            since,
        })
    }

    #[must_use]
    pub fn cookie(&self) -> String {
        format!(
            "losos_basic_job={}.{}; Path=/basic; Max-Age=86400; Secure; HttpOnly; SameSite=Lax",
            self.job, self.since
        )
    }

    #[must_use]
    pub fn gone() -> String {
        "losos_basic_job=; Path=/basic; Max-Age=0; Secure; HttpOnly; SameSite=Lax".to_string()
    }
}

/// How long a started rebuild may go unreported before the page stops
/// waiting for it (use-settings-form.ts's SETTLE_MS, a little longer: this
/// page asks less often).
pub const SETTLE_SECS: u64 = 30;

/// Where this browser's rebuild stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watch {
    /// Nothing to show.
    Idle,
    /// A rebuild runs: this browser's own, or one it joined.
    Building { ours: bool, message: String },
    /// Asked for, and not reported yet.
    Starting,
    /// Ours ended: done or failed, with the box's last line.
    Ended { done: bool, message: String },
}

#[must_use]
pub fn watch(status: &Status, mine: Option<&Mine>, now: u64) -> Watch {
    let ours = |s: &Status| mine.is_some_and(|m| s.job.as_deref() == Some(m.job.as_str()));
    if status.building() {
        return Watch::Building {
            ours: ours(status),
            message: status.message.clone(),
        };
    }
    let Some(m) = mine else { return Watch::Idle };
    if ours(status) && matches!(status.state.as_str(), "done" | "failed") {
        return Watch::Ended {
            done: status.state == "done",
            message: status.message.clone(),
        };
    }
    if now.saturating_sub(m.since) < SETTLE_SECS {
        Watch::Starting
    } else {
        Watch::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings() -> Value {
        json!({
            "sharingMyStorage": false, "hostName": "losos", "gpuEnable": false,
            "proxyEnable": true, "clusterEnable": false, "shareCompute": false,
            "computeWindowStart": "23:00", "computeWindowEnd": "07:00",
            "hardeningApparmor": false, "hardeningMalloc": false,
            "hardeningNosmt": false, "hardeningUsbguard": true
        })
    }

    #[test]
    fn the_file_reads_like_the_script_pages_file() {
        let extras = extras_of(&json!({
            "options": [
                {"name": "hostName", "set": "\"x\""},
                {"name": "cloud.trashDays", "set": "30"},
                {"name": "git.enable", "set": null}
            ],
            "stray": [{"key": "old.thing", "value": "true"}]
        }));
        assert_eq!(
            extras,
            vec![
                ("cloud.trashDays".into(), "30".into()),
                ("old.thing".into(), "true".into())
            ]
        );
        let nix = overrides_nix(&settings(), &extras);
        assert!(nix.starts_with(
            "{ ... }:\n{\n  losos.sharingMyStorage = false;\n  losos.hostName = \"losos\";\n"
        ));
        assert!(nix.contains("  losos.hardening.usbguard = true;\n  losos.cloud.trashDays = 30;\n  losos.old.thing = true;\n}\n"));
        assert_eq!(nix_string("a\"b${c}\\"), "\"a\\\"b\\${c}\\\\\"");
    }

    #[test]
    fn only_what_was_changed_is_sent() {
        let form = crate::web::form(b"was.gpuEnable=off&gpuEnable=on&was.hardeningMalloc=on&was.hardeningNosmt=off&was.hostName=losos&hostName=losos+");
        let c = changes(
            &form,
            &["gpuEnable", "hardeningMalloc", "hardeningNosmt"],
            &["hostName"],
        );
        assert_eq!(
            c,
            vec![
                Change {
                    key: "gpuEnable",
                    value: json!(true)
                },
                Change {
                    key: "hardeningMalloc",
                    value: json!(false)
                },
            ]
        );
    }

    #[test]
    fn host_names_have_the_script_pages_shape() {
        assert!(host_name_ok("losos-2"));
        for bad in ["", "-a", "a-", "a b", "a.b", &"a".repeat(64)] {
            assert!(!host_name_ok(bad), "{bad}");
        }
    }

    #[test]
    fn a_rebuild_is_ours_by_its_job() {
        let mine = Mine::parse("j-1.100").unwrap();
        assert_eq!(
            Mine::parse(mine.cookie()[16..].split(';').next().unwrap()),
            Some(mine.clone())
        );
        assert!(Mine::parse("a;b.1").is_none());
        let s = |state: &str, job: &str| Status {
            state: state.into(),
            message: "m".into(),
            job: Some(job.into()),
        };
        assert_eq!(
            watch(&s("building", "j-1"), Some(&mine), 101),
            Watch::Building {
                ours: true,
                message: "m".into()
            }
        );
        assert_eq!(
            watch(&s("building", "other"), None, 0),
            Watch::Building {
                ours: false,
                message: "m".into()
            }
        );
        assert_eq!(watch(&s("done", "j-0"), Some(&mine), 105), Watch::Starting);
        assert_eq!(watch(&s("done", "j-0"), Some(&mine), 200), Watch::Idle);
        assert_eq!(
            watch(&s("failed", "j-1"), Some(&mine), 900),
            Watch::Ended {
                done: false,
                message: "m".into()
            }
        );
    }
}
