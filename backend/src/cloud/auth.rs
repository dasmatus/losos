//! Who is calling, and the app passwords the desktop and phone apps sign
//! in with.
//!
//! Two ways in:
//!
//!   * **nginx names the person** in `X-Losos-Cloud-User` (and their name
//!     and address in `-Name` and `-Email`). It does so after LosOS Login
//!     vouched for a browser session, or after lososd checked a LosOS Login
//!     password on the WebDAV share (`backend/src/cloud_share.rs`). The
//!     header is believed only because nginx is the one process that can
//!     open the socket this server listens on (`modules/cloud.nix`), and
//!     because nginx sets it on every request, empty when nobody was named.
//!   * **an app password** under Basic auth, which this server checks
//!     itself. The desktop and phone apps get one through Login Flow v2
//!     ([`crate::cloud::nc`]); `losos cloud setup` gets one from
//!     `getapppassword`. Each is 72 random letters and digits, so a SHA-256
//!     of it is a safe thing to store and a cheap thing to check.

use crate::cloud::store::{Store, StoreError};
use crate::cloud::util::{now, random_token, sha256_hex};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const USER_HEADER: &str = "x-losos-cloud-user";
pub const NAME_HEADER: &str = "x-losos-cloud-name";
pub const EMAIL_HEADER: &str = "x-losos-cloud-email";

/// Length of an app password, as Nextcloud makes them.
pub const APP_PASSWORD_LEN: usize = 72;

