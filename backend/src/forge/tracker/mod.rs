//! LosOS Git's issues: one tracker across the box's repositories, in the
//! shape of Linear and Jira.
//!
//! An issue belongs to a project, which has a key, so an issue has a name
//! that is the same everywhere: `LOS-12`. A project has its own workflow
//! (states, each in one of five categories), labels, cycles and members,
//! and may be linked to repositories. An issue of a linked repository also
//! has the repository's number (`#12`): its row in `issues` is kept as the
//! number's registry, so pull requests are still numbered after it and a
//! Forgejo import still finds what it carried over.
//!
//! What moves an issue besides people: a commit or pull request that names
//! it (`Fixes LOS-12`, `Closes #12`) links to it, and closes it when the
//! commit reaches the default branch or the pull request is merged
//! ([`hooks`], the words in [`refs`]). Everyone who opened, was given,
//! commented on or was mentioned in an issue follows it and hears what
//! happens to it in their inbox.
//!
//! Where it came from: the issues a repository had before ([`adopt`], on
//! every start) and the boards of LosOS Cloud ([`boards`], once, by
//! `losos-git import-boards`).

pub mod adopt;
pub mod api;
pub mod boards;
pub mod hooks;
pub mod ics;
pub mod query;
pub mod refs;
pub mod store;

use serde::Serialize;

