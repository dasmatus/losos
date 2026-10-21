//! The records git does not keep: accounts, tokens, repositories and who
//! may work on them, in one SQLite file.
//!
//! One connection behind a mutex. A box serves a handful of people, every
//! statement here is a lookup by key, and SQLite in WAL mode makes the
//! writes durable without a server. The schema grows by appending to
//! [`MIGRATIONS`]; `PRAGMA user_version` records how many have run.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

/// Each entry runs once, in order, inside one transaction with the version
/// bump. Never edit one that has shipped; add another.
const MIGRATIONS: &[&str] = &[
    // 1: accounts, tokens, repositories, collaborators.
    "
    CREATE TABLE users (
        id          INTEGER PRIMARY KEY,
        login       TEXT NOT NULL,
        lower_login TEXT NOT NULL UNIQUE,
        full_name   TEXT NOT NULL DEFAULT '',
        email       TEXT NOT NULL DEFAULT '',
        is_admin    INTEGER NOT NULL DEFAULT 0,
        created     TEXT NOT NULL
    );
    CREATE TABLE tokens (
        id         INTEGER PRIMARY KEY,
        user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        name       TEXT NOT NULL,
        hash       TEXT NOT NULL UNIQUE,
        last_eight TEXT NOT NULL,
        scopes     TEXT NOT NULL,
        created    TEXT NOT NULL,
        last_used  TEXT,
        UNIQUE (user_id, name)
    );
    CREATE TABLE repos (
        id             INTEGER PRIMARY KEY,
        owner_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        name           TEXT NOT NULL,
        lower_name     TEXT NOT NULL,
        description    TEXT NOT NULL DEFAULT '',
        private        INTEGER NOT NULL DEFAULT 1,
        default_branch TEXT NOT NULL DEFAULT 'main',
        fork_of        INTEGER REFERENCES repos(id) ON DELETE SET NULL,
        created        TEXT NOT NULL,
        updated        TEXT NOT NULL,
        UNIQUE (owner_id, lower_name)
    );
    CREATE TABLE collaborators (
        repo_id    INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
        user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        permission TEXT NOT NULL CHECK (permission IN ('read', 'write', 'admin')),
        PRIMARY KEY (repo_id, user_id)
    );
    ",
    // 2: issues and pull requests with their comments, labels, assignees
    // and attachments, and what a Forgejo box brought along: its tokens
    // keep working (`salt`: hashed Forgejo's way), and every imported row
    // remembers its Forgejo id so a second import changes nothing.
    "
    ALTER TABLE tokens ADD COLUMN salt TEXT;
    CREATE INDEX tokens_last_eight ON tokens (last_eight) WHERE salt IS NOT NULL;
    CREATE TABLE issues (
        id           INTEGER PRIMARY KEY,
        repo_id      INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
        number       INTEGER NOT NULL,
        poster_id    INTEGER REFERENCES users(id) ON DELETE SET NULL,
        poster_name  TEXT NOT NULL DEFAULT '',
        title        TEXT NOT NULL,
        body         TEXT NOT NULL DEFAULT '',
        closed       INTEGER NOT NULL DEFAULT 0,
        is_pull      INTEGER NOT NULL DEFAULT 0,
        head_branch  TEXT,
        base_branch  TEXT,
        merged       INTEGER NOT NULL DEFAULT 0,
        merge_commit TEXT,
        created      TEXT NOT NULL,
        updated      TEXT NOT NULL,
        closed_at    TEXT,
        UNIQUE (repo_id, number)
    );
    CREATE TABLE issue_comments (
        id          INTEGER PRIMARY KEY,
        issue_id    INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
        poster_id   INTEGER REFERENCES users(id) ON DELETE SET NULL,
        poster_name TEXT NOT NULL DEFAULT '',
        body        TEXT NOT NULL DEFAULT '',
        created     TEXT NOT NULL,
        updated     TEXT NOT NULL
    );
    CREATE TABLE labels (
        id          INTEGER PRIMARY KEY,
        repo_id     INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
        name        TEXT NOT NULL,
        color       TEXT NOT NULL DEFAULT '',
        description TEXT NOT NULL DEFAULT '',
        UNIQUE (repo_id, name)
    );
    CREATE TABLE issue_labels (
        issue_id INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
        label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
        PRIMARY KEY (issue_id, label_id)
    );
    CREATE TABLE issue_assignees (
        issue_id INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
        user_id  INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        PRIMARY KEY (issue_id, user_id)
    );
    CREATE TABLE attachments (
        id         INTEGER PRIMARY KEY,
        uuid       TEXT NOT NULL UNIQUE,
        issue_id   INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
        comment_id INTEGER REFERENCES issue_comments(id) ON DELETE CASCADE,
        name       TEXT NOT NULL,
        size       INTEGER NOT NULL DEFAULT 0,
        created    TEXT NOT NULL
    );
    CREATE TABLE forgejo_ids (
        kind       TEXT NOT NULL,
        forgejo_id INTEGER NOT NULL,
        local_id   INTEGER NOT NULL,
        PRIMARY KEY (kind, forgejo_id)
    );
    ",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: i64,
    pub login: String,
    pub full_name: String,
    pub email: String,
    pub is_admin: bool,
    pub created: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Token {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    /// The last eight characters, so the owner can tell two apart.
    pub last_eight: String,
    pub scopes: Vec<String>,
    pub created: String,
    pub last_used: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Repo {
    pub id: i64,
    pub owner_id: i64,
    pub owner: String,
    pub name: String,
    pub description: String,
    pub private: bool,
    pub default_branch: String,
    pub fork_of: Option<i64>,
    pub created: String,
    pub updated: String,
}

/// What a person may do in one repository, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    None,
    Read,
    Write,
    Admin,
}

impl Permission {
    pub fn parse(s: &str) -> Option<Permission> {
        match s {
            "read" => Some(Permission::Read),
            "write" => Some(Permission::Write),
            "admin" => Some(Permission::Admin),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Permission::None => "none",
            Permission::Read => "read",
            Permission::Write => "write",
            Permission::Admin => "admin",
        }
    }
}

/// An issue, or a pull request (`is_pull`), of one repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub id: i64,
    pub repo_id: i64,
    pub number: i64,
    /// The account that opened it, or the name it had when that account is
    /// gone.
    pub poster: String,
    pub title: String,
    pub body: String,
    pub closed: bool,
    pub is_pull: bool,
    pub head_branch: Option<String>,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub merge_commit: Option<String>,
    pub created: String,
    pub updated: String,
    pub closed_at: Option<String>,
    pub labels: Vec<Label>,
    pub assignees: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comment {
    pub id: i64,
    pub poster: String,
    pub body: String,
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attachment {
    pub uuid: String,
    pub issue_id: i64,
    pub comment_id: Option<i64>,
    pub name: String,
    pub size: i64,
    pub created: String,
}

/// Which issues a list asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IssueFilter {
    /// `None`: open and closed.
    pub closed: Option<bool>,
    /// `None`: issues and pull requests.
    pub pulls: Option<bool>,
}

/// Typed so the API can answer 409 rather than 500.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0} already exists")]
pub struct Exists(pub String);

pub struct Store {
    conn: Mutex<Connection>,
}

const USER_COLS: &str = "id, login, full_name, email, is_admin, created";
const REPO_COLS: &str = "r.id, r.owner_id, u.login, r.name, r.description, r.private, \
     r.default_branch, r.fork_of, r.created, r.updated";

fn user_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        login: row.get(1)?,
        full_name: row.get(2)?,
        email: row.get(3)?,
        is_admin: row.get(4)?,
        created: row.get(5)?,
    })
}

