//! The people list for LosOS Desktop computers joined to this box
//! (`POST /api/devices/directory`, `GET /api/directory/people`).
//!
//! A company's computers log their people in with LosOS Login and keep each
//! account as a homed record, so a laptop logs in offline. Someone the
//! company removes or disables here would then still log in on a computer
//! until their next login that reaches the box. A joined computer therefore
//! reads the people list on a timer and locks the accounts that are gone.
//!
//! It reads it with a token of its own: `losos_dir_<id>_<secret>`, made by an
//! administrator when the computer joins, sent as `Authorization: Bearer`,
//! which nginx passes on without asking LosOS Login. The token is nobody: it
//! reaches the one `GET` and reads names, display names, e-mail addresses,
//! groups and whether each account is disabled, never a password hash or a
//! second factor. Besides that it asks for the computer's own Mesh CI jobs
//! (`runs-on: [losos-desktop, <computer>]`), which `losos-ci` keeps for it
//! and lososd relays. Only its SHA-256 is kept, in `directory.json` beside
//! `state.json`, so an erase takes it with the rest.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Most computers one box keeps a directory token for.
pub const MAX_COMPUTERS: usize = 256;
const PREFIX: &str = "losos_dir_";
const ID_HEX: usize = 8;
const SECRET_HEX: usize = 40;
/// `lastUsed` is written back at most this often, so a fleet's timers do
/// not rewrite the file on every read.
const TOUCH_SECS: i64 = 300;

/// The route a directory token reads the people list at.
pub const PEOPLE_ROUTE: &str = "/api/directory/people";
/// Mesh CI's jobs for the computer (`runs-on: [losos-desktop, <computer>]`,
/// `crate::ci::workflow::desktop_job`): asking for one, and reporting on it.
pub const JOBS_CLAIM_ROUTE: &str = "/api/directory/jobs/claim";
pub const JOBS_REPORT_ROUTE: &str = "/api/directory/jobs/report";

/// May a directory token make this call? The people list, and its own
/// Mesh CI jobs.
pub fn route_allowed(method: &str, path: &str) -> bool {
    (method.eq_ignore_ascii_case("GET") && path == PEOPLE_ROUTE)
        || (method.eq_ignore_ascii_case("POST")
            && (path == JOBS_CLAIM_ROUTE || path == JOBS_REPORT_ROUTE))
}

/// One joined computer as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: String,
    /// The computer's name, checked like every device name
    /// (`crate::devices::validate_device`).
    pub device: String,
    /// The administrator who joined it, `None` for the spare key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joined_by: Option<String>,
    pub created: i64,
    #[serde(default)]
    pub last_used: Option<i64>,
    pub sha256: String,
}

/// One joined computer as the admin pages see it: everything but the hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Computer {
    pub id: String,
    pub device: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joined_by: Option<String>,
    pub created: i64,
    pub last_used: Option<i64>,
}

impl From<&Record> for Computer {
    fn from(r: &Record) -> Computer {
        Computer {
            id: r.id.clone(),
            device: r.device.clone(),
            joined_by: r.joined_by.clone(),
            created: r.created,
            last_used: r.last_used,
        }
    }
}

