//! Forgejo's records, carried into LosOS Git once, on the first start after
//! a box stops running Forgejo (`losos-git import-forgejo`, run by
//! `losos-git-forgejo-import.service` in modules/git.nix).
//!
//! The unit exports the tables below from Forgejo's database as JSON, one
//! array of rows per table (`<from>/tables/<table>.json`, the shape both
//! `psql`'s `json_agg` and `sqlite3 -json` give), next to a full dump of the
//! database and a copy of the attachments. This reads the JSON and writes
//! what LosOS Git has a place for: accounts, repositories, collaborators,
//! tokens (hashed as Forgejo hashed them, so they keep working), labels,
//! issues, pull requests, comments, assignees and attachments.
//!
//! Nothing is lost quietly. Every table read is counted, every row not
//! carried over is counted with the reason, and the report says so
//! (`<from>/import.json`); the full dump stays beside it. Every imported row
//! remembers its Forgejo id (`forgejo_ids`), so running it again changes
//! nothing that is already there.

use super::db::Store;
use super::{names, repo_path};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// The tables the unit exports. The order is the order they are read in.
pub const TABLES: &[&str] = &[
    "user",
    "org_user",
    "repository",
    "collaboration",
    "access_token",
    "label",
    "issue",
    "pull_request",
    "comment",
    "issue_label",
    "issue_assignees",
    "attachment",
    "milestone",
    "release",
];

/// What happened, table by table.
#[derive(Debug, Default, Serialize)]
pub struct Report {
    /// Rows Forgejo had, per table.
    pub forgejo: BTreeMap<String, usize>,
    /// Rows now in LosOS Git (this run or an earlier one), per table.
    pub imported: BTreeMap<String, usize>,
    /// Every row left out, and why, in plain words.
    pub not_imported: Vec<String>,
}

impl Report {
    fn note(&mut self, line: String) {
        tracing::warn!("{line}");
        self.not_imported.push(line);
    }

    fn count(&mut self, table: &str) {
        *self.imported.entry(table.to_string()).or_default() += 1;
    }
}

type Row = serde_json::Map<String, Value>;

fn int(row: &Row, key: &str) -> i64 {
    match row.get(key) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        Some(Value::Bool(b)) => i64::from(*b),
        _ => 0,
    }
}

fn text(row: &Row, key: &str) -> String {
    match row.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Postgres says `true`, SQLite says `1`.
fn flag(row: &Row, key: &str) -> bool {
    match row.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(s)) => matches!(s.as_str(), "t" | "true" | "1"),
        _ => false,
    }
}

/// A Unix time as the records keep times; 0 (never) as now.
fn time(row: &Row, key: &str) -> String {
    let secs = int(row, key);
    if secs <= 0 {
        return super::now();
    }
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(super::now)
}