fn repo_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Repo> {
    Ok(Repo {
        id: row.get(0)?,
        owner_id: row.get(1)?,
        owner: row.get(2)?,
        name: row.get(3)?,
        description: row.get(4)?,
        private: row.get(5)?,
        default_branch: row.get(6)?,
        fork_of: row.get(7)?,
        created: row.get(8)?,
        updated: row.get(9)?,
    })
}

fn token_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Token> {
    let scopes: String = row.get(5)?;
    Ok(Token {
        id: row.get(0)?,
        user_id: row.get(1)?,
        name: row.get(2)?,
        last_eight: row.get(3)?,
        created: row.get(4)?,
        scopes: scopes
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        last_used: row.get(6)?,
    })
}

const ISSUE_COLS: &str = "i.id, i.repo_id, i.number, COALESCE(u.login, i.poster_name), i.title, \
     i.body, i.closed, i.is_pull, i.head_branch, i.base_branch, i.merged, i.merge_commit, \
     i.created, i.updated, i.closed_at";

fn issue_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Issue> {
    Ok(Issue {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        number: row.get(2)?,
        poster: row.get(3)?,
        title: row.get(4)?,
        body: row.get(5)?,
        closed: row.get(6)?,
        is_pull: row.get(7)?,
        head_branch: row.get(8)?,
        base_branch: row.get(9)?,
        merged: row.get(10)?,
        merge_commit: row.get(11)?,
        created: row.get(12)?,
        updated: row.get(13)?,
        closed_at: row.get(14)?,
        labels: Vec::new(),
        assignees: Vec::new(),
    })
}

