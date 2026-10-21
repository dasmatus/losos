//! What the mesh remembers: the boxes it has met, the shares it sent and
//! the shares it was sent. One SQLite file, `<dir>/mesh.db`.
//!
//! Outgoing shares live here until the cloud's `shares` table carries mesh
//! rows (kind "mesh"); incoming ones stay here (`incoming_mesh`).

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::cloud::collab::store::{FileId, Perm};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS pins (
  name TEXT PRIMARY KEY,
  key TEXT NOT NULL,
  seen INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS outgoing (
  id TEXT PRIMARY KEY,
  owner TEXT NOT NULL,
  owner_address TEXT NOT NULL,
  owner_name TEXT NOT NULL,
  file_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  folder INTEGER NOT NULL,
  recipient TEXT NOT NULL,
  remote TEXT NOT NULL,
  perm TEXT NOT NULL,
  state TEXT NOT NULL,
  created INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS outgoing_remote ON outgoing (remote);
CREATE TABLE IF NOT EXISTS incoming_mesh (
  id TEXT NOT NULL,
  remote TEXT NOT NULL,
  owner_remote TEXT NOT NULL,
  owner_name TEXT NOT NULL,
  recipient TEXT NOT NULL,
  name TEXT NOT NULL,
  folder INTEGER NOT NULL,
  size INTEGER NOT NULL,
  perm TEXT NOT NULL,
  state TEXT NOT NULL,
  created INTEGER NOT NULL,
  unsynced INTEGER NOT NULL DEFAULT 0,
  file_id INTEGER NOT NULL,
  PRIMARY KEY (remote, id)
);
";

/// What became of a box's key.
#[derive(Debug, PartialEq, Eq)]
pub enum Pin {
    /// Never met; now remembered.
    New,
    /// The key it had.
    Same,
    /// It answers with another key now; this is the one it had.
    Changed(String),
}

/// Where a share sent from here stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutState {
    /// The other box was not reached yet; it is tried again.
    Unsent,
    /// The other box has it; its person has not answered.
    Pending,
    Accepted,
    Declined,
}

impl OutState {
    fn as_str(self) -> &'static str {
        match self {
            OutState::Unsent => "unsent",
            OutState::Pending => "pending",
            OutState::Accepted => "accepted",
            OutState::Declined => "declined",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "pending" => OutState::Pending,
            "accepted" => OutState::Accepted,
            "declined" => OutState::Declined,
            _ => OutState::Unsent,
        }
    }
}

/// A share sent from this box.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Outgoing {
    pub id: String,
    /// The cloud login whose file it is.
    pub owner: String,
    /// `matus@mattbox.example`, as the other box shows it.
    pub owner_address: String,
    pub owner_name: String,
    pub file_id: FileId,
    pub name: String,
    pub folder: bool,
    /// `bob@boxb.example`.
    pub recipient: String,
    pub remote: String,
    pub perm: Perm,
    pub state: OutState,
    pub created: i64,
}

/// A share another box sent to someone here.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Incoming {
    pub id: String,
    pub remote: String,
    /// `alice@boxa.example`.
    pub owner_remote: String,
    pub owner_name: String,
    pub recipient: String,
    pub name: String,
    /// The file or folder's id on the owner's box, which every request
    /// about it names.
    pub file_id: FileId,
    pub folder: bool,
    pub size: u64,
    pub perm: Perm,
    /// "pending", "accepted" or "declined".
    pub state: String,
    pub created: i64,
    /// Edited here while the other box was out of reach.
    pub unsynced: bool,
}

fn perm_str(p: Perm) -> &'static str {
    match p {
        Perm::Edit => "edit",
        Perm::Read => "read",
        Perm::None => "none",
    }
}

fn perm_parse(s: &str) -> Perm {
    match s {
        "edit" => Perm::Edit,
        "read" => Perm::Read,
        _ => Perm::None,
    }
}

