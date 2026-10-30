//! The groups of units Settings › Logs offers: lososd, LosOS Cloud, the
//! edge's tunnel and the rest, as `modules/log-reader.nix` writes them to
//! `/etc/losos/log-units.json` for the services this box runs.
//!
//! ```json
//! { "sources": [ { "id": "lososd", "units": ["lososd.service"] },
//!                { "id": "kernel", "kernel": true } ] }
//! ```
//!
//! Two more are always there: `losos`, every unit of the list at once, and
//! `everything`, the whole journal. A unit's own name works too, so a
//! problem report can link to one the list does not hold.

use serde::Deserialize;
use std::path::Path;

/// Every LosOS unit at once.
pub const LOSOS: &str = "losos";
/// The whole journal.
pub const EVERYTHING: &str = "everything";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Source {
    pub id: String,
    /// Unit names, globs allowed (`losos-rebuild-*.service`).
    #[serde(default)]
    pub units: Vec<String>,
    /// The kernel's own lines rather than a unit's.
    #[serde(default)]
    pub kernel: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Catalogue {
    pub sources: Vec<Source>,
}

/// What a `unit=` asks the journal for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Units {
    /// These units' lines.
    Named(Vec<String>),
    /// The kernel's lines.
    Kernel,
    /// Every line.
    All,
}

impl Default for Catalogue {
    /// lososd alone, for a box whose list is missing.
    fn default() -> Catalogue {
        Catalogue {
            sources: vec![Source {
                id: "lososd".to_string(),
                units: vec!["lososd.service".to_string()],
                kernel: false,
            }],
        }
    }
}

impl Catalogue {
    /// The list in `path`, or [`Catalogue::default`] when it is missing or
    /// unreadable (said in the reader's own log).
    #[must_use]
    pub fn load(path: &Path) -> Catalogue {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Catalogue>(&bytes) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "the unit list could not be read");
                    Catalogue::default()
                }
            },
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "no unit list");
                Catalogue::default()
            }
        }
    }

    /// The ids on the Service list, in its order: the groups, then
    /// [`LOSOS`] and [`EVERYTHING`].
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.sources
            .iter()
            .map(|s| s.id.clone())
            .chain([LOSOS.to_string(), EVERYTHING.to_string()])
            .collect()
    }

    /// What `unit` (an id, or a unit's name) asks for; `None` for something
    /// that is neither.
    #[must_use]
    pub fn resolve(&self, unit: &str) -> Option<Units> {
        if unit == EVERYTHING {
            return Some(Units::All);
        }
        if unit == LOSOS {
            let mut all: Vec<String> = Vec::new();
            for s in &self.sources {
                for u in &s.units {
                    if !all.contains(u) {
                        all.push(u.clone());
                    }
                }
            }
            return Some(Units::Named(all));
        }
        if let Some(s) = self.sources.iter().find(|s| s.id == unit) {
            return Some(if s.kernel {
                Units::Kernel
            } else {
                Units::Named(s.units.clone())
            });
        }
        unit_name(unit).then(|| Units::Named(vec![unit.to_string()]))
    }
}

/// Whether `name` can be a systemd unit's name: no glob, no option, no
/// path. journalctl adds `.service` to one without a type.
#[must_use]
pub fn unit_name(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && !name.starts_with(['-', '.'])
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b":_.@-\\".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Catalogue {
        serde_json::from_str(
            r#"{"sources":[
                {"id":"lososd","units":["lososd.service"]},
                {"id":"rebuilds","units":["losos-rebuild-*.service","nixos-upgrade.service"]},
                {"id":"cloud","units":["losos-cloud.service","lososd.service"]},
                {"id":"kernel","kernel":true}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn ids_end_with_the_two_wide_ones() {
        assert_eq!(
            list().ids(),
            [
                "lososd",
                "rebuilds",
                "cloud",
                "kernel",
                "losos",
                "everything"
            ]
        );
    }

    #[test]
    fn a_source_a_name_or_nothing() {
        let c = list();
        assert_eq!(
            c.resolve("rebuilds"),
            Some(Units::Named(vec![
                "losos-rebuild-*.service".into(),
                "nixos-upgrade.service".into()
            ]))
        );
        assert_eq!(c.resolve("kernel"), Some(Units::Kernel));
        assert_eq!(c.resolve("everything"), Some(Units::All));
        assert_eq!(
            c.resolve("losos"),
            Some(Units::Named(vec![
                "lososd.service".into(),
                "losos-rebuild-*.service".into(),
                "nixos-upgrade.service".into(),
                "losos-cloud.service".into()
            ]))
        );
        assert_eq!(
            c.resolve("systemd-networkd.service"),
            Some(Units::Named(vec!["systemd-networkd.service".into()]))
        );
        assert_eq!(
            c.resolve("getty@tty1.service"),
            Some(Units::Named(vec!["getty@tty1.service".into()]))
        );
        for bad in ["*", "-f", "a b", "../x", "", "losos-*.service", "x;y"] {
            assert_eq!(c.resolve(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_missing_list_is_lososd_alone() {
        let c = Catalogue::load(Path::new("/nonexistent/log-units.json"));
        assert_eq!(c.ids(), ["lososd", "losos", "everything"]);
    }
}
