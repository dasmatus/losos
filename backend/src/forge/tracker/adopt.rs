//! Issues a repository had before the tracker, taken in.
//!
//! Every repository issue without a tracker issue (one a Forgejo box
//! brought over, or one from before this version) becomes one, in the
//! repository's project, made for it when it has none. It keeps its
//! number, so `#12` of the repository is `SITE-12`, its labels, its first
//! assignee (the rest follow it), its comments and its dates. Open issues
//! land in Todo, closed ones in Done. The repository's row stays as it
//! was: it holds the number and the Forgejo id. Runs on every start and
//! after an import, and changes nothing the second time.

use super::store::{insert_ticket, Actor, NewTicket};
use super::{key_for, Category};
use crate::forge::db::{Repo, Store};
use rusqlite::{params, OptionalExtension};

/// The project a repository's issues go to, made when it has none: named
/// and keyed after the repository, run by its owner, open to whoever may
/// see the repository.
pub fn project_for_repo(store: &Store, repo: &Repo) -> anyhow::Result<super::store::Project> {
    if let Some(p) = store.project_of_repo(repo.id)? {
        return Ok(p);
    }
    let key = store.free_project_key(&key_for(&repo.name))?;
    let p = store.create_project(&key, &repo.name, &repo.description, "", "private", None)?;
    store.update_project(
        p.id,
        None,
        None,
        None,
        None,
        Some(Some(repo.owner_id)),
        None,
    )?;
    store.link_repo(p.id, repo.id, true)?;
    tracing::info!(project = key.as_str(), repo = %format!("{}/{}", repo.owner, repo.name), "made a project for a repository's issues");
    Ok(store.project_by_id(p.id)?.unwrap_or(p))
}

