//! Pictures that reach someone in LosOS Chat and LosOS Mail: which of them
//! arrive blurred, whom the person trusts, and whether the nudity warning
//! is on.
//!
//! Every picture that arrives is blurred until the person presses it, unless
//! the sender is someone they trust (`trusted`), or their setting is
//! `strangers` and the sender is a person on this box or in their contacts.
//! The third setting, `nobody`, shows every picture at once; only a person
//! marked as an adult may choose it. The mark comes from one of two places:
//! a Binance purchase the person started finishing (Binance checks the
//! buyer's age before it takes a payment), or an administrator in People.
//! An administrator may mark a person as a child instead. A child keeps the
//! nudity warning on and may not choose `nobody`, and a Binance purchase
//! does not change an administrator's child mark.
//!
//! Nothing here looks at a picture. The blur is drawn by the person's own
//! browser, and so is the nudity warning (`nudityCheck`): the page scores
//! the picture on the device and the box never sees what it scored.
//!
//! The settings live in `$LOSOS_PICTURES_FILE` (`/var/lib/losos/pictures.json`,
//! in lososd's state directory), one entry per sign-in name, beside the
//! Binance orders that are waiting to finish.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::people::BadDetails;

const DEFAULT_FILE: &str = "/var/lib/losos/pictures.json";
/// Most people one person may trust.
pub const MAX_TRUSTED: usize = 500;
/// Longest address or chat ID a trusted entry may have.
const MAX_ID_LEN: usize = 255;
/// A Binance order not heard of for this long is dropped from the list of
/// orders waiting to finish: Binance's checkout link lasts minutes, and a
/// payment that takes longer than this has failed.
const ORDER_TTL_SECS: u64 = 30 * 24 * 60 * 60;
/// Most orders waiting at once, across the box.
const MAX_ORDERS: usize = 256;

/// Which pictures arrive blurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Blur {
    /// Every picture, but those from people the person trusts.
    #[default]
    Everyone,
    /// Pictures from people who are neither on this box, in the person's
    /// contacts nor trusted.
    Strangers,
    /// None. Only for a person marked as an adult.
    Nobody,
}

impl Blur {
    pub fn parse(s: &str) -> Option<Blur> {
        match s {
            "everyone" => Some(Blur::Everyone),
            "strangers" => Some(Blur::Strangers),
            "nobody" => Some(Blur::Nobody),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Age {
    Adult,
    Child,
}

impl Age {
    pub fn parse(s: &str) -> Option<Age> {
        match s {
            "adult" => Some(Age::Adult),
            "child" => Some(Age::Child),
            _ => None,
        }
    }
}

/// Who set a person's age mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum By {
    /// A Binance purchase the person started finished.
    Binance,
    /// An administrator, in People.
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub age: Age,
    pub by: By,
    /// Seconds since the epoch.
    pub since: u64,
}

/// One person's settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub blur: Blur,
    /// Chat IDs and mail addresses, folded to lower case.
    #[serde(default)]
    pub trusted: Vec<String>,
    /// The person's own choice; a child has it on whatever this says.
    #[serde(default)]
    pub nudity_check: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age: Option<Mark>,
}

impl Settings {
    pub fn is_child(&self) -> bool {
        self.age.is_some_and(|m| m.age == Age::Child)
    }

    pub fn is_adult(&self) -> bool {
        self.age.is_some_and(|m| m.age == Age::Adult)
    }

    /// The blur in force: a child's `nobody` (set before the mark) counts as
    /// `everyone`.
    pub fn blur(&self) -> Blur {
        if self.blur == Blur::Nobody && !self.is_adult() {
            Blur::Everyone
        } else {
            self.blur
        }
    }

    /// Whether the nudity warning is on.
    pub fn nudity_check(&self) -> bool {
        self.nudity_check || self.is_child()
    }

