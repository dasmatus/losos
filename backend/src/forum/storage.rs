//! SQLite storage for the forum (Phase 1).
use crate::forum::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

const MIGRATIONS: &[&str] = &["
    CREATE TABLE spaces (
        id INTEGER PRIMARY KEY,
        slug TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        created_by TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    CREATE TABLE channels (
        id INTEGER PRIMARY KEY,
        space_id INTEGER REFERENCES spaces(id) ON DELETE SET NULL,
        slug TEXT NOT NULL,
        name TEXT NOT NULL,
        kind TEXT NOT NULL,
        created_by TEXT NOT NULL,
        created_at TEXT NOT NULL,
        UNIQUE(space_id, slug)
    );
    CREATE TABLE topics (
        id INTEGER PRIMARY KEY,
        channel_id INTEGER NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
        title TEXT NOT NULL,
        created_by TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        closed INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE posts (
        id INTEGER PRIMARY KEY,
        topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
        author TEXT NOT NULL,
        body TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE TABLE messages (
        id INTEGER PRIMARY KEY,
        channel_id INTEGER NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
        thread_id INTEGER,
        author TEXT NOT NULL,
        body TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    CREATE TABLE memberships (
        subject TEXT NOT NULL,
        space_id INTEGER REFERENCES spaces(id) ON DELETE CASCADE,
        channel_id INTEGER REFERENCES channels(id) ON DELETE CASCADE,
        role TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    CREATE TABLE reactions (
        id INTEGER PRIMARY KEY,
        target_kind TEXT NOT NULL,
        target_id INTEGER NOT NULL,
        actor TEXT NOT NULL,
        emoji TEXT NOT NULL,
        created_at TEXT NOT NULL,
        UNIQUE(target_kind, target_id, actor, emoji)
    );
    CREATE TABLE mentions (
        id INTEGER PRIMARY KEY,
        target_kind TEXT NOT NULL,
        target_id INTEGER NOT NULL,
        target_name TEXT NOT NULL,
        is_assistant INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL
    );
    CREATE INDEX idx_topics_channel ON topics(channel_id);
    CREATE INDEX idx_posts_topic ON posts(topic_id);
    CREATE INDEX idx_messages_channel ON messages(channel_id);
    CREATE INDEX idx_reactions_target ON reactions(target_kind, target_id);
    CREATE INDEX idx_mentions_target ON mentions(target_kind, target_id);
    "];

#[derive(Debug, thiserror::Error)]
pub enum ForumStoreError {
    #[error("forum is disabled")]
    Disabled,
    #[error("forum storage is unavailable")]
    Unavailable,
    #[error("not found")]
    NotFound,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("storage error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct ForumDb {
    conn: Mutex<Connection>,
}

impl ForumDb {
    pub fn open(path: &Path) -> Result<Self, ForumStoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, ForumStoreError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), ForumStoreError> {
        let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.transaction()?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let version = usize::try_from(version).unwrap_or(0);
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            tx.execute_batch(sql)?;
            tx.pragma_update(
                None,
                "user_version",
                i64::try_from(i + 1).unwrap_or(i64::from(i as u32) + 1),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn now() -> String {
        // RFC3339-ish UTC without extra deps; good enough for Phase 1.
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("unix:{secs}")
    }

    fn channel_kind_to_str(kind: ChannelKind) -> &'static str {
        match kind {
            ChannelKind::Topic => "topic",
            ChannelKind::Channel => "channel",
            ChannelKind::Dm => "dm",
        }
    }

    fn channel_kind_from_str(s: &str) -> ChannelKind {
        match s {
            "channel" => ChannelKind::Channel,
            "dm" => ChannelKind::Dm,
            _ => ChannelKind::Topic,
        }
    }

    pub fn list_spaces(&self) -> Result<Vec<Space>, ForumStoreError> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, slug, name, description, created_by, created_at FROM spaces ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Space {
                id: r.get(0)?,
                slug: r.get(1)?,
                name: r.get(2)?,
                description: r.get(3)?,
                created_by: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn create_space(&self, actor: &str, input: &CreateSpace) -> Result<Space, ForumStoreError> {
        if input.slug.trim().is_empty() || input.name.trim().is_empty() {
            return Err(ForumStoreError::InvalidInput(
                "slug and name are required".into(),
            ));
        }
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Self::now();
        conn.execute(
            "INSERT INTO spaces(slug, name, description, created_by, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![input.slug.trim(), input.name.trim(), input.description, actor, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Space {
            id,
            slug: input.slug.trim().to_string(),
            name: input.name.trim().to_string(),
            description: input.description.clone(),
            created_by: actor.to_string(),
            created_at: now,
        })
    }

    pub fn list_channels(&self, space_id: Option<Id>) -> Result<Vec<Channel>, ForumStoreError> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let map = |r: &rusqlite::Row<'_>| {
            let kind_s: String = r.get(4)?;
            Ok(Channel {
                id: r.get(0)?,
                space_id: r.get(1)?,
                slug: r.get(2)?,
                name: r.get(3)?,
                kind: Self::channel_kind_from_str(&kind_s),
                created_by: r.get(5)?,
                created_at: r.get(6)?,
            })
        };
        if let Some(sid) = space_id {
            let mut stmt = conn.prepare(
                "SELECT id, space_id, slug, name, kind, created_by, created_at FROM channels WHERE space_id=?1 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map(params![sid], map)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, space_id, slug, name, kind, created_by, created_at FROM channels ORDER BY id ASC",
            )?;
            let rows = stmt.query_map([], map)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        }
    }

    pub fn create_channel(
        &self,
        actor: &str,
        input: &CreateChannel,
    ) -> Result<Channel, ForumStoreError> {
        if input.slug.trim().is_empty() || input.name.trim().is_empty() {
            return Err(ForumStoreError::InvalidInput(
                "slug and name are required".into(),
            ));
        }
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Self::now();
        let kind = Self::channel_kind_to_str(input.kind);
        conn.execute(
            "INSERT INTO channels(space_id, slug, name, kind, created_by, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![input.space_id, input.slug.trim(), input.name.trim(), kind, actor, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Channel {
            id,
            space_id: input.space_id,
            slug: input.slug.trim().to_string(),
            name: input.name.trim().to_string(),
            kind: input.kind,
            created_by: actor.to_string(),
            created_at: now,
        })
    }

    pub fn list_topics(&self, channel_id: Option<Id>) -> Result<Vec<Topic>, ForumStoreError> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let map = |r: &rusqlite::Row<'_>| {
            Ok(Topic {
                id: r.get(0)?,
                channel_id: r.get(1)?,
                title: r.get(2)?,
                created_by: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
                closed: r.get::<_, i64>(6)? != 0,
            })
        };
        if let Some(cid) = channel_id {
            let mut stmt = conn.prepare(
                "SELECT id, channel_id, title, created_by, created_at, updated_at, closed FROM topics WHERE channel_id=?1 ORDER BY updated_at DESC, id DESC",
            )?;
            let rows = stmt.query_map(params![cid], map)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, channel_id, title, created_by, created_at, updated_at, closed FROM topics ORDER BY updated_at DESC, id DESC",
            )?;
            let rows = stmt.query_map([], map)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        }
    }

    pub fn create_topic(
        &self,
        actor: &str,
        input: &CreateTopic,
    ) -> Result<(Topic, Post, Vec<Mention>), ForumStoreError> {
        if input.title.trim().is_empty() || input.body.trim().is_empty() {
            return Err(ForumStoreError::InvalidInput(
                "title and body are required".into(),
            ));
        }
        let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Self::now();
        let tx = conn.transaction()?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT id FROM channels WHERE id=?1",
                params![input.channel_id],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(ForumStoreError::NotFound);
        }
        tx.execute(
            "INSERT INTO topics(channel_id, title, created_by, created_at, updated_at, closed) VALUES(?1, ?2, ?3, ?4, ?4, 0)",
            params![input.channel_id, input.title.trim(), actor, now],
        )?;
        let topic_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO posts(topic_id, author, body, created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?4)",
            params![topic_id, actor, input.body, now],
        )?;
        let post_id = tx.last_insert_rowid();
        let mentions = crate::forum::extract_mentions(&input.body);
        for m in &mentions {
            tx.execute(
                "INSERT INTO mentions(target_kind, target_id, target_name, is_assistant, created_at) VALUES('post', ?1, ?2, ?3, ?4)",
                params![post_id, m.target, if m.is_assistant { 1 } else { 0 }, now],
            )?;
        }
        tx.commit()?;
        let topic = Topic {
            id: topic_id,
            channel_id: input.channel_id,
            title: input.title.trim().to_string(),
            created_by: actor.to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
            closed: false,
        };
        let post = Post {
            id: post_id,
            topic_id,
            author: actor.to_string(),
            body: input.body.clone(),
            created_at: now.clone(),
            updated_at: now,
        };
        Ok((topic, post, mentions))
    }

    pub fn list_posts(&self, topic_id: Id) -> Result<Vec<Post>, ForumStoreError> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, topic_id, author, body, created_at, updated_at FROM posts WHERE topic_id=?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![topic_id], |r| {
            Ok(Post {
                id: r.get(0)?,
                topic_id: r.get(1)?,
                author: r.get(2)?,
                body: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn create_post(
        &self,
        actor: &str,
        input: &CreatePost,
    ) -> Result<(Post, Vec<Mention>), ForumStoreError> {
        if input.body.trim().is_empty() {
            return Err(ForumStoreError::InvalidInput("body is required".into()));
        }
        let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Self::now();
        let tx = conn.transaction()?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT id FROM topics WHERE id=?1",
                params![input.topic_id],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(ForumStoreError::NotFound);
        }
        tx.execute(
            "INSERT INTO posts(topic_id, author, body, created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?4)",
            params![input.topic_id, actor, input.body, now],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "UPDATE topics SET updated_at=?1 WHERE id=?2",
            params![now, input.topic_id],
        )?;
        let mentions = crate::forum::extract_mentions(&input.body);
        for m in &mentions {
            tx.execute(
                "INSERT INTO mentions(target_kind, target_id, target_name, is_assistant, created_at) VALUES('post', ?1, ?2, ?3, ?4)",
                params![id, m.target, if m.is_assistant { 1 } else { 0 }, now],
            )?;
        }
        tx.commit()?;
        Ok((
            Post {
                id,
                topic_id: input.topic_id,
                author: actor.to_string(),
                body: input.body.clone(),
                created_at: now.clone(),
                updated_at: now,
            },
            mentions,
        ))
    }

    pub fn list_messages(&self, channel_id: Id) -> Result<Vec<Message>, ForumStoreError> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, thread_id, author, body, created_at FROM messages WHERE channel_id=?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![channel_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                channel_id: r.get(1)?,
                thread_id: r.get(2)?,
                author: r.get(3)?,
                body: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn create_message(
        &self,
        actor: &str,
        input: &CreateMessage,
    ) -> Result<(Message, Vec<Mention>), ForumStoreError> {
        if input.body.trim().is_empty() {
            return Err(ForumStoreError::InvalidInput("body is required".into()));
        }
        let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Self::now();
        let tx = conn.transaction()?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT id FROM channels WHERE id=?1",
                params![input.channel_id],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(ForumStoreError::NotFound);
        }
        tx.execute(
            "INSERT INTO messages(channel_id, thread_id, author, body, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![input.channel_id, input.thread_id, actor, input.body, now],
        )?;
        let id = tx.last_insert_rowid();
        let mentions = crate::forum::extract_mentions(&input.body);
        for m in &mentions {
            tx.execute(
                "INSERT INTO mentions(target_kind, target_id, target_name, is_assistant, created_at) VALUES('message', ?1, ?2, ?3, ?4)",
                params![id, m.target, if m.is_assistant { 1 } else { 0 }, now],
            )?;
        }
        tx.commit()?;
        Ok((
            Message {
                id,
                channel_id: input.channel_id,
                thread_id: input.thread_id,
                author: actor.to_string(),
                body: input.body.clone(),
                created_at: now,
            },
            mentions,
        ))
    }

    pub fn add_reaction(
        &self,
        actor: &str,
        target_kind: &str,
        target_id: Id,
        emoji: &str,
    ) -> Result<Reaction, ForumStoreError> {
        if emoji.trim().is_empty() {
            return Err(ForumStoreError::InvalidInput("emoji is required".into()));
        }
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Self::now();
        conn.execute(
            "INSERT OR IGNORE INTO reactions(target_kind, target_id, actor, emoji, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![target_kind, target_id, actor, emoji.trim(), now],
        )?;
        let id = conn.query_row(
            "SELECT id FROM reactions WHERE target_kind=?1 AND target_id=?2 AND actor=?3 AND emoji=?4",
            params![target_kind, target_id, actor, emoji.trim()],
            |r| r.get(0),
        )?;
        Ok(Reaction {
            id,
            target_kind: target_kind.to_string(),
            target_id,
            actor: actor.to_string(),
            emoji: emoji.trim().to_string(),
            created_at: now,
        })
    }

    pub fn add_membership(
        &self,
        subject: &str,
        space_id: Option<Id>,
        channel_id: Option<Id>,
        role: &str,
    ) -> Result<Membership, ForumStoreError> {
        if subject.trim().is_empty()
            || role.trim().is_empty()
            || (space_id.is_none() && channel_id.is_none())
        {
            return Err(ForumStoreError::InvalidInput(
                "subject, role, and scope are required".into(),
            ));
        }
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = Self::now();
        conn.execute(
            "INSERT INTO memberships(subject, space_id, channel_id, role, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![subject.trim(), space_id, channel_id, role.trim(), now],
        )?;
        Ok(Membership {
            subject: subject.trim().to_string(),
            space_id,
            channel_id,
            role: role.trim().to_string(),
            created_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> ForumDb {
        ForumDb::open_in_memory().unwrap()
    }

    #[test]
    fn creates_space_and_lists_it() {
        let db = db();
        let s = db
            .create_space(
                "alice",
                &CreateSpace {
                    slug: "g".into(),
                    name: "General".into(),
                    description: "".into(),
                },
            )
            .unwrap();
        assert_eq!(s.slug, "g");
        let list = db.list_spaces().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "General");
    }

    #[test]
    fn topic_creation_stores_post_and_mentions() {
        let db = db();
        let ch = db
            .create_channel(
                "alice",
                &CreateChannel {
                    space_id: None,
                    slug: "dev".into(),
                    name: "Dev".into(),
                    kind: ChannelKind::Topic,
                },
            )
            .unwrap();
        let (topic, post, mentions) = db
            .create_topic(
                "alice",
                &CreateTopic {
                    channel_id: ch.id,
                    title: "Hi".into(),
                    body: "Hey @LosOS and @bob".into(),
                },
            )
            .unwrap();
        assert_eq!(topic.title, "Hi");
        assert_eq!(post.topic_id, topic.id);
        assert!(mentions.iter().any(|m| m.is_assistant));
        let posts = db.list_posts(topic.id).unwrap();
        assert_eq!(posts.len(), 1);
    }

    #[test]
    fn message_rejects_missing_channel() {
        let db = db();
        let err = db
            .create_message(
                "alice",
                &CreateMessage {
                    channel_id: 999,
                    thread_id: None,
                    body: "hi".into(),
                },
            )
            .unwrap_err();
        assert!(matches!(err, ForumStoreError::NotFound));
    }

    #[test]
    fn lists_channels() {
        let db = db();
        db.create_channel(
            "alice",
            &CreateChannel {
                space_id: None,
                slug: "dev".into(),
                name: "Dev".into(),
                kind: ChannelKind::Channel,
            },
        )
        .unwrap();
        let list = db.list_channels(None).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, ChannelKind::Channel);
    }
}