/// Take in every repository issue that is not a tracker issue yet.
pub fn adopt_repo_issues(store: &Store) -> anyhow::Result<usize> {
    let waiting: Vec<i64> = {
        let conn = store.conn();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT i.repo_id FROM issues i
             WHERE i.is_pull = 0 AND NOT EXISTS (SELECT 1 FROM tickets t WHERE t.issue_id = i.id)",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut adopted = 0;
    for repo_id in waiting {
        let Some(repo) = store.repo_by_id(repo_id)? else {
            continue;
        };
        let project = project_for_repo(store, &repo)?;
        let todo = store
            .first_state(project.id, Category::Unstarted)?
            .or(store.first_state(project.id, Category::Backlog)?)
            .ok_or_else(|| anyhow::anyhow!("project {} has no open state", project.key))?;
        let done = store
            .first_state(project.id, Category::Done)?
            .ok_or_else(|| anyhow::anyhow!("project {} has no done state", project.key))?;
        let mut conn = store.conn();
        let tx = conn.transaction()?;
        struct Row {
            id: i64,
            number: i64,
            poster_id: Option<i64>,
            poster: String,
            title: String,
            body: String,
            closed: bool,
            created: String,
            updated: String,
            closed_at: Option<String>,
        }
        let rows: Vec<Row> = {
            let mut stmt = tx.prepare(
                "SELECT i.id, i.number, i.poster_id, COALESCE(u.login, i.poster_name), i.title, i.body,
                        i.closed, i.created, i.updated, i.closed_at
                 FROM issues i LEFT JOIN users u ON u.id = i.poster_id
                 WHERE i.repo_id = ?1 AND i.is_pull = 0
                   AND NOT EXISTS (SELECT 1 FROM tickets t WHERE t.issue_id = i.id)
                 ORDER BY i.number",
            )?;
            let rows = stmt.query_map([repo_id], |r| {
                Ok(Row {
                    id: r.get(0)?,
                    number: r.get(1)?,
                    poster_id: r.get(2)?,
                    poster: r.get(3)?,
                    title: r.get(4)?,
                    body: r.get(5)?,
                    closed: r.get(6)?,
                    created: r.get(7)?,
                    updated: r.get(8)?,
                    closed_at: r.get(9)?,
                })
            })?;
            rows.collect::<Result<_, _>>()?
        };
        for row in rows {
            let labels: Vec<(String, String, String)> = {
                let mut stmt = tx.prepare(
                    "SELECT l.name, l.color, l.description FROM issue_labels il
                     JOIN labels l ON l.id = il.label_id WHERE il.issue_id = ?1",
                )?;
                let rows = stmt.query_map([row.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                rows.collect::<Result<_, _>>()?
            };
            let mut label_ids = Vec::new();
            for (name, color, description) in labels {
                let color = if color.is_empty() || color.starts_with('#') {
                    color
                } else {
                    format!("#{color}")
                };
                tx.execute(
                    "INSERT INTO project_labels (project_id, name, color, description) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (project_id, name) DO NOTHING",
                    params![project.id, name, color, description],
                )?;
                label_ids.push(tx.query_row(
                    "SELECT id FROM project_labels WHERE project_id = ?1 AND name = ?2",
                    params![project.id, name],
                    |r| r.get::<_, i64>(0),
                )?);
            }
            let assignees: Vec<i64> = {
                let mut stmt = tx.prepare(
                    "SELECT a.user_id FROM issue_assignees a JOIN users u ON u.id = a.user_id
                     WHERE a.issue_id = ?1 ORDER BY u.lower_login",
                )?;
                let rows = stmt.query_map([row.id], |r| r.get(0))?;
                rows.collect::<Result<_, _>>()?
            };
            let taken = tx
                .query_row(
                    "SELECT 1 FROM tickets WHERE project_id = ?1 AND number = ?2",
                    params![project.id, row.number],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            let number = if taken {
                super::store::next_number(&tx, project.id)?
            } else {
                row.number
            };
            let actor = Actor {
                id: row.poster_id,
                name: row.poster.clone(),
            };
            let id = insert_ticket(
                &tx,
                project.id,
                &NewTicket {
                    title: row.title.clone(),
                    body: row.body.clone(),
                    state_id: Some(if row.closed { done.id } else { todo.id }),
                    assignee_id: assignees.first().copied(),
                    labels: label_ids,
                    ..NewTicket::default()
                },
                &actor,
                Some((number, row.id, &row.created)),
            )?;
            for a in assignees.iter().skip(1) {
                tx.execute(
                    "INSERT OR IGNORE INTO ticket_subscribers (ticket_id, user_id) VALUES (?1, ?2)",
                    params![id, a],
                )?;
            }
            tx.execute(
                "INSERT INTO ticket_comments (ticket_id, author_id, author_name, body, created, updated)
                 SELECT ?1, c.poster_id, COALESCE(u.login, c.poster_name), c.body, c.created, c.updated
                 FROM issue_comments c LEFT JOIN users u ON u.id = c.poster_id
                 WHERE c.issue_id = ?2 ORDER BY c.created, c.id",
                params![id, row.id],
            )?;
            tx.execute(
                "UPDATE tickets SET updated = ?2,
                     completed_at = CASE WHEN completed_at IS NOT NULL THEN COALESCE(?3, completed_at) END
                 WHERE id = ?1",
                params![id, row.updated, row.closed_at],
            )?;
            adopted += 1;
        }
        tx.commit()?;
    }
    if adopted > 0 {
        tracing::info!(adopted, "repository issues taken into the tracker");
    }
    Ok(adopted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::db::Visibility;

    #[test]
    fn repository_issues_become_tracker_issues_once() {
        let s = Store::in_memory().unwrap();
        let m = s.ensure_user("matus", "", "").unwrap();
        let e = s.ensure_user("eva", "", "").unwrap();
        let repo = s
            .create_repo(m.id, "losos", "The box", Visibility::Private, "main", None)
            .unwrap();
        {
            let conn = s.conn();
            conn.execute_batch(&format!(
                "INSERT INTO issues (repo_id, number, poster_id, title, body, closed, created, updated, closed_at)
                 VALUES ({r}, 12, {m}, 'Fix the backup script', 'It stops at 3 GiB.', 0,
                         '2026-10-01T10:00:00Z', '2026-10-02T10:00:00Z', NULL),
                        ({r}, 13, {m}, 'Old one', '', 1, '2026-09-01T10:00:00Z', '2026-09-03T10:00:00Z',
                         '2026-09-03T10:00:00Z');
                 INSERT INTO issues (repo_id, number, poster_id, title, is_pull, created, updated)
                 VALUES ({r}, 14, {m}, 'A pull request', 1, '2026-10-01T10:00:00Z', '2026-10-01T10:00:00Z');
                 INSERT INTO labels (repo_id, name, color) VALUES ({r}, 'bug', 'ee0701');
                 INSERT INTO issue_labels (issue_id, label_id)
                   SELECT i.id, l.id FROM issues i, labels l WHERE i.number = 12;
                 INSERT INTO issue_assignees (issue_id, user_id) SELECT id, {e} FROM issues WHERE number = 12;
                 INSERT INTO issue_comments (issue_id, poster_id, body, created, updated)
                   SELECT id, {e}, 'Seen it too.', '2026-10-01T11:00:00Z', '2026-10-01T11:00:00Z'
                   FROM issues WHERE number = 12;",
                r = repo.id,
                m = m.id,
                e = e.id
            ))
            .unwrap();
        }
        assert_eq!(adopt_repo_issues(&s).unwrap(), 2);
        assert_eq!(
            adopt_repo_issues(&s).unwrap(),
            0,
            "the second time changes nothing"
        );
        let p = s.project_of_repo(repo.id).unwrap().unwrap();
        assert_eq!(p.key, "LOS");
        assert_eq!(p.lead.as_deref(), Some("matus"));
        let t = s.ticket("LOS", 12).unwrap().unwrap();
        assert_eq!(t.title, "Fix the backup script");
        assert_eq!(t.state.name, "Todo");
        assert_eq!(t.assignee.as_deref(), Some("eva"));
        assert_eq!(t.labels[0].name, "bug");
        assert_eq!(t.labels[0].color, "#ee0701");
        assert_eq!(t.repo.as_ref().unwrap().number, 12);
        assert_eq!(t.created, "2026-10-01T10:00:00Z");
        assert_eq!(s.ticket_comments(t.id).unwrap()[0].author, "eva");
        let old = s.ticket("LOS", 13).unwrap().unwrap();
        assert_eq!(old.state.category, Category::Done);
        assert_eq!(old.completed_at.as_deref(), Some("2026-09-03T10:00:00Z"));
        // The next issue of the project comes after the repository's.
        let n = s
            .create_ticket(
                p.id,
                &NewTicket {
                    title: "New".into(),
                    ..Default::default()
                },
                &Actor::of(&m),
            )
            .unwrap();
        assert_eq!(n.key, "LOS-14");
        // Pull requests stay pull requests.
        assert!(s.ticket("LOS", 14).unwrap().unwrap().repo.is_none());
        assert!(s.issue(repo.id, 14).unwrap().unwrap().is_pull);
    }
}
