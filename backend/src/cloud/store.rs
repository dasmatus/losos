//! The file store: plain files on disk, and a SQLite table that gives each
//! of them what a sync client needs and a filesystem does not keep.
//!
//! Every file and folder has a row in `files` with
//!
//!   * a **file id** that never changes while the file lives, across renames
//!     and moves, and is never handed out twice;
//!   * an **ETag** that changes on every write to the file, and on a folder
//!     whenever anything below it changes. The desktop client walks only the
//!     folders whose ETag moved since its last run, so a change that does not
//!     reach the root is a change that never syncs;
//!   * the size (a folder's is the sum below it), the mtime the client set,
//!     and a SHA-1 of the content.
//!
//! The bytes live at `<root>/<owner>/files/<path>`, a tree anyone can read
//! with `ls`: the text search and the backup read it directly. A change made
//! there behind the store's back is picked up by [`Store::scan`].
//!
//! One connection behind a mutex. Writes hold it only for the rename and the
//! rows; the upload itself is streamed to a temporary file first
//! ([`Store::temp_file`]), outside the lock.

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// One file or folder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    #[serde(skip)]
    pub owner: String,
    /// Relative to the owner's root, `/`-separated, no leading slash; the
    /// root itself is the empty string.
    pub path: String,
    pub name: String,
    pub dir: bool,
    pub size: u64,
    /// Seconds since the epoch.
    pub mtime: i64,
    pub etag: String,
    pub mime: String,
    /// `SHA1:<hex>` for files written through the store.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    pub favorite: bool,
}

/// A file or folder in the trash bin.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trashed {
    pub id: i64,
    /// Where it was, relative to the owner's root.
    pub original: String,
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub deleted: i64,
    /// Its name inside `<owner>/trash/`.
    #[serde(skip)]
    pub stored: String,
}

/// An earlier content of a file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// The mtime the content had, which is also its name on disk.
    pub mtime: i64,
    pub size: u64,
    pub file: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not found")]
    NotFound,
    #[error("already exists")]
    Exists,
    /// The parent is missing, or is a file.
    #[error("conflict")]
    Conflict,
    /// If-Match or a similar precondition did not hold.
    #[error("precondition failed")]
    Precondition,
    /// A name or path the store will not hold.
    #[error("bad name")]
    BadName,
    /// The move or copy would put a folder inside itself.
    #[error("loop")]
    Loop,
    #[error("no space left")]
    Full,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// How many earlier contents of a file are kept.
pub const VERSIONS_KEPT: usize = 20;

pub struct Store {
    root: PathBuf,
    db: Mutex<Connection>,
    instance: String,
    etags: AtomicU64,
}

/// What a write did.
#[derive(Debug)]
pub struct Written {
    pub entry: Entry,
    pub created: bool,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

/// A login the store keeps a home for. The same shape nginx accepts from
/// LosOS Login (modules/identity.nix), so a name never becomes a path
/// component it should not be.
pub fn valid_owner(owner: &str) -> bool {
    let mut chars = owner.chars();
    matches!(chars.next(), Some('a'..='z'))
        && owner.len() <= 64
        && chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '.' | '_' | '-'))
}

/// One path component.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 255
        && !name.contains(['/', '\0', '\\'])
        && name.trim() == name
}

/// Normalise a client path: strip slashes at both ends, refuse empty
/// components, `.` and `..`. The root is `""`.
pub fn clean_path(path: &str) -> Result<String> {
    let mut out = Vec::new();
    for part in path.split('/') {
        if part.is_empty() {
            continue;
        }
        if !valid_name(part) {
            return Err(StoreError::BadName);
        }
        out.push(part);
    }
    Ok(out.join("/"))
}

pub fn parent_of(path: &str) -> Option<&str> {
    if path.is_empty() {
        return None;
    }
    Some(path.rsplit_once('/').map(|(p, _)| p).unwrap_or(""))
}

pub fn name_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(_, n)| n).unwrap_or(path)
}

pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

/// `path` is `dir` or below it.
pub fn is_within(path: &str, dir: &str) -> bool {
    dir.is_empty() || path == dir || path.starts_with(&format!("{dir}/"))
}

/// A content type from the name, for the client's icon and the browser.
pub fn mime_for(name: &str, dir: bool) -> String {
    if dir {
        return "httpd/unix-directory".into();
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    let m = match ext.as_deref() {
        Some("txt" | "log") => "text/plain",
        Some("md" | "markdown" | "note") => "text/markdown",
        // LosOS Office's own documents and presentations: MDX
        // (backend/mdx), text the shared editor opens as well.
        Some("paper") => "text/vnd.losos.paper",
        Some("slides") => "text/vnd.losos.slides",
        Some("html" | "htm") => "text/html",
        Some("css") => "text/css",
        Some("csv") => "text/csv",
        Some("js" | "mjs") => "text/javascript",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("gz" | "tgz") => "application/gzip",
        Some("tar") => "application/x-tar",
        Some("7z") => "application/x-7z-compressed",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("heic") => "image/heic",
        Some("heif") => "image/heif",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("tif" | "tiff") => "image/tiff",
        Some("mp3") => "audio/mpeg",
        Some("ogg" | "oga") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("flac") => "audio/flac",
        Some("wav") => "audio/wav",
        Some("m4a") => "audio/mp4",
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        Some("odt") => "application/vnd.oasis.opendocument.text",
        Some("ods") => "application/vnd.oasis.opendocument.spreadsheet",
        Some("odp") => "application/vnd.oasis.opendocument.presentation",
        Some("odg") => "application/vnd.oasis.opendocument.graphics",
        Some("doc") => "application/msword",
        Some("xls") => "application/vnd.ms-excel",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("ics") => "text/calendar",
        Some("vcf") => "text/vcard",
        _ => "application/octet-stream",
    };
    m.into()
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS files (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  owner    TEXT    NOT NULL,
  path     TEXT    NOT NULL,
  parent   INTEGER,
  name     TEXT    NOT NULL,
  dir      INTEGER NOT NULL,
  size     INTEGER NOT NULL,
  mtime    INTEGER NOT NULL,
  etag     TEXT    NOT NULL,
  mime     TEXT    NOT NULL,
  checksum TEXT,
  created  INTEGER NOT NULL,
  UNIQUE (owner, path)
);
CREATE INDEX IF NOT EXISTS files_parent ON files (parent);
CREATE TABLE IF NOT EXISTS favorites (
  owner TEXT    NOT NULL,
  file  INTEGER NOT NULL,
  PRIMARY KEY (owner, file)
);
CREATE TABLE IF NOT EXISTS trash (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  owner    TEXT    NOT NULL,
  original TEXT    NOT NULL,
  name     TEXT    NOT NULL,
  dir      INTEGER NOT NULL,
  size     INTEGER NOT NULL,
  deleted  INTEGER NOT NULL,
  stored   TEXT    NOT NULL UNIQUE
);
CREATE INDEX IF NOT EXISTS trash_owner ON trash (owner);
"#;

const COLUMNS: &str =
    "f.id, f.owner, f.path, f.name, f.dir, f.size, f.mtime, f.etag, f.mime, f.checksum, \
     EXISTS(SELECT 1 FROM favorites v WHERE v.owner = f.owner AND v.file = f.id)";

fn row_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: row.get(0)?,
        owner: row.get(1)?,
        path: row.get(2)?,
        name: row.get(3)?,
        dir: row.get::<_, i64>(4)? != 0,
        size: row.get::<_, i64>(5)?.max(0) as u64,
        mtime: row.get(6)?,
        etag: row.get(7)?,
        mime: row.get(8)?,
        checksum: row.get(9)?,
        favorite: row.get::<_, i64>(10)? != 0,
    })
}