    pub fn trusts(&self, id: &str) -> bool {
        let id = fold_id(id);
        self.trusted.contains(&id)
    }
}

/// What the page is told about one person's settings.
pub fn render(s: &Settings) -> Value {
    json!({
        "blur": s.blur(),
        "trusted": s.trusted,
        "nudityCheck": s.nudity_check(),
        "age": s.age,
        "child": s.is_child(),
        "mayShowAll": s.is_adult(),
    })
}

/// A change the person asks for (`POST /api/pictures`). Each field is
/// optional.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub blur: Option<Blur>,
    pub nudity_check: Option<bool>,
}

/// Read a change strictly: unknown fields and wrong types are refused.
pub fn parse_change(doc: &Value) -> Result<Change, String> {
    let Some(map) = doc.as_object() else {
        return Err("the body must be a JSON object".into());
    };
    let mut change = Change::default();
    for (key, value) in map {
        match key.as_str() {
            "blur" => {
                change.blur = Some(
                    value
                        .as_str()
                        .and_then(Blur::parse)
                        .ok_or("blur is everyone, strangers or nobody")?,
                );
            }
            "nudityCheck" => {
                change.nudity_check = Some(value.as_bool().ok_or("nudityCheck is true or false")?);
            }
            other => return Err(format!("{other} is not a picture setting")),
        }
    }
    Ok(change)
}

/// Why a change was refused, for the page to say in its own words.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// `nobody` without the adult mark.
    NotAdult,
    /// The nudity warning off, or `nobody`, for a child.
    Child,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::NotAdult => f.write_str(
                "showing every picture needs an adult mark: buy with Binance once, or ask an administrator",
            ),
            Refused::Child => {
                f.write_str("this account is marked as a child's, so the picture settings stay on")
            }
        }
    }
}

impl std::error::Error for Refused {}

impl Refused {
    /// The word the page keys its message on.
    pub fn code(&self) -> &'static str {
        match self {
            Refused::NotAdult => "adultRequired",
            Refused::Child => "childAccount",
        }
    }
}

/// A chat ID (`@name:server`) or a mail address, folded to lower case, or
/// `None` for anything else.
pub fn valid_id(id: &str) -> Option<String> {
    let id = id.trim();
    if id.is_empty()
        || id.len() > MAX_ID_LEN
        || id.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return None;
    }
    let ok = if let Some(rest) = id.strip_prefix('@') {
        matches!(rest.split_once(':'), Some((local, server)) if !local.is_empty() && !server.is_empty())
    } else {
        matches!(id.split_once('@'), Some((local, domain)) if !local.is_empty() && domain.contains('.') && !domain.contains('@'))
    };
    ok.then(|| fold_id(id))
}

fn fold_id(id: &str) -> String {
    id.trim().to_lowercase()
}