/// The tables, the seventh entry of `db::MIGRATIONS`.
pub const MIGRATION: &str = "
    CREATE TABLE projects (
        id          INTEGER PRIMARY KEY,
        key         TEXT NOT NULL UNIQUE,
        name        TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        color       TEXT NOT NULL DEFAULT '',
        lead_id     INTEGER REFERENCES users(id) ON DELETE SET NULL,
        visibility  TEXT NOT NULL DEFAULT 'private'
                    CHECK (visibility IN ('private', 'internal')),
        next_number INTEGER NOT NULL DEFAULT 1,
        archived    INTEGER NOT NULL DEFAULT 0,
        created     TEXT NOT NULL,
        updated     TEXT NOT NULL
    );
    CREATE TABLE project_members (
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        role       TEXT NOT NULL CHECK (role IN ('read', 'write', 'admin')),
        PRIMARY KEY (project_id, user_id)
    );
    CREATE TABLE project_repos (
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        repo_id    INTEGER NOT NULL UNIQUE REFERENCES repos(id) ON DELETE CASCADE,
        PRIMARY KEY (project_id, repo_id)
    );
    CREATE TABLE workflow_states (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name       TEXT NOT NULL,
        category   TEXT NOT NULL
                   CHECK (category IN ('backlog', 'unstarted', 'started', 'done', 'canceled')),
        color      TEXT NOT NULL DEFAULT '',
        position   INTEGER NOT NULL,
        UNIQUE (project_id, name)
    );
    CREATE TABLE project_labels (
        id          INTEGER PRIMARY KEY,
        project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name        TEXT NOT NULL,
        color       TEXT NOT NULL DEFAULT '',
        description TEXT NOT NULL DEFAULT '',
        UNIQUE (project_id, name)
    );
    CREATE TABLE cycles (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        number     INTEGER NOT NULL,
        name       TEXT NOT NULL DEFAULT '',
        starts     TEXT NOT NULL,
        ends       TEXT NOT NULL,
        UNIQUE (project_id, number)
    );
    CREATE TABLE tickets (
        id           INTEGER PRIMARY KEY,
        project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        number       INTEGER NOT NULL,
        title        TEXT NOT NULL,
        body         TEXT NOT NULL DEFAULT '',
        state_id     INTEGER NOT NULL REFERENCES workflow_states(id),
        priority     INTEGER NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 4),
        estimate     INTEGER,
        assignee_id  INTEGER REFERENCES users(id) ON DELETE SET NULL,
        creator_id   INTEGER REFERENCES users(id) ON DELETE SET NULL,
        creator_name TEXT NOT NULL DEFAULT '',
        cycle_id     INTEGER REFERENCES cycles(id) ON DELETE SET NULL,
        parent_id    INTEGER REFERENCES tickets(id) ON DELETE SET NULL,
        due          TEXT,
        sort         REAL NOT NULL DEFAULT 0,
        issue_id     INTEGER UNIQUE REFERENCES issues(id) ON DELETE SET NULL,
        created      TEXT NOT NULL,
        updated      TEXT NOT NULL,
        started_at   TEXT,
        completed_at TEXT,
        canceled_at  TEXT,
        UNIQUE (project_id, number)
    );
    CREATE INDEX tickets_state ON tickets (state_id);
    CREATE INDEX tickets_assignee ON tickets (assignee_id);
    CREATE INDEX tickets_parent ON tickets (parent_id);
    CREATE TABLE ticket_labels (
        ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
        label_id  INTEGER NOT NULL REFERENCES project_labels(id) ON DELETE CASCADE,
        PRIMARY KEY (ticket_id, label_id)
    );
    CREATE TABLE ticket_comments (
        id          INTEGER PRIMARY KEY,
        ticket_id   INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
        author_id   INTEGER REFERENCES users(id) ON DELETE SET NULL,
        author_name TEXT NOT NULL DEFAULT '',
        body        TEXT NOT NULL,
        created     TEXT NOT NULL,
        updated     TEXT NOT NULL
    );
    CREATE INDEX ticket_comments_ticket ON ticket_comments (ticket_id);
    CREATE TABLE ticket_relations (
        id      INTEGER PRIMARY KEY,
        from_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
        to_id   INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
        kind    TEXT NOT NULL CHECK (kind IN ('blocks', 'relates', 'duplicates')),
        created TEXT NOT NULL,
        UNIQUE (from_id, to_id, kind),
        CHECK (from_id <> to_id)
    );
    CREATE TABLE ticket_links (
        id         INTEGER PRIMARY KEY,
        ticket_id  INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
        repo_id    INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
        kind       TEXT NOT NULL CHECK (kind IN ('commit', 'pull')),
        target     TEXT NOT NULL,
        title      TEXT NOT NULL DEFAULT '',
        actor_name TEXT NOT NULL DEFAULT '',
        closes     INTEGER NOT NULL DEFAULT 0,
        created    TEXT NOT NULL,
        UNIQUE (ticket_id, repo_id, kind, target)
    );
    CREATE TABLE ticket_activity (
        id         INTEGER PRIMARY KEY,
        ticket_id  INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
        actor_id   INTEGER REFERENCES users(id) ON DELETE SET NULL,
        actor_name TEXT NOT NULL DEFAULT '',
        kind       TEXT NOT NULL,
        detail     TEXT NOT NULL DEFAULT '{}',
        created    TEXT NOT NULL
    );
    CREATE INDEX ticket_activity_ticket ON ticket_activity (ticket_id);
    CREATE TABLE ticket_subscribers (
        ticket_id INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
        user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        PRIMARY KEY (ticket_id, user_id)
    );
    CREATE TABLE saved_views (
        id       INTEGER PRIMARY KEY,
        owner_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        name     TEXT NOT NULL,
        query    TEXT NOT NULL DEFAULT '{}',
        layout   TEXT NOT NULL DEFAULT 'list' CHECK (layout IN ('list', 'board')),
        group_by TEXT NOT NULL DEFAULT 'state',
        shared   INTEGER NOT NULL DEFAULT 0,
        created  TEXT NOT NULL
    );
    CREATE TABLE inbox (
        id         INTEGER PRIMARY KEY,
        user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        ticket_id  INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
        kind       TEXT NOT NULL,
        actor_name TEXT NOT NULL DEFAULT '',
        detail     TEXT NOT NULL DEFAULT '{}',
        created    TEXT NOT NULL,
        read_at    TEXT
    );
    CREATE INDEX inbox_user ON inbox (user_id, read_at);
    CREATE TABLE board_ids (
        kind     TEXT NOT NULL,
        cloud_id INTEGER NOT NULL,
        local_id INTEGER NOT NULL,
        PRIMARY KEY (kind, cloud_id)
    );
";

/// Where a state sits in the flow of work. Linear's five.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Backlog,
    Unstarted,
    Started,
    Done,
    Canceled,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Backlog,
        Category::Unstarted,
        Category::Started,
        Category::Done,
        Category::Canceled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Backlog => "backlog",
            Category::Unstarted => "unstarted",
            Category::Started => "started",
            Category::Done => "done",
            Category::Canceled => "canceled",
        }
    }

    pub fn parse(s: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.as_str() == s)
    }

    /// Done or canceled: the issue is over, and counts as closed for its
    /// repository.
    pub fn closed(self) -> bool {
        matches!(self, Category::Done | Category::Canceled)
    }
}

/// The workflow a new project starts with: name, category, colour.
pub const DEFAULT_STATES: &[(&str, Category, &str)] = &[
    ("Backlog", Category::Backlog, "#94a3b8"),
    ("Todo", Category::Unstarted, "#64748b"),
    ("In Progress", Category::Started, "#eab308"),
    ("In Review", Category::Started, "#22c55e"),
    ("Done", Category::Done, "#6366f1"),
    ("Canceled", Category::Canceled, "#9ca3af"),
];