fn row_trashed(row: &rusqlite::Row<'_>) -> rusqlite::Result<Trashed> {
    Ok(Trashed {
        id: row.get(0)?,
        original: row.get(1)?,
        name: row.get(2)?,
        dir: row.get::<_, i64>(3)? != 0,
        size: row.get::<_, i64>(4)?.max(0) as u64,
        deleted: row.get(5)?,
        stored: row.get(6)?,
    })
}

fn get(tx: &Connection, owner: &str, path: &str) -> Result<Option<Entry>> {
    Ok(tx
        .query_row(
            &format!("SELECT {COLUMNS} FROM files f WHERE f.owner = ?1 AND f.path = ?2"),
            params![owner, path],
            row_entry,
        )
        .optional()?)
}

fn map_io(e: std::io::Error) -> StoreError {
    match e.kind() {
        std::io::ErrorKind::NotFound => StoreError::NotFound,
        std::io::ErrorKind::AlreadyExists => StoreError::Exists,
        std::io::ErrorKind::StorageFull => StoreError::Full,
        _ => StoreError::Io(e),
    }
}

impl Store {
    /// Open (or create) the store under `root`, with its database at
    /// `<root>/cloud.db` unless `db` names another file.
    pub fn open(root: &Path) -> Result<Store> {
        Self::open_waiting(root, std::time::Duration::from_secs(10))
    }

    /// [`Store::open`] for a background job, which waits up to `wait` for
    /// the server's writes (its start-up after a boot can hold the database
    /// for a while on a slow box) rather than failing on a locked database.
    pub fn open_waiting(root: &Path, wait: std::time::Duration) -> Result<Store> {
        fs::create_dir_all(root)?;
        let conn = Connection::open(root.join("cloud.db"))?;
        // First, so the pragmas below wait too.
        conn.busy_timeout(wait)?;
        Self::with_connection(root, conn)
    }

    fn with_connection(root: &Path, conn: Connection) -> Result<Store> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        let instance = match conn
            .query_row("SELECT value FROM meta WHERE key = 'instance'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
        {
            Some(v) => v,
            None => {
                let v = format!("oc{}", crate::cloud::util::random_token(10).to_lowercase());
                conn.execute(
                    "INSERT INTO meta (key, value) VALUES ('instance', ?1)",
                    params![v],
                )?;
                v
            }
        };
        Ok(Store {
            root: root.to_path_buf(),
            db: Mutex::new(conn),
            instance,
            etags: AtomicU64::new(now_micros()),
        })
    }

