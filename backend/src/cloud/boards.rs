//! Boards: columns of cards, the Kanban the cloud keeps (the schema agreed
//! with LosOS Git, where a card may stand for an issue). This part holds
//! the tables, what the Nextcloud import writes into them, and reading a
//! board back; editing comes with the board page.

use crate::cloud::store::{Result, Store, StoreError};
use crate::cloud::util::now;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS board (
    id        INTEGER PRIMARY KEY,
    title     TEXT NOT NULL,
    color     TEXT,
    owner     TEXT NOT NULL,
    repo      TEXT,
    archived  INTEGER NOT NULL DEFAULT 0,
    created   INTEGER NOT NULL,
    updated   INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS board_member (
    board_id   INTEGER NOT NULL REFERENCES board (id) ON DELETE CASCADE,
    login      TEXT NOT NULL,
    can_edit   INTEGER NOT NULL DEFAULT 0,
    can_share  INTEGER NOT NULL DEFAULT 0,
    can_manage INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (board_id, login)
);
CREATE TABLE IF NOT EXISTS board_column (
    id        INTEGER PRIMARY KEY,
    board_id  INTEGER NOT NULL REFERENCES board (id) ON DELETE CASCADE,
    title     TEXT NOT NULL,
    position  INTEGER NOT NULL,
    state     TEXT CHECK (state IN ('open', 'closed'))
);
CREATE TABLE IF NOT EXISTS board_label (
    id        INTEGER PRIMARY KEY,
    board_id  INTEGER NOT NULL REFERENCES board (id) ON DELETE CASCADE,
    name      TEXT NOT NULL,
    color     TEXT
);
CREATE TABLE IF NOT EXISTS card (
    id          INTEGER PRIMARY KEY,
    board_id    INTEGER NOT NULL REFERENCES board (id) ON DELETE CASCADE,
    column_id   INTEGER NOT NULL REFERENCES board_column (id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    due         INTEGER,
    link_repo   TEXT,
    link_number INTEGER,
    archived    INTEGER NOT NULL DEFAULT 0,
    created     INTEGER NOT NULL,
    updated     INTEGER NOT NULL,
    done_at     INTEGER
);
CREATE TABLE IF NOT EXISTS card_label (
    card_id   INTEGER NOT NULL REFERENCES card (id) ON DELETE CASCADE,
    label_id  INTEGER NOT NULL REFERENCES board_label (id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, label_id)
);
CREATE TABLE IF NOT EXISTS card_assignee (
    card_id   INTEGER NOT NULL REFERENCES card (id) ON DELETE CASCADE,
    login     TEXT NOT NULL,
    PRIMARY KEY (card_id, login)
);
CREATE TABLE IF NOT EXISTS card_comment (
    id        INTEGER PRIMARY KEY,
    card_id   INTEGER NOT NULL REFERENCES card (id) ON DELETE CASCADE,
    author    TEXT NOT NULL,
    body      TEXT NOT NULL,
    created   INTEGER NOT NULL
);
"#;

pub fn init(store: &Store) -> Result<()> {
    store.with_db(|db| Ok(db.execute_batch(SCHEMA)?))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: i64,
    pub title: String,
    pub color: Option<String>,
    pub owner: String,
    pub archived: bool,
    pub updated: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub id: i64,
    pub title: String,
    pub position: i64,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub position: i64,
    pub due: Option<i64>,
    pub archived: bool,
    pub done_at: Option<i64>,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub author: String,
    pub body: String,
    pub created: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Full {
    #[serde(flatten)]
    pub board: Board,
    pub labels: Vec<Label>,
    pub members: Vec<String>,
    pub columns: Vec<Column>,
}

/// The boards a person owns or was given.
pub fn list(store: &Store, login: &str) -> Result<Vec<Board>> {
    store.with_db(|db| {
        let mut st = db.prepare(
            "SELECT id, title, color, owner, archived, updated FROM board
             WHERE owner = ?1 OR id IN (SELECT board_id FROM board_member WHERE login = ?1)
             ORDER BY archived, title, id",
        )?;
        let rows = st.query_map(params![login], |r| {
            Ok(Board {
                id: r.get(0)?,
                title: r.get(1)?,
                color: r.get(2)?,
                owner: r.get(3)?,
                archived: r.get(4)?,
                updated: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

/// One board with everything on it, if `login` may see it.
pub fn get(store: &Store, login: &str, id: i64) -> Result<Full> {
    let board = list(store, login)?
        .into_iter()
        .find(|b| b.id == id)
        .ok_or(StoreError::NotFound)?;
    store.with_db(|db| {
        let labels = {
            let mut st =
                db.prepare("SELECT id, name, color FROM board_label WHERE board_id = ?1 ORDER BY id")?;
            let rows = st.query_map(params![id], |r| {
                Ok(Label {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let members = {
            let mut st =
                db.prepare("SELECT login FROM board_member WHERE board_id = ?1 ORDER BY login")?;
            let rows = st.query_map(params![id], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<Vec<String>>>()?
        };
        let mut columns = {
            let mut st = db.prepare(
                "SELECT id, title, position FROM board_column WHERE board_id = ?1 ORDER BY position, id",
            )?;
            let rows = st.query_map(params![id], |r| {
                Ok(Column {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    position: r.get(2)?,
                    cards: Vec::new(),
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for col in &mut columns {
            let mut st = db.prepare(
                "SELECT id, title, description, position, due, archived, done_at FROM card
                 WHERE column_id = ?1 ORDER BY position, id",
            )?;
            let rows = st.query_map(params![col.id], |r| {
                Ok(Card {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    description: r.get(2)?,
                    position: r.get(3)?,
                    due: r.get(4)?,
                    archived: r.get(5)?,
                    done_at: r.get(6)?,
                    labels: Vec::new(),
                    assignees: Vec::new(),
                    comments: Vec::new(),
                })
            })?;
            col.cards = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            for card in &mut col.cards {
                let mut st = db.prepare(
                    "SELECT l.name FROM card_label c JOIN board_label l ON l.id = c.label_id
                     WHERE c.card_id = ?1 ORDER BY l.id",
                )?;
                card.labels = st
                    .query_map(params![card.id], |r| r.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()?;
                let mut st = db
                    .prepare("SELECT login FROM card_assignee WHERE card_id = ?1 ORDER BY login")?;
                card.assignees = st
                    .query_map(params![card.id], |r| r.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()?;
                let mut st = db.prepare(
                    "SELECT author, body, created FROM card_comment WHERE card_id = ?1 ORDER BY created, id",
                )?;
                card.comments = st
                    .query_map(params![card.id], |r| {
                        Ok(Comment {
                            author: r.get(0)?,
                            body: r.get(1)?,
                            created: r.get(2)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
            }
        }
        Ok(Full {
            board,
            labels,
            members,
            columns,
        })
    })
}

/// A board as it comes in from elsewhere (the Nextcloud import), written in
/// one transaction.
#[derive(Debug, Clone, Default)]
pub struct NewBoard {
    pub title: String,
    pub color: Option<String>,
    pub owner: String,
    pub archived: bool,
    pub updated: Option<i64>,
    /// (login, edit, share, manage)
    pub members: Vec<(String, bool, bool, bool)>,
    pub labels: Vec<(String, Option<String>)>,
    pub columns: Vec<NewColumn>,
}

#[derive(Debug, Clone, Default)]
pub struct NewColumn {
    pub title: String,
    pub cards: Vec<NewCard>,
}

#[derive(Debug, Clone, Default)]
pub struct NewCard {
    pub title: String,
    pub description: String,
    pub due: Option<i64>,
    pub archived: bool,
    pub done_at: Option<i64>,
    pub created: Option<i64>,
    pub updated: Option<i64>,
    /// Indexes into [`NewBoard::labels`].
    pub labels: Vec<usize>,
    pub assignees: Vec<String>,
    /// (author, body, created)
    pub comments: Vec<(String, String, i64)>,
}

pub fn insert(store: &Store, b: &NewBoard) -> Result<i64> {
    let t = now();
    store.with_db(|db| {
        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO board (title, color, owner, archived, created, updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![b.title, b.color, b.owner, b.archived, t, b.updated.unwrap_or(t)],
        )?;
        let board = tx.last_insert_rowid();
        for (login, edit, share, manage) in &b.members {
            if *login == b.owner {
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO board_member (board_id, login, can_edit, can_share, can_manage)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![board, login, edit, share, manage],
            )?;
        }
        let mut label_ids = Vec::new();
        for (name, color) in &b.labels {
            tx.execute(
                "INSERT INTO board_label (board_id, name, color) VALUES (?1, ?2, ?3)",
                params![board, name, color],
            )?;
            label_ids.push(tx.last_insert_rowid());
        }
        for (ci, col) in b.columns.iter().enumerate() {
            tx.execute(
                "INSERT INTO board_column (board_id, title, position) VALUES (?1, ?2, ?3)",
                params![board, col.title, ci as i64],
            )?;
            let column = tx.last_insert_rowid();
            for (pi, c) in col.cards.iter().enumerate() {
                tx.execute(
                    "INSERT INTO card (board_id, column_id, position, title, description, due, archived, created, updated, done_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        board,
                        column,
                        pi as i64,
                        c.title,
                        c.description,
                        c.due,
                        c.archived,
                        c.created.unwrap_or(t),
                        c.updated.unwrap_or(t),
                        c.done_at
                    ],
                )?;
                let card = tx.last_insert_rowid();
                for li in &c.labels {
                    if let Some(id) = label_ids.get(*li) {
                        tx.execute(
                            "INSERT OR IGNORE INTO card_label (card_id, label_id) VALUES (?1, ?2)",
                            params![card, id],
                        )?;
                    }
                }
                for a in &c.assignees {
                    tx.execute(
                        "INSERT OR IGNORE INTO card_assignee (card_id, login) VALUES (?1, ?2)",
                        params![card, a],
                    )?;
                }
                for (author, body, created) in &c.comments {
                    tx.execute(
                        "INSERT INTO card_comment (card_id, author, body, created) VALUES (?1, ?2, ?3, ?4)",
                        params![card, author, body, created],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(board)
    })
}

/// How many boards and cards a person owns, for the import's report.
pub fn counts(store: &Store, owner: &str) -> Result<(i64, i64)> {
    store.with_db(|db| {
        let boards: i64 = db.query_row(
            "SELECT COUNT(*) FROM board WHERE owner = ?1",
            params![owner],
            |r| r.get(0),
        )?;
        let cards: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM card WHERE board_id IN (SELECT id FROM board WHERE owner = ?1)",
                params![owner],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        Ok((boards, cards))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_goes_in_whole_and_comes_back_for_its_people_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        init(&store).unwrap();
        let id = insert(
            &store,
            &NewBoard {
                title: "Garden shed".into(),
                owner: "alice".into(),
                members: vec![("bob".into(), true, false, false)],
                labels: vec![("Wood".into(), Some("31CC7C".into()))],
                columns: vec![
                    NewColumn {
                        title: "To do".into(),
                        cards: vec![NewCard {
                            title: "Buy boards".into(),
                            labels: vec![0],
                            assignees: vec!["bob".into()],
                            comments: vec![("alice".into(), "48 of them".into(), 5)],
                            ..Default::default()
                        }],
                    },
                    NewColumn {
                        title: "Done".into(),
                        cards: vec![],
                    },
                ],
                ..Default::default()
            },
        )
        .unwrap();
        let full = get(&store, "bob", id).unwrap();
        assert_eq!(full.columns.len(), 2);
        let card = &full.columns[0].cards[0];
        assert_eq!(card.labels, vec!["Wood".to_owned()]);
        assert_eq!(card.assignees, vec!["bob".to_owned()]);
        assert_eq!(card.comments[0].body, "48 of them");
        assert_eq!(full.members, vec!["bob".to_owned()]);
        assert!(matches!(get(&store, "eve", id), Err(StoreError::NotFound)));
        assert_eq!(counts(&store, "alice").unwrap(), (1, 1));
    }
}
