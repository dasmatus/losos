//! LosOS Login's own rules for the people on a box that signs them in
//! itself (`modules/identity.nix`): two-factor sign-in, passwords that run
//! out, and the one-time code Authelia asks for before a person adds an
//! authenticator app or a passkey.
//!
//! Authelia keeps each person's second factors in its own database; this
//! daemon only reads which ones a person has, and removes them when an
//! administrator resets them for someone who lost a phone. Two marks in the
//! people file carry the outcome to Authelia, whose access rules read them:
//!
//!   * [`crate::people::TWO_FACTOR`]: the person has a second factor, so
//!     Authelia asks for it on every sign-in.
//!   * [`crate::people::LOCKED`]: a rule caught them (two-factor sign-in is
//!     required and they have none, or their password is older than the
//!     box allows). Authelia keeps them out of LosOS Cloud and LosOS Git,
//!     and this daemon lets them into their own account and nothing else
//!     until they comply.
//!
//! Authelia sends that one-time code by mail, and a box has no mail: the
//! notifier writes the last message to a file instead. A person who has the
//! Own account access reads their code here by typing their password again
//! ([`Login::code_for`]); without it, nobody can add a second factor.
//!
//! The rules and the date each password was last set live beside the people
//! file, root's only (`rules.json`, `passwords.json`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const DEFAULT_DIR: &str = "/var/lib/losos-identity";
const DEFAULT_DB: &str = "/var/lib/authelia-losos/db.sqlite3";
const DEFAULT_NOTES: &str = "/var/lib/authelia-losos/notifications.txt";
/// The longest a password may be kept, when the rule is on: a little over
/// three years, so a typo cannot turn into a number that never trips.
pub const MAX_PASSWORD_DAYS: u32 = 1200;
const DAY_SECS: u64 = 24 * 60 * 60;

/// What an administrator sets on the Users pane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rules {
    /// Everyone needs an authenticator app or a passkey.
    #[serde(default)]
    pub two_factor: bool,
    /// A password runs out this many days after it was set; 0 never.
    #[serde(default)]
    pub password_days: u32,
}

/// Why a person is held to their own account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    /// Two-factor sign-in is required and they have no second factor.
    TwoFactor,
    /// Their password is older than the rule allows.
    Password,
}

/// A person's second factors, as Authelia stores them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Factors {
    /// An authenticator app (TOTP).
    pub app: bool,
    /// The names of their passkeys and security keys.
    pub passkeys: Vec<String>,
}

impl Factors {
    pub fn any(&self) -> bool {
        self.app || !self.passkeys.is_empty()
    }
}

/// What holds `rules` against a person with `factors` whose password was
/// set at `set_at` (seconds since the epoch), at `now`.
pub fn reasons(rules: Rules, factors: &Factors, set_at: u64, now: u64) -> Vec<Reason> {
    let mut out = Vec::new();
    if rules.two_factor && !factors.any() {
        out.push(Reason::TwoFactor);
    }
    if let Some(until) = expires_at(rules, set_at) {
        if now >= until {
            out.push(Reason::Password);
        }
    }
    out
}

/// When a password set at `set_at` runs out under `rules`, if it does.
pub fn expires_at(rules: Rules, set_at: u64) -> Option<u64> {
    (rules.password_days > 0).then(|| set_at + u64::from(rules.password_days) * DAY_SECS)
}

/// The one-time code in Authelia's notifier file for `email`, the newest
/// when there are several. Each message starts with `Date:`, then
/// `Recipient:` with the name and address, and the code sits alone on the
/// first non-blank line after the first rule of dashes in the body.
pub fn parse_code(text: &str, email: &str) -> Option<String> {
    let email = email.trim().to_ascii_lowercase();
    if email.is_empty() {
        return None;
    }
    let mut found = None;
    for message in text
        .split("\nDate: ")
        .map(|m| m.trim_start_matches("Date: "))
    {
        let mut lines = message.lines();
        let Some(recipient) = lines.find(|l| l.starts_with("Recipient:")) else {
            continue;
        };
        let to_them = recipient
            .to_ascii_lowercase()
            .split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '{' | '}'))
            .any(|word| word == email);
        if !to_them {
            continue;
        }
        let mut after_rule = false;
        for line in lines {
            let line = line.trim();
            if after_rule {
                if line.is_empty() {
                    continue;
                }
                let ok = !line.is_empty()
                    && line.len() <= 32
                    && line.chars().all(|c| c.is_ascii_alphanumeric());
                if ok {
                    found = Some(line.to_string());
                }
                break;
            }
            after_rule = line.len() >= 8 && line.chars().all(|c| c == '-');
        }
    }
    found
}

