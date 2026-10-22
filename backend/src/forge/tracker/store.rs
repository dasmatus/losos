//! The tracker's records in LosOS Git's database: projects and their
//! workflows, labels, cycles and people; issues with what was said and done
//! on them; saved views and each person's inbox.

use super::{key_of, Category, DEFAULT_STATES};
use crate::forge::auth::{Caller, Who};
use crate::forge::db::{Permission, Store, User};
use crate::forge::now;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub description: String,
    pub color: String,
    pub lead: Option<String>,
    /// `private`: its members and the people of its repositories;
    /// `internal`: everyone signed in to the box may read and write.
    pub visibility: String,
    pub archived: bool,
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowState {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub category: Category,
    pub color: String,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    pub id: i64,
    pub project_id: i64,
    pub number: i64,
    pub name: String,
    /// `YYYY-MM-DD`, both days inside.
    pub starts: String,
    pub ends: String,
}

/// The repository number an issue also has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepoNumber {
    pub owner: String,
    pub name: String,
    pub number: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    pub id: i64,
    pub project_id: i64,
    pub key: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: WorkflowState,
    pub priority: i64,
    pub estimate: Option<i64>,
    pub assignee: Option<String>,
    pub creator: String,
    pub cycle_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub parent_key: Option<String>,
    pub due: Option<String>,
    pub sort: f64,
    pub repo: Option<RepoNumber>,
    #[serde(skip)]
    pub issue_id: Option<i64>,
    pub labels: Vec<Label>,
    pub comments: i64,
    /// Sub-issues: how many, and how many are over.
    pub children: i64,
    pub children_closed: i64,
    pub created: String,
    pub updated: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub canceled_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: i64,
    pub author: String,
    pub body: String,
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: i64,
    pub actor: String,
    pub kind: String,
    pub detail: Value,
    pub created: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub id: i64,
    /// `blocks`, `blocked_by`, `relates`, `duplicates` or `duplicated_by`,
    /// as seen from the issue it is listed on.
    pub kind: String,
    pub other_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub id: i64,
    pub kind: String,
    pub owner: String,
    pub repo: String,
    pub target: String,
    pub title: String,
    pub actor: String,
    pub closes: bool,
    pub created: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub id: i64,
    pub owner: String,
    pub name: String,
    pub query: Value,
    pub layout: String,
    pub group_by: String,
    pub shared: bool,
    pub created: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: i64,
    pub ticket_id: i64,
    pub kind: String,
    pub actor: String,
    pub detail: Value,
    pub created: String,
    pub read: bool,
}

/// A new issue.
#[derive(Debug, Clone, Default)]
pub struct NewTicket {
    pub title: String,
    pub body: String,
    /// `None`: the project's first state of its first category with one,
    /// Todo for a new project.
    pub state_id: Option<i64>,
    pub priority: i64,
    pub estimate: Option<i64>,
    pub assignee_id: Option<i64>,
    pub labels: Vec<i64>,
    pub cycle_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub due: Option<String>,
    /// Also give it the next number of this repository.
    pub repo_id: Option<i64>,
}

/// What a change sets; `None` leaves a field, `Some(None)` clears it.
#[derive(Debug, Clone, Default)]
pub struct Change {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state_id: Option<i64>,
    pub priority: Option<i64>,
    pub estimate: Option<Option<i64>>,
    pub assignee_id: Option<Option<i64>>,
    pub cycle_id: Option<Option<i64>>,
    pub parent_id: Option<Option<i64>>,
    pub due: Option<Option<String>>,
    pub sort: Option<f64>,
    /// The whole set, replacing what it had.
    pub labels: Option<Vec<i64>>,
    pub add_labels: Vec<i64>,
    pub remove_labels: Vec<i64>,
}

impl Change {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.body.is_none()
            && self.state_id.is_none()
            && self.priority.is_none()
            && self.estimate.is_none()
            && self.assignee_id.is_none()
            && self.cycle_id.is_none()
            && self.parent_id.is_none()
            && self.due.is_none()
            && self.sort.is_none()
            && self.labels.is_none()
            && self.add_labels.is_empty()
            && self.remove_labels.is_empty()
    }
}

/// Who did it: an account, or a name (a commit's author, "LosOS Git").
#[derive(Debug, Clone)]
pub struct Actor {
    pub id: Option<i64>,
    pub name: String,
}

impl Actor {
    pub fn of(u: &User) -> Actor {
        Actor {
            id: Some(u.id),
            name: u.login.clone(),
        }
    }
    pub fn named(name: &str) -> Actor {
        Actor {
            id: None,
            name: name.to_string(),
        }
    }
}

const PROJECT_COLS: &str = "p.id, p.key, p.name, p.description, p.color, \
     (SELECT login FROM users WHERE id = p.lead_id), p.visibility, p.archived, p.created, p.updated";

fn project_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        key: r.get(1)?,
        name: r.get(2)?,
        description: r.get(3)?,
        color: r.get(4)?,
        lead: r.get(5)?,
        visibility: r.get(6)?,
        archived: r.get(7)?,
        created: r.get(8)?,
        updated: r.get(9)?,
    })
}

const STATE_COLS: &str = "s.id, s.project_id, s.name, s.category, s.color, s.position";

fn state_row(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<WorkflowState> {
    let cat: String = r.get(at + 3)?;
    Ok(WorkflowState {
        id: r.get(at)?,
        project_id: r.get(at + 1)?,
        name: r.get(at + 2)?,
        category: Category::parse(&cat).unwrap_or(Category::Unstarted),
        color: r.get(at + 4)?,
        position: r.get(at + 5)?,
    })
}

const TICKET_COLS: &str = "t.id, t.project_id, p.key, t.number, t.title, t.body, t.priority, \
     t.estimate, (SELECT login FROM users WHERE id = t.assignee_id), \
     COALESCE((SELECT login FROM users WHERE id = t.creator_id), t.creator_name), t.cycle_id, \
     t.parent_id, (SELECT pp.key || '-' || pt.number FROM tickets pt \
                   JOIN projects pp ON pp.id = pt.project_id WHERE pt.id = t.parent_id), \
     t.due, t.sort, t.issue_id, ru.login, r.name, i.number, \
     (SELECT COUNT(*) FROM ticket_comments c WHERE c.ticket_id = t.id), \
     (SELECT COUNT(*) FROM tickets ch WHERE ch.parent_id = t.id), \
     (SELECT COUNT(*) FROM tickets ch JOIN workflow_states cs ON cs.id = ch.state_id \
      WHERE ch.parent_id = t.id AND cs.category IN ('done', 'canceled')), \
     t.created, t.updated, t.started_at, t.completed_at, t.canceled_at, \
     s.id, s.project_id, s.name, s.category, s.color, s.position";

const TICKET_FROM: &str = "tickets t JOIN projects p ON p.id = t.project_id \
     JOIN workflow_states s ON s.id = t.state_id \
     LEFT JOIN issues i ON i.id = t.issue_id \
     LEFT JOIN repos r ON r.id = i.repo_id \
     LEFT JOIN users ru ON ru.id = r.owner_id";

fn ticket_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Ticket> {
    let key: String = r.get(2)?;
    let number: i64 = r.get(3)?;
    let owner: Option<String> = r.get(16)?;
    let name: Option<String> = r.get(17)?;
    let repo_number: Option<i64> = r.get(18)?;
    Ok(Ticket {
        id: r.get(0)?,
        project_id: r.get(1)?,
        key: key_of(&key, number),
        number,
        title: r.get(4)?,
        body: r.get(5)?,
        priority: r.get(6)?,
        estimate: r.get(7)?,
        assignee: r.get(8)?,
        creator: r.get(9)?,
        cycle_id: r.get(10)?,
        parent_id: r.get(11)?,
        parent_key: r.get(12)?,
        due: r.get(13)?,
        sort: r.get(14)?,
        issue_id: r.get(15)?,
        repo: match (owner, name, repo_number) {
            (Some(owner), Some(name), Some(number)) => Some(RepoNumber {
                owner,
                name,
                number,
            }),
            _ => None,
        },
        comments: r.get(19)?,
        children: r.get(20)?,
        children_closed: r.get(21)?,
        created: r.get(22)?,
        updated: r.get(23)?,
        started_at: r.get(24)?,
        completed_at: r.get(25)?,
        canceled_at: r.get(26)?,
        state: state_row(r, 27)?,
        labels: Vec::new(),
    })
}

fn labels_of(conn: &Connection, ticket_ids: &[i64]) -> anyhow::Result<HashMap<i64, Vec<Label>>> {
    let mut out: HashMap<i64, Vec<Label>> = HashMap::new();
    if ticket_ids.is_empty() {
        return Ok(out);
    }
    let mut stmt = conn.prepare(
        "SELECT tl.ticket_id, l.id, l.name, l.color, l.description FROM ticket_labels tl
         JOIN project_labels l ON l.id = tl.label_id ORDER BY l.name",
    )?;
    let wanted: std::collections::HashSet<i64> = ticket_ids.iter().copied().collect();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            Label {
                id: r.get(1)?,
                name: r.get(2)?,
                color: r.get(3)?,
                description: r.get(4)?,
            },
        ))
    })?;
    for row in rows {
        let (t, l) = row?;
        if wanted.contains(&t) {
            out.entry(t).or_default().push(l);
        }
    }
    Ok(out)
}

