//! Moderation: reports, mutes, suspensions and the log of what moderators
//! did, in one place for every app on the box.
//!
//! - A report (`POST /api/reports`) names an app, a reference inside it
//!   (a mail's id, a chat event's id, a profile's picture) and a reason.
//!   It never carries the thing itself: the queue shows who reported what
//!   and why, not the picture or the message. A report of a child being
//!   harmed keeps not even the reporter's note, and the only thing a
//!   moderator can do with it is hide it without opening it.
//! - A mute stops a person speaking: the routes `sections.json`'s `speech`
//!   lists answer 403 until it ends.
//! - A suspension disables the account in the people file, which LosOS
//!   Login reads on every request, until it ends; [`Moderation::lift_due`]
//!   enables it again, from a timer, because a suspended person never
//!   reaches this daemon to trigger it.
//! - Mail a moderator held stays in the recipient's Quarantine folder, out
//!   of the folder list, and does not open until it is let go.
//!
//! Everything is one JSON document, `/var/lib/losos/moderation.json`, and
//! every action is a line of `/var/lib/losos/moderation.log`.

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const DEFAULT_FILE: &str = "/var/lib/losos/moderation.json";
const DEFAULT_LOG: &str = "/var/lib/losos/moderation.log";
/// Reports kept in all; the oldest closed one goes first.
pub const MAX_REPORTS: usize = 500;
/// Open reports one person may have at a time.
pub const MAX_OPEN_PER_REPORTER: usize = 20;
/// The longest reference and note a report takes.
const MAX_REF: usize = 512;
const MAX_NOTE: usize = 1000;
/// The longest reason a moderator gives.
const MAX_REASON: usize = 300;
/// Log lines `GET /api/moderation` shows.
pub const LOG_SHOWN: usize = 200;

/// Write `bytes` to `path` through a temporary file and a rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let dir = path.parent().unwrap_or(Path::new("/"));
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp"));
    {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// A request the queue cannot take, in words the page shows as they are.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Refused {}

fn refuse<T>(status: u16, message: impl Into<String>) -> anyhow::Result<T> {
    Err(Refused {
        status,
        message: message.into(),
    }
    .into())
}

/// The apps a report can come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum App {
    Mail,
    Chat,
    Profile,
    Cloud,
    Git,
    Notes,
    Diagrams,
    Office,
    Calls,
}

impl App {
    fn parse(s: &str) -> Option<App> {
        serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
    }
}

/// Why someone reported something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Spam,
    Abuse,
    Sexual,
    /// A child being harmed. Kept apart from the rest all the way through.
    Child,
    Other,
}

impl Reason {
    fn parse(s: &str) -> Option<Reason> {
        serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
    }
}

/// Where a report stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Open,
    /// The thing reported is hidden: mail held, a picture taken down.
    Hidden,
    /// Looked at; nothing wrong.
    Dismissed,
    /// Dealt with some other way, a mute or a word with the person.
    Resolved,
}

/// One report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub id: String,
    pub at: u64,
    /// Who reported it.
    pub by: String,
    pub app: App,
    /// Whom it is about, when the app knows.
    #[serde(default)]
    pub about: Option<String>,
    /// What it is about inside the app: never its content.
    #[serde(rename = "ref")]
    pub reference: String,
    pub reason: Reason,
    #[serde(default)]
    pub note: String,
    pub state: State,
    #[serde(default)]
    pub handled_by: Option<String>,
    #[serde(default)]
    pub handled_at: Option<u64>,
}

/// A mute or a suspension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hold {
    pub at: u64,
    pub by: String,
    /// When it ends by itself, in seconds since 1970; `None` until lifted.
    #[serde(default)]
    pub until: Option<u64>,
    #[serde(default)]
    pub reason: String,
}

impl Hold {
    fn live(&self, now: u64) -> bool {
        self.until.is_none_or(|u| u > now)
    }
}

/// What holds one person.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Holds {
    #[serde(default)]
    pub muted: Option<Hold>,
    #[serde(default)]
    pub suspended: Option<Hold>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Document {
    #[serde(default)]
    people: BTreeMap<String, Holds>,
    #[serde(default)]
    reports: Vec<Report>,
    #[serde(default)]
    next: u64,
    /// Mail ids held in each person's Quarantine folder.
    #[serde(default)]
    held: BTreeMap<String, Vec<String>>,
}

