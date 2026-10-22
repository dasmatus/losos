//! LosOS Cloud's boards, taken into the tracker once
//! (`losos-git import-boards`, run by `losos-git-boards-import.service`
//! from a copy of the cloud's database).
//!
//! A board becomes a project, its columns the project's workflow, its cards
//! issues, with their labels, people, due dates, comments and order. A
//! board of a repository fills that repository's project instead, and a
//! card that stood for one of its issues moves that issue to the card's
//! column rather than making a second one. People keep what they could do
//! on the board: managers run the project, editors write, the rest read.
//! Every row taken remembers where it came from (`board_ids`), so a second
//! run changes nothing; nothing in the cloud's database is changed.

use super::store::{insert_ticket, Actor, NewTicket, Project};
use super::{key_for, Category};
use crate::forge::db::Store;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Transaction};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub boards: usize,
    pub projects: Vec<String>,
    pub cards: usize,
    /// Cards that were a repository's issue, which moved instead.
    pub linked: usize,
    pub comments: usize,
    pub skipped: Vec<String>,
}

fn rfc3339(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn date(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default()
        .format("%Y-%m-%d")
        .to_string()
}

/// Where a column's cards are in the flow of work, by what it is called and
/// whether the board said it closes issues.
pub fn category_of(title: &str, closed: bool, first: bool) -> Category {
    let t = title.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| t.contains(w));
    if closed
        || has(&[
            "done", "closed", "finished", "complete", "hotovo", "erledigt", "fertig",
        ])
    {
        Category::Done
    } else if has(&["cancel", "won't", "wontfix", "zrušen", "abgebrochen"]) {
        Category::Canceled
    } else if has(&["backlog", "someday", "ideas", "nápady", "ideen"]) {
        Category::Backlog
    } else if has(&[
        "doing",
        "progress",
        "review",
        "testing",
        "rozpracované",
        "in arbeit",
    ]) {
        Category::Started
    } else if first || has(&["to do", "todo", "to-do", "next", "open", "urobiť", "zu tun"]) {
        Category::Unstarted
    } else {
        Category::Started
    }
}

fn mapped(tx: &Transaction<'_>, kind: &str, cloud_id: i64) -> rusqlite::Result<Option<i64>> {
    tx.query_row(
        "SELECT local_id FROM board_ids WHERE kind = ?1 AND cloud_id = ?2",
        params![kind, cloud_id],
        |r| r.get(0),
    )
    .optional()
}

fn remember(tx: &Transaction<'_>, kind: &str, cloud_id: i64, local: i64) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO board_ids (kind, cloud_id, local_id) VALUES (?1, ?2, ?3)",
        params![kind, cloud_id, local],
    )?;
    Ok(())
}

fn user_id(store: &Store, login: &str) -> anyhow::Result<Option<i64>> {
    if crate::forge::names::owner(login).is_err() {
        return Ok(None);
    }
    Ok(Some(store.ensure_user(login, "", "")?.id))
}

struct Board {
    id: i64,
    title: String,
    color: Option<String>,
    owner: String,
    repo: Option<String>,
    archived: bool,
}

struct Column {
    id: i64,
    title: String,
    state: Option<String>,
}

struct Card {
    id: i64,
    column_id: i64,
    position: i64,
    title: String,
    description: String,
    due: Option<i64>,
    link: Option<(String, i64)>,
    archived: bool,
    created: i64,
    updated: i64,
    done_at: Option<i64>,
}

/// Take in the boards of the cloud database at `cloud_db`.
pub fn import(store: &Store, cloud_db: &Path) -> anyhow::Result<Report> {
    let cloud = Connection::open_with_flags(cloud_db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut report = Report::default();
    let has_boards: bool = cloud
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'board'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !has_boards {
        return Ok(report);
    }
    let boards: Vec<Board> = {
        let mut stmt = cloud
            .prepare("SELECT id, title, color, owner, repo, archived FROM board ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok(Board {
                id: r.get(0)?,
                title: r.get(1)?,
                color: r.get(2)?,
                owner: r.get(3)?,
                repo: r.get(4)?,
                archived: r.get(5)?,
            })
        })?;
        rows.collect::<Result<_, _>>()?
    };
    for b in boards {
        report.boards += 1;
        let project = board_project(store, &cloud, &b, &mut report)?;
        let Some(project) = project else {
            continue;
        };
        if !report.projects.contains(&project.key) {
            report.projects.push(project.key.clone());
        }
        cards(store, &cloud, &b, &project, &mut report)?;
    }
    Ok(report)
}

