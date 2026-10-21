//! The cloud index: one SQLite file with an FTS5 table of the text of the
//! owner's files, and a table of what each file looked like when it was read.
//!
//! [`Index::sync`] is incremental: it walks the tree, compares each file's
//! size and modification time with what it recorded, and reads only what
//! changed. A file that has no text is recorded too, so it is not opened
//! again until it changes. Each batch of files is one transaction, so a
//! search sees progress and an interrupted pass loses at most one batch.

use super::{split_marked, Answer, CloudHit};
use anyhow::Context;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Bumped when the tables change; an index of another version is rebuilt.
const SCHEMA: i64 = 1;
/// Files read per transaction.
const BATCH: usize = 32;
/// Words of context `snippet()` keeps around a match.
const SNIPPET_TOKENS: i64 = 14;

pub struct Index {
    conn: Connection,
}

/// What a pass did, for the journal.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub seen: usize,
    pub read: usize,
    pub with_text: usize,
    pub removed: usize,
}

impl Index {
    /// Open the index for writing, creating it, or rebuilding one of another
    /// schema version.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        // WAL lets a search read while a pass writes.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != SCHEMA {
            conn.execute_batch(
                "DROP TABLE IF EXISTS files;
                 DROP TABLE IF EXISTS body;
                 DROP TABLE IF EXISTS meta;",
            )?;
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                 path  TEXT PRIMARY KEY,
                 mtime INTEGER NOT NULL,
                 size  INTEGER NOT NULL,
                 docid INTEGER
             ) WITHOUT ROWID;
             CREATE VIRTUAL TABLE IF NOT EXISTS body USING fts5(
                 text,
                 tokenize = 'unicode61 remove_diacritics 2'
             );
             CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        conn.pragma_update(None, "user_version", SCHEMA)?;
        Ok(Self { conn })
    }

    /// Open an existing index to search it; `None` when there is none yet.
    pub fn open_existing(path: &Path) -> anyhow::Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("opening {}", path.display()))?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        Ok((version == SCHEMA).then_some(Self { conn }))
    }

    /// Bring the index up to date with the files under `root`. `text` reads
    /// one file (its path and size) and says what text it has.
    pub fn sync(
        &mut self,
        root: &Path,
        text: &dyn Fn(&Path, u64) -> Option<String>,
    ) -> anyhow::Result<Stats> {
        let mut known: HashMap<String, (i64, i64)> = HashMap::new();
        {
            let mut stmt = self.conn.prepare("SELECT path, mtime, size FROM files")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?;
            for row in rows {
                let (path, stamp) = row?;
                known.insert(path, stamp);
            }
        }

        let mut stats = Stats::default();
        let mut seen: HashSet<String> = HashSet::new();
        let mut changed: Vec<(String, i64, i64)> = Vec::new();
        for entry in walkdir::WalkDir::new(root).follow_links(false).min_depth(1) {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            let Some(rel) = rel.to_str() else { continue };
            // Half-finished uploads.
            if rel.ends_with(".part") {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX));
            let size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
            stats.seen += 1;
            seen.insert(rel.to_string());
            if known.get(rel) != Some(&(mtime, size)) {
                changed.push((rel.to_string(), mtime, size));
            }
        }

        for batch in changed.chunks(BATCH) {
            // Read outside the transaction, so a slow PDF holds no lock.
            let read: Vec<_> = batch
                .iter()
                .map(|(rel, mtime, size)| {
                    let body = text(&root.join(rel), u64::try_from(*size).unwrap_or(0));
                    (rel, *mtime, *size, body)
                })
                .collect();
            let tx = self.conn.transaction()?;
            for (rel, mtime, size, body) in read {
                drop_body(&tx, rel)?;
                let docid = match body {
                    Some(body) => {
                        tx.execute("INSERT INTO body (text) VALUES (?1)", params![body])?;
                        stats.with_text += 1;
                        Some(tx.last_insert_rowid())
                    }
                    None => None,
                };
                tx.execute(
                    "INSERT OR REPLACE INTO files (path, mtime, size, docid) VALUES (?1, ?2, ?3, ?4)",
                    params![rel, mtime, size, docid],
                )?;
                stats.read += 1;
            }
            tx.commit()?;
        }

        let gone: Vec<&String> = known.keys().filter(|p| !seen.contains(*p)).collect();
        if !gone.is_empty() {
            let tx = self.conn.transaction()?;
            for rel in gone {
                drop_body(&tx, rel)?;
                tx.execute("DELETE FROM files WHERE path = ?1", params![rel])?;
                stats.removed += 1;
            }
            tx.commit()?;
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('complete', '1')",
            [],
        )?;
        Ok(stats)
    }

    /// The files whose text has every one of `terms`, as word prefixes,
    /// best match first.
    pub fn search(&self, terms: &[String], limit: usize) -> anyhow::Result<Answer<CloudHit>> {
        let partial = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'complete'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .is_none();
        let Some(query) = fts_query(terms) else {
            return Ok(Answer {
                hits: Vec::new(),
                partial,
            });
        };
        let mut stmt = self.conn.prepare_cached(
            "SELECT f.path, snippet(body, 0, char(2), char(3), '…', ?3)
             FROM body JOIN files f ON f.docid = body.rowid
             WHERE body MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let hits = stmt
            .query_map(
                params![query, i64::try_from(limit).unwrap_or(20), SNIPPET_TOKENS],
                |r| {
                    Ok(CloudHit {
                        path: r.get(0)?,
                        snippet: split_marked(&squash(&r.get::<_, String>(1)?)),
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Answer { hits, partial })
    }
}

fn drop_body(tx: &rusqlite::Transaction<'_>, rel: &str) -> rusqlite::Result<()> {
    let old: Option<Option<i64>> = tx
        .query_row(
            "SELECT docid FROM files WHERE path = ?1",
            params![rel],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(Some(docid)) = old {
        tx.execute("DELETE FROM body WHERE rowid = ?1", params![docid])?;
    }
    Ok(())
}

/// The FTS5 query for `terms`: each one a quoted prefix, all required.
/// Quoting makes every character literal, so nothing typed is FTS5 syntax.
fn fts_query(terms: &[String]) -> Option<String> {
    let parts: Vec<String> = terms
        .iter()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" AND "))
}

/// Line breaks and runs of spaces in a snippet as one space.
fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(path: &Path, _size: u64) -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        (!text.is_empty()).then_some(text)
    }

    fn terms(q: &str) -> Vec<String> {
        crate::textsearch::terms(q)
    }

    #[test]
    fn finds_words_by_prefix_without_accents() {
        let dir = assert_fs::TempDir::new().unwrap();
        let files = dir.path().join("files");
        std::fs::create_dir_all(files.join("Documents")).unwrap();
        std::fs::write(
            files.join("Documents/tax.txt"),
            "Daňové priznanie for 2025\nnothing else",
        )
        .unwrap();
        std::fs::write(files.join("notes.md"), "shopping list: milk").unwrap();
        std::fs::write(files.join("empty.txt"), "").unwrap();

        let mut index = Index::open(&dir.path().join("index.sqlite")).unwrap();
        let stats = index.sync(&files, &read).unwrap();
        assert_eq!(
            stats,
            Stats {
                seen: 3,
                read: 3,
                with_text: 2,
                removed: 0
            }
        );

        let answer = index.search(&terms("danove prizn"), 10).unwrap();
        assert!(!answer.partial);
        assert_eq!(answer.hits.len(), 1);
        assert_eq!(answer.hits[0].path, "Documents/tax.txt");
        assert_eq!(answer.hits[0].snippet[1], "Daňové");
        assert!(index
            .search(&terms("milk tax"), 10)
            .unwrap()
            .hits
            .is_empty());
        // FTS5 syntax typed by the owner is just text.
        assert!(index
            .search(&terms("milk\" OR \"x"), 10)
            .unwrap()
            .hits
            .is_empty());
        assert_eq!(index.search(&terms("NEAR(milk"), 10).unwrap().hits.len(), 0);
    }

    #[test]
    fn a_second_pass_reads_only_what_changed() {
        let dir = assert_fs::TempDir::new().unwrap();
        let files = dir.path().join("files");
        std::fs::create_dir_all(&files).unwrap();
        std::fs::write(files.join("a.txt"), "alpha").unwrap();
        std::fs::write(files.join("b.txt"), "bravo").unwrap();
        let db = dir.path().join("index.sqlite");
        let mut index = Index::open(&db).unwrap();
        index.sync(&files, &read).unwrap();

        std::fs::write(files.join("b.txt"), "bravo charlie").unwrap();
        std::fs::remove_file(files.join("a.txt")).unwrap();
        std::fs::write(files.join("c.txt"), "delta").unwrap();
        let stats = index.sync(&files, &read).unwrap();
        assert_eq!(
            stats,
            Stats {
                seen: 2,
                read: 2,
                with_text: 2,
                removed: 1
            }
        );
        assert!(index.search(&terms("alpha"), 10).unwrap().hits.is_empty());
        assert_eq!(
            index.search(&terms("charl"), 10).unwrap().hits[0].path,
            "b.txt"
        );

        let stats = index.sync(&files, &read).unwrap();
        assert_eq!(stats.read, 0, "nothing changed, nothing read");

        // A search opens the same file on its own.
        let reader = Index::open_existing(&db).unwrap().unwrap();
        assert_eq!(
            reader.search(&terms("delta"), 10).unwrap().hits[0].path,
            "c.txt"
        );
    }

    #[test]
    fn no_index_yet_is_none_and_a_first_pass_in_progress_is_partial() {
        let dir = assert_fs::TempDir::new().unwrap();
        let db = dir.path().join("index.sqlite");
        assert!(Index::open_existing(&db).unwrap().is_none());
        let index = Index::open(&db).unwrap();
        assert!(index.search(&terms("x"), 5).unwrap().partial);
    }

    #[test]
    fn fts_queries_quote_every_term() {
        assert_eq!(
            fts_query(&terms("tax \"2025\"")).as_deref(),
            Some("\"tax\"* AND \"\"\"2025\"\"\"*")
        );
        assert_eq!(fts_query(&[]), None);
    }
}