/// Linear's priorities, 0 to 4. Urgent sorts first, none last.
pub const PRIORITIES: [&str; 5] = ["none", "urgent", "high", "medium", "low"];

/// The rank a priority sorts by: urgent 0, ..., low 3, none 4.
pub fn priority_rank(p: i64) -> i64 {
    if p == 0 {
        4
    } else {
        p - 1
    }
}

/// `LOS-12`.
pub fn key_of(project_key: &str, number: i64) -> String {
    format!("{project_key}-{number}")
}

/// `LOS-12` taken apart, upper-cased: `("LOS", 12)`.
pub fn parse_key(s: &str) -> Option<(String, i64)> {
    let (k, n) = s.trim().rsplit_once('-')?;
    let k = k.to_ascii_uppercase();
    if !valid_project_key(&k) {
        return None;
    }
    let n: i64 = n.parse().ok()?;
    (n > 0).then_some((k, n))
}

/// Two to six letters or digits, a letter first.
pub fn valid_project_key(k: &str) -> bool {
    (2..=6).contains(&k.len())
        && k.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && k.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// A colour for a label made without one: the usual names get the usual
/// colours, anything else one of the same eight, the same every time.
pub fn label_color(name: &str) -> &'static str {
    const COLORS: [&str; 8] = [
        "#d1453b", "#8b5cf6", "#2f7de1", "#c9861a", "#3a9a5b", "#d0508f", "#1c9aa6", "#6b7280",
    ];
    let n = name.to_lowercase();
    let i = match n.as_str() {
        "bug" | "defect" | "regression" | "broken" => 0,
        "feature" | "enhancement" | "improvement" => 1,
        "docs" | "documentation" => 2,
        "question" | "help" | "help wanted" => 3,
        "good first issue" | "easy" => 4,
        "security" => 5,
        "performance" | "speed" => 6,
        "chore" | "wontfix" | "duplicate" => 7,
        _ => {
            n.bytes()
                .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) as usize
                % COLORS.len()
        }
    };
    COLORS[i]
}

/// A key for a project of this name: the initials of its words when it
/// has two or more, its first three letters otherwise, upper case. The
/// caller makes it unique.
pub fn key_for(name: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            // `myProject` is two words.
            if ch.is_ascii_uppercase() && prev_lower && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev_lower = ch.is_ascii_lowercase();
            cur.push(ch.to_ascii_uppercase());
        } else {
            prev_lower = false;
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words.retain(|w| w.chars().next().is_some_and(|c| c.is_ascii_alphabetic()));
    let key: String = if words.len() >= 2 {
        words
            .iter()
            .take(4)
            .filter_map(|w| w.chars().next())
            .collect()
    } else {
        words
            .first()
            .map(|w| w.chars().take(3).collect())
            .unwrap_or_default()
    };
    if valid_project_key(&key) {
        key
    } else if key.len() == 1 {
        format!("{key}X")
    } else {
        "ISS".to_string()
    }
}

/// `YYYY-MM-DD`, a calendar date and nothing else.
pub fn valid_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok() && s.len() == 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_come_from_names() {
        assert_eq!(key_for("losos"), "LOS");
        assert_eq!(key_for("losos-config"), "LC");
        assert_eq!(key_for("Backup scripts for the box"), "BSFT");
        assert_eq!(key_for("myProject"), "MP");
        assert_eq!(key_for("x"), "XX");
        assert_eq!(key_for("42"), "ISS");
        assert_eq!(key_for("Zahrada"), "ZAH");
    }

    #[test]
    fn keys_parse() {
        assert_eq!(parse_key("LOS-12"), Some(("LOS".into(), 12)));
        assert_eq!(parse_key("los-3"), Some(("LOS".into(), 3)));
        assert_eq!(parse_key("LOS-0"), None);
        assert_eq!(parse_key("L-1"), None);
        assert_eq!(parse_key("LOS12"), None);
        assert_eq!(parse_key("1AB-2"), None);
    }

    #[test]
    fn priorities_sort_urgent_first_none_last() {
        let mut ps = vec![0, 4, 1, 3, 2];
        ps.sort_by_key(|p| priority_rank(*p));
        assert_eq!(ps, vec![1, 2, 3, 4, 0]);
    }

    #[test]
    fn dates_are_days() {
        assert!(valid_date("2026-10-22"));
        assert!(!valid_date("2026-13-01"));
        assert!(!valid_date("2026-10-22T00:00"));
    }
}
