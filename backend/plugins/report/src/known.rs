//! Problems the handbook already explains (`known.json` beside this crate).
//!
//! A report is for what nobody has written down yet. When a failure's own
//! words hold one of an entry's phrases, the box and the installer name the
//! handbook page instead of offering to send a report: the page says what
//! to do, and a maintainer reading the same thing again learns nothing.
//! A crash, a program the kernel killed for memory and a usage spike are
//! never documented this way: each is a bug, or a box too small, worth
//! hearing about.

use crate::normalise;
use serde_json::Value;

const KNOWN: &str = include_str!("../known.json");

/// A documented problem: where the handbook explains it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    pub id: String,
    /// The page under the handbook's root, `troubleshooting/out-of-room`.
    pub page: String,
    /// The page's title in English, Slovak and German.
    pub title: [String; 3],
}

impl Known {
    /// The title in `lang` (`en`, `sk`, `de`), English otherwise.
    #[must_use]
    pub fn title_in(&self, lang: &str) -> &str {
        match lang {
            "sk" => &self.title[1],
            "de" => &self.title[2],
            _ => &self.title[0],
        }
    }
}

/// Every entry of `known.json`.
#[must_use]
pub fn all() -> Vec<(Known, Vec<String>)> {
    let doc: Value = serde_json::from_str(KNOWN).unwrap_or(Value::Null);
    doc["known"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|k| {
            let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
            let known = Known {
                id: k["id"].as_str()?.to_owned(),
                page: k["page"].as_str()?.to_owned(),
                title: [
                    text(&k["title"]["en"]),
                    text(&k["title"]["sk"]),
                    text(&k["title"]["de"]),
                ],
            };
            let when = k["when"]
                .as_array()?
                .iter()
                .filter_map(|w| w.as_str().map(normalise))
                .filter(|w| !w.is_empty())
                .collect();
            Some((known, when))
        })
        .collect()
}

/// The page that explains a failure whose own words are `text`: its
/// message and the last lines of its log.
#[must_use]
pub fn documented(text: &str) -> Option<Known> {
    let text = normalise(text);
    all()
        .into_iter()
        .find(|(_, when)| when.iter().any(|w| text.contains(w.as_str())))
        .map(|(known, _)| known)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_loads_and_names_a_page() {
        let all = all();
        assert!(all.len() >= 3);
        for (known, when) in &all {
            assert!(!when.is_empty(), "{} has no phrases", known.id);
            assert!(known.title.iter().all(|t| !t.is_empty()), "{}", known.id);
            assert!(!known.page.starts_with('/'), "{}", known.page);
        }
    }

    #[test]
    fn a_full_disk_is_documented_and_a_crash_message_is_not() {
        let k =
            documented("cp: error writing '/mnt/nix/store/x': No space left on device").unwrap();
        assert_eq!(k.page, "troubleshooting/out-of-room");
        assert_eq!(k.title_in("de"), "Kein Platz mehr");
        assert_eq!(
            documented("Main process exited, code=dumped, status=11/SEGV"),
            None
        );
        let k = documented("No network: cache.nixos.org did not resolve within 600s").unwrap();
        assert_eq!(k.id, "installer-no-network");
    }
}
