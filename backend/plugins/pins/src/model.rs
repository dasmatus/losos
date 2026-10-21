//! What the owner pinned to the admin page's sidebar.
//!
//! A pin is a reference, never a link: an app tile of the Overview by its
//! id in the page's catalogue, an app installed from the Apps search by its
//! release name, a file or folder in LosOS cloud by its file id and path,
//! or a repository on LosOS Git by owner and name. The page turns each one
//! into an address (`admin-ui/app/src/lib/pins.ts`), so nothing stored here
//! can send a click anywhere but this box. The list is ordered, and the
//! order is the owner's: `PUT /api/pins` replaces it whole, which is also
//! how a pin moves.
//!
//! Like the look, the list is appliance state beside `state.json`, so a
//! phone and a laptop show the same sidebar, and writing it starts no
//! rebuild. The two lists the picker pins from (LosOS cloud's files and
//! LosOS Git's repositories) are still lososd's own, `pins.rs` there.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// How many pins the sidebar keeps. A sidebar of more is a second app grid.
pub const MAX_PINS: usize = 24;
/// Longest LosOS cloud path a pin takes, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;
/// Longest catalogue id of a LosOS cloud app.
pub const MAX_APP_ID_CHARS: usize = 24;
/// Longest owner or repository name on LosOS Git (Forgejo's own limit).
pub const MAX_REPO_PART_CHARS: usize = 100;
/// Longest release name of an installed app, as lososd's `apps.rs` names
/// them: the namespace prefix still has to fit a DNS label.
pub const MAX_RELEASE_CHARS: usize = 40;

/// One thing on the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Pin {
    /// An app tile of the Overview, by its id in the page's catalogue
    /// (`admin-ui/app/src/lib/apps.ts`): the apps of LosOS cloud, LosOS
    /// Git and this page's settings.
    Tile { app: String },
    /// An app installed from the Apps search, by release name.
    App { release: String },
    /// A file or folder in LosOS cloud. `id` opens it; `path` names it.
    File { id: u64, path: String, folder: bool },
    /// A repository on LosOS Git.
    Repo { owner: String, name: String },
}

impl Pin {
    /// What makes two pins the same pin.
    fn key(&self) -> String {
        match self {
            Pin::Tile { app } => format!("tile:{app}"),
            Pin::App { release } => format!("app:{release}"),
            Pin::File { id, .. } => format!("file:{id}"),
            Pin::Repo { owner, name } => format!("repo:{owner}/{name}"),
        }
    }
}

/// The document on disk, `pins.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pins {
    pub pins: Vec<Pin>,
}

/// Where the list lives: the file, or memory in the tests.
pub trait Store {
    /// The list, or an empty one when there is none yet or it does not
    /// parse: a damaged file costs the owner their pins, not the page.
    fn load(&mut self) -> Pins;
    fn save(&mut self, pins: &Pins) -> anyhow::Result<()>;
}

/// A request the owner can fix by sending a different one; the plugin
/// answers it as a 400 carrying the sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(pub String);

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Invalid {}

fn invalid<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::Error::new(Invalid(message.into())))
}

fn render(pins: &Pins) -> Value {
    json!({ "pins": pins.pins, "limit": MAX_PINS })
}

/// Read a `PUT /api/pins` body: `{"pins": [...]}`.
pub fn parse_pins(doc: &Value) -> anyhow::Result<Vec<Pin>> {
    let Some(list) = doc.get("pins").and_then(Value::as_array) else {
        return invalid("Send the pins as a list under \"pins\".");
    };
    list.iter()
        .map(|item| {
            serde_json::from_value::<Pin>(item.clone()).map_err(|_| {
                anyhow::Error::new(Invalid("That is not a pin this box knows.".into()))
            })
        })
        .collect()
}

/// Check one pin's fields.
pub fn validate_pin(pin: &Pin) -> anyhow::Result<()> {
    match pin {
        Pin::Tile { app } => {
            let ok = !app.is_empty()
                && app.chars().count() <= MAX_APP_ID_CHARS
                && app.chars().all(|c| c.is_ascii_lowercase());
            if !ok {
                return invalid("That is not an app on the Overview.");
            }
        }
        Pin::App { release } => {
            if !release_name(release) {
                return invalid("That is not the name of an installed app.");
            }
        }
        Pin::File { id, path, .. } => {
            if *id == 0 {
                return invalid("That file has no id in LosOS cloud.");
            }
            validate_path(path)?;
            if path == "/" {
                return invalid("Pin a folder inside LosOS cloud, not the whole of it.");
            }
        }
        Pin::Repo { owner, name } => {
            if !repo_part(owner) || !repo_part(name) {
                return invalid("That is not a repository on LosOS Git.");
            }
        }
    }
    Ok(())
}