/// One line of the log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub at: u64,
    pub by: String,
    pub action: String,
    #[serde(default)]
    pub about: Option<String>,
    #[serde(default)]
    pub detail: String,
}

/// A new report as `POST /api/reports` takes it.
#[derive(Debug, Clone, PartialEq)]
pub struct NewReport {
    pub app: App,
    pub about: Option<String>,
    pub reference: String,
    pub reason: Reason,
    pub note: String,
}

/// `POST /api/reports`'s body: `{"app", "ref", "reason", "about"?, "note"?}`.
pub fn parse_report(doc: &serde_json::Value) -> anyhow::Result<NewReport> {
    let text = |k: &str| doc.get(k).and_then(|v| v.as_str()).map(str::trim);
    let Some(app) = text("app").and_then(App::parse) else {
        return refuse(400, "app must be one of the box's apps");
    };
    let Some(reason) = text("reason").and_then(Reason::parse) else {
        return refuse(400, "reason must be spam, abuse, sexual, child or other");
    };
    let reference = text("ref").unwrap_or_default();
    if reference.is_empty() || reference.len() > MAX_REF || reference.chars().any(char::is_control)
    {
        return refuse(400, "ref names the thing reported, in one line");
    }
    let about = match text("about").filter(|a| !a.is_empty()) {
        Some(a) => match crate::people::validate_name(a) {
            Ok(a) => Some(a.to_string()),
            Err(_) => return refuse(400, "about must be a sign-in name"),
        },
        None => None,
    };
    // A report of a child being harmed carries the reference and nothing a
    // reporter wrote about what they saw.
    let note = if reason == Reason::Child {
        String::new()
    } else {
        text("note")
            .unwrap_or_default()
            .chars()
            .take(MAX_NOTE)
            .collect()
    };
    Ok(NewReport {
        app,
        about,
        reference: reference.to_string(),
        reason,
        note,
    })
}

/// What a moderator does with a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Hide,
    Release,
    Dismiss,
    Resolve,
}

impl Act {
    pub fn parse(s: &str) -> Option<Act> {
        match s {
            "hide" => Some(Act::Hide),
            "release" => Some(Act::Release),
            "dismiss" => Some(Act::Dismiss),
            "resolve" => Some(Act::Resolve),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Act::Hide => "hide",
            Act::Release => "release",
            Act::Dismiss => "dismiss",
            Act::Resolve => "resolve",
        }
    }
}

/// Whether a moderator can hide something in `app` from the queue: hold
/// a mail, take a picture down.
pub fn can_hide(app: App) -> bool {
    matches!(app, App::Mail | App::Chat | App::Profile)
}

/// Whether `act` may be done to `report` at all. A report of a child being
/// harmed is hidden without opening where the app can hide it, and
/// otherwise only closed once it has gone to the police; it is never
/// dismissed, and held mail of that kind is never let go.
pub fn check_act(report: &Report, act: Act) -> Result<(), Refused> {
    let no = |m: &str| {
        Err(Refused {
            status: 409,
            message: m.into(),
        })
    };
    let child = report.reason == Reason::Child;
    match (report.state, act) {
        (State::Hidden, Act::Release) if report.app == App::Mail && !child => Ok(()),
        (_, Act::Release) => no("only held mail can be let go"),
        (State::Open, Act::Hide) if can_hide(report.app) => Ok(()),
        (State::Open, Act::Hide) => no("this app cannot hide things from here yet"),
        (State::Open, Act::Resolve) if child && can_hide(report.app) => {
            no("hide it without opening it")
        }
        (State::Open, Act::Dismiss) if child => {
            no("a report of a child being harmed is never dismissed")
        }
        (State::Open, _) => Ok(()),
        _ => no("this report is closed"),
    }
}

/// Reports for the queue: open first, a child being harmed first of all,
/// then newest first.
pub fn queue_order(reports: &mut [Report]) {
    reports.sort_by(|a, b| {
        let rank = |r: &Report| (r.state != State::Open, r.reason != Reason::Child);
        rank(a).cmp(&rank(b)).then(b.at.cmp(&a.at))
    });
}

pub struct Moderation {
    path: PathBuf,
    log: PathBuf,
    lock: Mutex<()>,
}