pub struct Db {
    conn: Mutex<Connection>,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::with(Connection::open(path)?)
    }

    pub fn memory() -> rusqlite::Result<Self> {
        Self::with(Connection::open_in_memory()?)
    }

    fn with(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Db {
            conn: Mutex::new(conn),
        })
    }

    fn c(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Remember `name`'s key, or say how it compares to the one we had.
    pub fn pin(&self, name: &str, key: &str) -> rusqlite::Result<Pin> {
        let c = self.c();
        let had: Option<String> = c
            .query_row("SELECT key FROM pins WHERE name = ?1", [name], |r| r.get(0))
            .optional()?;
        match had {
            Some(k) if k == key => Ok(Pin::Same),
            Some(k) => Ok(Pin::Changed(k)),
            None => {
                c.execute(
                    "INSERT INTO pins (name, key, seen) VALUES (?1, ?2, ?3)",
                    params![name, key, now()],
                )?;
                Ok(Pin::New)
            }
        }
    }

    pub fn pinned(&self, name: &str) -> Option<String> {
        self.c()
            .query_row("SELECT key FROM pins WHERE name = ?1", [name], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    /// Forget a box's key: the owner checked the new one some other way.
    pub fn unpin(&self, name: &str) -> rusqlite::Result<()> {
        self.c()
            .execute("DELETE FROM pins WHERE name = ?1", [name])?;
        Ok(())
    }

    pub fn add_outgoing(&self, s: &Outgoing) -> rusqlite::Result<()> {
        self.c().execute(
            "INSERT INTO outgoing (id, owner, file_id, name, folder, recipient, remote, perm, state, created, owner_address, owner_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                s.id,
                s.owner,
                s.file_id,
                s.name,
                s.folder,
                s.recipient,
                s.remote,
                perm_str(s.perm),
                s.state.as_str(),
                s.created,
                s.owner_address,
                s.owner_name
            ],
        )?;
        Ok(())
    }

    fn out_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Outgoing> {
        Ok(Outgoing {
            id: r.get(0)?,
            owner: r.get(1)?,
            file_id: r.get(2)?,
            name: r.get(3)?,
            folder: r.get(4)?,
            recipient: r.get(5)?,
            remote: r.get(6)?,
            perm: perm_parse(&r.get::<_, String>(7)?),
            state: OutState::parse(&r.get::<_, String>(8)?),
            created: r.get(9)?,
            owner_address: r.get(10)?,
            owner_name: r.get(11)?,
        })
    }

    const OUT_COLS: &'static str =
        "id, owner, file_id, name, folder, recipient, remote, perm, state, created, owner_address, owner_name";

    pub fn outgoing(&self, id: &str) -> Option<Outgoing> {
        self.c()
            .query_row(
                &format!("SELECT {} FROM outgoing WHERE id = ?1", Self::OUT_COLS),
                [id],
                Self::out_row,
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Shares `owner` sent, newest first.
    pub fn outgoing_of(&self, owner: &str) -> Vec<Outgoing> {
        self.query_out("WHERE owner = ?1 ORDER BY created DESC", owner)
    }

    /// Shares sent to `recipient` (`bob@boxb`) on `remote`.
    pub fn outgoing_of_recipient(&self, recipient: &str, remote: &str) -> Vec<Outgoing> {
        let c = self.c();
        let Ok(mut st) = c.prepare(&format!(
            "SELECT {} FROM outgoing WHERE recipient = ?1 AND remote = ?2",
            Self::OUT_COLS
        )) else {
            return Vec::new();
        };
        st.query_map([recipient, remote], Self::out_row)
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    /// Shares to `remote` that it has not heard of yet.
    pub fn unsent_to(&self, remote: &str) -> Vec<Outgoing> {
        self.query_out("WHERE remote = ?1 AND state = 'unsent'", remote)
    }

    /// Every remote with a share waiting to go out.
    pub fn unsent_remotes(&self) -> Vec<String> {
        let c = self.c();
        let Ok(mut st) = c.prepare("SELECT DISTINCT remote FROM outgoing WHERE state = 'unsent'")
        else {
            return Vec::new();
        };
        st.query_map([], |r| r.get(0))
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    fn query_out(&self, tail: &str, arg: &str) -> Vec<Outgoing> {
        let c = self.c();
        let Ok(mut st) = c.prepare(&format!("SELECT {} FROM outgoing {tail}", Self::OUT_COLS))
        else {
            return Vec::new();
        };
        st.query_map([arg], Self::out_row)
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    pub fn set_out_state(&self, id: &str, state: OutState) -> rusqlite::Result<bool> {
        Ok(self.c().execute(
            "UPDATE outgoing SET state = ?2 WHERE id = ?1",
            params![id, state.as_str()],
        )? > 0)
    }

    pub fn set_out_perm(&self, id: &str, perm: Perm) -> rusqlite::Result<bool> {
        Ok(self.c().execute(
            "UPDATE outgoing SET perm = ?2 WHERE id = ?1",
            params![id, perm_str(perm)],
        )? > 0)
    }

    pub fn remove_outgoing(&self, id: &str) -> rusqlite::Result<bool> {
        Ok(self
            .c()
            .execute("DELETE FROM outgoing WHERE id = ?1", [id])?
            > 0)
    }

    /// Take a share offered by `remote`. An offer it already sent is
    /// updated (name, size, permission), keeping the person's answer.
    pub fn offer(&self, s: &Incoming) -> rusqlite::Result<()> {
        self.c().execute(
            "INSERT INTO incoming_mesh (id, remote, owner_remote, owner_name, recipient, name, folder, size, perm, state, created, file_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', ?10, ?11)
             ON CONFLICT (remote, id) DO UPDATE SET
               owner_name = excluded.owner_name, name = excluded.name, folder = excluded.folder,
               size = excluded.size, perm = excluded.perm",
            params![
                s.id,
                s.remote,
                s.owner_remote,
                s.owner_name,
                s.recipient,
                s.name,
                s.folder,
                s.size as i64,
                perm_str(s.perm),
                s.created,
                s.file_id
            ],
        )?;
        Ok(())
    }

    fn in_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Incoming> {
        Ok(Incoming {
            id: r.get(0)?,
            remote: r.get(1)?,
            owner_remote: r.get(2)?,
            owner_name: r.get(3)?,
            recipient: r.get(4)?,
            name: r.get(5)?,
            folder: r.get(6)?,
            size: r.get::<_, i64>(7)? as u64,
            perm: perm_parse(&r.get::<_, String>(8)?),
            state: r.get(9)?,
            created: r.get(10)?,
            unsynced: r.get(11)?,
            file_id: r.get(12)?,
        })
    }

    const IN_COLS: &'static str =
        "id, remote, owner_remote, owner_name, recipient, name, folder, size, perm, state, created, unsynced, file_id";

    /// Shares sent to `login`, newest first.
    /// Offers from `remote` nobody here has answered yet.
    pub fn unanswered_from(&self, remote: &str) -> u64 {
        self.c()
            .query_row(
                "SELECT count(*) FROM incoming_mesh WHERE remote = ?1 AND state = 'pending'",
                [remote],
                |r| r.get::<_, i64>(0),
            )
            .map_or(0, |n| n.max(0) as u64)
    }

    pub fn incoming_for(&self, login: &str) -> Vec<Incoming> {
        let c = self.c();
        let Ok(mut st) = c.prepare(&format!(
            "SELECT {} FROM incoming_mesh WHERE recipient = ?1 ORDER BY created DESC",
            Self::IN_COLS
        )) else {
            return Vec::new();
        };
        st.query_map([login], Self::in_row)
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    /// One incoming share, by its id alone (ids are random and unique).
    pub fn incoming(&self, id: &str) -> Option<Incoming> {
        self.c()
            .query_row(
                &format!("SELECT {} FROM incoming_mesh WHERE id = ?1", Self::IN_COLS),
                [id],
                Self::in_row,
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Copies edited here that the owner's box has not seen yet.
    pub fn unsynced(&self) -> Vec<Incoming> {
        let c = self.c();
        let Ok(mut st) = c.prepare(&format!(
            "SELECT {} FROM incoming_mesh WHERE unsynced = 1 AND state = 'accepted'",
            Self::IN_COLS
        )) else {
            return Vec::new();
        };
        st.query_map([], Self::in_row)
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    pub fn set_in_state(&self, remote: &str, id: &str, state: &str) -> rusqlite::Result<bool> {
        Ok(self.c().execute(
            "UPDATE incoming_mesh SET state = ?3 WHERE remote = ?1 AND id = ?2",
            params![remote, id, state],
        )? > 0)
    }

    pub fn set_in_perm(&self, remote: &str, id: &str, perm: Perm) -> rusqlite::Result<bool> {
        Ok(self.c().execute(
            "UPDATE incoming_mesh SET perm = ?3 WHERE remote = ?1 AND id = ?2",
            params![remote, id, perm_str(perm)],
        )? > 0)
    }

    pub fn set_unsynced(&self, id: &str, unsynced: bool) -> rusqlite::Result<()> {
        self.c().execute(
            "UPDATE incoming_mesh SET unsynced = ?2 WHERE id = ?1",
            params![id, unsynced],
        )?;
        Ok(())
    }

    pub fn remove_incoming(&self, remote: &str, id: &str) -> rusqlite::Result<bool> {
        Ok(self.c().execute(
            "DELETE FROM incoming_mesh WHERE remote = ?1 AND id = ?2",
            params![remote, id],
        )? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pinned_key_does_not_change_quietly() {
        let db = Db::memory().unwrap();
        assert_eq!(db.pin("boxb", "k1").unwrap(), Pin::New);
        assert_eq!(db.pin("boxb", "k1").unwrap(), Pin::Same);
        assert_eq!(db.pin("boxb", "k2").unwrap(), Pin::Changed("k1".into()));
        assert_eq!(db.pinned("boxb").as_deref(), Some("k1"));
        db.unpin("boxb").unwrap();
        assert_eq!(db.pin("boxb", "k2").unwrap(), Pin::New);
    }

    #[test]
    fn an_offer_sent_twice_keeps_the_answer() {
        let db = Db::memory().unwrap();
        let mut s = Incoming {
            id: "s1".into(),
            remote: "boxa".into(),
            owner_remote: "alice@boxa".into(),
            owner_name: "Alice".into(),
            recipient: "bob".into(),
            name: "notes.md".into(),
            file_id: 4,
            folder: false,
            size: 10,
            perm: Perm::Edit,
            state: "pending".into(),
            created: 1,
            unsynced: false,
        };
        db.offer(&s).unwrap();
        db.set_in_state("boxa", "s1", "accepted").unwrap();
        s.perm = Perm::Read;
        db.offer(&s).unwrap();
        let got = db.incoming_for("bob");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].state, "accepted");
        assert_eq!(got[0].perm, Perm::Read);
        // Another box cannot answer for boxa's share.
        assert!(!db.set_in_state("boxc", "s1", "declined").unwrap());
    }

    #[test]
    fn outgoing_round_trip() {
        let db = Db::memory().unwrap();
        let s = Outgoing {
            id: "x".into(),
            owner: "alice".into(),
            owner_address: "alice@boxa".into(),
            owner_name: "Alice".into(),
            file_id: 4,
            name: "n.md".into(),
            folder: false,
            recipient: "bob@boxb".into(),
            remote: "boxb".into(),
            perm: Perm::Edit,
            state: OutState::Unsent,
            created: 5,
        };
        db.add_outgoing(&s).unwrap();
        assert_eq!(db.outgoing("x"), Some(s.clone()));
        assert_eq!(db.unsent_remotes(), vec!["boxb".to_string()]);
        db.set_out_state("x", OutState::Pending).unwrap();
        assert!(db.unsent_to("boxb").is_empty());
        assert_eq!(db.outgoing_of("alice")[0].state, OutState::Pending);
    }
}