/// A release name as lososd's `apps::validate_release` takes it: small
/// letters, digits and dashes, a letter first, no dash last.
fn release_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_RELEASE_CHARS
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn repo_part(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part.chars().count() <= MAX_REPO_PART_CHARS
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A path inside LosOS cloud: from the top (`/`), no empty, `.` or `..`
/// part, nothing that is not printable.
pub fn validate_path(path: &str) -> anyhow::Result<()> {
    if !path.starts_with('/') || path.len() > MAX_PATH_BYTES {
        return invalid("That is not a folder in LosOS cloud.");
    }
    if path == "/" {
        return Ok(());
    }
    let ok = path
        .trim_end_matches('/')
        .split('/')
        .skip(1)
        .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(['\\']))
        && !path.chars().any(char::is_control);
    if ok {
        Ok(())
    } else {
        invalid("That is not a folder in LosOS cloud.")
    }
}

/// `GET /api/pins`.
pub fn cmd_pins(s: &mut impl Store) -> Value {
    render(&s.load())
}

/// `PUT /api/pins`: the whole list, in the owner's order.
pub fn cmd_put_pins(s: &mut impl Store, pins: &[Pin]) -> anyhow::Result<Value> {
    if pins.len() > MAX_PINS {
        return invalid(format!("The sidebar keeps {MAX_PINS} pins at most."));
    }
    let mut seen = std::collections::HashSet::new();
    for pin in pins {
        validate_pin(pin)?;
        if !seen.insert(pin.key()) {
            return invalid("That is pinned already.");
        }
    }
    let doc = Pins {
        pins: pins.to_vec(),
    };
    s.save(&doc)?;
    Ok(render(&doc))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list in memory.
    #[derive(Default)]
    struct Mem(Pins);

    impl Store for Mem {
        fn load(&mut self) -> Pins {
            self.0.clone()
        }
        fn save(&mut self, pins: &Pins) -> anyhow::Result<()> {
            self.0 = pins.clone();
            Ok(())
        }
    }

    fn file(id: u64, path: &str) -> Pin {
        Pin::File {
            id,
            path: path.into(),
            folder: false,
        }
    }

    #[test]
    fn a_list_round_trips_in_the_owners_order() {
        let mut s = Mem::default();
        let pins = vec![
            Pin::Repo {
                owner: "notshared".into(),
                name: "site".into(),
            },
            Pin::Tile {
                app: "calendar".into(),
            },
            file(42, "/Documents/plan.pdf"),
            Pin::App {
                release: "jellyfin".into(),
            },
        ];
        let out = cmd_put_pins(&mut s, &pins).unwrap();
        assert_eq!(out["limit"], MAX_PINS);
        assert_eq!(out["pins"][0]["kind"], "repo");
        assert_eq!(
            out["pins"][2],
            json!({"kind":"file","id":42,"path":"/Documents/plan.pdf","folder":false})
        );
        assert_eq!(cmd_pins(&mut s), out);
        assert_eq!(parse_pins(&out).unwrap(), pins);
    }

    #[test]
    fn duplicates_bad_fields_and_too_many_are_refused_and_nothing_is_written() {
        let mut s = Mem::default();
        let refuse = |s: &mut Mem, pins: &[Pin]| {
            let e = cmd_put_pins(s, pins).unwrap_err();
            assert!(e.downcast_ref::<Invalid>().is_some(), "untyped: {e}");
        };
        refuse(&mut s, &[file(1, "/a"), file(1, "/b")]);
        refuse(&mut s, &[file(0, "/a")]);
        refuse(&mut s, &[file(1, "/")]);
        refuse(&mut s, &[file(1, "/a/../etc")]);
        refuse(&mut s, &[file(1, "relative")]);
        refuse(
            &mut s,
            &[Pin::Tile {
                app: "Files".into(),
            }],
        );
        refuse(
            &mut s,
            &[Pin::App {
                release: "Bad Name".into(),
            }],
        );
        refuse(
            &mut s,
            &[Pin::App {
                release: "trailing-".into(),
            }],
        );
        refuse(
            &mut s,
            &[Pin::Repo {
                owner: "..".into(),
                name: "x".into(),
            }],
        );
        let many: Vec<Pin> = (1..=(MAX_PINS as u64 + 1)).map(|i| file(i, "/a")).collect();
        refuse(&mut s, &many);
        assert!(cmd_pins(&mut s)["pins"].as_array().unwrap().is_empty());
    }

    #[test]
    fn a_pin_with_a_field_of_its_own_choosing_is_not_a_pin() {
        // No href, no label: a stored pin cannot carry an address.
        let doc =
            json!({"pins": [{"kind": "repo", "owner": "a", "name": "b", "href": "https://x"}]});
        assert!(parse_pins(&doc).is_err());
        assert!(parse_pins(&json!({"pins": [{"kind": "link"}]})).is_err());
        assert!(parse_pins(&json!({})).is_err());
    }
}