/// The project a board goes to, made with the board's columns as its
/// workflow when the board is not a repository's.
fn board_project(
    store: &Store,
    cloud: &Connection,
    b: &Board,
    report: &mut Report,
) -> anyhow::Result<Option<Project>> {
    let columns: Vec<Column> = {
        let mut stmt = cloud.prepare(
            "SELECT id, title, state FROM board_column WHERE board_id = ?1 ORDER BY position, id",
        )?;
        let rows = stmt.query_map([b.id], |r| {
            Ok(Column {
                id: r.get(0)?,
                title: r.get(1)?,
                state: r.get(2)?,
            })
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let known: Option<i64> = {
        let mut conn = store.conn();
        let tx = conn.transaction()?;
        mapped(&tx, "board", b.id)?
    };
    let repo = match b.repo.as_deref().and_then(|r| r.split_once('/')) {
        Some((o, n)) => store.repo(o, n)?,
        None => None,
    };
    let project = match (known, &repo) {
        (Some(id), _) => store.project_by_id(id)?,
        (None, Some(repo)) => Some(super::adopt::project_for_repo(store, repo)?),
        (None, None) => {
            let owner = user_id(store, &b.owner)?;
            let key = store.free_project_key(&key_for(&b.title))?;
            let color = b
                .color
                .as_deref()
                .map(|c| {
                    if c.starts_with('#') || c.is_empty() {
                        c.to_string()
                    } else {
                        format!("#{c}")
                    }
                })
                .unwrap_or_default();
            let p = store.create_project(&key, &b.title, "", &color, "private", owner)?;
            if b.archived {
                store.update_project(p.id, None, None, None, None, None, Some(true))?;
            }
            // The board's columns are the workflow.
            let mut conn = store.conn();
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM workflow_states WHERE project_id = ?1", [p.id])?;
            let mut have: Vec<Category> = Vec::new();
            let mut names: Vec<String> = Vec::new();
            for (i, c) in columns.iter().enumerate() {
                let cat = category_of(&c.title, c.state.as_deref() == Some("closed"), i == 0);
                let mut name = c.title.trim().to_string();
                if name.is_empty() || names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                    name = format!(
                        "{} {}",
                        if name.is_empty() { "Column" } else { &name },
                        i + 1
                    );
                }
                names.push(name.clone());
                tx.execute(
                    "INSERT INTO workflow_states (project_id, name, category, color, position)
                     VALUES (?1, ?2, ?3, '', ?4)",
                    params![p.id, name, cat.as_str(), i as i64],
                )?;
                remember(&tx, "column", c.id, tx.last_insert_rowid())?;
                have.push(cat);
            }
            // Every workflow can finish and drop an issue.
            for (name, cat, color) in super::DEFAULT_STATES {
                let wanted = matches!(
                    cat,
                    Category::Unstarted | Category::Done | Category::Canceled
                );
                if wanted
                    && !have.contains(cat)
                    && !names.iter().any(|n| n.eq_ignore_ascii_case(name))
                {
                    tx.execute(
                        "INSERT INTO workflow_states (project_id, name, category, color, position)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![p.id, name, cat.as_str(), color, 100 + have.len() as i64],
                    )?;
                    have.push(*cat);
                }
            }
            remember(&tx, "board", b.id, p.id)?;
            tx.commit()?;
            drop(conn);
            store.project_by_id(p.id)?
        }
    };
    let Some(project) = project else {
        report
            .skipped
            .push(format!("board {} ({}): its project is gone", b.id, b.title));
        return Ok(None);
    };
    // A repository's board: its columns find the project's states by
    // name, else by what they are.
    {
        let flow = store.workflow(project.id)?;
        let mut conn = store.conn();
        let tx = conn.transaction()?;
        for (i, c) in columns.iter().enumerate() {
            if mapped(&tx, "column", c.id)?.is_some() {
                continue;
            }
            let cat = category_of(&c.title, c.state.as_deref() == Some("closed"), i == 0);
            let state = flow
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(c.title.trim()))
                .or_else(|| flow.iter().find(|s| s.category == cat))
                .or_else(|| flow.first());
            if let Some(s) = state {
                remember(&tx, "column", c.id, s.id)?;
            }
        }
        remember(&tx, "board", b.id, project.id)?;
        tx.commit()?;
    }
    // People keep what they could do on the board.
    let owner = user_id(store, &b.owner)?;
    if let Some(o) = owner {
        store.set_project_member(project.id, o, Some(crate::forge::db::Permission::Admin))?;
    }
    let members: Vec<(String, bool, bool)> = {
        let mut stmt = cloud
            .prepare("SELECT login, can_edit, can_manage FROM board_member WHERE board_id = ?1")?;
        let rows = stmt.query_map([b.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    for (login, edit, manage) in members {
        if let Some(u) = user_id(store, &login)? {
            if Some(u) == owner {
                continue;
            }
            use crate::forge::db::Permission;
            let role = if manage {
                Permission::Admin
            } else if edit {
                Permission::Write
            } else {
                Permission::Read
            };
            let had = store
                .project_members(project.id)?
                .into_iter()
                .find(|(l, _)| l.eq_ignore_ascii_case(&login))
                .map(|(_, p)| p)
                .unwrap_or(Permission::None);
            store.set_project_member(project.id, u, Some(role.max(had)))?;
        }
    }
    Ok(Some(project))
}

fn cards(
    store: &Store,
    cloud: &Connection,
    b: &Board,
    project: &Project,
    report: &mut Report,
) -> anyhow::Result<()> {
    let labels: HashMap<i64, (String, String)> = {
        let mut stmt =
            cloud.prepare("SELECT id, name, color FROM board_label WHERE board_id = ?1")?;
        let rows = stmt.query_map([b.id], |r| {
            let color: Option<String> = r.get(2)?;
            Ok((
                r.get::<_, i64>(0)?,
                (r.get::<_, String>(1)?, color.unwrap_or_default()),
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let list: Vec<Card> = {
        let mut stmt = cloud.prepare(
            "SELECT c.id, c.column_id, c.position, c.title, c.description, c.due, c.link_repo,
                    c.link_number, c.archived, c.created, c.updated, c.done_at
             FROM card c JOIN board_column col ON col.id = c.column_id
             WHERE c.board_id = ?1 ORDER BY col.position, c.position, c.id",
        )?;
        let rows = stmt.query_map([b.id], |r| {
            let repo: Option<String> = r.get(6)?;
            let number: Option<i64> = r.get(7)?;
            Ok(Card {
                id: r.get(0)?,
                column_id: r.get(1)?,
                position: r.get(2)?,
                title: r.get(3)?,
                description: r.get(4)?,
                due: r.get(5)?,
                link: repo.zip(number),
                archived: r.get(8)?,
                created: r.get(9)?,
                updated: r.get(10)?,
                done_at: r.get(11)?,
            })
        })?;
        rows.collect::<Result<_, _>>()?
    };
    for card in list {
        let card_labels: Vec<i64> = {
            let mut stmt = cloud.prepare("SELECT label_id FROM card_label WHERE card_id = ?1")?;
            let rows = stmt.query_map([card.id], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let assignees: Vec<String> = {
            let mut stmt = cloud
                .prepare("SELECT login FROM card_assignee WHERE card_id = ?1 ORDER BY login")?;
            let rows = stmt.query_map([card.id], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let comments: Vec<(String, String, i64)> = {
            let mut stmt = cloud.prepare(
                "SELECT author, body, created FROM card_comment WHERE card_id = ?1 ORDER BY created, id",
            )?;
            let rows = stmt.query_map([card.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut people: Vec<i64> = Vec::new();
        for a in &assignees {
            if let Some(id) = user_id(store, a)? {
                people.push(id);
            }
        }
        let mut label_ids = Vec::new();
        for l in &card_labels {
            if let Some((name, color)) = labels.get(l) {
                let color = if color.is_empty() || color.starts_with('#') {
                    color.clone()
                } else {
                    format!("#{color}")
                };
                label_ids.push(store.ensure_label(project.id, name, &color)?);
            }
        }
        let owner_actor = Actor {
            id: user_id(store, &b.owner)?,
            name: b.owner.clone(),
        };
        let linked = match &card.link {
            Some((repo, number)) => match repo.split_once('/') {
                Some((o, n)) => match store.repo(o, n)? {
                    Some(r) => store.ticket_of_repo(r.id, *number)?,
                    None => None,
                },
                None => None,
            },
            None => None,
        };
        let mut conn = store.conn();
        let tx = conn.transaction()?;
        if mapped(&tx, "card", card.id)?.is_some() {
            continue;
        }
        let Some(mut state_id) = mapped(&tx, "column", card.column_id)? else {
            report.skipped.push(format!(
                "card {} ({}): its column has no state",
                card.id, card.title
            ));
            continue;
        };
        // A card put away in a column that was not done was dropped.
        let category: String = tx.query_row(
            "SELECT category FROM workflow_states WHERE id = ?1",
            [state_id],
            |r| r.get(0),
        )?;
        if card.archived && category != "done" {
            if let Some(c) = tx
                .query_row(
                    "SELECT id FROM workflow_states WHERE project_id = ?1 AND category = 'canceled'
                     ORDER BY position LIMIT 1",
                    [project.id],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?
            {
                state_id = c;
            }
        }
        let sort = card.position as f64 * 1024.0;
        if let Some(t) = linked {
            // The issue moves where the card was.
            super::store::apply_change(
                &tx,
                t.id,
                &super::store::Change {
                    state_id: (t.project_id == project.id).then_some(state_id),
                    sort: Some(sort),
                    due: card.due.map(|d| Some(date(d))).filter(|_| t.due.is_none()),
                    ..Default::default()
                },
                &owner_actor,
            )?;
            remember(&tx, "card", card.id, t.id)?;
            tx.commit()?;
            report.linked += 1;
            continue;
        }
        let id = insert_ticket(
            &tx,
            project.id,
            &NewTicket {
                title: card.title.clone(),
                body: card.description.clone(),
                state_id: Some(state_id),
                assignee_id: people.first().copied(),
                labels: label_ids,
                due: card.due.map(date),
                ..Default::default()
            },
            &owner_actor,
            None,
        )?;
        let created = rfc3339(card.created);
        tx.execute(
            "UPDATE tickets SET sort = ?2, created = ?3, updated = ?4,
                 completed_at = CASE WHEN completed_at IS NOT NULL THEN COALESCE(?5, ?4) END,
                 canceled_at = CASE WHEN canceled_at IS NOT NULL THEN ?4 END
             WHERE id = ?1",
            params![
                id,
                sort,
                created,
                rfc3339(card.updated),
                card.done_at.map(rfc3339)
            ],
        )?;
        tx.execute(
            "UPDATE ticket_activity SET created = ?2 WHERE ticket_id = ?1 AND kind = 'created'",
            params![id, created],
        )?;
        for p in people.iter().skip(1) {
            tx.execute(
                "INSERT OR IGNORE INTO ticket_subscribers (ticket_id, user_id) VALUES (?1, ?2)",
                params![id, p],
            )?;
        }
        for (author, body, at) in comments {
            let author_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM users WHERE lower_login = ?1",
                    [author.to_lowercase()],
                    |r| r.get(0),
                )
                .optional()?;
            tx.execute(
                "INSERT INTO ticket_comments (ticket_id, author_id, author_name, body, created, updated)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![id, author_id, author, body, rfc3339(at)],
            )?;
            report.comments += 1;
        }
        remember(&tx, "card", card.id, id)?;
        tx.commit()?;
        report.cards += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::db::Visibility;

    fn cloud_db(dir: &Path, repo_issue: i64) -> std::path::PathBuf {
        let path = dir.join("cloud.db");
        let c = Connection::open(&path).unwrap();
        c.execute_batch(crate::cloud::boards::SCHEMA).unwrap();
        c.execute_batch(&format!(
            "INSERT INTO board (id, title, color, owner, repo, archived, created, updated)
               VALUES (1, 'Garden', '31cc7c', 'notshared', NULL, 0, 1760000000, 1760000000),
                      (2, 'losos', NULL, 'notshared', 'notshared/losos', 0, 1760000000, 1760000000);
             INSERT INTO board_member (board_id, login, can_edit) VALUES (1, 'eva', 1);
             INSERT INTO board_column (id, board_id, title, position, state) VALUES
               (10, 1, 'To do', 0, NULL), (11, 1, 'Doing', 1, NULL), (12, 1, 'Done', 2, 'closed'),
               (20, 2, 'Backlog', 0, 'open'), (21, 2, 'In progress', 1, 'open'), (22, 2, 'Done', 2, 'closed');
             INSERT INTO board_label (id, board_id, name, color) VALUES (5, 1, 'Spring', 'ff9900');
             INSERT INTO card (id, board_id, column_id, position, title, description, due, link_repo,
                               link_number, archived, created, updated, done_at) VALUES
               (100, 1, 10, 0, 'Plant tomatoes', 'After the frost.', 1762000000, NULL, NULL, 0,
                1760000100, 1760000200, NULL),
               (101, 1, 12, 0, 'Buy seeds', '', NULL, NULL, NULL, 0, 1760000100, 1760000300, 1760000300),
               (102, 1, 11, 0, 'Old idea', '', NULL, NULL, NULL, 1, 1760000100, 1760000300, NULL),
               (200, 2, 22, 0, 'Fix the backup script', '', NULL, 'notshared/losos', {repo_issue}, 0,
                1760000100, 1760000300, NULL);
             INSERT INTO card_label (card_id, label_id) VALUES (100, 5);
             INSERT INTO card_assignee (card_id, login) VALUES (100, 'eva'), (100, 'notshared');
             INSERT INTO card_comment (card_id, author, body, created) VALUES
               (100, 'eva', 'I have the pots.', 1760000150);"
        ))
        .unwrap();
        path
    }

    #[test]
    fn boards_become_projects_and_cards_issues_once() {
        let s = Store::in_memory().unwrap();
        let owner = s.ensure_user("notshared", "Matus", "").unwrap();
        let repo = s
            .create_repo(owner.id, "losos", "", Visibility::Private, "main", None)
            .unwrap();
        s.conn()
            .execute(
                "INSERT INTO issues (repo_id, number, poster_id, title, created, updated)
                 VALUES (?1, 12, ?2, 'Fix the backup script', '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z')",
                params![repo.id, owner.id],
            )
            .unwrap();
        super::super::adopt::adopt_repo_issues(&s).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let db = cloud_db(dir.path(), 12);

        let report = import(&s, &db).unwrap();
        assert_eq!(report.boards, 2);
        assert_eq!(report.cards, 3);
        assert_eq!(report.linked, 1);
        assert_eq!(report.comments, 1);
        assert_eq!(report.projects, ["GAR", "LOS"]);
        let again = import(&s, &db).unwrap();
        assert_eq!(
            (again.cards, again.linked),
            (0, 0),
            "a second run changes nothing"
        );

        let garden = s.project("GAR").unwrap().unwrap();
        assert_eq!(garden.color, "#31cc7c");
        let flow: Vec<(String, Category)> = s
            .workflow(garden.id)
            .unwrap()
            .into_iter()
            .map(|w| (w.name, w.category))
            .collect();
        assert_eq!(
            flow,
            [
                ("To do".to_string(), Category::Unstarted),
                ("Doing".to_string(), Category::Started),
                ("Done".to_string(), Category::Done),
                ("Canceled".to_string(), Category::Canceled),
            ]
        );
        let tickets = s.tickets_of(&[garden.id]).unwrap();
        let plant = tickets
            .iter()
            .find(|t| t.title == "Plant tomatoes")
            .unwrap();
        assert_eq!(plant.assignee.as_deref(), Some("eva"));
        assert_eq!(plant.due.as_deref(), Some("2025-11-01"));
        assert_eq!(plant.labels[0].color, "#ff9900");
        assert_eq!(plant.created, "2025-10-09T08:55:00Z");
        assert_eq!(
            s.ticket_comments(plant.id).unwrap()[0].body,
            "I have the pots."
        );
        assert_eq!(s.subscribers(plant.id).unwrap(), ["eva", "notshared"]);
        let seeds = tickets.iter().find(|t| t.title == "Buy seeds").unwrap();
        assert_eq!(seeds.state.category, Category::Done);
        let old = tickets.iter().find(|t| t.title == "Old idea").unwrap();
        assert_eq!(
            old.state.category,
            Category::Canceled,
            "an archived card was dropped"
        );
        let eva = s.user("eva").unwrap().unwrap();
        let members = s.project_members(garden.id).unwrap();
        assert!(members.contains(&("eva".to_string(), crate::forge::db::Permission::Write)));
        assert_eq!(eva.login, "eva");

        // The repository's board moved its issue to Done instead of copying it.
        let los = s.ticket("LOS", 12).unwrap().unwrap();
        assert_eq!(los.state.category, Category::Done);
        assert!(s.issue(repo.id, 12).unwrap().unwrap().closed);
        assert_eq!(s.tickets_of(&[los.project_id]).unwrap().len(), 1);
    }

    #[test]
    fn columns_are_read_for_what_they_mean() {
        assert_eq!(category_of("Nápady", false, true), Category::Backlog);
        assert_eq!(category_of("Up next", false, false), Category::Unstarted);
        assert_eq!(category_of("Waiting", false, false), Category::Started);
        assert_eq!(category_of("Anything", true, false), Category::Done);
        assert_eq!(category_of("Erledigt", false, false), Category::Done);
    }
}