    /// The other tables of the cloud live in the same database file; the
    /// modules that own them run their schema through this.
    pub fn with_db<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut db)
    }

    /// The instance id Nextcloud clients see at the end of every `oc:id`.
    pub fn instance(&self) -> &str {
        &self.instance
    }

    /// `oc:id`: the file id padded to eight digits, then the instance.
    pub fn oc_id(&self, id: i64) -> String {
        format!("{id:08}{}", self.instance)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn home(&self, owner: &str) -> PathBuf {
        self.root.join(owner)
    }

    /// Where an entry's bytes are.
    pub fn disk_path(&self, owner: &str, path: &str) -> PathBuf {
        let mut p = self.home(owner).join("files");
        if !path.is_empty() {
            p.push(path);
        }
        p
    }

    fn trash_dir(&self, owner: &str) -> PathBuf {
        self.home(owner).join("trash")
    }

    fn versions_dir(&self, owner: &str, id: i64) -> PathBuf {
        self.home(owner).join("versions").join(id.to_string())
    }

    /// The owner's folder for upload chunks, made on first use.
    pub fn uploads_dir(&self, owner: &str) -> Result<PathBuf> {
        let p = self.home(owner).join("uploads");
        fs::create_dir_all(&p)?;
        Ok(p)
    }

    fn new_etag(&self) -> String {
        format!("{:x}", self.etags.fetch_add(1, Ordering::Relaxed))
    }

    /// A temporary file on the same filesystem as the owner's files, so the
    /// commit is a rename. Removed on drop unless committed.
    pub fn temp_file(&self, owner: &str) -> Result<tempfile::NamedTempFile> {
        let dir = self.uploads_dir(owner)?;
        Ok(tempfile::Builder::new().prefix(".part-").tempfile_in(dir)?)
    }

    /// Make sure the owner has a home and a row for its root. Returns the
    /// root entry.
    pub fn ensure_home(&self, owner: &str) -> Result<Entry> {
        if !valid_owner(owner) {
            return Err(StoreError::BadName);
        }
        let files = self.disk_path(owner, "");
        fs::create_dir_all(&files)?;
        fs::create_dir_all(self.trash_dir(owner))?;
        let mut db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(e) = get(&db, owner, "")? {
            return Ok(e);
        }
        let tx = db.transaction()?;
        let t = now();
        tx.execute(
            "INSERT INTO files (owner, path, parent, name, dir, size, mtime, etag, mime, checksum, created)
             VALUES (?1, '', NULL, '', 1, 0, ?2, ?3, 'httpd/unix-directory', NULL, ?2)",
            params![owner, t, self.new_etag()],
        )?;
        let e = get(&tx, owner, "")?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        drop(db);
        // A home that already held files (a migration, or a disk restored
        // from a backup) gets its rows now.
        if fs::read_dir(&files)?.next().is_some() {
            self.scan(owner)?;
            return self.stat(owner, "");
        }
        Ok(e)
    }

    pub fn stat(&self, owner: &str, path: &str) -> Result<Entry> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        get(&db, owner, path)?.ok_or(StoreError::NotFound)
    }

    pub fn by_id(&self, owner: &str, id: i64) -> Result<Entry> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            &format!("SELECT {COLUMNS} FROM files f WHERE f.owner = ?1 AND f.id = ?2"),
            params![owner, id],
            row_entry,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
    }

    /// An entry by id alone, for the modules that resolve a file before
    /// they know whose it is (shares, office sessions).
    pub fn by_id_any(&self, id: i64) -> Result<Entry> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            &format!("SELECT {COLUMNS} FROM files f WHERE f.id = ?1"),
            params![id],
            row_entry,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
    }

    /// The children of a folder, folders first, then by name.
    pub fn list(&self, owner: &str, path: &str) -> Result<Vec<Entry>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let dir = get(&db, owner, path)?.ok_or(StoreError::NotFound)?;
        if !dir.dir {
            return Err(StoreError::Conflict);
        }
        let mut stmt = db.prepare(&format!(
            "SELECT {COLUMNS} FROM files f WHERE f.parent = ?1 ORDER BY f.dir DESC, f.name COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map(params![dir.id], row_entry)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Everything below a folder, depth first, the folder itself excluded.
    pub fn walk(&self, owner: &str, path: &str) -> Result<Vec<Entry>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(&format!(
            "SELECT {COLUMNS} FROM files f WHERE f.owner = ?1 AND (?2 = '' OR substr(f.path, 1, length(?2) + 1) = ?2 || '/') AND f.path != ?2 ORDER BY f.path"
        ))?;
        let rows = stmt.query_map(params![owner, path], row_entry)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Files whose name contains `q`, newest first.
    pub fn search(&self, owner: &str, q: &str, limit: usize) -> Result<Vec<Entry>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let pattern = format!(
            "%{}%",
            q.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut stmt = db.prepare(&format!(
            "SELECT {COLUMNS} FROM files f WHERE f.owner = ?1 AND f.path != '' AND f.name LIKE ?2 ESCAPE '\\' ORDER BY f.mtime DESC LIMIT ?3"
        ))?;
        let rows = stmt.query_map(params![owner, pattern, limit as i64], row_entry)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Files changed most recently, folders left out.
    pub fn recent(&self, owner: &str, limit: usize) -> Result<Vec<Entry>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(&format!(
            "SELECT {COLUMNS} FROM files f WHERE f.owner = ?1 AND f.dir = 0 ORDER BY f.mtime DESC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![owner, limit as i64], row_entry)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Files of one kind (`image/`, `video/`), newest first.
    pub fn by_mime_prefix(
        &self,
        owner: &str,
        prefixes: &[&str],
        limit: usize,
    ) -> Result<Vec<Entry>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut out = Vec::new();
        for prefix in prefixes {
            let mut stmt = db.prepare(&format!(
                "SELECT {COLUMNS} FROM files f WHERE f.owner = ?1 AND f.dir = 0 AND substr(f.mime, 1, length(?2)) = ?2 ORDER BY f.mtime DESC LIMIT ?3"
            ))?;
            let rows = stmt.query_map(params![owner, prefix, limit as i64], row_entry)?;
            for r in rows {
                out.push(r?);
            }
        }
        out.sort_by_key(|e| std::cmp::Reverse(e.mtime));
        out.truncate(limit);
        Ok(out)
    }

    pub fn favorites(&self, owner: &str) -> Result<Vec<Entry>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(&format!(
            "SELECT {COLUMNS} FROM files f JOIN favorites v ON v.file = f.id AND v.owner = f.owner WHERE f.owner = ?1 ORDER BY f.dir DESC, f.name COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map(params![owner], row_entry)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_favorite(&self, owner: &str, path: &str, on: bool) -> Result<Entry> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let e = get(&db, owner, path)?.ok_or(StoreError::NotFound)?;
        if on {
            db.execute(
                "INSERT OR IGNORE INTO favorites (owner, file) VALUES (?1, ?2)",
                params![owner, e.id],
            )?;
        } else {
            db.execute(
                "DELETE FROM favorites WHERE owner = ?1 AND file = ?2",
                params![owner, e.id],
            )?;
        }
        get(&db, owner, path)?.ok_or(StoreError::NotFound)
    }

    /// New ETag on the entry at `path` and every folder above it, folder
    /// sizes recomputed from their children, folder mtimes set to `t`.
    fn propagate(&self, tx: &Transaction<'_>, owner: &str, path: &str, t: i64) -> Result<()> {
        let mut cur = Some(path.to_owned());
        while let Some(p) = cur {
            let row: Option<(i64, i64)> = tx
                .query_row(
                    "SELECT id, dir FROM files WHERE owner = ?1 AND path = ?2",
                    params![owner, p],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((id, dir)) = row {
                if dir != 0 {
                    tx.execute(
                        "UPDATE files SET etag = ?1, mtime = MAX(mtime, ?2),
                           size = (SELECT COALESCE(SUM(c.size), 0) FROM files c WHERE c.parent = ?3)
                         WHERE id = ?3",
                        params![self.new_etag(), t, id],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE files SET etag = ?1 WHERE id = ?2",
                        params![self.new_etag(), id],
                    )?;
                }
            }
            cur = parent_of(&p).map(str::to_owned);
        }
        Ok(())
    }

    fn parent_row(tx: &Connection, owner: &str, path: &str) -> Result<Entry> {
        let parent = parent_of(path).ok_or(StoreError::Conflict)?;
        match get(tx, owner, parent)? {
            Some(p) if p.dir => Ok(p),
            _ => Err(StoreError::Conflict),
        }
    }

    pub fn mkdir(&self, owner: &str, path: &str) -> Result<Entry> {
        if path.is_empty() {
            return Err(StoreError::Exists);
        }
        let mut db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let tx = db.transaction()?;
        if get(&tx, owner, path)?.is_some() {
            return Err(StoreError::Exists);
        }
        let parent = Self::parent_row(&tx, owner, path)?;
        fs::create_dir(self.disk_path(owner, path)).map_err(map_io)?;
        let t = now();
        tx.execute(
            "INSERT INTO files (owner, path, parent, name, dir, size, mtime, etag, mime, checksum, created)
             VALUES (?1, ?2, ?3, ?4, 1, 0, ?5, ?6, 'httpd/unix-directory', NULL, ?5)",
            params![owner, path, parent.id, name_of(path), t, self.new_etag()],
        )?;
        self.propagate(&tx, owner, path, t)?;
        let e = get(&tx, owner, path)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(e)
    }

    /// Make every missing folder on the way to `path` (a folder itself).
    pub fn mkdir_all(&self, owner: &str, path: &str) -> Result<Entry> {
        let mut cur = String::new();
        for part in path.split('/').filter(|p| !p.is_empty()) {
            cur = join(&cur, part);
            match self.mkdir(owner, &cur) {
                Ok(_) | Err(StoreError::Exists) => {}
                Err(e) => return Err(e),
            }
        }
        let e = self.stat(owner, path)?;
        if !e.dir {
            return Err(StoreError::Conflict);
        }
        Ok(e)
    }

    /// Put the content in `temp` at `path`, replacing a file there.
    ///
    /// `if_match` is the ETag the client last saw (refused with
    /// [`StoreError::Precondition`] if it moved), `if_none_match_any` is
    /// `If-None-Match: *` (refused if anything is there). `mtime` is the
    /// client's own, else now. The old content becomes a version.
    #[allow(clippy::too_many_arguments)]
    pub fn commit_file(
        &self,
        owner: &str,
        path: &str,
        temp: tempfile::NamedTempFile,
        mtime: Option<i64>,
        if_match: Option<&str>,
        if_none_match_any: bool,
        checksum: Option<String>,
    ) -> Result<Written> {
        if path.is_empty() {
            return Err(StoreError::Conflict);
        }
        let size = temp.as_file().metadata()?.len();
        let t = now();
        let mtime = mtime.unwrap_or(t);
        let mut db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let tx = db.transaction()?;
        let existing = get(&tx, owner, path)?;
        if let Some(tag) = if_match {
            match &existing {
                Some(e) if tag == "*" || tag == e.etag => {}
                _ => return Err(StoreError::Precondition),
            }
        }
        if if_none_match_any && existing.is_some() {
            return Err(StoreError::Precondition);
        }
        if existing.as_ref().is_some_and(|e| e.dir) {
            return Err(StoreError::Conflict);
        }
        let parent = Self::parent_row(&tx, owner, path)?;
        let disk = self.disk_path(owner, path);
        if let Some(old) = &existing {
            self.keep_version(owner, old);
        }
        let file = temp.persist(&disk).map_err(|e| map_io(e.error))?;
        let when = UNIX_EPOCH + std::time::Duration::from_secs(mtime.max(0) as u64);
        let _ = file.set_modified(when);
        drop(file);
        let mime = mime_for(name_of(path), false);
        let created = existing.is_none();
        match existing {
            Some(old) => {
                tx.execute(
                    "UPDATE files SET size = ?1, mtime = ?2, mime = ?3, checksum = ?4 WHERE id = ?5",
                    params![size as i64, mtime, mime, checksum, old.id],
                )?;
            }
            None => {
                tx.execute(
                    "INSERT INTO files (owner, path, parent, name, dir, size, mtime, etag, mime, checksum, created)
                     VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![owner, path, parent.id, name_of(path), size as i64, mtime, self.new_etag(), mime, checksum, t],
                )?;
            }
        }
        self.propagate(&tx, owner, path, t)?;
        let entry = get(&tx, owner, path)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(Written { entry, created })
    }

    /// Set a file's mtime, as a client asks after a move or through
    /// PROPPATCH. The ETag moves with it, so the change syncs.
    pub fn set_mtime(&self, owner: &str, path: &str, mtime: i64) -> Result<Entry> {
        let mut db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let tx = db.transaction()?;
        let e = get(&tx, owner, path)?.ok_or(StoreError::NotFound)?;
        if e.mtime == mtime {
            return Ok(e);
        }
        let when = UNIX_EPOCH + std::time::Duration::from_secs(mtime.max(0) as u64);
        if let Ok(f) = fs::File::options()
            .write(true)
            .open(self.disk_path(owner, path))
        {
            let _ = f.set_modified(when);
        } else if e.dir {
            if let Ok(f) = fs::File::open(self.disk_path(owner, path)) {
                let _ = f.set_modified(when);
            }
        }
        tx.execute(
            "UPDATE files SET mtime = ?1 WHERE id = ?2",
            params![mtime, e.id],
        )?;
        self.propagate(&tx, owner, path, now())?;
        // The folder's own mtime is what the client asked for, not now.
        tx.execute(
            "UPDATE files SET mtime = ?1 WHERE id = ?2",
            params![mtime, e.id],
        )?;
        let e = get(&tx, owner, path)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(e)
    }

    /// Write a small file from memory, through the same commit.
    pub fn write_bytes(&self, owner: &str, path: &str, bytes: &[u8]) -> Result<Written> {
        let mut temp = self.temp_file(owner)?;
        temp.write_all(bytes)?;
        let sum = crate::cloud::util::sha1_hex(bytes);
        self.commit_file(
            owner,
            path,
            temp,
            None,
            None,
            false,
            Some(format!("SHA1:{sum}")),
        )
    }

    pub fn read_bytes(&self, owner: &str, path: &str) -> Result<Vec<u8>> {
        let e = self.stat(owner, path)?;
        if e.dir {
            return Err(StoreError::Conflict);
        }
        let mut buf = Vec::with_capacity(e.size as usize);
        fs::File::open(self.disk_path(owner, path))
            .map_err(map_io)?
            .read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn keep_version(&self, owner: &str, old: &Entry) {
        let dir = self.versions_dir(owner, old.id);
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        let mut name = old.mtime;
        while dir.join(name.to_string()).exists() {
            name += 1;
        }
        // A hard link keeps the old bytes at no cost: the new content
        // arrives by rename, which replaces the name, not the inode.
        let from = self.disk_path(owner, &old.path);
        if fs::hard_link(&from, dir.join(name.to_string())).is_err() {
            let _ = fs::copy(&from, dir.join(name.to_string()));
        }
        // Only the newest are kept.
        if let Ok(rd) = fs::read_dir(&dir) {
            let mut all: Vec<i64> = rd
                .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
                .collect();
            all.sort_unstable();
            if all.len() > VERSIONS_KEPT {
                for v in &all[..all.len() - VERSIONS_KEPT] {
                    let _ = fs::remove_file(dir.join(v.to_string()));
                }
            }
        }
    }

    /// Earlier contents of the file at `path`, newest first.
    pub fn versions(&self, owner: &str, path: &str) -> Result<Vec<Version>> {
        let e = self.stat(owner, path)?;
        self.versions_of(owner, e.id)
    }

    pub fn versions_of(&self, owner: &str, id: i64) -> Result<Vec<Version>> {
        let dir = self.versions_dir(owner, id);
        let mut out = Vec::new();
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let Some(mtime) = e.file_name().to_str().and_then(|n| n.parse().ok()) else {
                    continue;
                };
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                out.push(Version {
                    mtime,
                    size,
                    file: id,
                });
            }
        }
        out.sort_by_key(|e| std::cmp::Reverse(e.mtime));
        Ok(out)
    }

    pub fn version_path(&self, owner: &str, id: i64, mtime: i64) -> Result<PathBuf> {
        let p = self.versions_dir(owner, id).join(mtime.to_string());
        if p.is_file() {
            Ok(p)
        } else {
            Err(StoreError::NotFound)
        }
    }

    /// Put an earlier content back; the current one becomes a version.
    pub fn restore_version(&self, owner: &str, id: i64, mtime: i64) -> Result<Entry> {
        let e = self.by_id(owner, id)?;
        let from = self.version_path(owner, id, mtime)?;
        let mut temp = self.temp_file(owner)?;
        let bytes = fs::read(&from)?;
        temp.write_all(&bytes)?;
        let sum = crate::cloud::util::sha1_hex(&bytes);
        let w = self.commit_file(
            owner,
            &e.path,
            temp,
            Some(mtime),
            None,
            false,
            Some(format!("SHA1:{sum}")),
        )?;
        let _ = fs::remove_file(&from);
        Ok(w.entry)
    }

    /// Move to the trash bin. The rows go, the bytes move to
    /// `<owner>/trash/<name>.d<time>`, and a `trash` row remembers where
    /// they came from.
    pub fn delete(&self, owner: &str, path: &str, if_match: Option<&str>) -> Result<Trashed> {
        if path.is_empty() {
            return Err(StoreError::Conflict);
        }
        let mut db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let tx = db.transaction()?;
        let e = get(&tx, owner, path)?.ok_or(StoreError::NotFound)?;
        if let Some(tag) = if_match {
            if tag != "*" && tag != e.etag {
                return Err(StoreError::Precondition);
            }
        }
        let t = now();
        let trash = self.trash_dir(owner);
        fs::create_dir_all(&trash)?;
        let mut stored = format!("{}.d{t}", e.name);
        let mut n = 1;
        while trash.join(&stored).exists() {
            stored = format!("{}.d{t}-{n}", e.name);
            n += 1;
        }
        fs::rename(self.disk_path(owner, path), trash.join(&stored)).map_err(map_io)?;
        self.drop_rows(&tx, owner, path)?;
        tx.execute(
            "INSERT INTO trash (owner, original, name, dir, size, deleted, stored) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![owner, path, e.name, e.dir as i64, e.size as i64, t, stored],
        )?;
        let id = tx.last_insert_rowid();
        if let Some(parent) = parent_of(path) {
            self.propagate(&tx, owner, parent, t)?;
        }
        tx.commit()?;
        Ok(Trashed {
            id,
            original: path.to_owned(),
            name: e.name,
            dir: e.dir,
            size: e.size,
            deleted: t,
            stored,
        })
    }

    fn drop_rows(&self, tx: &Transaction<'_>, owner: &str, path: &str) -> Result<()> {
        // Versions of the files that go are kept with the trash only as long
        // as the rows are; a file deleted for good takes its history along.
        let ids: Vec<i64> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM files WHERE owner = ?1 AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
            )?;
            let rows = stmt.query_map(params![owner, path], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for id in &ids {
            tx.execute(
                "DELETE FROM favorites WHERE owner = ?1 AND file = ?2",
                params![owner, id],
            )?;
            let _ = fs::remove_dir_all(self.versions_dir(owner, *id));
        }
        tx.execute(
            "DELETE FROM files WHERE owner = ?1 AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
            params![owner, path],
        )?;
        Ok(())
    }

    /// Take a file or folder from elsewhere on the same disk straight into
    /// the trash bin (a rename), as if it had been deleted from `original`
    /// at `deleted`. The Nextcloud import moves Nextcloud's trash in so.
    pub fn adopt_trashed(
        &self,
        owner: &str,
        from: &Path,
        original: &str,
        deleted: i64,
    ) -> Result<Trashed> {
        let name = original.rsplit('/').next().unwrap_or(original).to_owned();
        if !valid_name(&name) {
            return Err(StoreError::BadName);
        }
        let meta = fs::symlink_metadata(from).map_err(map_io)?;
        if meta.file_type().is_symlink() {
            return Err(StoreError::BadName);
        }
        let size = if meta.is_dir() {
            walkdir_size(from)
        } else {
            meta.len()
        };
        let trash = self.trash_dir(owner);
        fs::create_dir_all(&trash)?;
        let mut stored = format!("{name}.d{deleted}");
        let mut n = 1;
        while trash.join(&stored).exists() {
            stored = format!("{name}.d{deleted}-{n}");
            n += 1;
        }
        fs::rename(from, trash.join(&stored)).map_err(map_io)?;
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "INSERT INTO trash (owner, original, name, dir, size, deleted, stored) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![owner, original, name, meta.is_dir() as i64, size as i64, deleted, stored],
        )?;
        Ok(Trashed {
            id: db.last_insert_rowid(),
            original: original.to_owned(),
            name,
            dir: meta.is_dir(),
            size,
            deleted,
            stored,
        })
    }

    pub fn trash(&self, owner: &str) -> Result<Vec<Trashed>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(
            "SELECT id, original, name, dir, size, deleted, stored FROM trash WHERE owner = ?1 ORDER BY deleted DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![owner], row_trashed)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn trashed(&self, owner: &str, id: i64) -> Result<Trashed> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            "SELECT id, original, name, dir, size, deleted, stored FROM trash WHERE owner = ?1 AND id = ?2",
            params![owner, id],
            row_trashed,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
    }

    pub fn trashed_by_stored(&self, owner: &str, stored: &str) -> Result<Trashed> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            "SELECT id, original, name, dir, size, deleted, stored FROM trash WHERE owner = ?1 AND stored = ?2",
            params![owner, stored],
            row_trashed,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
    }

    pub fn trash_disk_path(&self, owner: &str, t: &Trashed) -> PathBuf {
        self.trash_dir(owner).join(&t.stored)
    }

    /// Put a trashed entry back where it was, or at `to` if given. A missing
    /// parent folder is made again; a name taken in the meantime gets
    /// " (restored)" added.
    pub fn restore(&self, owner: &str, id: i64, to: Option<&str>) -> Result<Entry> {
        let t = self.trashed(owner, id)?;
        let mut target = match to {
            Some(p) => p.to_owned(),
            None => t.original.clone(),
        };
        if let Some(parent) = parent_of(&target) {
            if !parent.is_empty() {
                self.mkdir_all(owner, parent)?;
            }
        }
        if to.is_none() {
            let mut n = 1;
            while self.stat(owner, &target).is_ok() {
                let (stem, ext) = match t.name.rsplit_once('.') {
                    Some((s, e)) if !s.is_empty() && !t.dir => (s.to_owned(), format!(".{e}")),
                    _ => (t.name.clone(), String::new()),
                };
                let suffix = if n == 1 {
                    " (restored)".to_owned()
                } else {
                    format!(" (restored {n})")
                };
                target = join(
                    parent_of(&t.original).unwrap_or(""),
                    &format!("{stem}{suffix}{ext}"),
                );
                n += 1;
            }
        } else if self.stat(owner, &target).is_ok() {
            return Err(StoreError::Exists);
        }
        fs::rename(
            self.trash_disk_path(owner, &t),
            self.disk_path(owner, &target),
        )
        .map_err(map_io)?;
        {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            db.execute(
                "DELETE FROM trash WHERE owner = ?1 AND id = ?2",
                params![owner, id],
            )?;
        }
        self.scan_path(owner, &target)?;
        self.stat(owner, &target)
    }

    /// Delete a trashed entry for good.
    pub fn purge(&self, owner: &str, id: i64) -> Result<()> {
        let t = self.trashed(owner, id)?;
        let p = self.trash_disk_path(owner, &t);
        if t.dir {
            let _ = fs::remove_dir_all(&p);
        } else {
            let _ = fs::remove_file(&p);
        }
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.execute(
            "DELETE FROM trash WHERE owner = ?1 AND id = ?2",
            params![owner, id],
        )?;
        Ok(())
    }

    pub fn empty_trash(&self, owner: &str) -> Result<usize> {
        let all = self.trash(owner)?;
        for t in &all {
            self.purge(owner, t.id)?;
        }
        Ok(all.len())
    }

    /// Trash older than `days` goes for good. Returns how many.
    pub fn expire_trash(&self, owner: &str, days: i64) -> Result<usize> {
        let cutoff = now() - days * 86_400;
        let old: Vec<i64> = self
            .trash(owner)?
            .into_iter()
            .filter(|t| t.deleted < cutoff)
            .map(|t| t.id)
            .collect();
        for id in &old {
            self.purge(owner, *id)?;
        }
        Ok(old.len())
    }

    /// Move or rename. The entry keeps its file id and its ETag; both
    /// parents' chains get new ones. `overwrite` replaces what is at `to`
    /// (into the trash).
    pub fn rename(&self, owner: &str, from: &str, to: &str, overwrite: bool) -> Result<Written> {
        if from.is_empty() || to.is_empty() {
            return Err(StoreError::Conflict);
        }
        if from == to {
            return Ok(Written {
                entry: self.stat(owner, from)?,
                created: false,
            });
        }
        if is_within(to, from) {
            return Err(StoreError::Loop);
        }
        let existed = match self.stat(owner, to) {
            Ok(_) if !overwrite => return Err(StoreError::Precondition),
            Ok(_) => {
                self.delete(owner, to, None)?;
                true
            }
            Err(StoreError::NotFound) => false,
            Err(e) => return Err(e),
        };
        let mut db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let tx = db.transaction()?;
        let e = get(&tx, owner, from)?.ok_or(StoreError::NotFound)?;
        let parent = Self::parent_row(&tx, owner, to)?;
        fs::rename(self.disk_path(owner, from), self.disk_path(owner, to)).map_err(map_io)?;
        tx.execute(
            "UPDATE files SET path = ?1 || substr(path, length(?2) + 1)
             WHERE owner = ?3 AND substr(path, 1, length(?2) + 1) = ?2 || '/'",
            params![to, from, owner],
        )?;
        tx.execute(
            "UPDATE files SET path = ?1, name = ?2, parent = ?3, mime = ?4 WHERE id = ?5",
            params![
                to,
                name_of(to),
                parent.id,
                mime_for(name_of(to), e.dir),
                e.id
            ],
        )?;
        let t = now();
        if let Some(p) = parent_of(from) {
            self.propagate(&tx, owner, p, t)?;
        }
        // The moved entry keeps its own ETag; only the chain above it moves.
        if let Some(p) = parent_of(to) {
            self.propagate(&tx, owner, p, t)?;
        }
        let entry = get(&tx, owner, to)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(Written {
            entry,
            created: !existed,
        })
    }

    /// Copy a file or a whole folder. Copies get new file ids.
    pub fn copy(&self, owner: &str, from: &str, to: &str, overwrite: bool) -> Result<Written> {
        if from.is_empty() || to.is_empty() {
            return Err(StoreError::Conflict);
        }
        if is_within(to, from) {
            return Err(StoreError::Loop);
        }
        let src = self.stat(owner, from)?;
        let existed = match self.stat(owner, to) {
            Ok(_) if !overwrite => return Err(StoreError::Precondition),
            Ok(_) => {
                self.delete(owner, to, None)?;
                true
            }
            Err(StoreError::NotFound) => false,
            Err(e) => return Err(e),
        };
        {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            Self::parent_row(&db, owner, to)?;
        }
        if src.dir {
            self.mkdir(owner, to)?;
            for child in self.walk(owner, from)? {
                let rel = &child.path[from.len() + 1..];
                let dest = join(to, rel);
                if child.dir {
                    self.mkdir(owner, &dest)?;
                } else {
                    self.copy_file(owner, &child, &dest)?;
                }
            }
        } else {
            self.copy_file(owner, &src, to)?;
        }
        Ok(Written {
            entry: self.stat(owner, to)?,
            created: !existed,
        })
    }

    fn copy_file(&self, owner: &str, src: &Entry, to: &str) -> Result<Written> {
        let temp = self.temp_file(owner)?;
        fs::copy(self.disk_path(owner, &src.path), temp.path()).map_err(map_io)?;
        self.commit_file(
            owner,
            to,
            temp,
            Some(src.mtime),
            None,
            false,
            src.checksum.clone(),
        )
    }

    /// Bring the rows of one entry (and everything below it) in line with
    /// the disk: new files get rows, vanished ones lose them, a file whose
    /// size or mtime changed gets a new ETag. Then the chain above it.
    pub fn scan_path(&self, owner: &str, path: &str) -> Result<usize> {
        let disk = self.disk_path(owner, path);
        let mut changed = 0;
        let mut db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let tx = db.transaction()?;
        let t = now();
        changed += self.scan_rec(&tx, owner, path, &disk, t)?;
        if changed > 0 {
            match parent_of(path) {
                Some(p) => self.propagate(&tx, owner, p, t)?,
                None => self.propagate(&tx, owner, "", t)?,
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// The whole home.
    pub fn scan(&self, owner: &str) -> Result<usize> {
        self.scan_path(owner, "")
    }

    fn scan_rec(
        &self,
        tx: &Transaction<'_>,
        owner: &str,
        path: &str,
        disk: &Path,
        t: i64,
    ) -> Result<usize> {
        let meta = match fs::symlink_metadata(disk) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if get(tx, owner, path)?.is_some() && !path.is_empty() {
                    self.drop_rows(tx, owner, path)?;
                    return Ok(1);
                }
                return Ok(0);
            }
            Err(e) => return Err(e.into()),
        };
        // Links are not followed: a link out of the home would hand out
        // whatever it points at.
        if meta.file_type().is_symlink() {
            return Ok(0);
        }
        let dir = meta.is_dir();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(t);
        let mut changed = 0;
        let row = get(tx, owner, path)?;
        match &row {
            Some(r) if r.dir != dir => {
                self.drop_rows(tx, owner, path)?;
                changed += 1;
                return Ok(changed + self.scan_rec(tx, owner, path, disk, t)?);
            }
            Some(r) if !dir && (r.size != meta.len() || r.mtime != mtime) => {
                tx.execute(
                    "UPDATE files SET size = ?1, mtime = ?2, etag = ?3, checksum = NULL WHERE id = ?4",
                    params![meta.len() as i64, mtime, self.new_etag(), r.id],
                )?;
                changed += 1;
            }
            Some(_) => {}
            None => {
                let parent_id: Option<i64> = match parent_of(path) {
                    Some(p) => Some(get(tx, owner, p)?.ok_or(StoreError::Conflict)?.id),
                    None => None,
                };
                let name = name_of(path);
                if !path.is_empty() && !valid_name(name) {
                    return Ok(0);
                }
                tx.execute(
                    "INSERT INTO files (owner, path, parent, name, dir, size, mtime, etag, mime, checksum, created)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10)",
                    params![owner, path, parent_id, name, dir as i64, if dir { 0 } else { meta.len() as i64 }, mtime, self.new_etag(), mime_for(name, dir), t],
                )?;
                changed += 1;
            }
        }
        if dir {
            let mut seen = std::collections::HashSet::new();
            for e in fs::read_dir(disk)? {
                let e = e?;
                let Some(name) = e.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                if !valid_name(&name) {
                    continue;
                }
                let child = join(path, &name);
                changed += self.scan_rec(tx, owner, &child, &e.path(), t)?;
                seen.insert(name);
            }
            let id = get(tx, owner, path)?.ok_or(StoreError::NotFound)?.id;
            let gone: Vec<String> = {
                let mut stmt = tx.prepare("SELECT path, name FROM files WHERE parent = ?1")?;
                let rows = stmt.query_map(params![id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?;
                rows.filter_map(|r| r.ok())
                    .filter(|(_, n)| !seen.contains(n))
                    .map(|(p, _)| p)
                    .collect()
            };
            for p in gone {
                self.drop_rows(tx, owner, &p)?;
                changed += 1;
            }
            if changed > 0 {
                tx.execute(
                    "UPDATE files SET etag = ?1, size = (SELECT COALESCE(SUM(c.size), 0) FROM files c WHERE c.parent = ?2) WHERE id = ?2",
                    params![self.new_etag(), id],
                )?;
            }
        }
        Ok(changed)
    }

    /// The logins that have a home.
    pub fn owners(&self) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare("SELECT owner FROM files WHERE path = '' ORDER BY owner")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Bytes used by the owner's files.
    pub fn used(&self, owner: &str) -> Result<u64> {
        Ok(self.stat(owner, "")?.size)
    }

    /// Bytes free on the filesystem the store is on.
    pub fn free(&self) -> u64 {
        rustix::fs::statvfs(&self.root)
            .map(|s| s.f_bavail.saturating_mul(s.f_frsize))
            .unwrap_or(0)
    }
}

/// The bytes under a folder, for a trash row.
fn walkdir_size(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => total += walkdir_size(&e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        s.ensure_home("alice").unwrap();
        (dir, s)
    }

    fn put(s: &Store, path: &str, body: &[u8]) -> Written {
        s.write_bytes("alice", path, body).unwrap()
    }

    #[test]
    fn paths_are_cleaned_and_bad_names_refused() {
        assert_eq!(clean_path("/a//b/").unwrap(), "a/b");
        assert_eq!(clean_path("").unwrap(), "");
        assert!(clean_path("a/../b").is_err());
        assert!(clean_path("a/./b").is_err());
        assert!(clean_path("a/ b").is_err());
        assert!(valid_owner("notshared"));
        assert!(!valid_owner("../x"));
        assert!(!valid_owner("Alice"));
    }

    #[test]
    fn office_documents_are_text() {
        assert_eq!(mime_for("Plan.paper", false), "text/vnd.losos.paper");
        assert_eq!(mime_for("Talk.SLIDES", false), "text/vnd.losos.slides");
        assert_eq!(mime_for("Notes.md", false), "text/markdown");
    }

    #[test]
    fn a_write_changes_every_etag_up_to_the_root() {
        let (_d, s) = store();
        s.mkdir("alice", "a").unwrap();
        s.mkdir("alice", "a/b").unwrap();
        s.mkdir("alice", "c").unwrap();
        let before: Vec<String> = ["", "a", "a/b", "c"]
            .iter()
            .map(|p| s.stat("alice", p).unwrap().etag)
            .collect();
        put(&s, "a/b/f.txt", b"hello");
        let after: Vec<String> = ["", "a", "a/b", "c"]
            .iter()
            .map(|p| s.stat("alice", p).unwrap().etag)
            .collect();
        assert_ne!(before[0], after[0]);
        assert_ne!(before[1], after[1]);
        assert_ne!(before[2], after[2]);
        assert_eq!(before[3], after[3], "a sibling folder keeps its ETag");
        assert_eq!(s.stat("alice", "").unwrap().size, 5);
        assert_eq!(s.stat("alice", "a").unwrap().size, 5);
    }

    #[test]
    fn an_overwrite_keeps_the_id_and_changes_the_etag() {
        let (_d, s) = store();
        let one = put(&s, "f.txt", b"one").entry;
        let two = put(&s, "f.txt", b"two two");
        assert!(!two.created);
        assert_eq!(one.id, two.entry.id);
        assert_ne!(one.etag, two.entry.etag);
        assert_eq!(two.entry.size, 7);
        assert_eq!(s.read_bytes("alice", "f.txt").unwrap(), b"two two");
        let versions = s.versions("alice", "f.txt").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].size, 3);
    }

    #[test]
    fn if_match_guards_an_overwrite() {
        let (_d, s) = store();
        let e = put(&s, "f.txt", b"one").entry;
        let temp = s.temp_file("alice").unwrap();
        let r = s.commit_file("alice", "f.txt", temp, None, Some("nope"), false, None);
        assert!(matches!(r, Err(StoreError::Precondition)));
        let temp = s.temp_file("alice").unwrap();
        let r = s.commit_file("alice", "f.txt", temp, None, Some(&e.etag), false, None);
        assert!(r.is_ok());
        let temp = s.temp_file("alice").unwrap();
        let r = s.commit_file("alice", "f.txt", temp, None, None, true, None);
        assert!(matches!(r, Err(StoreError::Precondition)));
    }

    #[test]
    fn a_write_into_a_missing_folder_is_a_conflict() {
        let (_d, s) = store();
        let temp = s.temp_file("alice").unwrap();
        let r = s.commit_file("alice", "nope/f.txt", temp, None, None, false, None);
        assert!(matches!(r, Err(StoreError::Conflict)));
    }

    #[test]
    fn a_move_keeps_ids_below_and_moves_both_chains() {
        let (_d, s) = store();
        s.mkdir("alice", "from").unwrap();
        s.mkdir("alice", "to").unwrap();
        s.mkdir("alice", "from/dir").unwrap();
        let f = put(&s, "from/dir/f.txt", b"abc").entry;
        let dir = s.stat("alice", "from/dir").unwrap();
        let from_tag = s.stat("alice", "from").unwrap().etag;
        let to_tag = s.stat("alice", "to").unwrap().etag;
        let moved = s
            .rename("alice", "from/dir", "to/dir2", false)
            .unwrap()
            .entry;
        assert_eq!(moved.id, dir.id);
        assert_eq!(moved.etag, dir.etag, "the moved folder keeps its ETag");
        let f2 = s.stat("alice", "to/dir2/f.txt").unwrap();
        assert_eq!(f2.id, f.id);
        assert_ne!(s.stat("alice", "from").unwrap().etag, from_tag);
        assert_ne!(s.stat("alice", "to").unwrap().etag, to_tag);
        assert_eq!(s.stat("alice", "from").unwrap().size, 0);
        assert_eq!(s.stat("alice", "to").unwrap().size, 3);
        assert!(s.disk_path("alice", "to/dir2/f.txt").is_file());
        assert!(matches!(
            s.stat("alice", "from/dir"),
            Err(StoreError::NotFound)
        ));
    }

    #[test]
    fn a_folder_cannot_move_into_itself() {
        let (_d, s) = store();
        s.mkdir("alice", "a").unwrap();
        assert!(matches!(
            s.rename("alice", "a", "a/b", false),
            Err(StoreError::Loop)
        ));
    }

    #[test]
    fn a_move_onto_a_name_needs_overwrite() {
        let (_d, s) = store();
        put(&s, "a.txt", b"a");
        put(&s, "b.txt", b"b");
        assert!(matches!(
            s.rename("alice", "a.txt", "b.txt", false),
            Err(StoreError::Precondition)
        ));
        let w = s.rename("alice", "a.txt", "b.txt", true).unwrap();
        assert!(!w.created);
        assert_eq!(s.read_bytes("alice", "b.txt").unwrap(), b"a");
        assert_eq!(s.trash("alice").unwrap().len(), 1);
    }

    #[test]
    fn a_copy_makes_new_ids() {
        let (_d, s) = store();
        s.mkdir("alice", "a").unwrap();
        let f = put(&s, "a/f.txt", b"abc").entry;
        s.copy("alice", "a", "b", false).unwrap();
        let g = s.stat("alice", "b/f.txt").unwrap();
        assert_ne!(f.id, g.id);
        assert_eq!(s.read_bytes("alice", "b/f.txt").unwrap(), b"abc");
        assert_eq!(s.stat("alice", "").unwrap().size, 6);
    }

    #[test]
    fn delete_goes_to_the_trash_and_restores_with_a_new_id() {
        let (_d, s) = store();
        s.mkdir("alice", "a").unwrap();
        put(&s, "a/f.txt", b"abc");
        let root_tag = s.stat("alice", "").unwrap().etag;
        let t = s.delete("alice", "a", None).unwrap();
        assert_ne!(s.stat("alice", "").unwrap().etag, root_tag);
        assert_eq!(s.stat("alice", "").unwrap().size, 0);
        assert!(s.stat("alice", "a/f.txt").is_err());
        assert_eq!(s.trash("alice").unwrap()[0].original, "a");
        let back = s.restore("alice", t.id, None).unwrap();
        assert_eq!(back.path, "a");
        assert_eq!(s.read_bytes("alice", "a/f.txt").unwrap(), b"abc");
        assert!(s.trash("alice").unwrap().is_empty());
    }

    #[test]
    fn a_restore_onto_a_taken_name_gets_a_new_one() {
        let (_d, s) = store();
        put(&s, "f.txt", b"old");
        let t = s.delete("alice", "f.txt", None).unwrap();
        put(&s, "f.txt", b"new");
        let back = s.restore("alice", t.id, None).unwrap();
        assert_eq!(back.path, "f (restored).txt");
    }

    #[test]
    fn purge_and_expiry() {
        let (_d, s) = store();
        put(&s, "f.txt", b"x");
        let t = s.delete("alice", "f.txt", None).unwrap();
        assert!(s.trash_disk_path("alice", &t).exists());
        assert_eq!(s.expire_trash("alice", 30).unwrap(), 0);
        assert_eq!(s.expire_trash("alice", -1).unwrap(), 1);
        assert!(!s.trash_disk_path("alice", &t).exists());
    }

    #[test]
    fn scan_picks_up_changes_made_on_disk() {
        let (_d, s) = store();
        put(&s, "keep.txt", b"k");
        let root = s.stat("alice", "").unwrap().etag;
        fs::create_dir(s.disk_path("alice", "new")).unwrap();
        fs::write(s.disk_path("alice", "new/x.txt"), b"hello").unwrap();
        assert!(s.scan("alice").unwrap() > 0);
        assert_eq!(s.stat("alice", "new/x.txt").unwrap().size, 5);
        assert_ne!(s.stat("alice", "").unwrap().etag, root);
        assert_eq!(s.stat("alice", "").unwrap().size, 6);
        fs::remove_file(s.disk_path("alice", "keep.txt")).unwrap();
        s.scan("alice").unwrap();
        assert!(s.stat("alice", "keep.txt").is_err());
        let again = s.stat("alice", "").unwrap().etag;
        assert_eq!(s.scan("alice").unwrap(), 0, "a second scan finds nothing");
        assert_eq!(s.stat("alice", "").unwrap().etag, again);
    }

    #[test]
    fn a_home_with_files_already_on_disk_is_scanned_on_first_use() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("bob/files/Photos")).unwrap();
        fs::write(dir.path().join("bob/files/Photos/a.jpg"), b"jpeg").unwrap();
        let s = Store::open(dir.path()).unwrap();
        let root = s.ensure_home("bob").unwrap();
        assert_eq!(root.size, 4);
        let e = s.stat("bob", "Photos/a.jpg").unwrap();
        assert_eq!(e.mime, "image/jpeg");
    }

    #[test]
    fn a_background_job_waits_for_the_servers_write() {
        let dir = tempfile::tempdir().unwrap();
        drop(Store::open(dir.path()).unwrap());
        let server = Connection::open(dir.path().join("cloud.db")).unwrap();
        server.execute_batch("BEGIN IMMEDIATE").unwrap();
        let done = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            server.execute_batch("COMMIT").unwrap();
        });
        let s = Store::open_waiting(dir.path(), std::time::Duration::from_secs(5)).unwrap();
        s.ensure_home("bob").unwrap();
        done.join().unwrap();
    }

    #[test]
    fn ids_survive_a_reopen_and_are_not_reused() {
        let dir = tempfile::tempdir().unwrap();
        let (id, instance) = {
            let s = Store::open(dir.path()).unwrap();
            s.ensure_home("alice").unwrap();
            let e = s.write_bytes("alice", "f", b"1").unwrap().entry;
            s.delete("alice", "f", None).unwrap();
            (e.id, s.instance().to_owned())
        };
        let s = Store::open(dir.path()).unwrap();
        assert_eq!(s.instance(), instance);
        let e = s.write_bytes("alice", "g", b"1").unwrap().entry;
        assert!(e.id > id);
        assert!(s.oc_id(e.id).ends_with(&instance));
        assert!(s.oc_id(e.id).starts_with("0000000"));
    }

    #[test]
    fn favorites_search_and_recent() {
        let (_d, s) = store();
        put(&s, "Report.odt", b"r");
        put(&s, "photo.jpg", b"p");
        s.set_favorite("alice", "photo.jpg", true).unwrap();
        assert_eq!(s.favorites("alice").unwrap()[0].name, "photo.jpg");
        assert!(s.stat("alice", "photo.jpg").unwrap().favorite);
        assert_eq!(s.search("alice", "repo", 10).unwrap()[0].name, "Report.odt");
        assert!(s.search("alice", "%", 10).unwrap().is_empty());
        assert_eq!(s.recent("alice", 10).unwrap().len(), 2);
        assert_eq!(
            s.by_mime_prefix("alice", &["image/"], 10).unwrap()[0].name,
            "photo.jpg"
        );
        s.set_favorite("alice", "photo.jpg", false).unwrap();
        assert!(s.favorites("alice").unwrap().is_empty());
    }

    #[test]
    fn versions_are_capped_and_restorable() {
        let (_d, s) = store();
        for i in 0..(VERSIONS_KEPT + 5) {
            let temp = {
                let mut t = s.temp_file("alice").unwrap();
                t.write_all(format!("v{i}").as_bytes()).unwrap();
                t
            };
            s.commit_file(
                "alice",
                "f.txt",
                temp,
                Some(1000 + i as i64),
                None,
                false,
                None,
            )
            .unwrap();
        }
        let v = s.versions("alice", "f.txt").unwrap();
        assert_eq!(v.len(), VERSIONS_KEPT);
        let e = s.stat("alice", "f.txt").unwrap();
        let oldest = v.last().unwrap().mtime;
        s.restore_version("alice", e.id, oldest).unwrap();
        assert_eq!(s.read_bytes("alice", "f.txt").unwrap(), b"v4");
    }
}