/// The files beside the people file and Authelia's database.
pub struct Login {
    dir: PathBuf,
    db: PathBuf,
    notes: PathBuf,
    lock: Mutex<()>,
}

fn env_path(name: &str, default: &str) -> PathBuf {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from(default), PathBuf::from)
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Login {
    pub fn new(
        dir: impl Into<PathBuf>,
        db: impl Into<PathBuf>,
        notes: impl Into<PathBuf>,
    ) -> Login {
        Login {
            dir: dir.into(),
            db: db.into(),
            notes: notes.into(),
            lock: Mutex::new(()),
        }
    }

    /// `$LOSOS_PEOPLE_FILE`'s directory, `$LOSOS_AUTHELIA_DB` and
    /// `$LOSOS_AUTHELIA_NOTES` (`modules/identity.nix`).
    pub fn from_env(people: &Path) -> Login {
        let dir = people
            .parent()
            .map_or_else(|| PathBuf::from(DEFAULT_DIR), Path::to_path_buf);
        Login::new(
            dir,
            env_path("LOSOS_AUTHELIA_DB", DEFAULT_DB),
            env_path("LOSOS_AUTHELIA_NOTES", DEFAULT_NOTES),
        )
    }

    fn read_json<T: for<'de> Deserialize<'de> + Default>(&self, file: &str) -> T {
        std::fs::read(self.dir.join(file))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write_json<T: Serialize>(&self, file: &str, value: &T) -> anyhow::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!(".{file}.tmp"));
        {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(value)?)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, self.dir.join(file))?;
        Ok(())
    }

    pub fn rules(&self) -> Rules {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.read_json("rules.json")
    }

    pub fn set_rules(&self, rules: Rules) -> anyhow::Result<()> {
        if rules.password_days > MAX_PASSWORD_DAYS {
            anyhow::bail!("a password can be kept for at most {MAX_PASSWORD_DAYS} days");
        }
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.write_json("rules.json", &rules)
    }

    /// When `name` last set a password. A person with no date yet (an
    /// account older than the rule) gets `now`, so turning the rule on
    /// gives everyone its full span rather than locking them at once.
    pub fn password_set(&self, name: &str, now: u64) -> u64 {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut dates: BTreeMap<String, u64> = self.read_json("passwords.json");
        if let Some(at) = dates.get(name) {
            return *at;
        }
        dates.insert(name.to_string(), now);
        if let Err(e) = self.write_json("passwords.json", &dates) {
            tracing::warn!(error = ?e, "could not record when a password was set");
        }
        now
    }

    /// `name` set a password at `at`.
    pub fn record_password(&self, name: &str, at: u64) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut dates: BTreeMap<String, u64> = self.read_json("passwords.json");
        dates.insert(name.to_string(), at);
        self.write_json("passwords.json", &dates)
    }

    /// Forget `name`'s date, when they are removed.
    pub fn forget(&self, name: &str) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut dates: BTreeMap<String, u64> = self.read_json("passwords.json");
        if dates.remove(name).is_some() {
            self.write_json("passwords.json", &dates)?;
        }
        Ok(())
    }

    fn open_db(&self, write: bool) -> Option<rusqlite::Connection> {
        use rusqlite::OpenFlags;
        if !self.db.exists() {
            return None;
        }
        let flags = if write {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        };
        match rusqlite::Connection::open_with_flags(
            &self.db,
            flags | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => {
                let _ = c.busy_timeout(std::time::Duration::from_secs(2));
                Some(c)
            }
            Err(e) => {
                tracing::warn!(error = ?e, "could not open LosOS Login's database");
                None
            }
        }
    }

    /// Hand any journal file this daemon made beside the database back to
    /// the database's owner: Authelia runs as its own user and could not
    /// write to the database past a root-owned one.
    fn give_back(&self) {
        use std::os::unix::fs::MetadataExt;
        let Ok(meta) = std::fs::metadata(&self.db) else {
            return;
        };
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut side = self.db.clone().into_os_string();
            side.push(suffix);
            let side = PathBuf::from(side);
            let theirs = std::fs::metadata(&side)
                .is_ok_and(|m| m.uid() == meta.uid() && m.gid() == meta.gid());
            if side.exists() && !theirs {
                if let Err(e) = std::os::unix::fs::chown(&side, Some(meta.uid()), Some(meta.gid()))
                {
                    tracing::warn!(error = ?e, "could not hand a database journal back to LosOS Login");
                }
            }
        }
    }

    /// `name`'s second factors. None when Authelia has not made its
    /// database yet.
    pub fn factors(&self, name: &str) -> Factors {
        let factors = self.read_factors(name);
        self.give_back();
        factors
    }

    fn read_factors(&self, name: &str) -> Factors {
        let Some(db) = self.open_db(false) else {
            return Factors::default();
        };
        let app = db
            .query_row(
                "SELECT COUNT(*) FROM totp_configurations WHERE username = ?1",
                [name],
                |r| r.get::<_, i64>(0),
            )
            .is_ok_and(|n| n > 0);
        let passkeys = db
            .prepare(
                "SELECT description FROM webauthn_credentials WHERE username = ?1 \
                 ORDER BY created_at",
            )
            .and_then(|mut q| {
                q.query_map([name], |r| r.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap_or_default();
        Factors { app, passkeys }
    }

    /// Remove every second factor `name` has, for someone who lost the
    /// phone or key they signed in with.
    pub fn reset_factors(&self, name: &str) -> anyhow::Result<()> {
        let done = self.delete_factors(name);
        self.give_back();
        done
    }

    fn delete_factors(&self, name: &str) -> anyhow::Result<()> {
        let Some(db) = self.open_db(true) else {
            return Ok(());
        };
        // The codes already used go too: Authelia refuses a code from a new
        // app in a 30-second step an old one was used in, so someone who
        // signed in a moment ago could not add their new app.
        for table in [
            "totp_configurations",
            "totp_history",
            "webauthn_credentials",
        ] {
            match db.execute(&format!("DELETE FROM {table} WHERE username = ?1"), [name]) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(_, Some(msg)))
                    if msg.contains("no such table") => {}
                Err(e) => return Err(anyhow::anyhow!("removing {name}'s second factors: {e}")),
            }
        }
        Ok(())
    }

    /// The one-time code Authelia last sent to `email`.
    pub fn code_for(&self, email: &str) -> Option<String> {
        let text = std::fs::read_to_string(&self.notes).ok()?;
        parse_code(&text, email)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "Date: 2026-10-17 12:00:00.1 +0000 UTC\n\
Recipient: { Eva Novak eva@mattbox.local }\n\
Subject: Confirm your identity\n\
Body: Hi Eva Novak,\n\
This notification has been sent to you in order to verify your identity to change security details for your account at mattbox.local.\n\
The following one-time code should only be used in the prompt displayed in your browser.\n\
\n\
----------------------------------------\n\
\n\
K7QW2MXA\n\
\n\
----------------------------------------\n\
If you did not initiate the process your credentials may have been compromised.\n";

    #[test]
    fn the_code_is_found_for_its_recipient_only() {
        assert_eq!(
            parse_code(NOTE, "eva@mattbox.local").as_deref(),
            Some("K7QW2MXA")
        );
        assert_eq!(
            parse_code(NOTE, "EVA@mattbox.local").as_deref(),
            Some("K7QW2MXA")
        );
        assert_eq!(parse_code(NOTE, "oma@mattbox.local"), None);
        assert_eq!(parse_code(NOTE, "mattbox.local"), None);
        assert_eq!(parse_code(NOTE, ""), None);
    }

    #[test]
    fn the_newest_code_wins_and_other_messages_are_skipped() {
        let other = NOTE
            .replace("eva@mattbox.local", "oma@mattbox.local")
            .replace("K7QW2MXA", "OMACODE1");
        let newer = NOTE.replace("K7QW2MXA", "NEWER123");
        let text = format!("{NOTE}\n{other}\n{newer}");
        assert_eq!(
            parse_code(&text, "eva@mattbox.local").as_deref(),
            Some("NEWER123")
        );
        assert_eq!(
            parse_code(&text, "oma@mattbox.local").as_deref(),
            Some("OMACODE1")
        );
        let notice = "Date: x\nRecipient: { Eva eva@mattbox.local }\nSubject: Second factor added\nBody: hello\n";
        assert_eq!(parse_code(notice, "eva@mattbox.local"), None);
    }

    #[test]
    fn the_rules_hold_people_to_their_account() {
        let none = Factors::default();
        let app = Factors {
            app: true,
            passkeys: vec![],
        };
        let key = Factors {
            app: false,
            passkeys: vec!["laptop".into()],
        };
        let off = Rules::default();
        assert!(reasons(off, &none, 0, 10 * DAY_SECS).is_empty());
        let two = Rules {
            two_factor: true,
            password_days: 0,
        };
        assert_eq!(reasons(two, &none, 0, 0), vec![Reason::TwoFactor]);
        assert!(reasons(two, &app, 0, 0).is_empty());
        assert!(reasons(two, &key, 0, 0).is_empty());
        let days = Rules {
            two_factor: false,
            password_days: 30,
        };
        assert!(reasons(days, &none, 0, 30 * DAY_SECS - 1).is_empty());
        assert_eq!(
            reasons(days, &none, 0, 30 * DAY_SECS),
            vec![Reason::Password]
        );
        let both = Rules {
            two_factor: true,
            password_days: 30,
        };
        assert_eq!(
            reasons(both, &none, 0, 31 * DAY_SECS),
            vec![Reason::TwoFactor, Reason::Password]
        );
    }

    #[test]
    fn rules_and_dates_round_trip_and_an_unknown_date_starts_now() {
        let dir = assert_fs::TempDir::new().unwrap();
        let login = Login::new(dir.path(), dir.path().join("db"), dir.path().join("notes"));
        assert_eq!(login.rules(), Rules::default());
        let rules = Rules {
            two_factor: true,
            password_days: 90,
        };
        login.set_rules(rules).unwrap();
        assert_eq!(login.rules(), rules);
        assert!(login
            .set_rules(Rules {
                two_factor: false,
                password_days: MAX_PASSWORD_DAYS + 1
            })
            .is_err());
        assert_eq!(login.password_set("eva", 500), 500);
        assert_eq!(
            login.password_set("eva", 900),
            500,
            "the first sight is kept"
        );
        login.record_password("eva", 1000).unwrap();
        assert_eq!(login.password_set("eva", 2000), 1000);
        login.forget("eva").unwrap();
        assert_eq!(login.password_set("eva", 3000), 3000);
    }

    #[test]
    fn factors_are_read_from_and_removed_from_authelias_database() {
        let dir = assert_fs::TempDir::new().unwrap();
        let db_path = dir.path().join("db.sqlite3");
        let login = Login::new(dir.path(), &db_path, dir.path().join("notes"));
        assert_eq!(login.factors("eva"), Factors::default(), "no database yet");
        let db = rusqlite::Connection::open(&db_path).unwrap();
        db.execute_batch(
            "CREATE TABLE totp_configurations (id INTEGER PRIMARY KEY, username TEXT, secret BLOB);
             CREATE TABLE totp_history (id INTEGER PRIMARY KEY, username TEXT, step TEXT);
             INSERT INTO totp_history (username, step) VALUES ('eva', '1'), ('oma', '1');
             CREATE TABLE webauthn_credentials (id INTEGER PRIMARY KEY, created_at INTEGER, username TEXT, description TEXT);
             INSERT INTO totp_configurations (username, secret) VALUES ('eva', x'00');
             INSERT INTO webauthn_credentials (created_at, username, description) VALUES (2, 'eva', 'phone'), (1, 'eva', 'laptop'), (1, 'oma', 'key');",
        )
        .unwrap();
        drop(db);
        assert_eq!(
            login.factors("eva"),
            Factors {
                app: true,
                passkeys: vec!["laptop".into(), "phone".into()]
            }
        );
        assert!(login.factors("matus").passkeys.is_empty());
        login.reset_factors("eva").unwrap();
        assert!(!login.factors("eva").any());
        assert_eq!(login.factors("oma").passkeys, vec!["key".to_string()]);
        let history = |name: &str| -> i64 {
            rusqlite::Connection::open(&db_path)
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM totp_history WHERE username = ?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!((history("eva"), history("oma")), (0, 1));
    }
}