impl Moderation {
    pub fn new(path: impl Into<PathBuf>, log: impl Into<PathBuf>) -> Moderation {
        Moderation {
            path: path.into(),
            log: log.into(),
            lock: Mutex::new(()),
        }
    }

    pub fn from_env() -> Moderation {
        let env = |k: &str, d: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map_or_else(|| PathBuf::from(d), PathBuf::from)
        };
        Moderation::new(
            env("LOSOS_MODERATION_FILE", DEFAULT_FILE),
            env("LOSOS_MODERATION_LOG", DEFAULT_LOG),
        )
    }

    fn read(&self) -> anyhow::Result<Document> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Document::default()),
            Err(e) => Err(anyhow::anyhow!("reading {}: {e}", self.path.display())),
        }
    }

    fn write(&self, doc: &Document) -> anyhow::Result<()> {
        write_atomic(&self.path, &serde_json::to_vec_pretty(doc)?)
    }

    /// Add a line to the log. A log that cannot be written is said in the
    /// daemon's own log; the action it describes has happened.
    pub fn note(&self, at: u64, by: &str, action: &str, about: Option<&str>, detail: &str) {
        let entry = Entry {
            at,
            by: by.to_string(),
            action: action.to_string(),
            about: about.map(str::to_string),
            detail: detail.chars().take(MAX_REASON).collect(),
        };
        let line = serde_json::to_string(&entry).unwrap_or_default();
        let done = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.log)
            .and_then(|mut f| writeln!(f, "{line}"));
        if let Err(e) = done {
            tracing::error!(error = ?e, path = %self.log.display(), "moderation log write failed");
        }
    }

    /// The newest `n` lines of the log, newest first.
    pub fn log(&self, n: usize) -> Vec<Entry> {
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        let mut out: Vec<Entry> = text
            .lines()
            .rev()
            .filter_map(|l| serde_json::from_str(l).ok())
            .take(n)
            .collect();
        out.truncate(n);
        out
    }

    /// File a report. Returns its id.
    pub fn report(&self, by: &str, new: NewReport, now: u64) -> anyhow::Result<String> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let open_by = doc
            .reports
            .iter()
            .filter(|r| r.by == by && r.state == State::Open)
            .count();
        if open_by >= MAX_OPEN_PER_REPORTER {
            return refuse(
                429,
                "you have many reports waiting; the moderators will get to them",
            );
        }
        // An exact repeat is the same report.
        if let Some(same) = doc.reports.iter().find(|r| {
            r.by == by && r.state == State::Open && r.app == new.app && r.reference == new.reference
        }) {
            return Ok(same.id.clone());
        }
        if doc.reports.len() >= MAX_REPORTS {
            match doc.reports.iter().position(|r| r.state != State::Open) {
                Some(i) => {
                    doc.reports.remove(i);
                }
                None => return refuse(429, "the moderators have too many reports waiting"),
            }
        }
        doc.next += 1;
        let id = format!("r{}", doc.next);
        let reason = new.reason;
        let app = new.app;
        let about = new.about.clone();
        doc.reports.push(Report {
            id: id.clone(),
            at: now,
            by: by.to_string(),
            app,
            about: new.about,
            reference: new.reference,
            reason,
            note: new.note,
            state: State::Open,
            handled_by: None,
            handled_at: None,
        });
        self.write(&doc)?;
        drop(_g);
        let reason = serde_json::to_value(reason).unwrap_or_default();
        let app = serde_json::to_value(app).unwrap_or_default();
        self.note(
            now,
            by,
            "report",
            about.as_deref(),
            &format!(
                "{} in {}",
                reason.as_str().unwrap_or(""),
                app.as_str().unwrap_or("")
            ),
        );
        Ok(id)
    }

    /// Every report, in queue order.
    pub fn reports(&self) -> Vec<Report> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.read().map(|d| d.reports).unwrap_or_default();
        queue_order(&mut all);
        all
    }

    /// One report.
    pub fn find(&self, id: &str) -> Option<Report> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.read().ok()?.reports.into_iter().find(|r| r.id == id)
    }

    /// Record what a moderator did with report `id`. The caller has done
    /// the hiding or letting go in the app first.
    pub fn settle(&self, id: &str, act: Act, by: &str, now: u64) -> anyhow::Result<Report> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let Some(report) = doc.reports.iter_mut().find(|r| r.id == id) else {
            return refuse(404, "no such report");
        };
        check_act(report, act)?;
        report.state = match act {
            Act::Hide => State::Hidden,
            Act::Release => State::Resolved,
            Act::Dismiss => State::Dismissed,
            Act::Resolve => State::Resolved,
        };
        report.handled_by = Some(by.to_string());
        report.handled_at = Some(now);
        let done = report.clone();
        self.write(&doc)?;
        drop(_g);
        self.note(now, by, act.as_str(), done.about.as_deref(), &done.id);
        Ok(done)
    }

    /// What holds everyone, as stored (ended holds included until
    /// [`Moderation::lift_due`] clears them).
    pub fn holds(&self) -> BTreeMap<String, Holds> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.read().map(|d| d.people).unwrap_or_default()
    }

    /// The mute on `name` that holds now, if any.
    pub fn muted(&self, name: &str, now: u64) -> Option<Hold> {
        self.holds()
            .remove(name)
            .and_then(|h| h.muted)
            .filter(|m| m.live(now))
    }

    /// The suspension on `name`, if any.
    pub fn suspended(&self, name: &str, now: u64) -> Option<Hold> {
        self.holds()
            .remove(name)
            .and_then(|h| h.suspended)
            .filter(|s| s.live(now))
    }

    /// Mute `name`, or change how long for.
    pub fn mute(&self, name: &str, hold: Hold) -> anyhow::Result<()> {
        self.set(name, |h| h.muted = Some(hold.clone()))?;
        let until = until_words(hold.until);
        self.note(
            hold.at,
            &hold.by,
            "mute",
            Some(name),
            &with_reason(&until, &hold.reason),
        );
        Ok(())
    }

    /// Let `name` speak again.
    pub fn unmute(&self, name: &str, by: &str, now: u64) -> anyhow::Result<()> {
        self.set(name, |h| h.muted = None)?;
        self.note(now, by, "unmute", Some(name), "");
        Ok(())
    }

    /// Record a suspension; the caller has disabled the account.
    pub fn suspend(&self, name: &str, hold: Hold) -> anyhow::Result<()> {
        self.set(name, |h| h.suspended = Some(hold.clone()))?;
        let until = until_words(hold.until);
        self.note(
            hold.at,
            &hold.by,
            "suspend",
            Some(name),
            &with_reason(&until, &hold.reason),
        );
        Ok(())
    }

    /// Record the end of a suspension; the caller enables the account.
    pub fn unsuspend(&self, name: &str, by: &str, now: u64) -> anyhow::Result<()> {
        self.set(name, |h| h.suspended = None)?;
        self.note(now, by, "unsuspend", Some(name), "");
        Ok(())
    }

    fn set(&self, name: &str, change: impl FnOnce(&mut Holds)) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let holds = doc.people.entry(name.to_string()).or_default();
        change(holds);
        if holds.muted.is_none() && holds.suspended.is_none() {
            doc.people.remove(name);
        }
        self.write(&doc)
    }

    /// End the holds whose time has come: mutes are dropped, and each
    /// suspension that ended is handed to `enable` and dropped once it
    /// says the account is open again. Returns the names let back in.
    pub fn lift_due(
        &self,
        now: u64,
        mut enable: impl FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<String>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let mut changed = false;
        let mut back = Vec::new();
        let mut unmuted = Vec::new();
        for (name, holds) in &mut doc.people {
            if holds.muted.as_ref().is_some_and(|m| !m.live(now)) {
                holds.muted = None;
                unmuted.push(name.clone());
                changed = true;
            }
            if holds.suspended.as_ref().is_some_and(|s| !s.live(now)) {
                match enable(name) {
                    Ok(()) => {
                        holds.suspended = None;
                        back.push(name.clone());
                        changed = true;
                    }
                    Err(e) => tracing::warn!(error = ?e, name, "a suspension could not end"),
                }
            }
        }
        doc.people
            .retain(|_, h| h.muted.is_some() || h.suspended.is_some());
        if changed {
            self.write(&doc)?;
        }
        drop(_g);
        for name in &unmuted {
            self.note(now, "box", "mute ended", Some(name), "");
        }
        for name in &back {
            self.note(now, "box", "suspension ended", Some(name), "");
        }
        Ok(back)
    }

    /// Whether mail `id` of `name`'s is held.
    pub fn is_held(&self, name: &str, id: &str) -> bool {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.read().is_ok_and(|d| {
            d.held
                .get(name)
                .is_some_and(|ids| ids.iter().any(|i| i == id))
        })
    }

    /// Mark mail `id` of `name`'s held, or let it go.
    pub fn hold_mail(&self, name: &str, id: &str, held: bool) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let ids = doc.held.entry(name.to_string()).or_default();
        ids.retain(|i| i != id);
        if held {
            ids.push(id.to_string());
        }
        if ids.is_empty() {
            doc.held.remove(name);
        }
        self.write(&doc)
    }

    /// Forget everything about a removed person but the log and the
    /// reports they made or were the subject of.
    pub fn forget(&self, name: &str) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let had = doc.people.remove(name).is_some() | doc.held.remove(name).is_some();
        if had {
            self.write(&doc)?;
        }
        Ok(())
    }
}