/// What went wrong, as the HTTP layer answers it.
#[derive(Debug, thiserror::Error)]
pub enum DirectoryError {
    /// 400: the sentence is shown.
    #[error("{0}")]
    Invalid(String),
    /// 409.
    #[error("{MAX_COMPUTERS} computers are already joined; take one out first")]
    Full,
    /// 404.
    #[error("no joined computer has that id")]
    NotFound,
    #[error(transparent)]
    Io(#[from] anyhow::Error),
}

fn sha256_hex(s: &str) -> String {
    crate::boxid::sha256(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn random_hex(n: usize) -> String {
    let mut bytes = vec![0u8; n / 2];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The id part of a well-formed token.
fn token_id(token: &str) -> Option<&str> {
    let rest = token.strip_prefix(PREFIX)?;
    let (id, secret) = rest.split_once('_')?;
    let hex = |s: &str, n: usize| {
        s.len() == n
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    (hex(id, ID_HEX) && hex(secret, SECRET_HEX)).then_some(id)
}

/// Whether `token` has a directory token's shape, so the HTTP layer asks
/// this store and not LosOS Login about it.
pub fn is_directory_token(token: &str) -> bool {
    token.starts_with(PREFIX)
}

/// The joined computers' file. One lock around every read-modify-write.
pub struct Directory {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Directory {
    pub fn new(path: impl Into<PathBuf>) -> Directory {
        Directory {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    /// `directory.json` in lososd's state directory, beside `state.json`.
    pub fn from_env() -> Directory {
        Directory::new(
            PathBuf::from(
                std::env::var("LOSOS_STATE_DIR").unwrap_or_else(|_| "/var/lib/losos".into()),
            )
            .join("directory.json"),
        )
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> anyhow::Result<Vec<Record>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(anyhow::anyhow!("reading {}: {e}", self.path.display())),
        }
    }

    fn write(&self, records: &[Record]) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec_pretty(records)?;
        crate::io_backend::atomic_write_secret(&self.path, &bytes)
    }

    /// Every joined computer, oldest first.
    pub fn list(&self) -> Result<Vec<Computer>, DirectoryError> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read()?.iter().map(Computer::from).collect())
    }

    /// Join `device`: a new token for it, answered once. A computer that
    /// joins again under the same name replaces its old token, so a
    /// reinstalled laptop does not leave one behind.
    pub fn join(
        &self,
        device: &str,
        joined_by: Option<&str>,
        now: i64,
    ) -> Result<(String, Computer), DirectoryError> {
        let device = crate::devices::validate_device(device).map_err(DirectoryError::Invalid)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut records = self.read()?;
        records.retain(|r| r.device != device);
        if records.len() >= MAX_COMPUTERS {
            return Err(DirectoryError::Full);
        }
        let id = loop {
            let id = random_hex(ID_HEX);
            if !records.iter().any(|r| r.id == id) {
                break id;
            }
        };
        let token = format!("{PREFIX}{id}_{}", random_hex(SECRET_HEX));
        let record = Record {
            id,
            device,
            joined_by: joined_by.map(str::to_string),
            created: now,
            last_used: None,
            sha256: sha256_hex(&token),
        };
        let computer = Computer::from(&record);
        records.push(record);
        self.write(&records)?;
        Ok((token, computer))
    }

    /// Take a computer out: its token stops working at once.
    pub fn remove(&self, id: &str) -> Result<(), DirectoryError> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut records = self.read()?;
        let before = records.len();
        records.retain(|r| r.id != id);
        if records.len() == before {
            return Err(DirectoryError::NotFound);
        }
        self.write(&records)?;
        Ok(())
    }

    /// The computer `token` belongs to, when it is one of ours. Every
    /// failure is the same `None`.
    pub fn check(&self, token: &str, now: i64) -> Option<Computer> {
        let id = token_id(token)?;
        let hash = sha256_hex(token);
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut records = self.read().ok()?;
        let record = records.iter_mut().find(|r| r.id == id)?;
        let same = record.sha256.len() == hash.len()
            && record
                .sha256
                .bytes()
                .zip(hash.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0;
        if !same {
            return None;
        }
        let computer = Computer::from(&*record);
        if record.last_used.is_none_or(|t| now - t >= TOUCH_SECS) {
            record.last_used = Some(now);
            // A failed write costs a stale "last used", not the read.
            let _ = self.write(&records);
        }
        Some(computer)
    }
}

/// The people list a joined computer reads: what it needs to lock an
/// account and keep a name in step, and nothing it could sign in with.
pub fn people(listed: Vec<crate::people::Listed>) -> serde_json::Value {
    serde_json::json!({ "people": listed })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Directory) {
        let dir = tempfile::tempdir().unwrap();
        let store = Directory::new(dir.path().join("directory.json"));
        (dir, store)
    }

    #[test]
    fn a_token_reads_as_its_computer_and_keeps_only_a_hash() {
        let (_dir, store) = store();
        let (token, joined) = store.join(" ada-laptop ", Some("matus"), 100).unwrap();
        assert!(token.starts_with("losos_dir_"));
        assert!(is_directory_token(&token));
        let found = store.check(&token, 200).unwrap();
        assert_eq!(found.device, "ada-laptop");
        assert_eq!(found.joined_by.as_deref(), Some("matus"));
        assert_eq!(found.id, joined.id);
    }

    #[test]
    fn a_wrong_or_misshapen_token_is_nobody() {
        let (_dir, store) = store();
        let (token, _) = store.join("laptop", None, 100).unwrap();
        let mut wrong = token.clone();
        let last = wrong.pop().unwrap();
        wrong.push(if last == '0' { '1' } else { '0' });
        assert!(store.check(&wrong, 200).is_none());
        assert!(store.check("losos_dir_zz", 200).is_none());
        assert!(store.check("losos_pat_00000000_0", 200).is_none());
        let file = std::fs::read_to_string(store.path()).unwrap();
        assert!(!file.contains(&token), "the token itself is never written");
    }

    #[test]
    fn joining_again_under_the_same_name_replaces_the_old_token() {
        let (_dir, store) = store();
        let (old, _) = store.join("laptop", None, 100).unwrap();
        let (new, _) = store.join("laptop", None, 200).unwrap();
        assert!(store.check(&old, 300).is_none());
        assert!(store.check(&new, 300).is_some());
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn a_computer_taken_out_stops_reading() {
        let (_dir, store) = store();
        let (token, joined) = store.join("laptop", None, 100).unwrap();
        store.remove(&joined.id).unwrap();
        assert!(store.check(&token, 200).is_none());
        assert!(matches!(
            store.remove(&joined.id),
            Err(DirectoryError::NotFound)
        ));
    }

    #[test]
    fn a_bad_name_is_refused() {
        let (_dir, store) = store();
        assert!(matches!(
            store.join("a/b", None, 1),
            Err(DirectoryError::Invalid(_))
        ));
    }

    #[test]
    fn use_is_written_back_now_and_then() {
        let (_dir, store) = store();
        let (token, _) = store.join("laptop", None, 100).unwrap();
        store.check(&token, 1_000).unwrap();
        store.check(&token, 1_010).unwrap();
        assert_eq!(store.list().unwrap()[0].last_used, Some(1_000));
        store.check(&token, 1_000 + TOUCH_SECS).unwrap();
        assert_eq!(store.list().unwrap()[0].last_used, Some(1_000 + TOUCH_SECS));
    }

    #[test]
    fn only_the_people_list_is_reachable() {
        assert!(route_allowed("GET", PEOPLE_ROUTE));
        assert!(!route_allowed("POST", PEOPLE_ROUTE));
        assert!(!route_allowed("GET", "/api/people"));
        assert!(!route_allowed("GET", "/api/directory/people/x"));
        assert!(route_allowed("POST", JOBS_CLAIM_ROUTE));
        assert!(route_allowed("POST", JOBS_REPORT_ROUTE));
        assert!(!route_allowed("GET", JOBS_CLAIM_ROUTE));
        assert!(!route_allowed("POST", "/api/ci/runs/1/cancel"));
    }

    #[test]
    fn the_list_carries_no_password_hash() {
        let v = people(vec![crate::people::Listed {
            name: "ada".into(),
            display_name: "Ada Lovelace".into(),
            email: "ada@example.com".into(),
            groups: vec!["losos-admins".into()],
            disabled: false,
        }]);
        assert_eq!(v["people"][0]["displayName"], "Ada Lovelace");
        assert!(!v.to_string().contains("password"));
    }
}
