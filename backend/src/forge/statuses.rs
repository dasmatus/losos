//! Commit statuses: what a check said about a commit, the shape GitHub and
//! Gitea use. Mesh CI writes one per job (`CI / test (push)`), the AI
//! reviewer one per review, and anything with a token that may push can
//! write its own through the API. The pull request page shows the head
//! commit's statuses as its Checks box.
//!
//! One row per repository, commit and context: a new state for the same
//! context replaces the old one.

use super::db::Store;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// The table, the third entry of `db::MIGRATIONS`.
pub const MIGRATION: &str = "
    CREATE TABLE commit_statuses (
        id          INTEGER PRIMARY KEY,
        repo_id     INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
        sha         TEXT NOT NULL,
        context     TEXT NOT NULL,
        state       TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        target_url  TEXT NOT NULL DEFAULT '',
        creator     TEXT NOT NULL DEFAULT '',
        created     TEXT NOT NULL,
        updated     TEXT NOT NULL,
        UNIQUE (repo_id, sha, context)
    );
    CREATE INDEX commit_statuses_sha ON commit_statuses (repo_id, sha);
";

/// The states a status may be in. `running` and `cancelled` are LosOS's
/// own; `warning` is Gitea's.
pub const STATES: &[&str] = &[
    "pending",
    "running",
    "success",
    "failure",
    "error",
    "cancelled",
    "warning",
];

/// A status as it is written.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewStatus {
    pub state: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, alias = "target_url")]
    pub target_url: String,
}

impl NewStatus {
    /// Why this status is refused, or `None`.
    pub fn fault(&self) -> Option<&'static str> {
        if !STATES.contains(&self.state.as_str()) {
            Some("The state is pending, running, success, failure, error, cancelled or warning.")
        } else if self.context.len() > 255 || self.context.contains('\n') {
            Some("The context is one line of at most 255 characters.")
        } else if self.description.len() > 1000 {
            Some("The description is at most 1000 characters.")
        } else if self.target_url.len() > 2000
            || !(self.target_url.is_empty()
                || self.target_url.starts_with('/')
                || self.target_url.starts_with("https://")
                || self.target_url.starts_with("http://"))
        {
            Some("The target URL is a path on this box or an http(s) address.")
        } else {
            None
        }
    }
}

/// A status as it is read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub context: String,
    pub state: String,
    pub description: String,
    pub target_url: String,
    pub creator: String,
    pub created: String,
    pub updated: String,
}

/// Whether `sha` is a full commit id.
pub fn is_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit())
}

/// Write `status` for `sha` in repository `repo_id`.
pub fn set(
    store: &Store,
    repo_id: i64,
    sha: &str,
    status: &NewStatus,
    creator: &str,
) -> anyhow::Result<()> {
    let now = super::now();
    let context = if status.context.trim().is_empty() {
        "default"
    } else {
        status.context.trim()
    };
    store.conn().execute(
        "INSERT INTO commit_statuses
           (repo_id, sha, context, state, description, target_url, creator, created, updated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT (repo_id, sha, context) DO UPDATE SET
           state = excluded.state, description = excluded.description,
           target_url = excluded.target_url, creator = excluded.creator,
           updated = excluded.updated",
        params![
            repo_id,
            sha.to_ascii_lowercase(),
            context,
            status.state,
            status.description,
            status.target_url,
            creator,
            now
        ],
    )?;
    Ok(())
}

/// Every status of `sha`, oldest context first.
pub fn list(store: &Store, repo_id: i64, sha: &str) -> anyhow::Result<Vec<Status>> {
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT context, state, description, target_url, creator, created, updated
         FROM commit_statuses WHERE repo_id = ?1 AND sha = ?2 ORDER BY created, context",
    )?;
    let rows = stmt.query_map(params![repo_id, sha.to_ascii_lowercase()], |r| {
        Ok(Status {
            context: r.get(0)?,
            state: r.get(1)?,
            description: r.get(2)?,
            target_url: r.get(3)?,
            creator: r.get(4)?,
            created: r.get(5)?,
            updated: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// One state for many: failure if any failed, pending while any runs,
/// success when all passed, `none` with nothing to say.
pub fn combined(statuses: &[Status]) -> &'static str {
    if statuses.is_empty() {
        "none"
    } else if statuses
        .iter()
        .any(|s| matches!(s.state.as_str(), "failure" | "error" | "cancelled"))
    {
        "failure"
    } else if statuses
        .iter()
        .any(|s| matches!(s.state.as_str(), "pending" | "running"))
    {
        "pending"
    } else {
        "success"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(state: &str) -> Status {
        Status {
            context: "c".into(),
            state: state.into(),
            description: String::new(),
            target_url: String::new(),
            creator: String::new(),
            created: String::new(),
            updated: String::new(),
        }
    }

    #[test]
    fn a_new_state_replaces_the_old_one() {
        let store = Store::in_memory().unwrap();
        let user = store.ensure_user("matus", "Matus", "").unwrap();
        let repo = store
            .create_repo(user.id, "site", "", true, "main", None)
            .unwrap();
        let sha = "A".repeat(40);
        let mut s = NewStatus {
            state: "pending".into(),
            context: "CI / build (push)".into(),
            description: "Waiting for a box".into(),
            target_url: "/ci/runs/1".into(),
        };
        assert!(s.fault().is_none());
        set(&store, repo.id, &sha, &s, "losos-ci").unwrap();
        s.state = "success".into();
        set(&store, repo.id, &sha, &s, "losos-ci").unwrap();
        let all = list(&store, repo.id, &sha.to_lowercase()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].state, "success");
        assert_eq!(combined(&all), "success");
    }

    #[test]
    fn combined_states() {
        assert_eq!(combined(&[]), "none");
        assert_eq!(combined(&[st("success"), st("running")]), "pending");
        assert_eq!(combined(&[st("pending"), st("failure")]), "failure");
        assert_eq!(combined(&[st("success"), st("warning")]), "success");
    }

    #[test]
    fn odd_statuses_are_refused() {
        let s = |state: &str, url: &str| NewStatus {
            state: state.into(),
            context: "x".into(),
            description: String::new(),
            target_url: url.into(),
        };
        assert!(s("done", "").fault().is_some());
        assert!(s("success", "javascript:alert(1)").fault().is_some());
        assert!(s("success", "https://example.org/x").fault().is_none());
    }
}