fn fill(conn: &Connection, mut list: Vec<Ticket>) -> anyhow::Result<Vec<Ticket>> {
    let ids: Vec<i64> = list.iter().map(|t| t.id).collect();
    let mut labels = labels_of(conn, &ids)?;
    for t in &mut list {
        t.labels = labels.remove(&t.id).unwrap_or_default();
    }
    Ok(list)
}

fn json_of(detail: Value) -> String {
    detail.to_string()
}

/// Write one line of an issue's history.
fn record(
    tx: &Transaction<'_>,
    ticket_id: i64,
    actor: &Actor,
    kind: &str,
    detail: Value,
    at: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO ticket_activity (ticket_id, actor_id, actor_name, kind, detail, created)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![ticket_id, actor.id, actor.name, kind, json_of(detail), at],
    )?;
    Ok(())
}

fn subscribe_tx(tx: &Transaction<'_>, ticket_id: i64, user_id: i64) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO ticket_subscribers (ticket_id, user_id) VALUES (?1, ?2)",
        params![ticket_id, user_id],
    )?;
    Ok(())
}

/// Tell everyone who follows the issue but `actor`, and `also` (someone
/// it was just given to, someone mentioned) whether they follow it or not.
fn notify(
    tx: &Transaction<'_>,
    ticket_id: i64,
    actor: &Actor,
    kind: &str,
    detail: &Value,
    also: &[i64],
    at: &str,
) -> rusqlite::Result<()> {
    let mut people: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT user_id FROM ticket_subscribers WHERE ticket_id = ?1")?;
        let rows = stmt.query_map([ticket_id], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    for a in also {
        if !people.contains(a) {
            people.push(*a);
        }
    }
    for p in people {
        if Some(p) == actor.id {
            continue;
        }
        tx.execute(
            "INSERT INTO inbox (user_id, ticket_id, kind, actor_name, detail, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![p, ticket_id, kind, actor.name, detail.to_string(), at],
        )?;
    }
    Ok(())
}

/// The accounts `text` mentions that exist.
fn mentioned(tx: &Transaction<'_>, text: &str) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut out = Vec::new();
    for login in super::refs::mentions(text) {
        if let Some(id) = tx
            .query_row(
                "SELECT id FROM users WHERE lower_login = ?1 AND is_bot = 0",
                [&login],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
        {
            out.push((id, login));
        }
    }
    Ok(out)
}

/// Keep the repository's own row of the issue in step: its title, text
/// and whether it is closed, which Forgejo's API answers from.
fn sync_issue_row(tx: &Transaction<'_>, ticket_id: i64) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE issues SET
             title = (SELECT title FROM tickets WHERE id = ?1),
             body = (SELECT body FROM tickets WHERE id = ?1),
             closed = (SELECT s.category IN ('done', 'canceled') FROM tickets t
                       JOIN workflow_states s ON s.id = t.state_id WHERE t.id = ?1),
             closed_at = (SELECT COALESCE(t.completed_at, t.canceled_at) FROM tickets t
                          WHERE t.id = ?1),
             updated = (SELECT updated FROM tickets WHERE id = ?1)
         WHERE id = (SELECT issue_id FROM tickets WHERE id = ?1)",
        [ticket_id],
    )?;
    Ok(())
}

fn login_of(tx: &Transaction<'_>, id: Option<i64>) -> rusqlite::Result<Option<String>> {
    match id {
        None => Ok(None),
        Some(id) => tx
            .query_row("SELECT login FROM users WHERE id = ?1", [id], |r| r.get(0))
            .optional(),
    }
}

fn state_of(tx: &Transaction<'_>, id: i64) -> rusqlite::Result<WorkflowState> {
    tx.query_row(
        &format!("SELECT {STATE_COLS} FROM workflow_states s WHERE s.id = ?1"),
        [id],
        |r| state_row(r, 0),
    )
}

impl Store {
    // ── Projects ──────────────────────────────────────────────────────────

    pub fn projects(&self) -> anyhow::Result<Vec<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {PROJECT_COLS} FROM projects p ORDER BY p.archived, p.name COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map([], project_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn project(&self, key: &str) -> anyhow::Result<Option<Project>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects p WHERE p.key = ?1"),
                [key.to_ascii_uppercase()],
                project_row,
            )
            .optional()?)
    }

    pub fn project_by_id(&self, id: i64) -> anyhow::Result<Option<Project>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT {PROJECT_COLS} FROM projects p WHERE p.id = ?1"),
                [id],
                project_row,
            )
            .optional()?)
    }

    /// A key no project has yet: `want`, or `want` with a digit after it.
    pub fn free_project_key(&self, want: &str) -> anyhow::Result<String> {
        let conn = self.conn();
        let taken = |k: &str| -> rusqlite::Result<bool> {
            conn.query_row("SELECT 1 FROM projects WHERE key = ?1", [k], |_| Ok(()))
                .optional()
                .map(|o| o.is_some())
        };
        if !taken(want)? {
            return Ok(want.to_string());
        }
        let stem: String = want.chars().take(5).collect();
        for n in 2..100 {
            let k = format!("{stem}{n}");
            if super::valid_project_key(&k) && !taken(&k)? {
                return Ok(k);
            }
        }
        anyhow::bail!("no free key near {want}")
    }

    /// A new project with the default workflow; `admin` (if any) runs it.
    pub fn create_project(
        &self,
        key: &str,
        name: &str,
        description: &str,
        color: &str,
        visibility: &str,
        admin: Option<i64>,
    ) -> anyhow::Result<Project> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let at = now();
        let inserted = tx.execute(
            "INSERT INTO projects (key, name, description, color, lead_id, visibility, created, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![key, name, description, color, admin, visibility, at],
        );
        match inserted {
            Err(rusqlite::Error::SqliteFailure(f, _))
                if f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
            {
                return Err(
                    crate::forge::db::Exists(format!("a project with the key {key}")).into(),
                );
            }
            other => {
                other?;
            }
        }
        let id = tx.last_insert_rowid();
        for (i, (n, c, col)) in DEFAULT_STATES.iter().enumerate() {
            tx.execute(
                "INSERT INTO workflow_states (project_id, name, category, color, position)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, n, c.as_str(), col, i as i64],
            )?;
        }
        if let Some(a) = admin {
            tx.execute(
                "INSERT INTO project_members (project_id, user_id, role) VALUES (?1, ?2, 'admin')",
                params![id, a],
            )?;
        }
        tx.commit()?;
        drop(conn);
        self.project_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("project {key} vanished"))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_project(
        &self,
        id: i64,
        name: Option<&str>,
        description: Option<&str>,
        color: Option<&str>,
        visibility: Option<&str>,
        lead: Option<Option<i64>>,
        archived: Option<bool>,
    ) -> anyhow::Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE projects SET name = COALESCE(?2, name), description = COALESCE(?3, description),
                 color = COALESCE(?4, color), visibility = COALESCE(?5, visibility),
                 archived = COALESCE(?6, archived), updated = ?7 WHERE id = ?1",
            params![id, name, description, color, visibility, archived, now()],
        )?;
        if let Some(lead) = lead {
            conn.execute(
                "UPDATE projects SET lead_id = ?2 WHERE id = ?1",
                params![id, lead],
            )?;
        }
        Ok(())
    }

    pub fn project_members(&self, project_id: i64) -> anyhow::Result<Vec<(String, Permission)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT u.login, m.role FROM project_members m JOIN users u ON u.id = m.user_id
             WHERE m.project_id = ?1 ORDER BY u.lower_login",
        )?;
        let rows = stmt.query_map([project_id], |r| {
            let role: String = r.get(1)?;
            Ok((
                r.get(0)?,
                Permission::parse(&role).unwrap_or(Permission::Read),
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_project_member(
        &self,
        project_id: i64,
        user_id: i64,
        role: Option<Permission>,
    ) -> anyhow::Result<()> {
        let conn = self.conn();
        match role {
            Some(p) if p > Permission::None => {
                conn.execute(
                    "INSERT INTO project_members (project_id, user_id, role) VALUES (?1, ?2, ?3)
                     ON CONFLICT (project_id, user_id) DO UPDATE SET role = excluded.role",
                    params![project_id, user_id, p.as_str()],
                )?;
            }
            _ => {
                conn.execute(
                    "DELETE FROM project_members WHERE project_id = ?1 AND user_id = ?2",
                    params![project_id, user_id],
                )?;
            }
        }
        Ok(())
    }

    /// The repositories linked to a project, by id.
    pub fn project_repo_ids(&self, project_id: i64) -> anyhow::Result<Vec<i64>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT repo_id FROM project_repos WHERE project_id = ?1 ORDER BY repo_id")?;
        let rows = stmt.query_map([project_id], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The project a repository's issues go to.
    pub fn project_of_repo(&self, repo_id: i64) -> anyhow::Result<Option<Project>> {
        Ok(self
            .conn()
            .query_row(
                &format!(
                    "SELECT {PROJECT_COLS} FROM projects p JOIN project_repos pr ON pr.project_id = p.id
                     WHERE pr.repo_id = ?1"
                ),
                [repo_id],
                project_row,
            )
            .optional()?)
    }

    /// Link a repository to a project (moving it from another one), or
    /// unlink it.
    pub fn link_repo(&self, project_id: i64, repo_id: i64, linked: bool) -> anyhow::Result<()> {
        let conn = self.conn();
        if linked {
            conn.execute(
                "INSERT INTO project_repos (project_id, repo_id) VALUES (?1, ?2)
                 ON CONFLICT (repo_id) DO UPDATE SET project_id = excluded.project_id",
                params![project_id, repo_id],
            )?;
        } else {
            conn.execute(
                "DELETE FROM project_repos WHERE project_id = ?1 AND repo_id = ?2",
                params![project_id, repo_id],
            )?;
        }
        Ok(())
    }

    /// What `caller` may do in a project: the best of its own role, the
    /// project being open to the box, and what it may do in the project's
    /// repositories (their administrators run it with them).
    pub fn project_permission(
        &self,
        caller: &Caller,
        project: &Project,
    ) -> anyhow::Result<Permission> {
        let user = match &caller.who {
            Who::Admin => return Ok(Permission::Admin),
            Who::Nobody => return Ok(Permission::None),
            Who::Person(u) if u.is_admin => return Ok(Permission::Admin),
            Who::Person(u) => u.clone(),
        };
        let mut best = if project.visibility == "internal" {
            Permission::Write
        } else {
            Permission::None
        };
        let role: Option<String> = self
            .conn()
            .query_row(
                "SELECT role FROM project_members WHERE project_id = ?1 AND user_id = ?2",
                params![project.id, user.id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(p) = role.as_deref().and_then(Permission::parse) {
            best = best.max(p);
        }
        for repo_id in self.project_repo_ids(project.id)? {
            if best == Permission::Admin {
                break;
            }
            if let Some(repo) = self.repo_by_id(repo_id)? {
                best = best.max(caller.permission(self, &repo)?);
            }
        }
        // A token is held to its scopes.
        Ok(
            if best >= Permission::Write && !caller.allows("issue", true) {
                Permission::Read.min(best)
            } else if !caller.allows("issue", false) {
                Permission::None
            } else {
                best
            },
        )
    }

    // ── Workflow, labels, cycles ──────────────────────────────────────────

    pub fn workflow(&self, project_id: i64) -> anyhow::Result<Vec<WorkflowState>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {STATE_COLS} FROM workflow_states s WHERE s.project_id = ?1
             ORDER BY CASE s.category WHEN 'backlog' THEN 0 WHEN 'unstarted' THEN 1
                      WHEN 'started' THEN 2 WHEN 'done' THEN 3 ELSE 4 END, s.position"
        ))?;
        let rows = stmt.query_map([project_id], |r| state_row(r, 0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The first state of `category` in a project's workflow.
    pub fn first_state(
        &self,
        project_id: i64,
        category: Category,
    ) -> anyhow::Result<Option<WorkflowState>> {
        Ok(self
            .workflow(project_id)?
            .into_iter()
            .find(|s| s.category == category))
    }

    pub fn add_state(
        &self,
        project_id: i64,
        name: &str,
        category: Category,
        color: &str,
    ) -> anyhow::Result<i64> {
        let conn = self.conn();
        let position: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM workflow_states WHERE project_id = ?1",
            [project_id],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO workflow_states (project_id, name, category, color, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![project_id, name, category.as_str(), color, position],
        )
        .map_err(|e| unique(e, &format!("a state called {name}")))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_state(
        &self,
        id: i64,
        name: Option<&str>,
        color: Option<&str>,
        position: Option<i64>,
    ) -> anyhow::Result<()> {
        self.conn()
            .execute(
                "UPDATE workflow_states SET name = COALESCE(?2, name), color = COALESCE(?3, color),
                     position = COALESCE(?4, position) WHERE id = ?1",
                params![id, name, color, position],
            )
            .map_err(|e| unique(e, "a state of that name"))?;
        Ok(())
    }

    /// Remove a state; its issues move to `into` first.
    pub fn delete_state(&self, id: i64, into: i64) -> anyhow::Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE tickets SET state_id = ?2 WHERE state_id = ?1",
            params![id, into],
        )?;
        tx.execute("DELETE FROM workflow_states WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn project_labels(&self, project_id: i64) -> anyhow::Result<Vec<Label>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, color, description FROM project_labels WHERE project_id = ?1
             ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([project_id], |r| {
            Ok(Label {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                description: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The label of this name in a project, made if missing.
    pub fn ensure_label(&self, project_id: i64, name: &str, color: &str) -> anyhow::Result<i64> {
        let color = if color.is_empty() {
            super::label_color(name)
        } else {
            color
        };
        let conn = self.conn();
        conn.execute(
            "INSERT INTO project_labels (project_id, name, color) VALUES (?1, ?2, ?3)
             ON CONFLICT (project_id, name) DO NOTHING",
            params![project_id, name, color],
        )?;
        Ok(conn.query_row(
            "SELECT id FROM project_labels WHERE project_id = ?1 AND name = ?2",
            params![project_id, name],
            |r| r.get(0),
        )?)
    }

    pub fn update_label(
        &self,
        id: i64,
        name: Option<&str>,
        color: Option<&str>,
        description: Option<&str>,
    ) -> anyhow::Result<()> {
        self.conn()
            .execute(
                "UPDATE project_labels SET name = COALESCE(?2, name), color = COALESCE(?3, color),
                     description = COALESCE(?4, description) WHERE id = ?1",
                params![id, name, color, description],
            )
            .map_err(|e| unique(e, "a label of that name"))?;
        Ok(())
    }

    pub fn delete_label(&self, id: i64) -> anyhow::Result<()> {
        self.conn()
            .execute("DELETE FROM project_labels WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn cycles(&self, project_id: i64) -> anyhow::Result<Vec<Cycle>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, number, name, starts, ends FROM cycles WHERE project_id = ?1
             ORDER BY starts",
        )?;
        let rows = stmt.query_map([project_id], cycle_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn cycle(&self, id: i64) -> anyhow::Result<Option<Cycle>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id, project_id, number, name, starts, ends FROM cycles WHERE id = ?1",
                [id],
                cycle_row,
            )
            .optional()?)
    }

    pub fn add_cycle(
        &self,
        project_id: i64,
        name: &str,
        starts: &str,
        ends: &str,
    ) -> anyhow::Result<Cycle> {
        let id = {
            let conn = self.conn();
            let number: i64 = conn.query_row(
                "SELECT COALESCE(MAX(number), 0) + 1 FROM cycles WHERE project_id = ?1",
                [project_id],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT INTO cycles (project_id, number, name, starts, ends) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![project_id, number, name, starts, ends],
            )?;
            conn.last_insert_rowid()
        };
        self.cycle(id)?
            .ok_or_else(|| anyhow::anyhow!("cycle {id} vanished"))
    }

    pub fn update_cycle(
        &self,
        id: i64,
        name: Option<&str>,
        starts: Option<&str>,
        ends: Option<&str>,
    ) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE cycles SET name = COALESCE(?2, name), starts = COALESCE(?3, starts),
                 ends = COALESCE(?4, ends) WHERE id = ?1",
            params![id, name, starts, ends],
        )?;
        Ok(())
    }

    pub fn delete_cycle(&self, id: i64) -> anyhow::Result<()> {
        self.conn()
            .execute("DELETE FROM cycles WHERE id = ?1", [id])?;
        Ok(())
    }

    // ── Issues ────────────────────────────────────────────────────────────

    /// Every issue of these projects.
    pub fn tickets_of(&self, project_ids: &[i64]) -> anyhow::Result<Vec<Ticket>> {
        if project_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn();
        let list = project_ids
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut stmt = conn.prepare(&format!(
            "SELECT {TICKET_COLS} FROM {TICKET_FROM} WHERE t.project_id IN ({list})
             ORDER BY t.sort, t.id DESC"
        ))?;
        let rows: Vec<Ticket> = stmt.query_map([], ticket_row)?.collect::<Result<_, _>>()?;
        fill(&conn, rows)
    }

    pub fn ticket_by_id(&self, id: i64) -> anyhow::Result<Option<Ticket>> {
        let conn = self.conn();
        let t = conn
            .query_row(
                &format!("SELECT {TICKET_COLS} FROM {TICKET_FROM} WHERE t.id = ?1"),
                [id],
                ticket_row,
            )
            .optional()?;
        Ok(fill(&conn, t.into_iter().collect())?.pop())
    }

    /// `LOS-12`.
    pub fn ticket(&self, project_key: &str, number: i64) -> anyhow::Result<Option<Ticket>> {
        let id: Option<i64> = self
            .conn()
            .query_row(
                "SELECT t.id FROM tickets t JOIN projects p ON p.id = t.project_id
                 WHERE p.key = ?1 AND t.number = ?2",
                params![project_key.to_ascii_uppercase(), number],
                |r| r.get(0),
            )
            .optional()?;
        id.map_or(Ok(None), |id| self.ticket_by_id(id))
    }

    /// The issue that is `#number` of a repository.
    pub fn ticket_of_repo(&self, repo_id: i64, number: i64) -> anyhow::Result<Option<Ticket>> {
        let id: Option<i64> = self
            .conn()
            .query_row(
                "SELECT t.id FROM tickets t JOIN issues i ON i.id = t.issue_id
                 WHERE i.repo_id = ?1 AND i.number = ?2",
                params![repo_id, number],
                |r| r.get(0),
            )
            .optional()?;
        id.map_or(Ok(None), |id| self.ticket_by_id(id))
    }

    pub fn children(&self, parent_id: i64) -> anyhow::Result<Vec<Ticket>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {TICKET_COLS} FROM {TICKET_FROM} WHERE t.parent_id = ?1 ORDER BY t.sort, t.number"
        ))?;
        let rows: Vec<Ticket> = stmt
            .query_map([parent_id], ticket_row)?
            .collect::<Result<_, _>>()?;
        fill(&conn, rows)
    }

    /// A new issue in a project, as `actor`. Its maker and the person it
    /// is given to follow it.
    pub fn create_ticket(
        &self,
        project_id: i64,
        new: &NewTicket,
        actor: &Actor,
    ) -> anyhow::Result<Ticket> {
        let id = {
            let mut conn = self.conn();
            let tx = conn.transaction()?;
            let id = insert_ticket(&tx, project_id, new, actor, None)?;
            tx.commit()?;
            id
        };
        self.ticket_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("issue {id} vanished"))
    }

    /// Change an issue as `actor`, writing each change to its history and
    /// telling the people who follow it. What changed comes back.
    pub fn change_ticket(
        &self,
        id: i64,
        change: &Change,
        actor: &Actor,
    ) -> anyhow::Result<Vec<String>> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let changed = apply_change(&tx, id, change, actor)?;
        tx.commit()?;
        Ok(changed)
    }

    /// Move an issue to another project: it takes that project's next
    /// number, the first state of the same category, its labels by name.
    pub fn move_ticket(&self, id: i64, to_project: i64, actor: &Actor) -> anyhow::Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let (from_project, state_id): (i64, i64) = tx.query_row(
            "SELECT project_id, state_id FROM tickets WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if from_project == to_project {
            return Ok(());
        }
        let old_key: String = tx.query_row(
            "SELECT p.key || '-' || t.number FROM tickets t JOIN projects p ON p.id = t.project_id
             WHERE t.id = ?1",
            [id],
            |r| r.get(0),
        )?;
        let category = state_of(&tx, state_id)?.category;
        let new_state: i64 = tx.query_row(
            "SELECT id FROM workflow_states WHERE project_id = ?1
             ORDER BY category <> ?2, position LIMIT 1",
            params![to_project, category.as_str()],
            |r| r.get(0),
        )?;
        let number = next_number(&tx, to_project)?;
        let names: Vec<(String, String)> = {
            let mut stmt = tx.prepare(
                "SELECT l.name, l.color FROM ticket_labels tl JOIN project_labels l ON l.id = tl.label_id
                 WHERE tl.ticket_id = ?1",
            )?;
            let rows = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        tx.execute("DELETE FROM ticket_labels WHERE ticket_id = ?1", [id])?;
        for (name, color) in names {
            tx.execute(
                "INSERT INTO project_labels (project_id, name, color) VALUES (?1, ?2, ?3)
                 ON CONFLICT (project_id, name) DO NOTHING",
                params![to_project, name, color],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO ticket_labels (ticket_id, label_id)
                 SELECT ?1, id FROM project_labels WHERE project_id = ?2 AND name = ?3",
                params![id, to_project, name],
            )?;
        }
        let at = now();
        tx.execute(
            "UPDATE tickets SET project_id = ?2, number = ?3, state_id = ?4, cycle_id = NULL,
                 updated = ?5 WHERE id = ?1",
            params![id, to_project, number, new_state, at],
        )?;
        let new_key: String = tx.query_row(
            "SELECT key || '-' || ?2 FROM projects WHERE id = ?1",
            params![to_project, number],
            |r| r.get(0),
        )?;
        record(
            &tx,
            id,
            actor,
            "moved",
            json!({ "from": old_key, "to": new_key }),
            &at,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_ticket(&self, id: i64) -> anyhow::Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        // The repository keeps its number taken, as GitHub does.
        tx.execute("DELETE FROM tickets WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn ticket_comments(&self, ticket_id: i64) -> anyhow::Result<Vec<Comment>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT c.id, COALESCE(u.login, c.author_name), c.body, c.created, c.updated
             FROM ticket_comments c LEFT JOIN users u ON u.id = c.author_id
             WHERE c.ticket_id = ?1 ORDER BY c.created, c.id",
        )?;
        let rows = stmt.query_map([ticket_id], |r| {
            Ok(Comment {
                id: r.get(0)?,
                author: r.get(1)?,
                body: r.get(2)?,
                created: r.get(3)?,
                updated: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Who wrote a comment, and on which issue.
    pub fn ticket_comment_owner(
        &self,
        comment_id: i64,
    ) -> anyhow::Result<Option<(i64, Option<i64>)>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT ticket_id, author_id FROM ticket_comments WHERE id = ?1",
                [comment_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// A comment. The writer and everyone it mentions follow the issue
    /// from now on.
    pub fn comment_ticket(
        &self,
        ticket_id: i64,
        body: &str,
        actor: &Actor,
    ) -> anyhow::Result<Comment> {
        let id = {
            let mut conn = self.conn();
            let tx = conn.transaction()?;
            let at = now();
            tx.execute(
                "INSERT INTO ticket_comments (ticket_id, author_id, author_name, body, created, updated)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![ticket_id, actor.id, actor.name, body, at],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "UPDATE tickets SET updated = ?2 WHERE id = ?1",
                params![ticket_id, at],
            )?;
            let excerpt: String = body.chars().take(140).collect();
            let detail = json!({ "comment": id, "excerpt": excerpt });
            let mentions = mentioned(&tx, body)?;
            let new_people: Vec<i64> = mentions.iter().map(|m| m.0).collect();
            // Mentioned people hear of it as a mention, the rest as a comment.
            for (uid, _) in &mentions {
                if Some(*uid) != actor.id {
                    tx.execute(
                        "INSERT INTO inbox (user_id, ticket_id, kind, actor_name, detail, created)
                         VALUES (?1, ?2, 'mention', ?3, ?4, ?5)",
                        params![uid, ticket_id, actor.name, detail.to_string(), at],
                    )?;
                }
            }
            {
                let mut stmt =
                    tx.prepare("SELECT user_id FROM ticket_subscribers WHERE ticket_id = ?1")?;
                let followers: Vec<i64> = stmt
                    .query_map([ticket_id], |r| r.get(0))?
                    .collect::<Result<_, _>>()?;
                drop(stmt);
                for f in followers {
                    if Some(f) == actor.id || new_people.contains(&f) {
                        continue;
                    }
                    tx.execute(
                        "INSERT INTO inbox (user_id, ticket_id, kind, actor_name, detail, created)
                         VALUES (?1, ?2, 'comment', ?3, ?4, ?5)",
                        params![f, ticket_id, actor.name, detail.to_string(), at],
                    )?;
                }
            }
            if let Some(a) = actor.id {
                subscribe_tx(&tx, ticket_id, a)?;
            }
            for p in new_people {
                subscribe_tx(&tx, ticket_id, p)?;
            }
            tx.commit()?;
            id
        };
        self.ticket_comments(ticket_id)?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| anyhow::anyhow!("comment {id} vanished"))
    }

    pub fn edit_ticket_comment(&self, id: i64, body: &str) -> anyhow::Result<()> {
        self.conn().execute(
            "UPDATE ticket_comments SET body = ?2, updated = ?3 WHERE id = ?1",
            params![id, body, now()],
        )?;
        Ok(())
    }

    pub fn delete_ticket_comment(&self, id: i64) -> anyhow::Result<()> {
        self.conn()
            .execute("DELETE FROM ticket_comments WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn ticket_activity(&self, ticket_id: i64) -> anyhow::Result<Vec<Activity>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT a.id, COALESCE(u.login, a.actor_name), a.kind, a.detail, a.created
             FROM ticket_activity a LEFT JOIN users u ON u.id = a.actor_id
             WHERE a.ticket_id = ?1 ORDER BY a.created, a.id",
        )?;
        let rows = stmt.query_map([ticket_id], |r| {
            let detail: String = r.get(3)?;
            Ok(Activity {
                id: r.get(0)?,
                actor: r.get(1)?,
                kind: r.get(2)?,
                detail: serde_json::from_str(&detail).unwrap_or(Value::Null),
                created: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ── Relations, links, followers ───────────────────────────────────────

    pub fn relations(&self, ticket_id: i64) -> anyhow::Result<Vec<Relation>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, kind, to_id, 0 FROM ticket_relations WHERE from_id = ?1
             UNION ALL
             SELECT id, kind, from_id, 1 FROM ticket_relations WHERE to_id = ?1
             ORDER BY 1",
        )?;
        let rows = stmt.query_map([ticket_id], |r| {
            let kind: String = r.get(1)?;
            let inverse: bool = r.get(3)?;
            Ok(Relation {
                id: r.get(0)?,
                kind: match (kind.as_str(), inverse) {
                    ("blocks", true) => "blocked_by".to_string(),
                    ("duplicates", true) => "duplicated_by".to_string(),
                    _ => kind,
                },
                other_id: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// `kind` as seen from `from`: `blocks`, `blocked_by`, `relates`,
    /// `duplicates`. A duplicate is closed as canceled.
    pub fn relate(&self, from: i64, kind: &str, to: i64, actor: &Actor) -> anyhow::Result<()> {
        let (a, b, k) = match kind {
            "blocks" => (from, to, "blocks"),
            "blocked_by" => (to, from, "blocks"),
            "relates" => (from, to, "relates"),
            "duplicates" => (from, to, "duplicates"),
            other => anyhow::bail!("{other} is not a relation"),
        };
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let at = now();
        let added = tx.execute(
            "INSERT OR IGNORE INTO ticket_relations (from_id, to_id, kind, created) VALUES (?1, ?2, ?3, ?4)",
            params![a, b, k, at],
        )?;
        if added > 0 {
            let key_of_id = |id: i64| -> rusqlite::Result<String> {
                tx.query_row(
                    "SELECT p.key || '-' || t.number FROM tickets t JOIN projects p ON p.id = t.project_id
                     WHERE t.id = ?1",
                    [id],
                    |r| r.get(0),
                )
            };
            record(
                &tx,
                from,
                actor,
                "relation",
                json!({ "kind": kind, "other": key_of_id(to)? }),
                &at,
            )?;
            if k == "duplicates" {
                let canceled: Option<i64> = tx
                    .query_row(
                        "SELECT s.id FROM workflow_states s JOIN tickets t ON t.project_id = s.project_id
                         WHERE t.id = ?1 AND s.category = 'canceled' ORDER BY s.position LIMIT 1",
                        [a],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(state) = canceled {
                    apply_change(
                        &tx,
                        a,
                        &Change {
                            state_id: Some(state),
                            ..Change::default()
                        },
                        actor,
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn relation_ends(&self, id: i64) -> anyhow::Result<Option<(i64, i64)>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT from_id, to_id FROM ticket_relations WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn unrelate(&self, id: i64) -> anyhow::Result<()> {
        self.conn()
            .execute("DELETE FROM ticket_relations WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn links(&self, ticket_id: i64) -> anyhow::Result<Vec<Link>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT l.id, l.kind, u.login, r.name, l.target, l.title, l.actor_name, l.closes, l.created
             FROM ticket_links l JOIN repos r ON r.id = l.repo_id JOIN users u ON u.id = r.owner_id
             WHERE l.ticket_id = ?1 ORDER BY l.created, l.id",
        )?;
        let rows = stmt.query_map([ticket_id], |r| {
            Ok(Link {
                id: r.get(0)?,
                kind: r.get(1)?,
                owner: r.get(2)?,
                repo: r.get(3)?,
                target: r.get(4)?,
                title: r.get(5)?,
                actor: r.get(6)?,
                closes: r.get(7)?,
                created: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A commit or pull request names an issue. Once per target; a later
    /// closing mention marks it closing.
    #[allow(clippy::too_many_arguments)]
    pub fn add_link(
        &self,
        ticket_id: i64,
        repo_id: i64,
        kind: &str,
        target: &str,
        title: &str,
        closes: bool,
        actor: &Actor,
    ) -> anyhow::Result<bool> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let at = now();
        let added = tx.execute(
            "INSERT OR IGNORE INTO ticket_links (ticket_id, repo_id, kind, target, title, actor_name, closes, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![ticket_id, repo_id, kind, target, title, actor.name, closes, at],
        )? > 0;
        if added {
            let (owner, name): (String, String) = tx.query_row(
                "SELECT u.login, r.name FROM repos r JOIN users u ON u.id = r.owner_id WHERE r.id = ?1",
                [repo_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let detail = json!({ "kind": kind, "repo": format!("{owner}/{name}"), "target": target, "title": title });
            record(&tx, ticket_id, actor, "link", detail.clone(), &at)?;
            notify(&tx, ticket_id, actor, "link", &detail, &[], &at)?;
        } else if closes {
            tx.execute(
                "UPDATE ticket_links SET closes = 1 WHERE ticket_id = ?1 AND repo_id = ?2 AND kind = ?3 AND target = ?4",
                params![ticket_id, repo_id, kind, target],
            )?;
        }
        tx.commit()?;
        Ok(added)
    }

    /// The issues a pull request said it closes.
    pub fn closed_by_pull(&self, repo_id: i64, number: i64) -> anyhow::Result<Vec<i64>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT ticket_id FROM ticket_links WHERE repo_id = ?1 AND kind = 'pull' AND target = ?2 AND closes = 1",
        )?;
        let rows = stmt.query_map(params![repo_id, number.to_string()], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn subscribers(&self, ticket_id: i64) -> anyhow::Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT u.login FROM ticket_subscribers s JOIN users u ON u.id = s.user_id
             WHERE s.ticket_id = ?1 ORDER BY u.lower_login",
        )?;
        let rows = stmt.query_map([ticket_id], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_subscribed(&self, ticket_id: i64, user_id: i64, on: bool) -> anyhow::Result<()> {
        let conn = self.conn();
        if on {
            conn.execute(
                "INSERT OR IGNORE INTO ticket_subscribers (ticket_id, user_id) VALUES (?1, ?2)",
                params![ticket_id, user_id],
            )?;
        } else {
            conn.execute(
                "DELETE FROM ticket_subscribers WHERE ticket_id = ?1 AND user_id = ?2",
                params![ticket_id, user_id],
            )?;
        }
        Ok(())
    }

    // ── Views and the inbox ───────────────────────────────────────────────

    /// A person's own views and the ones others share.
    pub fn views_for(&self, user_id: i64) -> anyhow::Result<Vec<View>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT v.id, u.login, v.name, v.query, v.layout, v.group_by, v.shared, v.created
             FROM saved_views v JOIN users u ON u.id = v.owner_id
             WHERE v.owner_id = ?1 OR v.shared = 1 ORDER BY v.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([user_id], view_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn view(&self, id: i64) -> anyhow::Result<Option<(View, i64)>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT v.id, u.login, v.name, v.query, v.layout, v.group_by, v.shared, v.created, v.owner_id
                 FROM saved_views v JOIN users u ON u.id = v.owner_id WHERE v.id = ?1",
                [id],
                |r| Ok((view_row(r)?, r.get(8)?)),
            )
            .optional()?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_view(
        &self,
        id: Option<i64>,
        owner_id: i64,
        name: &str,
        query: &Value,
        layout: &str,
        group_by: &str,
        shared: bool,
    ) -> anyhow::Result<i64> {
        let conn = self.conn();
        match id {
            Some(id) => {
                conn.execute(
                    "UPDATE saved_views SET name = ?2, query = ?3, layout = ?4, group_by = ?5, shared = ?6
                     WHERE id = ?1",
                    params![id, name, query.to_string(), layout, group_by, shared],
                )?;
                Ok(id)
            }
            None => {
                conn.execute(
                    "INSERT INTO saved_views (owner_id, name, query, layout, group_by, shared, created)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![owner_id, name, query.to_string(), layout, group_by, shared, now()],
                )?;
                Ok(conn.last_insert_rowid())
            }
        }
    }

    pub fn delete_view(&self, id: i64) -> anyhow::Result<()> {
        self.conn()
            .execute("DELETE FROM saved_views WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn inbox(&self, user_id: i64, limit: i64) -> anyhow::Result<Vec<InboxItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, ticket_id, kind, actor_name, detail, created, read_at IS NOT NULL
             FROM inbox WHERE user_id = ?1 ORDER BY created DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![user_id, limit], |r| {
            let detail: String = r.get(4)?;
            Ok(InboxItem {
                id: r.get(0)?,
                ticket_id: r.get(1)?,
                kind: r.get(2)?,
                actor: r.get(3)?,
                detail: serde_json::from_str(&detail).unwrap_or(Value::Null),
                created: r.get(5)?,
                read: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn unread(&self, user_id: i64) -> anyhow::Result<i64> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM inbox WHERE user_id = ?1 AND read_at IS NULL",
            [user_id],
            |r| r.get(0),
        )?)
    }

    /// Mark some or all of a person's inbox read (or unread again).
    pub fn mark_read(&self, user_id: i64, ids: Option<&[i64]>, read: bool) -> anyhow::Result<()> {
        let conn = self.conn();
        let at = read.then(now);
        match ids {
            None => {
                conn.execute(
                    "UPDATE inbox SET read_at = ?2 WHERE user_id = ?1",
                    params![user_id, at],
                )?;
            }
            Some(ids) => {
                for id in ids {
                    conn.execute(
                        "UPDATE inbox SET read_at = ?3 WHERE user_id = ?1 AND id = ?2",
                        params![user_id, id, at],
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn cycle_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Cycle> {
    Ok(Cycle {
        id: r.get(0)?,
        project_id: r.get(1)?,
        number: r.get(2)?,
        name: r.get(3)?,
        starts: r.get(4)?,
        ends: r.get(5)?,
    })
}

fn view_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<View> {
    let q: String = r.get(3)?;
    Ok(View {
        id: r.get(0)?,
        owner: r.get(1)?,
        name: r.get(2)?,
        query: serde_json::from_str(&q).unwrap_or_else(|_| json!({})),
        layout: r.get(4)?,
        group_by: r.get(5)?,
        shared: r.get(6)?,
        created: r.get(7)?,
    })
}

fn unique(e: rusqlite::Error, what: &str) -> anyhow::Error {
    match &e {
        rusqlite::Error::SqliteFailure(f, _)
            if f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            crate::forge::db::Exists(what.to_string()).into()
        }
        _ => e.into(),
    }
}

/// A project's next issue number, taken.
pub(super) fn next_number(tx: &Transaction<'_>, project_id: i64) -> rusqlite::Result<i64> {
    let n: i64 = tx.query_row(
        "SELECT MAX(next_number, COALESCE((SELECT MAX(number) + 1 FROM tickets WHERE project_id = ?1), 1))
         FROM projects WHERE id = ?1",
        [project_id],
        |r| r.get(0),
    )?;
    tx.execute(
        "UPDATE projects SET next_number = ?2 WHERE id = ?1",
        params![project_id, n + 1],
    )?;
    Ok(n)
}

/// Write a new issue. `number` is given when it is adopted from a
/// repository (it keeps the repository's) and taken from the project
/// otherwise.
pub(super) fn insert_ticket(
    tx: &Transaction<'_>,
    project_id: i64,
    new: &NewTicket,
    actor: &Actor,
    adopted: Option<(i64, i64, &str)>,
) -> anyhow::Result<i64> {
    let state_id = match new.state_id {
        Some(s) => s,
        None => tx.query_row(
            "SELECT id FROM workflow_states WHERE project_id = ?1
             ORDER BY category <> 'unstarted', position LIMIT 1",
            [project_id],
            |r| r.get(0),
        )?,
    };
    let state = state_of(tx, state_id)?;
    if state.project_id != project_id {
        anyhow::bail!("that state is another project's");
    }
    let (number, issue_id, at) = match adopted {
        Some((number, issue_id, at)) => {
            tx.execute(
                "UPDATE projects SET next_number = MAX(next_number, ?2 + 1) WHERE id = ?1",
                params![project_id, number],
            )?;
            (number, Some(issue_id), at.to_string())
        }
        None => {
            let at = now();
            let number = next_number(tx, project_id)?;
            match new.repo_id {
                // One number for both, so LOS-12 is #12 of the repository:
                // the larger of the two that are free, the other skipping.
                Some(repo_id) => {
                    let free: i64 = tx.query_row(
                        "SELECT COALESCE(MAX(number), 0) + 1 FROM issues WHERE repo_id = ?1",
                        [repo_id],
                        |r| r.get(0),
                    )?;
                    let n = number.max(free);
                    tx.execute(
                        "UPDATE projects SET next_number = MAX(next_number, ?2 + 1) WHERE id = ?1",
                        params![project_id, n],
                    )?;
                    tx.execute(
                        "INSERT INTO issues (repo_id, number, poster_id, poster_name, title, body, created, updated)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                        params![repo_id, n, actor.id, actor.name, new.title, new.body, at],
                    )?;
                    (n, Some(tx.last_insert_rowid()), at)
                }
                None => (number, None, at),
            }
        }
    };
    // New issues go to the top of the manual order.
    let sort: f64 = tx.query_row(
        "SELECT COALESCE(MIN(sort), 0) - 1024 FROM tickets WHERE project_id = ?1",
        [project_id],
        |r| r.get(0),
    )?;
    let started = (state.category == Category::Started).then(|| at.clone());
    let completed = (state.category == Category::Done).then(|| at.clone());
    let canceled = (state.category == Category::Canceled).then(|| at.clone());
    tx.execute(
        "INSERT INTO tickets (project_id, number, title, body, state_id, priority, estimate,
             assignee_id, creator_id, creator_name, cycle_id, parent_id, due, sort, issue_id,
             created, updated, started_at, completed_at, canceled_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16, ?17, ?18, ?19)",
        params![
            project_id,
            number,
            new.title,
            new.body,
            state_id,
            new.priority,
            new.estimate,
            new.assignee_id,
            actor.id,
            actor.name,
            new.cycle_id,
            new.parent_id,
            new.due,
            sort,
            issue_id,
            at,
            started,
            completed,
            canceled,
        ],
    )?;
    let id = tx.last_insert_rowid();
    for l in &new.labels {
        tx.execute(
            "INSERT OR IGNORE INTO ticket_labels (ticket_id, label_id) VALUES (?1, ?2)",
            params![id, l],
        )?;
    }
    record(tx, id, actor, "created", json!({}), &at)?;
    if let Some(a) = actor.id {
        subscribe_tx(tx, id, a)?;
    }
    let mentions = mentioned(tx, &new.body)?;
    if adopted.is_none() {
        if let Some(a) = new.assignee_id {
            subscribe_tx(tx, id, a)?;
            if Some(a) != actor.id {
                tx.execute(
                    "INSERT INTO inbox (user_id, ticket_id, kind, actor_name, detail, created)
                     VALUES (?1, ?2, 'assigned', ?3, '{}', ?4)",
                    params![a, id, actor.name, at],
                )?;
            }
        }
        for (uid, _) in &mentions {
            subscribe_tx(tx, id, *uid)?;
            if Some(*uid) != actor.id && Some(*uid) != new.assignee_id {
                tx.execute(
                    "INSERT INTO inbox (user_id, ticket_id, kind, actor_name, detail, created)
                     VALUES (?1, ?2, 'mention', ?3, '{}', ?4)",
                    params![uid, id, actor.name, at],
                )?;
            }
        }
    } else if let Some(a) = new.assignee_id {
        subscribe_tx(tx, id, a)?;
    }
    sync_issue_row(tx, id)?;
    Ok(id)
}

/// One change, inside a transaction the caller commits.
pub(super) fn apply_change(
    tx: &Transaction<'_>,
    id: i64,
    c: &Change,
    actor: &Actor,
) -> anyhow::Result<Vec<String>> {
    struct Old {
        title: String,
        body: String,
        state_id: i64,
        priority: i64,
        estimate: Option<i64>,
        assignee_id: Option<i64>,
        cycle_id: Option<i64>,
        parent_id: Option<i64>,
        due: Option<String>,
        project_id: i64,
    }
    let old = tx.query_row(
        "SELECT title, body, state_id, priority, estimate, assignee_id, cycle_id, parent_id, due, project_id
         FROM tickets WHERE id = ?1",
        [id],
        |r| {
            Ok(Old {
                title: r.get(0)?,
                body: r.get(1)?,
                state_id: r.get(2)?,
                priority: r.get(3)?,
                estimate: r.get(4)?,
                assignee_id: r.get(5)?,
                cycle_id: r.get(6)?,
                parent_id: r.get(7)?,
                due: r.get(8)?,
                project_id: r.get(9)?,
            })
        },
    )?;
    let at = now();
    let mut changed = Vec::new();
    let mut touch = false;

    if let Some(t) = &c.title {
        if *t != old.title {
            tx.execute(
                "UPDATE tickets SET title = ?2 WHERE id = ?1",
                params![id, t],
            )?;
            record(
                tx,
                id,
                actor,
                "title",
                json!({ "from": old.title, "to": t }),
                &at,
            )?;
            changed.push("title".to_string());
        }
    }
    if let Some(b) = &c.body {
        if *b != old.body {
            tx.execute("UPDATE tickets SET body = ?2 WHERE id = ?1", params![id, b])?;
            record(tx, id, actor, "description", json!({}), &at)?;
            changed.push("body".to_string());
            let before = super::refs::mentions(&old.body);
            for (uid, login) in mentioned(tx, b)? {
                if before.contains(&login) {
                    continue;
                }
                subscribe_tx(tx, id, uid)?;
                if Some(uid) != actor.id {
                    tx.execute(
                        "INSERT INTO inbox (user_id, ticket_id, kind, actor_name, detail, created)
                         VALUES (?1, ?2, 'mention', ?3, '{}', ?4)",
                        params![uid, id, actor.name, at],
                    )?;
                }
            }
        }
    }
    if let Some(s) = c.state_id {
        if s != old.state_id {
            let from = state_of(tx, old.state_id)?;
            let to = state_of(tx, s)?;
            if to.project_id != old.project_id {
                anyhow::bail!("that state is another project's");
            }
            tx.execute(
                "UPDATE tickets SET state_id = ?2,
                     started_at = CASE WHEN ?3 = 'started' THEN COALESCE(started_at, ?4)
                                       WHEN ?3 IN ('backlog', 'unstarted') THEN NULL ELSE started_at END,
                     completed_at = CASE WHEN ?3 = 'done' THEN ?4 ELSE NULL END,
                     canceled_at = CASE WHEN ?3 = 'canceled' THEN ?4 ELSE NULL END
                 WHERE id = ?1",
                params![id, s, to.category.as_str(), at],
            )?;
            let detail = json!({ "from": from.name, "to": to.name, "category": to.category });
            record(tx, id, actor, "state", detail.clone(), &at)?;
            notify(tx, id, actor, "state", &detail, &[], &at)?;
            changed.push("state".to_string());
        }
    }
    if let Some(p) = c.priority {
        if !(0..=4).contains(&p) {
            anyhow::bail!("a priority is 0 to 4");
        }
        if p != old.priority {
            tx.execute(
                "UPDATE tickets SET priority = ?2 WHERE id = ?1",
                params![id, p],
            )?;
            record(
                tx,
                id,
                actor,
                "priority",
                json!({ "from": super::PRIORITIES[old.priority as usize], "to": super::PRIORITIES[p as usize] }),
                &at,
            )?;
            changed.push("priority".to_string());
        }
    }
    if let Some(e) = c.estimate {
        if e != old.estimate {
            tx.execute(
                "UPDATE tickets SET estimate = ?2 WHERE id = ?1",
                params![id, e],
            )?;
            record(
                tx,
                id,
                actor,
                "estimate",
                json!({ "from": old.estimate, "to": e }),
                &at,
            )?;
            changed.push("estimate".to_string());
        }
    }
    if let Some(a) = c.assignee_id {
        if a != old.assignee_id {
            tx.execute(
                "UPDATE tickets SET assignee_id = ?2 WHERE id = ?1",
                params![id, a],
            )?;
            let detail = json!({ "from": login_of(tx, old.assignee_id)?, "to": login_of(tx, a)? });
            record(tx, id, actor, "assignee", detail.clone(), &at)?;
            if let Some(a) = a {
                subscribe_tx(tx, id, a)?;
                if Some(a) != actor.id {
                    tx.execute(
                        "INSERT INTO inbox (user_id, ticket_id, kind, actor_name, detail, created)
                         VALUES (?1, ?2, 'assigned', ?3, ?4, ?5)",
                        params![a, id, actor.name, detail.to_string(), at],
                    )?;
                }
            }
            changed.push("assignee".to_string());
        }
    }
    if let Some(cy) = c.cycle_id {
        if cy != old.cycle_id {
            if let Some(cy) = cy {
                let p: i64 =
                    tx.query_row("SELECT project_id FROM cycles WHERE id = ?1", [cy], |r| {
                        r.get(0)
                    })?;
                if p != old.project_id {
                    anyhow::bail!("that cycle is another project's");
                }
            }
            tx.execute(
                "UPDATE tickets SET cycle_id = ?2 WHERE id = ?1",
                params![id, cy],
            )?;
            let name = |cid: Option<i64>| -> rusqlite::Result<Option<String>> {
                cid.map(|cid| {
                    tx.query_row(
                        "SELECT CASE name WHEN '' THEN 'Cycle ' || number ELSE name END FROM cycles WHERE id = ?1",
                        [cid],
                        |r| r.get(0),
                    )
                })
                .transpose()
            };
            record(
                tx,
                id,
                actor,
                "cycle",
                json!({ "from": name(old.cycle_id)?, "to": name(cy)? }),
                &at,
            )?;
            changed.push("cycle".to_string());
        }
    }
    if let Some(p) = c.parent_id {
        if p != old.parent_id {
            if let Some(p) = p {
                // No issue may end up its own ancestor.
                let mut at_id = Some(p);
                while let Some(cur) = at_id {
                    if cur == id {
                        anyhow::bail!("an issue cannot be under itself");
                    }
                    at_id = tx
                        .query_row("SELECT parent_id FROM tickets WHERE id = ?1", [cur], |r| {
                            r.get(0)
                        })
                        .optional()?
                        .flatten();
                }
            }
            tx.execute(
                "UPDATE tickets SET parent_id = ?2 WHERE id = ?1",
                params![id, p],
            )?;
            let key = |tid: Option<i64>| -> rusqlite::Result<Option<String>> {
                tid.map(|tid| {
                    tx.query_row(
                        "SELECT pp.key || '-' || t.number FROM tickets t JOIN projects pp ON pp.id = t.project_id
                         WHERE t.id = ?1",
                        [tid],
                        |r| r.get(0),
                    )
                })
                .transpose()
            };
            record(
                tx,
                id,
                actor,
                "parent",
                json!({ "from": key(old.parent_id)?, "to": key(p)? }),
                &at,
            )?;
            changed.push("parent".to_string());
        }
    }
    if let Some(d) = &c.due {
        if let Some(d) = d {
            if !super::valid_date(d) {
                anyhow::bail!("a due date is YYYY-MM-DD");
            }
        }
        if *d != old.due {
            tx.execute("UPDATE tickets SET due = ?2 WHERE id = ?1", params![id, d])?;
            record(
                tx,
                id,
                actor,
                "due",
                json!({ "from": old.due, "to": d }),
                &at,
            )?;
            changed.push("due".to_string());
        }
    }
    if let Some(s) = c.sort {
        if s.is_finite() {
            tx.execute("UPDATE tickets SET sort = ?2 WHERE id = ?1", params![id, s])?;
            touch = true;
        }
    }
    let current: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT label_id FROM ticket_labels WHERE ticket_id = ?1")?;
        let rows = stmt.query_map([id], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut want: Vec<i64> = c.labels.clone().unwrap_or_else(|| current.clone());
    for l in &c.add_labels {
        if !want.contains(l) {
            want.push(*l);
        }
    }
    want.retain(|l| !c.remove_labels.contains(l));
    let label_name = |lid: i64| -> rusqlite::Result<String> {
        tx.query_row(
            "SELECT name FROM project_labels WHERE id = ?1",
            [lid],
            |r| r.get(0),
        )
    };
    for l in &want {
        if !current.contains(l) {
            let p: i64 = tx.query_row(
                "SELECT project_id FROM project_labels WHERE id = ?1",
                [l],
                |r| r.get(0),
            )?;
            if p != old.project_id {
                anyhow::bail!("that label is another project's");
            }
            tx.execute(
                "INSERT OR IGNORE INTO ticket_labels (ticket_id, label_id) VALUES (?1, ?2)",
                params![id, l],
            )?;
            record(
                tx,
                id,
                actor,
                "label_added",
                json!({ "label": label_name(*l)? }),
                &at,
            )?;
            changed.push("labels".to_string());
        }
    }
    for l in &current {
        if !want.contains(l) {
            record(
                tx,
                id,
                actor,
                "label_removed",
                json!({ "label": label_name(*l)? }),
                &at,
            )?;
            tx.execute(
                "DELETE FROM ticket_labels WHERE ticket_id = ?1 AND label_id = ?2",
                params![id, l],
            )?;
            changed.push("labels".to_string());
        }
    }
    changed.dedup();
    if !changed.is_empty() || touch {
        // A reorder alone is not news; it does not move `updated`.
        if !changed.is_empty() {
            tx.execute(
                "UPDATE tickets SET updated = ?2 WHERE id = ?1",
                params![id, at],
            )?;
        }
        sync_issue_row(tx, id)?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Store, User, User) {
        let s = Store::in_memory().unwrap();
        let m = s.ensure_user("matus", "Matus", "").unwrap();
        let e = s.ensure_user("eva", "Eva", "").unwrap();
        (s, m, e)
    }

    #[test]
    fn a_project_starts_with_a_workflow_and_numbers_its_issues() {
        let (s, m, _) = store();
        let p = s
            .create_project("LOS", "LosOS", "", "", "private", Some(m.id))
            .unwrap();
        let flow = s.workflow(p.id).unwrap();
        assert_eq!(
            flow.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            [
                "Backlog",
                "Todo",
                "In Progress",
                "In Review",
                "Done",
                "Canceled"
            ]
        );
        let a = s
            .create_ticket(
                p.id,
                &NewTicket {
                    title: "One".into(),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        let b = s
            .create_ticket(
                p.id,
                &NewTicket {
                    title: "Two".into(),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        assert_eq!((a.key.as_str(), b.key.as_str()), ("LOS-1", "LOS-2"));
        assert_eq!(a.state.name, "Todo");
        assert!(b.sort < a.sort, "new issues go on top");
        assert!(s
            .create_project("LOS", "Again", "", "", "private", None)
            .unwrap_err()
            .downcast_ref::<crate::forge::db::Exists>()
            .is_some());
    }

    #[test]
    fn changes_are_history_and_reach_the_people_who_follow() {
        let (s, m, e) = store();
        let p = s
            .create_project("LOS", "LosOS", "", "", "private", Some(m.id))
            .unwrap();
        let t = s
            .create_ticket(
                p.id,
                &NewTicket {
                    title: "Backups".into(),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        let done = s.first_state(p.id, Category::Done).unwrap().unwrap();
        let bug = s.ensure_label(p.id, "bug", "#f00").unwrap();
        let changed = s
            .change_ticket(
                t.id,
                &Change {
                    assignee_id: Some(Some(e.id)),
                    priority: Some(1),
                    add_labels: vec![bug],
                    due: Some(Some("2026-11-01".into())),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        assert_eq!(changed, ["priority", "assignee", "due", "labels"]);
        // Eva was given it: she hears of it and follows it.
        let inbox = s.inbox(e.id, 10).unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].kind, "assigned");
        s.change_ticket(
            t.id,
            &Change {
                state_id: Some(done.id),
                ..Default::default()
            },
            &Actor::of(&e),
        )
        .unwrap();
        let t = s.ticket_by_id(t.id).unwrap().unwrap();
        assert_eq!(t.state.category, Category::Done);
        assert!(t.completed_at.is_some());
        assert_eq!(t.labels[0].name, "bug");
        // Matus made it and hears that Eva finished it; Eva hears nothing of her own.
        assert_eq!(s.inbox(m.id, 10).unwrap()[0].kind, "state");
        assert_eq!(s.unread(e.id).unwrap(), 1);
        let kinds: Vec<String> = s
            .ticket_activity(t.id)
            .unwrap()
            .into_iter()
            .map(|a| a.kind)
            .collect();
        assert_eq!(
            kinds,
            [
                "created",
                "priority",
                "assignee",
                "due",
                "label_added",
                "state"
            ]
        );
    }

    #[test]
    fn mentions_follow_and_comments_reach_followers() {
        let (s, m, e) = store();
        let p = s
            .create_project("LOS", "LosOS", "", "", "internal", Some(m.id))
            .unwrap();
        let t = s
            .create_ticket(
                p.id,
                &NewTicket {
                    title: "Disk".into(),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        s.comment_ticket(t.id, "@eva can you look?", &Actor::of(&m))
            .unwrap();
        assert_eq!(s.inbox(e.id, 5).unwrap()[0].kind, "mention");
        assert_eq!(s.subscribers(t.id).unwrap(), ["eva", "matus"]);
        s.comment_ticket(t.id, "On it.", &Actor::of(&e)).unwrap();
        assert_eq!(s.inbox(m.id, 5).unwrap()[0].kind, "comment");
        assert_eq!(s.ticket_by_id(t.id).unwrap().unwrap().comments, 2);
    }

    #[test]
    fn relations_read_from_both_ends_and_a_duplicate_is_canceled() {
        let (s, m, _) = store();
        let p = s
            .create_project("LOS", "LosOS", "", "", "private", Some(m.id))
            .unwrap();
        let mk = |title: &str| {
            s.create_ticket(
                p.id,
                &NewTicket {
                    title: title.into(),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap()
        };
        let (a, b, c) = (mk("a"), mk("b"), mk("c"));
        s.relate(a.id, "blocks", b.id, &Actor::of(&m)).unwrap();
        assert_eq!(s.relations(b.id).unwrap()[0].kind, "blocked_by");
        s.relate(c.id, "duplicates", a.id, &Actor::of(&m)).unwrap();
        assert_eq!(
            s.ticket_by_id(c.id).unwrap().unwrap().state.category,
            Category::Canceled
        );
        assert_eq!(s.relations(a.id).unwrap()[1].kind, "duplicated_by");
    }

    #[test]
    fn no_issue_is_its_own_ancestor() {
        let (s, m, _) = store();
        let p = s
            .create_project("LOS", "LosOS", "", "", "private", Some(m.id))
            .unwrap();
        let a = s
            .create_ticket(
                p.id,
                &NewTicket {
                    title: "a".into(),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        let b = s
            .create_ticket(
                p.id,
                &NewTicket {
                    title: "b".into(),
                    parent_id: Some(a.id),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        assert_eq!(s.ticket_by_id(a.id).unwrap().unwrap().children, 1);
        assert!(s
            .change_ticket(
                a.id,
                &Change {
                    parent_id: Some(Some(b.id)),
                    ..Default::default()
                },
                &Actor::of(&m)
            )
            .is_err());
    }

    #[test]
    fn moving_takes_the_new_projects_number_and_state() {
        let (s, m, _) = store();
        let a = s
            .create_project("LOS", "LosOS", "", "", "private", Some(m.id))
            .unwrap();
        let b = s
            .create_project("WEB", "Web", "", "", "private", Some(m.id))
            .unwrap();
        let t = s
            .create_ticket(
                a.id,
                &NewTicket {
                    title: "x".into(),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        let l = s.ensure_label(a.id, "bug", "").unwrap();
        s.change_ticket(
            t.id,
            &Change {
                add_labels: vec![l],
                ..Default::default()
            },
            &Actor::of(&m),
        )
        .unwrap();
        s.move_ticket(t.id, b.id, &Actor::of(&m)).unwrap();
        let t = s.ticket_by_id(t.id).unwrap().unwrap();
        assert_eq!(t.key, "WEB-1");
        assert_eq!(t.state.name, "Todo");
        assert_eq!(t.labels[0].name, "bug");
        assert_ne!(t.labels[0].id, l);
    }

    #[test]
    fn a_repository_issue_takes_the_repositorys_next_number() {
        let (s, m, _) = store();
        let repo = s
            .create_repo(
                m.id,
                "site",
                "",
                crate::forge::db::Visibility::Private,
                "main",
                None,
            )
            .unwrap();
        s.create_issue(repo.id, m.id, "A pull request", "", Some(("fix", "main")))
            .unwrap();
        let p = s
            .create_project("SITE", "site", "", "", "private", Some(m.id))
            .unwrap();
        s.link_repo(p.id, repo.id, true).unwrap();
        let t = s
            .create_ticket(
                p.id,
                &NewTicket {
                    title: "Broken".into(),
                    repo_id: Some(repo.id),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        assert_eq!(t.repo.as_ref().unwrap().number, 2);
        assert_eq!(s.ticket_of_repo(repo.id, 2).unwrap().unwrap().id, t.id);
        // The repository's own row says what the issue says.
        let done = s.first_state(p.id, Category::Done).unwrap().unwrap();
        s.change_ticket(
            t.id,
            &Change {
                state_id: Some(done.id),
                title: Some("Fixed".into()),
                ..Default::default()
            },
            &Actor::of(&m),
        )
        .unwrap();
        let row = s.issue(repo.id, 2).unwrap().unwrap();
        assert!(row.closed);
        assert_eq!(row.title, "Fixed");
        // The next pull request comes after it.
        let pr = s
            .create_issue(repo.id, m.id, "Another", "", Some(("b", "main")))
            .unwrap();
        assert_eq!(pr.number, 3);
    }
}