fn read_table(from: &Path, table: &str) -> anyhow::Result<Vec<Row>> {
    let path = from.join("tables").join(format!("{table}.json"));
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(anyhow::anyhow!("reading {}: {e}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("{} is not JSON: {e}", path.display()))?;
    match doc {
        Value::Array(rows) => Ok(rows
            .into_iter()
            .filter_map(|r| match r {
                Value::Object(o) => Some(o),
                _ => None,
            })
            .collect()),
        Value::Null => Ok(Vec::new()),
        _ => anyhow::bail!("{} is not a list of rows", path.display()),
    }
}

/// Forgejo's scopes as LosOS Git knows them. Families LosOS Git has no use
/// for (packages, notifications, organisations, ActivityPub) are dropped.
fn scopes(forgejo: &str) -> Vec<String> {
    let mut out: Vec<String> = forgejo
        .split(',')
        .map(str::trim)
        .filter(|s| {
            *s == "all"
                || s.split_once(':').is_some_and(|(level, family)| {
                    matches!(level, "read" | "write")
                        && super::auth::SCOPE_FAMILIES.contains(&family)
                })
        })
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn mapped(tx: &Transaction<'_>, kind: &str, forgejo_id: i64) -> rusqlite::Result<Option<i64>> {
    tx.query_row(
        "SELECT local_id FROM forgejo_ids WHERE kind = ?1 AND forgejo_id = ?2",
        params![kind, forgejo_id],
        |r| r.get(0),
    )
    .optional()
}

fn remember(tx: &Transaction<'_>, kind: &str, forgejo_id: i64, local: i64) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO forgejo_ids (kind, forgejo_id, local_id) VALUES (?1, ?2, ?3)",
        params![kind, forgejo_id, local],
    )?;
    Ok(())
}

/// Read the export in `from` into `store`. Attachments are copied into
/// `<state_dir>/attachments/`. One transaction: a failure leaves LosOS Git
/// as it was, and the next start tries again.
pub fn import(store: &Store, state_dir: &Path, from: &Path) -> anyhow::Result<Report> {
    let mut tables = BTreeMap::new();
    for t in TABLES {
        tables.insert(*t, read_table(from, t)?);
    }
    let mut report = Report::default();
    // What the export itself could not do (no database found, a table
    // missing), one line each.
    if let Ok(text) = std::fs::read_to_string(from.join("export-notes.txt")) {
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            report.note(line.to_string());
        }
    }
    for (t, rows) in &tables {
        report.forgejo.insert((*t).to_string(), rows.len());
    }
    let repos_root = state_dir.join("repositories");
    let attachments = state_dir.join("attachments");

    let mut conn = store.conn();
    let tx = conn.transaction()?;

    // ── Accounts and organisations ───────────────────────────────────────
    let mut org_names: BTreeMap<i64, String> = BTreeMap::new();
    for row in &tables["user"] {
        let id = int(row, "id");
        let login = text(row, "name");
        let kind = int(row, "type");
        // 0 a person, 1 an organisation; anything else (bots, the ghost
        // account) has nothing of its own to carry.
        if kind != 0 && kind != 1 {
            report.note(format!(
                "Forgejo's built-in account \"{login}\" (type {kind}) was left out."
            ));
            continue;
        }
        if let Err(why) = names::owner(&login) {
            report.note(format!(
                "The account \"{login}\" was left out: {why}. Its rows are in the database dump."
            ));
            continue;
        }
        let local = match mapped(&tx, "user", id)? {
            Some(l) => l,
            None => {
                tx.execute(
                    "INSERT INTO users (login, lower_login, full_name, email, is_admin, created)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT (lower_login) DO UPDATE SET
                       full_name = CASE WHEN users.full_name = '' THEN excluded.full_name
                                        ELSE users.full_name END,
                       email = CASE WHEN users.email = '' THEN excluded.email
                                    ELSE users.email END,
                       is_admin = users.is_admin OR excluded.is_admin",
                    params![
                        login,
                        login.to_lowercase(),
                        text(row, "full_name"),
                        if kind == 0 { text(row, "email") } else { String::new() },
                        kind == 0 && flag(row, "is_admin"),
                        time(row, "created_unix"),
                    ],
                )?;
                let local: i64 = tx.query_row(
                    "SELECT id FROM users WHERE lower_login = ?1",
                    [login.to_lowercase()],
                    |r| r.get(0),
                )?;
                remember(&tx, "user", id, local)?;
                local
            }
        };
        let _ = local;
        if kind == 1 {
            org_names.insert(id, login.clone());
        }
        report.count("user");
    }
    for name in org_names.values() {
        report.note(format!(
            "The organisation \"{name}\" is an account of its own now; its members can \
             work on its repositories as collaborators, its teams were not carried over."
        ));
    }

    // ── Repositories ─────────────────────────────────────────────────────
    let mut repo_owner: BTreeMap<i64, i64> = BTreeMap::new(); // forgejo repo -> forgejo owner
    for row in &tables["repository"] {
        let id = int(row, "id");
        let owner_f = int(row, "owner_id");
        let name = text(row, "name");
        let Some(owner) = mapped(&tx, "user", owner_f)? else {
            report.note(format!(
                "The repository \"{}/{name}\" was left out: its owner was.",
                text(row, "owner_name")
            ));
            continue;
        };
        let owner_login: String =
            tx.query_row("SELECT login FROM users WHERE id = ?1", [owner], |r| r.get(0))?;
        if let Err(why) = names::repo(&name) {
            report.note(format!(
                "The repository \"{owner_login}/{name}\" was left out: {why}. Its Git data \
                 is still on disk."
            ));
            continue;
        }
        if !repo_path(&repos_root, &owner_login, &name).is_dir() {
            report.note(format!(
                "The repository \"{owner_login}/{name}\" had no Git data on disk, so only \
                 its record was left out."
            ));
            continue;
        }
        repo_owner.insert(id, owner_f);
        let branch = match text(row, "default_branch") {
            b if b.is_empty() => "main".to_string(),
            b => b,
        };
        let local = match mapped(&tx, "repo", id)? {
            Some(l) => l,
            None => {
                tx.execute(
                    "INSERT INTO repos (owner_id, name, lower_name, description, private,
                                        default_branch, created, updated)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT (owner_id, lower_name) DO UPDATE SET
                       description = excluded.description, private = excluded.private,
                       default_branch = excluded.default_branch, created = excluded.created",
                    params![
                        owner,
                        name,
                        name.to_lowercase(),
                        text(row, "description"),
                        flag(row, "is_private"),
                        branch,
                        time(row, "created_unix"),
                        time(row, "updated_unix"),
                    ],
                )?;
                let local: i64 = tx.query_row(
                    "SELECT id FROM repos WHERE owner_id = ?1 AND lower_name = ?2",
                    params![owner, name.to_lowercase()],
                    |r| r.get(0),
                )?;
                remember(&tx, "repo", id, local)?;
                local
            }
        };
        let _ = local;
        report.count("repository");
    }
    for row in &tables["repository"] {
        let fork = int(row, "fork_id");
        if fork > 0 {
            if let (Some(local), Some(parent)) =
                (mapped(&tx, "repo", int(row, "id"))?, mapped(&tx, "repo", fork)?)
            {
                tx.execute(
                    "UPDATE repos SET fork_of = ?2 WHERE id = ?1",
                    params![local, parent],
                )?;
            }
        }
    }

    // ── Who may work where ───────────────────────────────────────────────
    for row in &tables["collaboration"] {
        let (Some(repo), Some(user)) = (
            mapped(&tx, "repo", int(row, "repo_id"))?,
            mapped(&tx, "user", int(row, "user_id"))?,
        ) else {
            report.note(format!(
                "A collaborator of a repository that was left out (Forgejo row {}).",
                int(row, "id")
            ));
            continue;
        };
        let permission = match int(row, "mode") {
            1 => "read",
            2 => "write",
            _ => "admin",
        };
        tx.execute(
            "INSERT INTO collaborators (repo_id, user_id, permission) VALUES (?1, ?2, ?3)
             ON CONFLICT (repo_id, user_id) DO NOTHING",
            params![repo, user, permission],
        )?;
        report.count("collaboration");
    }
    for row in &tables["org_user"] {
        let org = int(row, "org_id");
        let Some(member) = mapped(&tx, "user", int(row, "uid"))? else {
            continue;
        };
        for (repo_f, _) in repo_owner.iter().filter(|(_, owner)| **owner == org) {
            if let Some(repo) = mapped(&tx, "repo", *repo_f)? {
                tx.execute(
                    "INSERT INTO collaborators (repo_id, user_id, permission)
                     VALUES (?1, ?2, 'write') ON CONFLICT (repo_id, user_id) DO NOTHING",
                    params![repo, member],
                )?;
            }
        }
        report.count("org_user");
    }

    // ── Tokens ───────────────────────────────────────────────────────────
    for row in &tables["access_token"] {
        let id = int(row, "id");
        let name = text(row, "name");
        let Some(user) = mapped(&tx, "user", int(row, "uid"))? else {
            report.note(format!(
                "The token \"{name}\" belonged to an account that was left out."
            ));
            continue;
        };
        if mapped(&tx, "token", id)?.is_some() {
            report.count("access_token");
            continue;
        }
        let (hash, salt, last_eight) = (
            text(row, "token_hash"),
            text(row, "token_salt"),
            text(row, "token_last_eight"),
        );
        if hash.is_empty() || salt.is_empty() || last_eight.len() != 8 {
            report.note(format!(
                "The token \"{name}\" was stored in a form LosOS Git cannot check; make a \
                 new one with `losos git setup`."
            ));
            continue;
        }
        let granted = scopes(&text(row, "scope"));
        if granted.is_empty() {
            report.note(format!(
                "The token \"{name}\" only allowed things LosOS Git does not have ({}).",
                text(row, "scope")
            ));
            continue;
        }
        // A token of the same name made since keeps the name; this one is
        // told apart.
        let taken: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM tokens WHERE user_id = ?1 AND name = ?2)",
            params![user, name],
            |r| r.get(0),
        )?;
        let local_name = if taken { format!("{name} (from Forgejo)") } else { name.clone() };
        tx.execute(
            "INSERT INTO tokens (user_id, name, hash, last_eight, scopes, created, last_used, salt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
            params![
                user,
                local_name,
                hash,
                last_eight,
                granted.join(","),
                time(row, "created_unix"),
                salt,
            ],
        )?;
        remember(&tx, "token", id, tx.last_insert_rowid())?;
        report.count("access_token");
    }

    // ── Labels ───────────────────────────────────────────────────────────
    for row in &tables["label"] {
        let id = int(row, "id");
        let name = text(row, "name");
        let Some(repo) = mapped(&tx, "repo", int(row, "repo_id"))? else {
            if int(row, "org_id") > 0 {
                report.note(format!(
                    "The organisation-wide label \"{name}\" was left out; issues that had it \
                     say so in the database dump."
                ));
            } else {
                report.note(format!("The label \"{name}\" of a repository that was left out."));
            }
            continue;
        };
        if mapped(&tx, "label", id)?.is_none() {
            tx.execute(
                "INSERT INTO labels (repo_id, name, color, description) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (repo_id, name) DO NOTHING",
                params![repo, name, text(row, "color"), text(row, "description")],
            )?;
            let local: i64 = tx.query_row(
                "SELECT id FROM labels WHERE repo_id = ?1 AND name = ?2",
                params![repo, name],
                |r| r.get(0),
            )?;
            remember(&tx, "label", id, local)?;
        }
        report.count("label");
    }

    // ── Issues and pull requests ─────────────────────────────────────────
    let poster = |tx: &Transaction<'_>, row: &Row, key: &str| -> rusqlite::Result<(Option<i64>, String)> {
        let user = mapped(tx, "user", int(row, key))?;
        let mut name = text(row, "original_author");
        if name.is_empty() {
            name = "ghost".to_string();
        }
        Ok((user, name))
    };
    for row in &tables["issue"] {
        let id = int(row, "id");
        let number = int(row, "index");
        let Some(repo) = mapped(&tx, "repo", int(row, "repo_id"))? else {
            report.note(format!(
                "Issue #{number} of a repository that was left out (Forgejo issue {id})."
            ));
            continue;
        };
        if mapped(&tx, "issue", id)?.is_none() {
            let (poster_id, poster_name) = poster(&tx, row, "poster_id")?;
            let closed = flag(row, "is_closed");
            tx.execute(
                "INSERT INTO issues (repo_id, number, poster_id, poster_name, title, body,
                                     closed, is_pull, created, updated, closed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    repo,
                    number,
                    poster_id,
                    poster_name,
                    text(row, "name"),
                    text(row, "content"),
                    closed,
                    flag(row, "is_pull"),
                    time(row, "created_unix"),
                    time(row, "updated_unix"),
                    closed.then(|| time(row, "closed_unix")),
                ],
            )?;
            remember(&tx, "issue", id, tx.last_insert_rowid())?;
        }
        report.count("issue");
    }
    for row in &tables["pull_request"] {
        let Some(issue) = mapped(&tx, "issue", int(row, "issue_id"))? else {
            report.note(format!(
                "A pull request whose issue was left out (Forgejo row {}).",
                int(row, "id")
            ));
            continue;
        };
        let (head_repo, base_repo) = (int(row, "head_repo_id"), int(row, "base_repo_id"));
        let mut head = text(row, "head_branch");
        if head_repo != 0 && head_repo != base_repo {
            if let Some(fork) = mapped(&tx, "repo", head_repo)? {
                let (o, n): (String, String) = tx.query_row(
                    "SELECT u.login, r.name FROM repos r JOIN users u ON u.id = r.owner_id
                     WHERE r.id = ?1",
                    [fork],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                head = format!("{o}/{n}:{head}");
            }
        }
        let merge_commit = text(row, "merged_commit_id");
        tx.execute(
            "UPDATE issues SET is_pull = 1, head_branch = ?2, base_branch = ?3, merged = ?4,
                               merge_commit = ?5 WHERE id = ?1",
            params![
                issue,
                head,
                text(row, "base_branch"),
                flag(row, "has_merged"),
                (!merge_commit.is_empty()).then_some(merge_commit),
            ],
        )?;
        report.count("pull_request");
    }

    // ── Comments: what people wrote. Timeline entries (a close, a label
    // added, a commit referenced) are Forgejo's own bookkeeping and stay
    // in the dump.
    let mut events: BTreeMap<i64, usize> = BTreeMap::new();
    for row in &tables["comment"] {
        let id = int(row, "id");
        let kind = int(row, "type");
        let body = text(row, "content");
        // 0 a comment, 21 a comment on a line of code, 22 a review.
        let body = match kind {
            0 => body,
            21 => format!(
                "On `{}` line {}:\n\n{body}",
                text(row, "tree_path"),
                int(row, "line").abs()
            ),
            22 if !body.trim().is_empty() => body,
            _ => {
                *events.entry(kind).or_default() += 1;
                continue;
            }
        };
        let Some(issue) = mapped(&tx, "issue", int(row, "issue_id"))? else {
            report.note(format!("A comment on an issue that was left out (Forgejo comment {id})."));
            continue;
        };
        if mapped(&tx, "comment", id)?.is_none() {
            let (poster_id, poster_name) = poster(&tx, row, "poster_id")?;
            tx.execute(
                "INSERT INTO issue_comments (issue_id, poster_id, poster_name, body, created, updated)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    issue,
                    poster_id,
                    poster_name,
                    body,
                    time(row, "created_unix"),
                    time(row, "updated_unix"),
                ],
            )?;
            remember(&tx, "comment", id, tx.last_insert_rowid())?;
        }
        report.count("comment");
    }
    let event_total: usize = events.values().sum();
    if event_total > 0 {
        report.note(format!(
            "{event_total} timeline entries (state changes, label and assignee changes, \
             references) were not carried over; the issues' current state was. They are in \
             the database dump."
        ));
    }

    for row in &tables["issue_label"] {
        let (Some(issue), Some(label)) = (
            mapped(&tx, "issue", int(row, "issue_id"))?,
            mapped(&tx, "label", int(row, "label_id"))?,
        ) else {
            report.note(format!(
                "A label on an issue, one of which was left out (Forgejo row {}).",
                int(row, "id")
            ));
            continue;
        };
        tx.execute(
            "INSERT OR IGNORE INTO issue_labels (issue_id, label_id) VALUES (?1, ?2)",
            params![issue, label],
        )?;
        report.count("issue_label");
    }
    for row in &tables["issue_assignees"] {
        let (Some(issue), Some(user)) = (
            mapped(&tx, "issue", int(row, "issue_id"))?,
            mapped(&tx, "user", int(row, "assignee_id"))?,
        ) else {
            report.note(format!(
                "An assignee of an issue, one of which was left out (Forgejo row {}).",
                int(row, "id")
            ));
            continue;
        };
        tx.execute(
            "INSERT OR IGNORE INTO issue_assignees (issue_id, user_id) VALUES (?1, ?2)",
            params![issue, user],
        )?;
        report.count("issue_assignees");
    }

    // ── Attachments ──────────────────────────────────────────────────────
    for row in &tables["attachment"] {
        let uuid = text(row, "uuid");
        let name = text(row, "name");
        let valid = uuid.len() >= 3 && uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        if !valid {
            report.note(format!("The attachment \"{name}\" had no usable id."));
            continue;
        }
        let Some(issue) = mapped(&tx, "issue", int(row, "issue_id"))? else {
            if int(row, "release_id") > 0 {
                report.note(format!(
                    "The release file \"{name}\" was left out (LosOS Git has no releases); \
                     it is in the copy of Forgejo's attachments."
                ));
            } else {
                report.note(format!("The attachment \"{name}\" of an issue that was left out."));
            }
            continue;
        };
        let source = from
            .join("attachments")
            .join(&uuid[0..1])
            .join(&uuid[1..2])
            .join(&uuid);
        if !source.is_file() {
            report.note(format!(
                "The attachment \"{name}\" had a record but no file in Forgejo's data."
            ));
            continue;
        }
        std::fs::create_dir_all(&attachments)?;
        let target = attachments.join(&uuid);
        if !target.is_file() {
            std::fs::copy(&source, &target)?;
        }
        let comment = match int(row, "comment_id") {
            0 => None,
            c => mapped(&tx, "comment", c)?,
        };
        tx.execute(
            "INSERT OR IGNORE INTO attachments (uuid, issue_id, comment_id, name, size, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![uuid, issue, comment, name, int(row, "size"), time(row, "created_unix")],
        )?;
        report.count("attachment");
    }

    for (table, what) in [("milestone", "milestones"), ("release", "releases")] {
        let n = tables[table].len();
        if n > 0 {
            report.note(format!(
                "{n} {what} were not carried over (LosOS Git has none); they are in the \
                 database dump."
            ));
        }
    }

    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::db::forgejo_token_hash;
    use serde_json::json;

    fn write(dir: &Path, table: &str, rows: Value) {
        std::fs::create_dir_all(dir.join("tables")).unwrap();
        std::fs::write(
            dir.join("tables").join(format!("{table}.json")),
            rows.to_string(),
        )
        .unwrap();
    }

    fn fixture() -> (assert_fs::TempDir, assert_fs::TempDir) {
        let state = assert_fs::TempDir::new().unwrap();
        let from = assert_fs::TempDir::new().unwrap();
        std::fs::create_dir_all(state.path().join("repositories/matus/notes.git")).unwrap();
        std::fs::create_dir_all(state.path().join("repositories/team/site.git")).unwrap();
        let f = from.path();
        // Postgres's shapes: booleans as booleans.
        write(f, "user", json!([
            {"id": 1, "name": "Matus", "full_name": "Matus K", "email": "m@box.local",
             "is_admin": true, "type": 0, "created_unix": 1_760_000_000},
            {"id": 2, "name": "eva", "full_name": "", "email": "eva@box.local",
             "is_admin": false, "type": 0, "created_unix": 1_760_000_000},
            {"id": 3, "name": "team", "type": 1, "created_unix": 1_760_000_000},
            {"id": 4, "name": "forgejo-actions", "type": 4}
        ]));
        write(f, "org_user", json!([{"id": 1, "uid": 2, "org_id": 3}]));
        write(f, "repository", json!([
            {"id": 10, "owner_id": 1, "owner_name": "Matus", "name": "notes",
             "description": "My notes", "is_private": true, "default_branch": "main",
             "created_unix": 1_760_000_100, "updated_unix": 1_760_000_200, "fork_id": 0},
            {"id": 11, "owner_id": 3, "owner_name": "team", "name": "site",
             "is_private": false, "default_branch": "trunk", "fork_id": 0},
            {"id": 12, "owner_id": 1, "owner_name": "Matus", "name": "gone",
             "is_private": true, "default_branch": "main", "fork_id": 0}
        ]));
        write(f, "collaboration", json!([{"id": 1, "repo_id": 10, "user_id": 2, "mode": 1}]));
        let salt = "s4lt";
        let value = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1234abcd";
        write(f, "access_token", json!([
            {"id": 5, "uid": 1, "name": "laptop", "token_hash": forgejo_token_hash(value, salt),
             "token_salt": salt, "token_last_eight": "1234abcd",
             "scope": "write:repository,read:user,read:package", "created_unix": 1_760_000_300},
            {"id": 6, "uid": 1, "name": "packages", "token_hash": "x", "token_salt": "y",
             "token_last_eight": "00000000", "scope": "write:package"}
        ]));
        write(f, "label", json!([
            {"id": 7, "repo_id": 10, "org_id": 0, "name": "bug", "color": "#ee0701", "description": ""}
        ]));
        // SQLite's shapes: booleans as numbers.
        write(f, "issue", json!([
            {"id": 20, "repo_id": 10, "index": 1, "poster_id": 1, "name": "Fix the backup script",
             "content": "It stops at 3 a.m.", "is_closed": 0, "is_pull": 0,
             "created_unix": 1_760_001_000, "updated_unix": 1_760_001_100, "closed_unix": 0},
            {"id": 21, "repo_id": 10, "index": 2, "poster_id": 99, "original_author": "old-eva",
             "name": "Faster backups", "content": "", "is_closed": 1, "is_pull": 1,
             "created_unix": 1_760_002_000, "updated_unix": 1_760_002_100, "closed_unix": 1_760_002_200}
        ]));
        write(f, "pull_request", json!([
            {"id": 1, "issue_id": 21, "head_repo_id": 10, "base_repo_id": 10,
             "head_branch": "fast", "base_branch": "main", "has_merged": 1,
             "merged_commit_id": "abc123"}
        ]));
        write(f, "comment", json!([
            {"id": 30, "type": 0, "poster_id": 2, "issue_id": 20, "content": "Seen it too.",
             "created_unix": 1_760_001_500, "updated_unix": 1_760_001_500},
            {"id": 31, "type": 2, "poster_id": 1, "issue_id": 21, "content": ""},
            {"id": 32, "type": 21, "poster_id": 1, "issue_id": 21, "content": "Why sleep?",
             "tree_path": "backup.sh", "line": -12}
        ]));
        write(f, "issue_label", json!([{"id": 1, "issue_id": 20, "label_id": 7}]));
        write(f, "issue_assignees", json!([{"id": 1, "issue_id": 20, "assignee_id": 2}]));
        write(f, "attachment", json!([
            {"id": 1, "uuid": "ab12cd34-0000-4000-8000-000000000001", "issue_id": 20,
             "comment_id": 0, "release_id": 0, "name": "log.txt", "size": 5},
            {"id": 2, "uuid": "ff12cd34-0000-4000-8000-000000000002", "issue_id": 0,
             "comment_id": 0, "release_id": 3, "name": "v1.tar.gz", "size": 9}
        ]));
        let a = f.join("attachments/a/b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("ab12cd34-0000-4000-8000-000000000001"), "hello").unwrap();
        write(f, "milestone", json!([{"id": 1}]));
        (state, from)
    }

    #[test]
    fn forgejo_records_land_and_every_gap_is_named() {
        let (state, from) = fixture();
        let store = Store::in_memory().unwrap();
        let r = import(&store, state.path(), from.path()).unwrap();

        let matus = store.user("matus").unwrap().unwrap();
        assert!(matus.is_admin && matus.full_name == "Matus K");
        assert!(store.user("team").unwrap().is_some(), "an organisation is an account");
        assert!(store.user("forgejo-actions").unwrap().is_none());

        let notes = store.repo("matus", "notes").unwrap().unwrap();
        assert!(notes.private && notes.description == "My notes");
        let site = store.repo("team", "site").unwrap().unwrap();
        assert_eq!(site.default_branch, "trunk");
        assert!(store.repo("matus", "gone").unwrap().is_none(), "no Git data, no record");
        let eva = store.user("eva").unwrap().unwrap();
        assert_eq!(store.collaborator(notes.id, eva.id).unwrap(), super::super::db::Permission::Read);
        assert_eq!(store.collaborator(site.id, eva.id).unwrap(), super::super::db::Permission::Write);

        // The old token opens LosOS Git; the package-only one does not exist.
        let (who, t) = store
            .token_owner("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1234abcd")
            .unwrap()
            .unwrap();
        assert_eq!(who.login, "Matus");
        assert_eq!(t.scopes, vec!["read:user", "write:repository"]);
        assert_eq!(store.tokens(matus.id).unwrap().len(), 1);

        let issues = store
            .issues(notes.id, super::super::db::IssueFilter { closed: None, pulls: None })
            .unwrap();
        assert_eq!(issues.len(), 2);
        let one = store.issue(notes.id, 1).unwrap().unwrap();
        assert_eq!(one.title, "Fix the backup script");
        assert_eq!(one.poster, "Matus");
        assert_eq!(one.labels[0].name, "bug");
        assert_eq!(one.assignees, vec!["eva"]);
        let two = store.issue(notes.id, 2).unwrap().unwrap();
        assert!(two.is_pull && two.closed && two.merged);
        assert_eq!(two.poster, "old-eva", "a gone account keeps its name");
        assert_eq!(
            (two.head_branch.as_deref(), two.base_branch.as_deref()),
            (Some("fast"), Some("main"))
        );
        assert_eq!(store.comments(one.id).unwrap()[0].body, "Seen it too.");
        assert!(store.comments(two.id).unwrap()[0]
            .body
            .starts_with("On `backup.sh` line 12:"));
        let att = store.attachments(one.id).unwrap();
        assert_eq!(att[0].name, "log.txt");
        assert_eq!(
            std::fs::read_to_string(state.path().join("attachments").join(&att[0].uuid)).unwrap(),
            "hello"
        );

        let notes_text = r.not_imported.join("\n");
        for expected in [
            "forgejo-actions",
            "\"Matus/gone\" had no Git data",
            "\"packages\" only allowed",
            "1 timeline entries",
            "release file \"v1.tar.gz\"",
            "1 milestones",
            "organisation \"team\"",
        ] {
            assert!(notes_text.contains(expected), "{expected} not in:\n{notes_text}");
        }
        assert_eq!(r.forgejo["issue"], 2);
        assert_eq!(r.imported["issue"], 2);

        // Again: nothing doubles.
        let again = import(&store, state.path(), from.path()).unwrap();
        assert_eq!(again.imported["issue"], 2);
        assert_eq!(
            store
                .issues(notes.id, super::super::db::IssueFilter { closed: None, pulls: None })
                .unwrap()
                .len(),
            2
        );
        assert_eq!(store.comments(one.id).unwrap().len(), 1);
        assert_eq!(store.tokens(matus.id).unwrap().len(), 1);
    }

    #[test]
    fn forgejo_scopes_keep_what_losos_git_has() {
        assert_eq!(scopes("all"), vec!["all"]);
        assert_eq!(
            scopes("read:package,write:issue,read:repository"),
            vec!["read:repository", "write:issue"]
        );
        assert!(scopes("public-only,read:notification").is_empty());
    }
}