fn unique_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                || f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
    )
}

/// SHA-256 of a token, hex. Tokens are 160 random bits, so a plain hash is
/// enough to keep a copy of the database from opening anything.
pub fn token_hash(token: &str) -> String {
    hex(ring::digest::digest(&ring::digest::SHA256, token.as_bytes()).as_ref())
}

/// Forgejo's hash of a token: PBKDF2-HMAC-SHA256, 10 000 rounds, 50 bytes,
/// hex. Tokens a Forgejo box made were imported with it and their salt, so
/// the computers that hold them keep working.
pub fn forgejo_token_hash(token: &str, salt: &str) -> String {
    let mut out = [0u8; 50];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        std::num::NonZeroU32::new(10_000).expect("non-zero"),
        salt.as_bytes(),
        token.as_bytes(),
        &mut out,
    );
    hex(&out)
}

/// An imported Forgejo token whose value this is, if any. Found by its last
/// eight characters first, so at most a few hashes are worked out.
fn legacy_token(conn: &Connection, value: &str) -> anyhow::Result<Option<Token>> {
    if value.len() < 8 || !value.is_char_boundary(value.len() - 8) {
        return Ok(None);
    }
    let mut stmt = conn.prepare(
        "SELECT id, user_id, name, last_eight, created, scopes, last_used, hash, salt
         FROM tokens WHERE salt IS NOT NULL AND last_eight = ?1",
    )?;
    let rows = stmt.query_map([&value[value.len() - 8..]], |row| {
        Ok((
            token_row(row)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;
    for row in rows {
        let (token, hash, salt) = row?;
        // Comparing two hashes: how long it takes says nothing of the token.
        if forgejo_token_hash(value, &salt) == hash {
            return Ok(Some(token));
        }
    }
    Ok(None)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A new token: 20 random bytes as 40 hex characters, the shape git
/// credential helpers and Forgejo's tokens had.
pub fn new_token() -> anyhow::Result<String> {
    use ring::rand::SecureRandom;
    let mut bytes = [0u8; 20];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("no randomness for a token"))?;
    Ok(hex(&bytes))
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Store> {
        let conn = Connection::open(path)?;
        Store::init(conn)
    }

    pub fn in_memory() -> anyhow::Result<Store> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> anyhow::Result<Store> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    pub(crate) fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A panic while the lock was held leaves a connection that is still
        // consistent (every write is a statement or a transaction).
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    // ── Accounts ──────────────────────────────────────────────────────────

    pub fn user(&self, login: &str) -> anyhow::Result<Option<User>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {USER_COLS} FROM users WHERE lower_login = ?1"),
                [login.to_lowercase()],
                user_row,
            )
            .optional()?)
    }

    pub fn user_by_id(&self, id: i64) -> anyhow::Result<Option<User>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {USER_COLS} FROM users WHERE id = ?1"),
                [id],
                user_row,
            )
            .optional()?)
    }

    pub fn users(&self) -> anyhow::Result<Vec<User>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {USER_COLS} FROM users ORDER BY lower_login"
        ))?;
        let rows = stmt.query_map([], user_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The account `login`, made if it is not there. A name or address
    /// given replaces the one on record; an empty one leaves it.
    pub fn ensure_user(&self, login: &str, full_name: &str, email: &str) -> anyhow::Result<User> {
        crate::forge::names::owner(login).map_err(|e| anyhow::anyhow!(e))?;
        {
            let conn = self.conn();
            conn.execute(
                "INSERT INTO users (login, lower_login, full_name, email, created)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (lower_login) DO NOTHING",
                params![login, login.to_lowercase(), full_name, email, super::now()],
            )?;
            if !full_name.is_empty() {
                conn.execute(
                    "UPDATE users SET full_name = ?2 WHERE lower_login = ?1",
                    params![login.to_lowercase(), full_name],
                )?;
            }
            if !email.is_empty() {
                conn.execute(
                    "UPDATE users SET email = ?2 WHERE lower_login = ?1",
                    params![login.to_lowercase(), email],
                )?;
            }
        }
        self.user(login)?
            .ok_or_else(|| anyhow::anyhow!("the account {login} vanished"))
    }

    pub fn set_admin(&self, user_id: i64, admin: bool) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE users SET is_admin = ?2 WHERE id = ?1",
            params![user_id, admin],
        )?;
        Ok(())
    }

    // ── Tokens ────────────────────────────────────────────────────────────

    /// A new token for `user_id`, and its value, which is never stored and
    /// never shown again.
    pub fn create_token(
        &self,
        user_id: i64,
        name: &str,
        scopes: &[String],
    ) -> anyhow::Result<(Token, String)> {
        let value = new_token()?;
        let last_eight = value[value.len() - 8..].to_string();
        let conn = self.conn();
        let inserted = conn.execute(
            "INSERT INTO tokens (user_id, name, hash, last_eight, scopes, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                user_id,
                name,
                token_hash(&value),
                last_eight,
                scopes.join(","),
                super::now()
            ],
        );
        match inserted {
            Err(e) if unique_violation(&e) => {
                return Err(Exists(format!("a token named \"{name}\"")).into())
            }
            other => other?,
        };
        let id = conn.last_insert_rowid();
        let token = conn.query_row(
            "SELECT id, user_id, name, last_eight, created, scopes, last_used
             FROM tokens WHERE id = ?1",
            [id],
            token_row,
        )?;
        Ok((token, value))
    }

    /// Whose token `value` is, and the token. Marks it used.
    pub fn token_owner(&self, value: &str) -> anyhow::Result<Option<(User, Token)>> {
        let conn = self.conn();
        let token = conn
            .query_row(
                "SELECT id, user_id, name, last_eight, created, scopes, last_used
                 FROM tokens WHERE hash = ?1",
                [token_hash(value)],
                token_row,
            )
            .optional()?;
        let token = match token {
            Some(t) => Some(t),
            None => legacy_token(&conn, value)?,
        };
        let Some(token) = token else { return Ok(None) };
        conn.execute(
            "UPDATE tokens SET last_used = ?2 WHERE id = ?1",
            params![token.id, super::now()],
        )?;
        let user = conn.query_row(
            &format!("SELECT {USER_COLS} FROM users WHERE id = ?1"),
            [token.user_id],
            user_row,
        )?;
        Ok(Some((user, token)))
    }

    pub fn tokens(&self, user_id: i64) -> anyhow::Result<Vec<Token>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, last_eight, created, scopes, last_used
             FROM tokens WHERE user_id = ?1 ORDER BY created, id",
        )?;
        let rows = stmt.query_map([user_id], token_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Delete a token by its id or its name. Whether there was one.
    pub fn delete_token(&self, user_id: i64, id_or_name: &str) -> anyhow::Result<bool> {
        let n = match id_or_name.parse::<i64>() {
            Ok(id) => self.conn().execute(
                "DELETE FROM tokens WHERE user_id = ?1 AND (id = ?2 OR name = ?3)",
                params![user_id, id, id_or_name],
            )?,
            Err(_) => self.conn().execute(
                "DELETE FROM tokens WHERE user_id = ?1 AND name = ?2",
                params![user_id, id_or_name],
            )?,
        };
        Ok(n > 0)
    }

    // ── Repositories ──────────────────────────────────────────────────────

    pub fn repo(&self, owner: &str, name: &str) -> anyhow::Result<Option<Repo>> {
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {REPO_COLS} FROM repos r JOIN users u ON u.id = r.owner_id
                     WHERE u.lower_login = ?1 AND r.lower_name = ?2"
                ),
                params![owner.to_lowercase(), name.to_lowercase()],
                repo_row,
            )
            .optional()?)
    }

    pub fn repo_by_id(&self, id: i64) -> anyhow::Result<Option<Repo>> {
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {REPO_COLS} FROM repos r JOIN users u ON u.id = r.owner_id
                     WHERE r.id = ?1"
                ),
                [id],
                repo_row,
            )
            .optional()?)
    }

    /// Every repository, for an administrator and for the start-up scan.
    pub fn all_repos(&self) -> anyhow::Result<Vec<Repo>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {REPO_COLS} FROM repos r JOIN users u ON u.id = r.owner_id
             ORDER BY u.lower_login, r.lower_name"
        ))?;
        let rows = stmt.query_map([], repo_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The repositories `owner_id` owns.
    pub fn repos_of(&self, owner_id: i64) -> anyhow::Result<Vec<Repo>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {REPO_COLS} FROM repos r JOIN users u ON u.id = r.owner_id
             WHERE r.owner_id = ?1 ORDER BY r.lower_name"
        ))?;
        let rows = stmt.query_map([owner_id], repo_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The repositories `user_id` works on as a collaborator.
    pub fn repos_shared_with(&self, user_id: i64) -> anyhow::Result<Vec<Repo>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {REPO_COLS} FROM repos r JOIN users u ON u.id = r.owner_id
             JOIN collaborators c ON c.repo_id = r.id
             WHERE c.user_id = ?1 ORDER BY u.lower_login, r.lower_name"
        ))?;
        let rows = stmt.query_map([user_id], repo_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn create_repo(
        &self,
        owner_id: i64,
        name: &str,
        description: &str,
        private: bool,
        default_branch: &str,
        fork_of: Option<i64>,
    ) -> anyhow::Result<Repo> {
        let now = super::now();
        let id = {
            let conn = self.conn();
            let inserted = conn.execute(
                "INSERT INTO repos (owner_id, name, lower_name, description, private,
                                    default_branch, fork_of, created, updated)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    owner_id,
                    name,
                    name.to_lowercase(),
                    description,
                    private,
                    default_branch,
                    fork_of,
                    now
                ],
            );
            match inserted {
                Err(e) if unique_violation(&e) => {
                    return Err(Exists(format!("a repository named \"{name}\"")).into())
                }
                other => other?,
            };
            conn.last_insert_rowid()
        };
        self.repo_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("the new repository vanished"))
    }

    /// Change what is said about a repository. `None` leaves a field.
    pub fn update_repo(
        &self,
        id: i64,
        name: Option<&str>,
        description: Option<&str>,
        private: Option<bool>,
        default_branch: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn();
        if let Some(name) = name {
            let renamed = conn.execute(
                "UPDATE repos SET name = ?2, lower_name = ?3 WHERE id = ?1",
                params![id, name, name.to_lowercase()],
            );
            match renamed {
                Err(e) if unique_violation(&e) => {
                    return Err(Exists(format!("a repository named \"{name}\"")).into())
                }
                other => other?,
            };
        }
        if let Some(d) = description {
            conn.execute(
                "UPDATE repos SET description = ?2 WHERE id = ?1",
                params![id, d],
            )?;
        }
        if let Some(p) = private {
            conn.execute(
                "UPDATE repos SET private = ?2 WHERE id = ?1",
                params![id, p],
            )?;
        }
        if let Some(b) = default_branch {
            conn.execute(
                "UPDATE repos SET default_branch = ?2 WHERE id = ?1",
                params![id, b],
            )?;
        }
        conn.execute(
            "UPDATE repos SET updated = ?2 WHERE id = ?1",
            params![id, super::now()],
        )?;
        Ok(())
    }

    /// A push landed.
    pub fn touch_repo(&self, id: i64) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE repos SET updated = ?2 WHERE id = ?1",
            params![id, super::now()],
        )?;
        Ok(())
    }

    pub fn delete_repo(&self, id: i64) -> anyhow::Result<()> {
        self.conn()
            .execute("DELETE FROM repos WHERE id = ?1", [id])?;
        Ok(())
    }

    // ── Collaborators ─────────────────────────────────────────────────────

    pub fn collaborators(&self, repo_id: i64) -> anyhow::Result<Vec<(User, Permission)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT u.id, u.login, u.full_name, u.email, u.is_admin, u.created, c.permission
             FROM collaborators c JOIN users u ON u.id = c.user_id
             WHERE c.repo_id = ?1 ORDER BY u.lower_login",
        )?;
        let rows = stmt.query_map([repo_id], |row| {
            let p: String = row.get(6)?;
            Ok((
                user_row(row)?,
                Permission::parse(&p).unwrap_or(Permission::Read),
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn collaborator(&self, repo_id: i64, user_id: i64) -> anyhow::Result<Permission> {
        let p: Option<String> = self
            .conn()
            .query_row(
                "SELECT permission FROM collaborators WHERE repo_id = ?1 AND user_id = ?2",
                params![repo_id, user_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(p.and_then(|p| Permission::parse(&p))
            .unwrap_or(Permission::None))
    }

    pub fn set_collaborator(
        &self,
        repo_id: i64,
        user_id: i64,
        permission: Permission,
    ) -> anyhow::Result<()> {
        let conn = self.conn();
        if permission == Permission::None {
            conn.execute(
                "DELETE FROM collaborators WHERE repo_id = ?1 AND user_id = ?2",
                params![repo_id, user_id],
            )?;
        } else {
            conn.execute(
                "INSERT INTO collaborators (repo_id, user_id, permission) VALUES (?1, ?2, ?3)
                 ON CONFLICT (repo_id, user_id) DO UPDATE SET permission = excluded.permission",
                params![repo_id, user_id, permission.as_str()],
            )?;
        }
        Ok(())
    }
}

impl Store {
    // ── Issues ────────────────────────────────────────────────────────────

    /// The labels and assignees an issue row does not carry.
    fn fill_issue(conn: &Connection, mut issue: Issue) -> anyhow::Result<Issue> {
        let mut stmt = conn.prepare(
            "SELECT l.id, l.name, l.color, l.description FROM issue_labels il
             JOIN labels l ON l.id = il.label_id WHERE il.issue_id = ?1 ORDER BY l.name",
        )?;
        issue.labels = stmt
            .query_map([issue.id], |r| {
                Ok(Label {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                    description: r.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut stmt = conn.prepare(
            "SELECT u.login FROM issue_assignees a JOIN users u ON u.id = a.user_id
             WHERE a.issue_id = ?1 ORDER BY u.lower_login",
        )?;
        issue.assignees = stmt
            .query_map([issue.id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(issue)
    }

    /// A repository's issues, newest first.
    pub fn issues(&self, repo_id: i64, filter: IssueFilter) -> anyhow::Result<Vec<Issue>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {ISSUE_COLS} FROM issues i LEFT JOIN users u ON u.id = i.poster_id
             WHERE i.repo_id = ?1 AND (?2 IS NULL OR i.closed = ?2)
               AND (?3 IS NULL OR i.is_pull = ?3)
             ORDER BY i.number DESC"
        ))?;
        let rows: Vec<Issue> = stmt
            .query_map(params![repo_id, filter.closed, filter.pulls], issue_row)?
            .collect::<Result<_, _>>()?;
        rows.into_iter()
            .map(|i| Store::fill_issue(&conn, i))
            .collect()
    }

    pub fn issue(&self, repo_id: i64, number: i64) -> anyhow::Result<Option<Issue>> {
        let conn = self.conn();
        let issue = conn
            .query_row(
                &format!(
                    "SELECT {ISSUE_COLS} FROM issues i LEFT JOIN users u ON u.id = i.poster_id
                     WHERE i.repo_id = ?1 AND i.number = ?2"
                ),
                params![repo_id, number],
                issue_row,
            )
            .optional()?;
        issue.map(|i| Store::fill_issue(&conn, i)).transpose()
    }

    pub fn comments(&self, issue_id: i64) -> anyhow::Result<Vec<Comment>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT c.id, COALESCE(u.login, c.poster_name), c.body, c.created, c.updated
             FROM issue_comments c LEFT JOIN users u ON u.id = c.poster_id
             WHERE c.issue_id = ?1 ORDER BY c.created, c.id",
        )?;
        let rows = stmt.query_map([issue_id], |r| {
            Ok(Comment {
                id: r.get(0)?,
                poster: r.get(1)?,
                body: r.get(2)?,
                created: r.get(3)?,
                updated: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn labels(&self, repo_id: i64) -> anyhow::Result<Vec<Label>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, color, description FROM labels WHERE repo_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map([repo_id], |r| {
            Ok(Label {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                description: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// An attachment and the repository its issue is in.
    pub fn attachment(&self, uuid: &str) -> anyhow::Result<Option<(Attachment, i64)>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT a.uuid, a.issue_id, a.comment_id, a.name, a.size, a.created, i.repo_id
                 FROM attachments a JOIN issues i ON i.id = a.issue_id WHERE a.uuid = ?1",
                [uuid],
                |r| {
                    Ok((
                        Attachment {
                            uuid: r.get(0)?,
                            issue_id: r.get(1)?,
                            comment_id: r.get(2)?,
                            name: r.get(3)?,
                            size: r.get(4)?,
                            created: r.get(5)?,
                        },
                        r.get(6)?,
                    ))
                },
            )
            .optional()?)
    }

    pub fn attachments(&self, issue_id: i64) -> anyhow::Result<Vec<Attachment>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT uuid, issue_id, comment_id, name, size, created FROM attachments
             WHERE issue_id = ?1 ORDER BY created, id",
        )?;
        let rows = stmt.query_map([issue_id], |r| {
            Ok(Attachment {
                uuid: r.get(0)?,
                issue_id: r.get(1)?,
                comment_id: r.get(2)?,
                name: r.get(3)?,
                size: r.get(4)?,
                created: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_are_made_once_and_keep_their_case() {
        let s = Store::in_memory().unwrap();
        let a = s.ensure_user("Jana", "Jana Novak", "").unwrap();
        let b = s.ensure_user("jana", "", "jana@example.org").unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(b.login, "Jana");
        assert_eq!(
            b.full_name, "Jana Novak",
            "an empty name leaves the old one"
        );
        assert_eq!(b.email, "jana@example.org");
        assert!(s.ensure_user("settings", "", "").is_err());
    }

    #[test]
    fn tokens_are_stored_as_hashes_and_found_by_value() {
        let s = Store::in_memory().unwrap();
        let u = s.ensure_user("notshared", "", "").unwrap();
        let (t, value) = s
            .create_token(u.id, "LosOS on laptop", &["write:repository".into()])
            .unwrap();
        assert_eq!(value.len(), 40);
        assert_eq!(t.last_eight, value[32..]);
        assert_eq!(t.scopes, vec!["write:repository".to_string()]);
        let err = s.create_token(u.id, "LosOS on laptop", &[]).unwrap_err();
        assert!(err.downcast_ref::<Exists>().is_some());
        let (who, found) = s.token_owner(&value).unwrap().unwrap();
        assert_eq!(who.login, "notshared");
        assert!(found.last_used.is_none(), "the row read before the touch");
        assert!(s.tokens(u.id).unwrap()[0].last_used.is_some());
        assert!(s.token_owner("nope").unwrap().is_none());
        let stored: String = s
            .conn()
            .query_row("SELECT hash FROM tokens", [], |r| r.get(0))
            .unwrap();
        assert_ne!(stored, value);
        assert!(s.delete_token(u.id, "LosOS on laptop").unwrap());
        assert!(!s.delete_token(u.id, "LosOS on laptop").unwrap());
        assert!(s.token_owner(&value).unwrap().is_none());
    }

    #[test]
    fn forgejo_tokens_open_with_their_old_value() {
        let s = Store::in_memory().unwrap();
        let u = s.ensure_user("matus", "", "").unwrap();
        let value = "0123456789abcdef0123456789abcdef89abcdef";
        let salt = "Xy3kQ9mW2p";
        s.conn()
            .execute(
                "INSERT INTO tokens (user_id, name, hash, last_eight, scopes, created, salt)
                 VALUES (?1, 'laptop', ?2, '89abcdef', 'write:repository', '2026-01-01T00:00:00Z', ?3)",
                params![u.id, forgejo_token_hash(value, salt), salt],
            )
            .unwrap();
        let (who, t) = s.token_owner(value).unwrap().unwrap();
        assert_eq!((who.login.as_str(), t.name.as_str()), ("matus", "laptop"));
        assert!(s
            .token_owner("ffffffffffffffffffffffffffffffff89abcdef")
            .unwrap()
            .is_none());
        assert!(s.token_owner("short").unwrap().is_none());
    }

    #[test]
    fn repositories_are_unique_per_owner_whatever_the_case() {
        let s = Store::in_memory().unwrap();
        let u = s.ensure_user("notshared", "", "").unwrap();
        let v = s.ensure_user("jana", "", "").unwrap();
        let r = s
            .create_repo(u.id, "Losos-Config", "", true, "main", None)
            .unwrap();
        assert_eq!(r.owner, "notshared");
        assert!(s
            .create_repo(u.id, "losos-config", "", true, "main", None)
            .unwrap_err()
            .downcast_ref::<Exists>()
            .is_some());
        s.create_repo(v.id, "losos-config", "", false, "main", None)
            .unwrap();
        assert_eq!(
            s.repo("NOTSHARED", "losos-config").unwrap().unwrap().id,
            r.id
        );
        s.update_repo(
            r.id,
            Some("config"),
            Some("mine"),
            Some(false),
            Some("trunk"),
        )
        .unwrap();
        let r = s.repo("notshared", "config").unwrap().unwrap();
        assert_eq!(
            (r.description.as_str(), r.private, r.default_branch.as_str()),
            ("mine", false, "trunk")
        );
        s.set_collaborator(r.id, v.id, Permission::Write).unwrap();
        assert_eq!(s.collaborator(r.id, v.id).unwrap(), Permission::Write);
        assert_eq!(s.repos_shared_with(v.id).unwrap().len(), 1);
        s.set_collaborator(r.id, v.id, Permission::None).unwrap();
        assert_eq!(s.collaborator(r.id, v.id).unwrap(), Permission::None);
        s.delete_repo(r.id).unwrap();
        assert!(s.repo("notshared", "config").unwrap().is_none());
    }
}