/// Why a sender's pictures show at once, or `None` when they arrive
/// blurred. `known` says whether the sender is on this box or in the
/// person's contacts; it is asked only when the answer depends on it.
pub fn shown_because(
    s: &Settings,
    sender: &str,
    known: impl FnOnce() -> bool,
) -> Option<&'static str> {
    if s.trusts(sender) {
        return Some("trusted");
    }
    match s.blur() {
        Blur::Nobody => Some("setting"),
        Blur::Strangers if known() => Some("known"),
        Blur::Strangers | Blur::Everyone => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    name: String,
    at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Document {
    #[serde(default)]
    people: BTreeMap<String, Settings>,
    /// Binance orders waiting to finish, by order ID, with the person who
    /// started each.
    #[serde(default)]
    orders: BTreeMap<String, Order>,
}

/// Every person's picture settings. One lock around every read-modify-write.
pub struct Pictures {
    file: PathBuf,
    lock: Mutex<()>,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Pictures {
    pub fn new(file: impl Into<PathBuf>) -> Pictures {
        Pictures {
            file: file.into(),
            lock: Mutex::new(()),
        }
    }

    pub fn from_env() -> Pictures {
        Pictures::new(
            std::env::var_os("LOSOS_PICTURES_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_FILE)),
        )
    }

    fn check_name(name: &str) -> anyhow::Result<()> {
        crate::people::validate_name(name)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn read(&self) -> anyhow::Result<Document> {
        match std::fs::read(&self.file) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", self.file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Document::default()),
            Err(e) => Err(anyhow::anyhow!("reading {}: {e}", self.file.display())),
        }
    }

    fn write(&self, doc: &Document) -> anyhow::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.file.with_extension("tmp");
        {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(doc)?)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.file)?;
        Ok(())
    }

    fn edit<T>(
        &self,
        name: &str,
        f: impl FnOnce(&mut Settings) -> anyhow::Result<T>,
    ) -> anyhow::Result<Settings> {
        Pictures::check_name(name)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let mut settings = doc.people.get(name).cloned().unwrap_or_default();
        f(&mut settings)?;
        if settings == Settings::default() {
            doc.people.remove(name);
        } else {
            doc.people.insert(name.to_string(), settings.clone());
        }
        self.write(&doc)?;
        Ok(settings)
    }

    /// `name`'s settings, or the defaults.
    pub fn get(&self, name: &str) -> anyhow::Result<Settings> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read()?.people.remove(name).unwrap_or_default())
    }

    /// Everyone's age mark, for People.
    pub fn marks(&self) -> anyhow::Result<BTreeMap<String, Mark>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self
            .read()?
            .people
            .into_iter()
            .filter_map(|(name, s)| s.age.map(|m| (name, m)))
            .collect())
    }

    /// The person's own change. Refused with [`Refused`] when the age mark
    /// does not allow it.
    pub fn set(&self, name: &str, change: &Change) -> anyhow::Result<Settings> {
        self.edit(name, |s| {
            if s.is_child()
                && (change.nudity_check == Some(false) || change.blur == Some(Blur::Nobody))
            {
                return Err(Refused::Child.into());
            }
            if change.blur == Some(Blur::Nobody) && !s.is_adult() {
                return Err(Refused::NotAdult.into());
            }
            if let Some(blur) = change.blur {
                s.blur = blur;
            }
            if let Some(on) = change.nudity_check {
                s.nudity_check = on;
            }
            Ok(())
        })
    }

    /// Trust `id`, or stop trusting it.
    pub fn trust(&self, name: &str, id: &str, trusted: bool) -> anyhow::Result<Settings> {
        let Some(id) = valid_id(id) else {
            return Err(BadDetails(
                "id is a chat ID such as @ana:box.local or a mail address".into(),
            )
            .into());
        };
        self.edit(name, |s| {
            s.trusted.retain(|t| *t != id);
            if trusted {
                if s.trusted.len() >= MAX_TRUSTED {
                    return Err(
                        BadDetails(format!("at most {MAX_TRUSTED} people can be trusted")).into(),
                    );
                }
                s.trusted.push(id);
                s.trusted.sort();
            }
            Ok(())
        })
    }

    /// An administrator's mark: adult, child, or none.
    pub fn set_age(&self, name: &str, age: Option<Age>) -> anyhow::Result<Settings> {
        self.edit(name, |s| {
            s.age = age.map(|age| Mark {
                age,
                by: By::Admin,
                since: now(),
            });
            Ok(())
        })
    }

    /// Note that `name` started the Binance order `order_id`.
    pub fn order_started(&self, name: &str, order_id: &str) -> anyhow::Result<()> {
        Pictures::check_name(name)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let now = now();
        doc.orders
            .retain(|_, o| now.saturating_sub(o.at) < ORDER_TTL_SECS);
        while doc.orders.len() >= MAX_ORDERS {
            let Some(oldest) = doc
                .orders
                .iter()
                .min_by_key(|(_, o)| o.at)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            doc.orders.remove(&oldest);
        }
        doc.orders.insert(
            order_id.to_string(),
            Order {
                name: name.to_string(),
                at: now,
            },
        );
        self.write(&doc)
    }

    /// Binance said where `order_id` is. A finished purchase (`done`: the
    /// coins left Binance, or `kept`: they stayed in the buyer's account)
    /// marks the person who started it as an adult, unless an
    /// administrator marked them as a child. A finished or failed order is
    /// then forgotten. Returns the person marked, if one was.
    pub fn order_stage(&self, order_id: &str, stage: &str) -> anyhow::Result<Option<String>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let finished = matches!(stage, "done" | "kept");
        if !finished && stage != "failed" {
            return Ok(None);
        }
        let Some(order) = doc.orders.remove(order_id) else {
            return Ok(None);
        };
        let mut marked = None;
        if finished {
            let s = doc.people.entry(order.name.clone()).or_default();
            let admin_child = s
                .age
                .is_some_and(|m| m.age == Age::Child && m.by == By::Admin);
            if !admin_child && !s.is_adult() {
                s.age = Some(Mark {
                    age: Age::Adult,
                    by: By::Binance,
                    since: now(),
                });
                marked = Some(order.name);
            }
        }
        self.write(&doc)?;
        Ok(marked)
    }

    /// Forget everything about `name`, for a person taken off the box.
    pub fn forget(&self, name: &str) -> anyhow::Result<()> {
        Pictures::check_name(name)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let before = (doc.people.len(), doc.orders.len());
        doc.people.remove(name);
        doc.orders.retain(|_, o| o.name != name);
        if before != (doc.people.len(), doc.orders.len()) {
            self.write(&doc)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (assert_fs::TempDir, Pictures) {
        let dir = assert_fs::TempDir::new().unwrap();
        let pictures = Pictures::new(dir.path().join("pictures.json"));
        (dir, pictures)
    }

    fn refused(e: anyhow::Error) -> Refused {
        e.downcast::<Refused>().unwrap()
    }

    const ORDER: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn a_new_person_has_every_picture_blurred_and_nothing_stored() {
        let (_d, p) = store();
        let s = p.get("ana").unwrap();
        assert_eq!(s.blur(), Blur::Everyone);
        assert!(!s.nudity_check());
        assert_eq!(render(&s)["mayShowAll"], json!(false));
        assert!(!p.file.exists());
    }

    #[test]
    fn showing_every_picture_needs_the_adult_mark() {
        let (_d, p) = store();
        let nobody = Change {
            blur: Some(Blur::Nobody),
            ..Change::default()
        };
        assert_eq!(
            refused(p.set("ana", &nobody).unwrap_err()),
            Refused::NotAdult
        );
        p.set_age("ana", Some(Age::Adult)).unwrap();
        assert_eq!(p.set("ana", &nobody).unwrap().blur(), Blur::Nobody);
        // The mark taken back: the stored choice no longer counts.
        p.set_age("ana", None).unwrap();
        assert_eq!(p.get("ana").unwrap().blur(), Blur::Everyone);
    }

    #[test]
    fn a_child_keeps_the_warning_and_the_blur() {
        let (_d, p) = store();
        p.set_age("tom", Some(Age::Child)).unwrap();
        let s = p.get("tom").unwrap();
        assert!(s.nudity_check());
        let off = Change {
            nudity_check: Some(false),
            ..Change::default()
        };
        assert_eq!(refused(p.set("tom", &off).unwrap_err()), Refused::Child);
        let strangers = Change {
            blur: Some(Blur::Strangers),
            ..Change::default()
        };
        assert_eq!(p.set("tom", &strangers).unwrap().blur(), Blur::Strangers);
    }

    #[test]
    fn a_finished_binance_purchase_marks_whoever_started_it() {
        let (_d, p) = store();
        p.order_started("ana", ORDER).unwrap();
        assert_eq!(p.order_stage(ORDER, "paying").unwrap(), None);
        assert_eq!(
            p.order_stage(ORDER, "done").unwrap().as_deref(),
            Some("ana")
        );
        let mark = p.get("ana").unwrap().age.unwrap();
        assert_eq!((mark.age, mark.by), (Age::Adult, By::Binance));
        // Asked again, the order is gone and nothing changes.
        assert_eq!(p.order_stage(ORDER, "done").unwrap(), None);
    }

    #[test]
    fn coins_kept_on_binance_count_and_a_failed_order_does_not() {
        let (_d, p) = store();
        p.order_started("ana", ORDER).unwrap();
        p.order_started("bo", &ORDER.replace('0', "1")).unwrap();
        assert_eq!(
            p.order_stage(ORDER, "kept").unwrap().as_deref(),
            Some("ana")
        );
        assert_eq!(
            p.order_stage(&ORDER.replace('0', "1"), "failed").unwrap(),
            None
        );
        assert!(p.get("bo").unwrap().age.is_none());
    }

    #[test]
    fn an_order_nobody_here_started_marks_nobody() {
        let (_d, p) = store();
        assert_eq!(p.order_stage(ORDER, "done").unwrap(), None);
        assert!(p.marks().unwrap().is_empty());
    }

    #[test]
    fn a_purchase_never_overrules_an_administrators_child_mark() {
        let (_d, p) = store();
        p.set_age("tom", Some(Age::Child)).unwrap();
        p.order_started("tom", ORDER).unwrap();
        assert_eq!(p.order_stage(ORDER, "done").unwrap(), None);
        assert!(p.get("tom").unwrap().is_child());
    }

    #[test]
    fn trusted_people_are_folded_kept_once_and_shown_at_once() {
        let (_d, p) = store();
        p.trust("ana", "@Bo:Box.local", true).unwrap();
        p.trust("ana", "@bo:box.local", true).unwrap();
        let s = p.trust("ana", "Eva@Example.org", true).unwrap();
        assert_eq!(s.trusted, vec!["@bo:box.local", "eva@example.org"]);
        assert_eq!(
            shown_because(&s, "@BO:box.local", || false),
            Some("trusted")
        );
        assert_eq!(shown_because(&s, "@cy:box.local", || true), None);
        let s = p.trust("ana", "eva@example.org", false).unwrap();
        assert_eq!(s.trusted, vec!["@bo:box.local"]);
    }

    #[test]
    fn only_chat_ids_and_addresses_can_be_trusted() {
        for bad in [
            "",
            "bo",
            "@bo",
            "@:box",
            "bo@",
            "bo@box",
            "a@b@c.d",
            "@bo:b\u{7}ox",
            "b o@x.org",
        ] {
            assert_eq!(valid_id(bad), None, "{bad:?}");
        }
        let (_d, p) = store();
        assert!(p.trust("ana", "nobody", true).is_err());
    }

    #[test]
    fn strangers_blurs_only_people_the_box_does_not_know() {
        let s = Settings {
            blur: Blur::Strangers,
            ..Settings::default()
        };
        assert_eq!(shown_because(&s, "@bo:box.local", || true), Some("known"));
        assert_eq!(shown_because(&s, "@bo:elsewhere.org", || false), None);
        let everyone = Settings::default();
        assert_eq!(
            shown_because(&everyone, "@bo:box.local", || panic!("not asked")),
            None
        );
    }

    #[test]
    fn changes_are_read_strictly() {
        assert_eq!(
            parse_change(&json!({"blur": "strangers", "nudityCheck": true})).unwrap(),
            Change {
                blur: Some(Blur::Strangers),
                nudity_check: Some(true)
            }
        );
        assert!(parse_change(&json!({"blur": "some"})).is_err());
        assert!(parse_change(&json!({"nudityCheck": "yes"})).is_err());
        assert!(parse_change(&json!({"age": "adult"})).is_err());
        assert!(parse_change(&json!([])).is_err());
    }

    #[test]
    fn a_removed_person_leaves_nothing_behind() {
        let (_d, p) = store();
        p.set_age("ana", Some(Age::Adult)).unwrap();
        p.order_started("ana", ORDER).unwrap();
        p.forget("ana").unwrap();
        assert_eq!(p.get("ana").unwrap(), Settings::default());
        assert_eq!(p.order_stage(ORDER, "done").unwrap(), None);
    }
}