/// Failed app-password checks allowed per login per window.
const FAILURES: u32 = 10;
const FAILURE_WINDOW: Duration = Duration::from_secs(300);
/// How long a Login Flow v2 waits for the person to grant it.
pub const FLOW_TTL: i64 = 20 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Via {
    /// nginx named the person.
    Header,
    /// An app password, by its id.
    AppPassword(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Who {
    pub login: String,
    pub name: String,
    pub email: String,
    pub via: Via,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPassword {
    pub id: i64,
    pub name: String,
    pub created: i64,
    pub last_used: i64,
}

/// A Login Flow v2 as the poller and the grant page see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flow {
    pub client: String,
    pub created: i64,
    pub granted: Option<(String, String)>,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS app_passwords (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  login     TEXT    NOT NULL,
  name      TEXT    NOT NULL,
  hash      TEXT    NOT NULL UNIQUE,
  created   INTEGER NOT NULL,
  last_used INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS app_passwords_login ON app_passwords (login);
CREATE TABLE IF NOT EXISTS login_flows (
  poll_hash    TEXT    PRIMARY KEY,
  login_token  TEXT    NOT NULL UNIQUE,
  client       TEXT    NOT NULL,
  created      INTEGER NOT NULL,
  login        TEXT,
  app_password TEXT
);
CREATE TABLE IF NOT EXISTS people (
  login TEXT PRIMARY KEY,
  name  TEXT NOT NULL,
  email TEXT NOT NULL,
  seen  INTEGER NOT NULL
);
"#;

pub struct Auth {
    failures: Mutex<HashMap<String, (u32, Instant)>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No credentials at all.
    None,
    /// Credentials that did not check out.
    Wrong,
    /// Too many wrong ones lately.
    Throttled,
}

/// The name and password in a Basic header.
pub fn basic(header: &str) -> Option<(String, String)> {
    use base64::Engine;
    let b64 = header
        .strip_prefix("Basic ")
        .or_else(|| header.strip_prefix("basic "))?;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    let s = String::from_utf8(raw).ok()?;
    let (u, p) = s.split_once(':')?;
    Some((u.to_owned(), p.to_owned()))
}

impl Auth {
    pub fn new(store: &Store) -> Result<Auth, StoreError> {
        store.with_db(|db| Ok(db.execute_batch(SCHEMA)?))?;
        Ok(Auth {
            failures: Mutex::new(HashMap::new()),
        })
    }

    /// Who a request is from, given its headers. `trust_header` is false
    /// only in tests that exercise the app-password path alone.
    pub fn identify(
        &self,
        store: &Store,
        user: Option<&str>,
        name: Option<&str>,
        email: Option<&str>,
        authorization: Option<&str>,
    ) -> Result<Who, Refusal> {
        if let Some(login) = user.filter(|u| !u.is_empty()) {
            if !crate::cloud::store::valid_owner(login) {
                return Err(Refusal::Wrong);
            }
            let who = Who {
                login: login.to_owned(),
                name: name.filter(|n| !n.is_empty()).unwrap_or(login).to_owned(),
                email: email.unwrap_or_default().to_owned(),
                via: Via::Header,
            };
            let _ = self.remember(store, &who);
            return Ok(who);
        }
        let (login, password) = authorization.and_then(basic).ok_or(Refusal::None)?;
        if !crate::cloud::store::valid_owner(&login) {
            return Err(Refusal::Wrong);
        }
        if self.throttled(&login) {
            return Err(Refusal::Throttled);
        }
        match self.check_app_password(store, &login, &password) {
            Some(id) => {
                let (name, email) = self.person(store, &login);
                Ok(Who {
                    login,
                    name,
                    email,
                    via: Via::AppPassword(id),
                })
            }
            None => {
                self.fail(&login);
                Err(Refusal::Wrong)
            }
        }
    }

    fn throttled(&self, login: &str) -> bool {
        let mut f = self.failures.lock().unwrap_or_else(|p| p.into_inner());
        match f.get(login) {
            Some((n, since)) if since.elapsed() < FAILURE_WINDOW => *n >= FAILURES,
            Some(_) => {
                f.remove(login);
                false
            }
            None => false,
        }
    }

    fn fail(&self, login: &str) {
        let mut f = self.failures.lock().unwrap_or_else(|p| p.into_inner());
        if f.len() > 10_000 {
            f.clear();
        }
        let e = f.entry(login.to_owned()).or_insert((0, Instant::now()));
        if e.1.elapsed() >= FAILURE_WINDOW {
            *e = (0, Instant::now());
        }
        e.0 += 1;
    }

    /// The name and address LosOS Login last gave for a login.
    pub fn person(&self, store: &Store, login: &str) -> (String, String) {
        store
            .with_db(|db| {
                Ok(db
                    .query_row(
                        "SELECT name, email FROM people WHERE login = ?1",
                        params![login],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?)
            })
            .ok()
            .flatten()
            .unwrap_or_else(|| (login.to_owned(), String::new()))
    }

    fn remember(&self, store: &Store, who: &Who) -> Result<(), StoreError> {
        store.with_db(|db| {
            db.execute(
                "INSERT INTO people (login, name, email, seen) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (login) DO UPDATE SET name = excluded.name, email = excluded.email, seen = excluded.seen
                 WHERE people.name != excluded.name OR people.email != excluded.email OR people.seen < excluded.seen - 3600",
                params![who.login, who.name, who.email, now()],
            )?;
            Ok(())
        })
    }

    /// Everyone the box has seen, for share pickers.
    pub fn people(&self, store: &Store) -> Result<Vec<(String, String)>, StoreError> {
        store.with_db(|db| {
            let mut stmt =
                db.prepare("SELECT login, name FROM people ORDER BY name COLLATE NOCASE")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    fn check_app_password(&self, store: &Store, login: &str, password: &str) -> Option<i64> {
        // Nextcloud's own apps send the password with or without dashes in
        // some versions; only the letters and digits count.
        let cleaned: String = password
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        if cleaned.len() != APP_PASSWORD_LEN {
            return None;
        }
        let hash = sha256_hex(cleaned.as_bytes());
        store
            .with_db(|db| {
                let id: Option<i64> = db
                    .query_row(
                        "SELECT id FROM app_passwords WHERE hash = ?1 AND login = ?2",
                        params![hash, login],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(id) = id {
                    db.execute(
                        "UPDATE app_passwords SET last_used = ?1 WHERE id = ?2 AND last_used < ?1 - 60",
                        params![now(), id],
                    )?;
                }
                Ok(id)
            })
            .ok()
            .flatten()
    }

    /// Make an app password for `login`, named after the client. Returns
    /// it in the clear, the one time it is ever seen.
    pub fn new_app_password(
        &self,
        store: &Store,
        login: &str,
        name: &str,
    ) -> Result<(i64, String), StoreError> {
        let password = random_token(APP_PASSWORD_LEN);
        let hash = sha256_hex(password.as_bytes());
        let name: String = name.chars().take(200).collect();
        let id = store.with_db(|db| {
            db.execute(
                "INSERT INTO app_passwords (login, name, hash, created, last_used) VALUES (?1, ?2, ?3, ?4, ?4)",
                params![login, name, hash, now()],
            )?;
            Ok(db.last_insert_rowid())
        })?;
        Ok((id, password))
    }

    pub fn app_passwords(
        &self,
        store: &Store,
        login: &str,
    ) -> Result<Vec<AppPassword>, StoreError> {
        store.with_db(|db| {
            let mut stmt = db.prepare(
                "SELECT id, name, created, last_used FROM app_passwords WHERE login = ?1 ORDER BY last_used DESC",
            )?;
            let rows = stmt.query_map(params![login], |r| {
                Ok(AppPassword {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    created: r.get(2)?,
                    last_used: r.get(3)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn revoke(&self, store: &Store, login: &str, id: i64) -> Result<bool, StoreError> {
        store.with_db(|db| {
            Ok(db.execute(
                "DELETE FROM app_passwords WHERE login = ?1 AND id = ?2",
                params![login, id],
            )? > 0)
        })
    }

    /// Start a Login Flow v2. Returns the poll token and the login token.
    pub fn start_flow(&self, store: &Store, client: &str) -> Result<(String, String), StoreError> {
        let poll = random_token(128);
        let login = random_token(128);
        let client: String = client.chars().take(200).collect();
        store.with_db(|db| {
            db.execute(
                "DELETE FROM login_flows WHERE created < ?1",
                params![now() - FLOW_TTL],
            )?;
            db.execute(
                "INSERT INTO login_flows (poll_hash, login_token, client, created) VALUES (?1, ?2, ?3, ?4)",
                params![sha256_hex(poll.as_bytes()), login, client, now()],
            )?;
            Ok(())
        })?;
        Ok((poll, login))
    }

    /// The flow behind a login token, for the grant page.
    pub fn flow(&self, store: &Store, login_token: &str) -> Result<Option<Flow>, StoreError> {
        store.with_db(|db| {
            Ok(db
                .query_row(
                    "SELECT client, created, login, app_password FROM login_flows WHERE login_token = ?1 AND created >= ?2",
                    params![login_token, now() - FLOW_TTL],
                    |r| {
                        let login: Option<String> = r.get(2)?;
                        let pw: Option<String> = r.get(3)?;
                        Ok(Flow {
                            client: r.get(0)?,
                            created: r.get(1)?,
                            granted: login.zip(pw),
                        })
                    },
                )
                .optional()?)
        })
    }

    /// The person said yes: make the app password and park it for the poll.
    pub fn grant_flow(
        &self,
        store: &Store,
        login_token: &str,
        who: &Who,
    ) -> Result<bool, StoreError> {
        let Some(flow) = self.flow(store, login_token)? else {
            return Ok(false);
        };
        if flow.granted.is_some() {
            return Ok(false);
        }
        let (_, password) = self.new_app_password(store, &who.login, &flow.client)?;
        store.with_db(|db| {
            Ok(db.execute(
                "UPDATE login_flows SET login = ?1, app_password = ?2 WHERE login_token = ?3 AND login IS NULL",
                params![who.login, password, login_token],
            )? > 0)
        })
    }

    /// The poll: the login and app password once granted, exactly once.
    pub fn poll_flow(
        &self,
        store: &Store,
        poll: &str,
    ) -> Result<Option<(String, String)>, StoreError> {
        let hash = sha256_hex(poll.as_bytes());
        store.with_db(|db| {
            let row: Option<(Option<String>, Option<String>)> = db
                .query_row(
                    "SELECT login, app_password FROM login_flows WHERE poll_hash = ?1 AND created >= ?2",
                    params![hash, now() - FLOW_TTL],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            match row {
                Some((Some(login), Some(pw))) => {
                    db.execute("DELETE FROM login_flows WHERE poll_hash = ?1", params![hash])?;
                    Ok(Some((login, pw)))
                }
                _ => Ok(None),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Store, Auth) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        let a = Auth::new(&s).unwrap();
        (d, s, a)
    }

    fn basic_header(u: &str, p: &str) -> String {
        use base64::Engine;
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{u}:{p}"))
        )
    }

    #[test]
    fn the_header_names_the_person() {
        let (_d, s, a) = setup();
        let who = a
            .identify(&s, Some("alice"), Some("Alice A."), Some("a@x"), None)
            .unwrap();
        assert_eq!(who.login, "alice");
        assert_eq!(who.name, "Alice A.");
        assert_eq!(who.via, Via::Header);
        assert_eq!(a.person(&s, "alice"), ("Alice A.".into(), "a@x".into()));
        assert!(a.identify(&s, Some("../root"), None, None, None).is_err());
    }

    #[test]
    fn app_passwords_open_only_their_own_login() {
        let (_d, s, a) = setup();
        let (id, pw) = a.new_app_password(&s, "alice", "Desktop").unwrap();
        assert_eq!(pw.len(), APP_PASSWORD_LEN);
        let who = a
            .identify(&s, None, None, None, Some(&basic_header("alice", &pw)))
            .unwrap();
        assert_eq!(who.via, Via::AppPassword(id));
        assert_eq!(
            a.identify(&s, None, None, None, Some(&basic_header("bob", &pw))),
            Err(Refusal::Wrong)
        );
        assert_eq!(a.identify(&s, None, None, None, None), Err(Refusal::None));
        assert!(a.revoke(&s, "alice", id).unwrap());
        assert_eq!(
            a.identify(&s, None, None, None, Some(&basic_header("alice", &pw))),
            Err(Refusal::Wrong)
        );
    }

    #[test]
    fn wrong_passwords_are_throttled() {
        let (_d, s, a) = setup();
        let (_, pw) = a.new_app_password(&s, "alice", "Desktop").unwrap();
        for _ in 0..FAILURES {
            let _ = a.identify(&s, None, None, None, Some(&basic_header("alice", "x")));
        }
        assert_eq!(
            a.identify(&s, None, None, None, Some(&basic_header("alice", &pw))),
            Err(Refusal::Throttled)
        );
    }

    #[test]
    fn a_login_flow_hands_out_its_password_once() {
        let (_d, s, a) = setup();
        let (poll, token) = a.start_flow(&s, "Nextcloud desktop (laptop)").unwrap();
        assert_eq!(a.poll_flow(&s, &poll).unwrap(), None);
        let flow = a.flow(&s, &token).unwrap().unwrap();
        assert_eq!(flow.client, "Nextcloud desktop (laptop)");
        let who = Who {
            login: "alice".into(),
            name: "Alice".into(),
            email: String::new(),
            via: Via::Header,
        };
        assert!(a.grant_flow(&s, &token, &who).unwrap());
        assert!(!a.grant_flow(&s, &token, &who).unwrap(), "granted once");
        let (login, pw) = a.poll_flow(&s, &poll).unwrap().unwrap();
        assert_eq!(login, "alice");
        assert!(a
            .identify(&s, None, None, None, Some(&basic_header("alice", &pw)))
            .is_ok());
        assert_eq!(a.poll_flow(&s, &poll).unwrap(), None, "only once");
        assert_eq!(
            a.app_passwords(&s, "alice").unwrap()[0].name,
            "Nextcloud desktop (laptop)"
        );
    }
}