fn until_words(until: Option<u64>) -> String {
    match until {
        Some(t) => format!("until {t}"),
        None => "until lifted".to_string(),
    }
}

fn with_reason(until: &str, reason: &str) -> String {
    if reason.is_empty() {
        until.to_string()
    } else {
        format!("{until}: {reason}")
    }
}

/// A moderator's reason, one line and not too long.
pub fn clean_reason(reason: &str) -> String {
    reason
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_REASON)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store() -> (assert_fs::TempDir, Moderation) {
        let dir = assert_fs::TempDir::new().unwrap();
        let m = Moderation::new(dir.path().join("m.json"), dir.path().join("m.log"));
        (dir, m)
    }

    fn new(reason: Reason, reference: &str) -> NewReport {
        NewReport {
            app: App::Mail,
            about: Some("eva".into()),
            reference: reference.into(),
            reason,
            note: "rude".into(),
        }
    }

    #[test]
    fn a_report_is_a_reference_and_a_reason() {
        let r = parse_report(&json!({
            "app": "mail", "ref": "Mabc", "reason": "spam", "about": "eva", "note": " buy now "
        }))
        .unwrap();
        assert_eq!(r.app, App::Mail);
        assert_eq!(r.note, "buy now");
        assert!(parse_report(&json!({"app": "fax", "ref": "x", "reason": "spam"})).is_err());
        assert!(parse_report(&json!({"app": "mail", "ref": "", "reason": "spam"})).is_err());
        assert!(parse_report(&json!({"app": "mail", "ref": "x", "reason": "rude"})).is_err());
        assert!(parse_report(
            &json!({"app": "mail", "ref": "x", "reason": "spam", "about": "Eva!"})
        )
        .is_err());
    }

    #[test]
    fn a_child_report_keeps_no_words_and_can_only_be_hidden() {
        let r = parse_report(&json!({
            "app": "mail", "ref": "Mabc", "reason": "child", "note": "what I saw"
        }))
        .unwrap();
        assert_eq!(r.note, "");
        let (_d, m) = store();
        let id = m.report("ada", r, 10).unwrap();
        for act in [Act::Dismiss, Act::Resolve, Act::Release] {
            assert!(m.settle(&id, act, "mo", 11).is_err(), "{act:?}");
        }
        assert_eq!(
            m.settle(&id, Act::Hide, "mo", 12).unwrap().state,
            State::Hidden
        );
        assert!(m.settle(&id, Act::Hide, "mo", 13).is_err());
        assert!(m.settle(&id, Act::Release, "mo", 14).is_err());
        // Where the app cannot hide it from here, it is closed once it has
        // gone to the police, and still never dismissed.
        let mut cloud = new(Reason::Child, "file");
        cloud.app = App::Cloud;
        let id = m.report("ada", cloud, 20).unwrap();
        assert!(m.settle(&id, Act::Hide, "mo", 21).is_err());
        assert!(m.settle(&id, Act::Dismiss, "mo", 21).is_err());
        assert_eq!(
            m.settle(&id, Act::Resolve, "mo", 22).unwrap().state,
            State::Resolved
        );
    }

    #[test]
    fn the_queue_puts_open_child_reports_first() {
        let (_d, m) = store();
        let spam = m.report("ada", new(Reason::Spam, "1"), 10).unwrap();
        let child = m.report("ada", new(Reason::Child, "2"), 5).unwrap();
        let abuse = m.report("bob", new(Reason::Abuse, "3"), 20).unwrap();
        m.settle(&abuse, Act::Dismiss, "mo", 21).unwrap();
        let ids: Vec<String> = m.reports().into_iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![child, spam, abuse]);
    }

    #[test]
    fn a_repeat_is_one_report_and_a_flood_is_refused() {
        let (_d, m) = store();
        let a = m.report("ada", new(Reason::Spam, "x"), 1).unwrap();
        assert_eq!(m.report("ada", new(Reason::Spam, "x"), 2).unwrap(), a);
        for i in 1..MAX_OPEN_PER_REPORTER {
            m.report("ada", new(Reason::Spam, &format!("m{i}")), 3)
                .unwrap();
        }
        let e = m
            .report("ada", new(Reason::Spam, "one more"), 4)
            .unwrap_err();
        assert_eq!(e.downcast_ref::<Refused>().unwrap().status, 429);
        assert!(m.report("bob", new(Reason::Spam, "one more"), 4).is_ok());
    }

    #[test]
    fn hiding_works_for_mail_and_profiles_and_release_only_for_mail() {
        let (_d, m) = store();
        let mut cloud = new(Reason::Abuse, "file");
        cloud.app = App::Cloud;
        let id = m.report("ada", cloud, 1).unwrap();
        assert!(m.settle(&id, Act::Hide, "mo", 2).is_err());
        let mail = m.report("ada", new(Reason::Spam, "M1"), 1).unwrap();
        m.settle(&mail, Act::Hide, "mo", 2).unwrap();
        assert_eq!(
            m.settle(&mail, Act::Release, "mo", 3).unwrap().state,
            State::Resolved
        );
    }

    #[test]
    fn holds_end_on_time_and_the_log_says_so() {
        let (_d, m) = store();
        let hold = |until| Hold {
            at: 100,
            by: "mo".into(),
            until,
            reason: "flooding".into(),
        };
        m.mute("eva", hold(Some(200))).unwrap();
        m.suspend("bob", hold(Some(150))).unwrap();
        m.suspend("cid", hold(None)).unwrap();
        assert!(m.muted("eva", 199).is_some());
        assert!(m.muted("eva", 200).is_none());
        assert!(m.suspended("cid", u64::MAX - 1).is_some());
        let mut enabled = Vec::new();
        let back = m
            .lift_due(160, |n| {
                enabled.push(n.to_string());
                Ok(())
            })
            .unwrap();
        assert_eq!(back, vec!["bob".to_string()]);
        assert_eq!(enabled, vec!["bob".to_string()]);
        assert!(m.holds().contains_key("eva"));
        m.lift_due(250, |_| Ok(())).unwrap();
        assert!(!m.holds().contains_key("eva"));
        assert!(m.holds().contains_key("cid"));
        let log: Vec<String> = m.log(10).into_iter().map(|e| e.action).collect();
        assert_eq!(
            log,
            vec![
                "mute ended",
                "suspension ended",
                "suspend",
                "suspend",
                "mute"
            ]
        );
        assert_eq!(m.log(10)[3].detail, "until 150: flooding");
    }

    #[test]
    fn a_suspension_that_cannot_end_stays() {
        let (_d, m) = store();
        m.suspend(
            "bob",
            Hold {
                at: 1,
                by: "mo".into(),
                until: Some(2),
                reason: String::new(),
            },
        )
        .unwrap();
        assert!(m.lift_due(3, |_| anyhow::bail!("disk")).unwrap().is_empty());
        assert!(m.holds()["bob"].suspended.is_some());
    }

    #[test]
    fn held_mail_is_kept_per_person_and_forgotten_with_them() {
        let (_d, m) = store();
        m.hold_mail("ada", "M1", true).unwrap();
        assert!(m.is_held("ada", "M1"));
        assert!(!m.is_held("bob", "M1"));
        m.hold_mail("ada", "M1", false).unwrap();
        assert!(!m.is_held("ada", "M1"));
        m.hold_mail("ada", "M2", true).unwrap();
        m.forget("ada").unwrap();
        assert!(!m.is_held("ada", "M2"));
    }

    #[test]
    fn a_reason_is_one_short_line() {
        assert_eq!(clean_reason(" spam\nlinks "), "spam links");
        assert_eq!(clean_reason(&"x".repeat(900)).len(), MAX_REASON);
    }
}
