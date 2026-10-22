//! Where the documents live: the trait a file store implements for the WOPI
//! host, and [`DirStore`], a plain folder.
//!
//! LosOS Cloud implements [`OfficeStore`] over its own store: `file_id` is
//! its integer file id, `version` its ETag, `user` the LosOS Login name, and
//! every call checks that person's access afresh, so a share taken away
//! mid-session stops the next save. [`DirStore`] is a folder everyone who
//! may sign in shares; the standalone `losos-office` binary and the tests use
//! it.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

/// What the WOPI host needs to know about a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    /// The file's name, extension included, no folder.
    pub name: String,
    pub size: u64,
    /// Changes on every write. coolwsd compares it to tell whether the file
    /// changed under an open document.
    pub version: String,
    pub modified: SystemTime,
    /// The owner's login.
    pub owner: String,
    /// Whether this person may change the file.
    pub writable: bool,
}

/// How [`OfficeStore::create`] treats a name that is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateMode {
    /// Pick a free name next to it ("Report (2).docx").
    Unique,
    /// Refuse with [`StoreError::Exists`].
    Exact,
    /// Replace the file that is there.
    Overwrite,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("no such file")]
    NotFound,
    #[error("not allowed")]
    Forbidden,
    #[error("a file of that name exists")]
    Exists,
    #[error("that is not a valid file name")]
    InvalidName,
    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => StoreError::NotFound,
            std::io::ErrorKind::PermissionDenied => StoreError::Forbidden,
            std::io::ErrorKind::AlreadyExists => StoreError::Exists,
            _ => StoreError::Io(e.to_string()),
        }
    }
}

/// A file store the WOPI host reads and writes through. Every call names the
/// person it is made for, and the store decides what they may do.
pub trait OfficeStore: Send + Sync {
    fn stat(&self, user: &str, file_id: &str) -> Result<FileInfo, StoreError>;
    fn read(&self, user: &str, file_id: &str) -> Result<Vec<u8>, StoreError>;
    /// Replace the file's contents. Answers the file as it is afterwards.
    fn write(&self, user: &str, file_id: &str, bytes: &[u8]) -> Result<FileInfo, StoreError>;
    /// A new file named `name` in the folder `beside` is in ("Save as",
    /// "Export as"). Answers its id and what it is.
    fn create(
        &self,
        user: &str,
        beside: &str,
        name: &str,
        bytes: &[u8],
        mode: CreateMode,
    ) -> Result<(String, FileInfo), StoreError>;
    /// Give the file a new name in the same folder. Answers its id, which a
    /// store whose ids follow names changes, and what it is now.
    fn rename(
        &self,
        user: &str,
        file_id: &str,
        new_name: &str,
    ) -> Result<(String, FileInfo), StoreError>;
    /// Whether a file's id changes with its name. A rename in such a store
    /// would leave the open document's WOPISrc pointing at nothing, so the
    /// editor does not offer one.
    fn ids_follow_names(&self) -> bool {
        false
    }
}

/// Whether `name` can be a file name: one path component, no control
/// characters, at most 255 bytes, and not a dot file.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.contains('/')
        && !name.contains('\\')
        && !name.chars().any(char::is_control)
        && name.trim() == name
}

/// A folder on disk, shared by everyone who may sign in. A file's id is its
/// path under the folder, hex-encoded, so an id names one file and nothing
/// outside the folder can be named at all.
#[derive(Debug, Clone)]
pub struct DirStore {
    root: PathBuf,
    owner: String,
}

impl DirStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            owner: "losos".into(),
        }
    }

    /// The id of the file at `relative` (`/`-separated) under the folder.
    pub fn id_for(relative: &str) -> String {
        relative
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    }

    fn relative(id: &str) -> Result<PathBuf, StoreError> {
        if id.is_empty() || !id.len().is_multiple_of(2) {
            return Err(StoreError::NotFound);
        }
        let bytes = (0..id.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&id[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| StoreError::NotFound)?;
        let text = String::from_utf8(bytes).map_err(|_| StoreError::NotFound)?;
        let path = PathBuf::from(&text);
        let plain = path
            .components()
            .all(|c| matches!(c, Component::Normal(n) if n.to_str().is_some_and(valid_name)));
        if !plain {
            return Err(StoreError::NotFound);
        }
        Ok(path)
    }

    fn path(&self, id: &str) -> Result<PathBuf, StoreError> {
        let relative = Self::relative(id)?;
        // A symlink could point anywhere, a folder on the way as much as the
        // file itself: every step must be a real folder, the last a plain
        // file, as [`Self::list`] sees them.
        let mut path = self.root.clone();
        let mut parts = relative.components().peekable();
        while let Some(part) = parts.next() {
            path.push(part);
            let kind = std::fs::symlink_metadata(&path)?.file_type();
            let fits = if parts.peek().is_some() {
                kind.is_dir()
            } else {
                kind.is_file()
            };
            if !fits {
                return Err(StoreError::NotFound);
            }
        }
        Ok(path)
    }

    fn info(&self, path: &Path) -> Result<FileInfo, StoreError> {
        let meta = std::fs::metadata(path)?;
        let modified = meta.modified()?;
        let nanos = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Ok(FileInfo {
            name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string(),
            size: meta.len(),
            version: format!("{nanos:x}-{:x}", meta.len()),
            modified,
            owner: self.owner.clone(),
            writable: !meta.permissions().readonly(),
        })
    }

    /// Every file under the folder, with its id and its folder (`/`-separated,
    /// empty at the top), sorted by folder and name. Dot files, symlinks and
    /// names that could not be ids are left out, as [`Self::path`] would
    /// refuse them; so is anything more than eight folders down.
    pub fn list(&self) -> Result<Vec<Listed>, StoreError> {
        let mut out = Vec::new();
        self.walk(&self.root, "", 0, &mut out)?;
        out.sort_by(|a, b| (&a.folder, &a.info.name).cmp(&(&b.folder, &b.info.name)));
        Ok(out)
    }

    fn walk(
        &self,
        dir: &Path,
        folder: &str,
        depth: u32,
        out: &mut Vec<Listed>,
    ) -> Result<(), StoreError> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if !valid_name(&name) {
                continue;
            }
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_dir() && depth < 8 {
                let inner = if folder.is_empty() {
                    name
                } else {
                    format!("{folder}/{name}")
                };
                self.walk(&path, &inner, depth + 1, out)?;
            } else if kind.is_file() {
                out.push(Listed {
                    id: self.id_of(&path),
                    folder: folder.to_string(),
                    info: self.info(&path)?,
                });
            }
        }
        Ok(())
    }

    fn id_of(&self, path: &Path) -> String {
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        Self::id_for(&rel.to_string_lossy())
    }
}

/// One file in [`DirStore::list`].
#[derive(Debug, Clone)]
pub struct Listed {
    pub id: String,
    pub folder: String,
    pub info: FileInfo,
}

/// Write `bytes` to `path` all at once: a temporary file beside it, then a
/// rename, so a reader never sees half a document.
fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{name}.office-{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&tmp, meta.permissions());
    }
    std::fs::rename(&tmp, path)
}

/// "Report.docx", "Report (2).docx", "Report (3).docx", …
fn numbered(name: &str, n: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({n}).{ext}"),
        _ => format!("{name} ({n})"),
    }
}

impl OfficeStore for DirStore {
    fn ids_follow_names(&self) -> bool {
        true
    }

    fn stat(&self, _user: &str, file_id: &str) -> Result<FileInfo, StoreError> {
        self.info(&self.path(file_id)?)
    }

    fn read(&self, _user: &str, file_id: &str) -> Result<Vec<u8>, StoreError> {
        Ok(std::fs::read(self.path(file_id)?)?)
    }

    fn write(&self, _user: &str, file_id: &str, bytes: &[u8]) -> Result<FileInfo, StoreError> {
        let path = self.path(file_id)?;
        if !self.info(&path)?.writable {
            return Err(StoreError::Forbidden);
        }
        replace(&path, bytes)?;
        self.info(&path)
    }

    fn create(
        &self,
        _user: &str,
        beside: &str,
        name: &str,
        bytes: &[u8],
        mode: CreateMode,
    ) -> Result<(String, FileInfo), StoreError> {
        if !valid_name(name) {
            return Err(StoreError::InvalidName);
        }
        let dir = self
            .path(beside)?
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.root.clone());
        let mut path = dir.join(name);
        match mode {
            CreateMode::Overwrite => {}
            CreateMode::Exact if path.exists() => return Err(StoreError::Exists),
            CreateMode::Exact => {}
            CreateMode::Unique => {
                let mut n = 2;
                while path.exists() {
                    path = dir.join(numbered(name, n));
                    n += 1;
                }
            }
        }
        replace(&path, bytes)?;
        Ok((self.id_of(&path), self.info(&path)?))
    }

    fn rename(
        &self,
        _user: &str,
        file_id: &str,
        new_name: &str,
    ) -> Result<(String, FileInfo), StoreError> {
        if !valid_name(new_name) {
            return Err(StoreError::InvalidName);
        }
        let path = self.path(file_id)?;
        let to = path.with_file_name(new_name);
        if to != path && to.exists() {
            return Err(StoreError::Exists);
        }
        std::fs::rename(&path, &to)?;
        Ok((self.id_of(&to), self.info(&to)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (assert_fs::TempDir, DirStore) {
        let dir = assert_fs::TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("Work")).unwrap();
        std::fs::write(dir.path().join("Work/Plan.docx"), b"v1").unwrap();
        let s = DirStore::new(dir.path());
        (dir, s)
    }

    #[test]
    fn ids_name_files_under_the_folder_and_nothing_else() {
        let (dir, s) = store();
        let id = DirStore::id_for("Work/Plan.docx");
        assert_eq!(s.read("u", &id).unwrap(), b"v1");
        std::fs::write(dir.path().join("Work/.hidden.docx"), b"x").unwrap();
        for bad in [
            "../etc/passwd",
            "/etc/passwd",
            "Work/../Work/Plan.docx",
            "Work",
            "Work/.hidden.docx",
            "",
        ] {
            let err = s.read("u", &DirStore::id_for(bad));
            assert_eq!(err, Err(StoreError::NotFound), "{bad}");
        }
        assert_eq!(s.read("u", "zz"), Err(StoreError::NotFound));
        assert_eq!(s.read("u", "abc"), Err(StoreError::NotFound));
    }

    #[test]
    fn the_list_is_every_plain_file_with_its_folder() {
        let (dir, store) = store();
        std::fs::create_dir_all(dir.path().join("Trips/2026")).unwrap();
        std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();
        std::fs::write(dir.path().join("budget.ods"), b"x").unwrap();
        std::fs::write(dir.path().join("Trips/2026/Trip plan.odt"), b"yy").unwrap();
        std::fs::write(dir.path().join(".hidden/secret.odt"), b"z").unwrap();
        std::fs::write(dir.path().join(".dot.odt"), b"z").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", dir.path().join("link.odt")).unwrap();

        let list = store.list().unwrap();
        let seen: Vec<_> = list
            .iter()
            .map(|l| (l.folder.as_str(), l.info.name.as_str(), l.info.size))
            .collect();
        // store() starts with Work/Plan.docx.
        assert_eq!(
            seen,
            [
                ("", "budget.ods", 1),
                ("Trips/2026", "Trip plan.odt", 2),
                ("Work", "Plan.docx", 2)
            ]
        );
        let trip = &list[1];
        assert_eq!(trip.id, DirStore::id_for("Trips/2026/Trip plan.odt"));
        assert_eq!(store.stat("u", &trip.id).unwrap().name, "Trip plan.odt");
    }

    #[test]
    fn a_symlink_out_of_the_folder_is_not_a_file() {
        let (dir, s) = store();
        std::os::unix::fs::symlink("/etc/hostname", dir.path().join("Work/link.docx")).unwrap();
        assert_eq!(
            s.read("u", &DirStore::id_for("Work/link.docx")),
            Err(StoreError::NotFound)
        );
    }

    #[test]
    fn a_symlinked_folder_out_of_the_folder_is_not_a_way_in() {
        let (dir, s) = store();
        let outside = assert_fs::TempDir::new().unwrap();
        std::fs::write(outside.path().join("secret.docx"), b"no").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("Out")).unwrap();
        let id = DirStore::id_for("Out/secret.docx");
        assert_eq!(s.read("u", &id), Err(StoreError::NotFound));
        assert_eq!(s.write("u", &id, b"x"), Err(StoreError::NotFound));
        assert_eq!(
            s.create("u", &id, "new.docx", b"x", CreateMode::Unique),
            Err(StoreError::NotFound)
        );
        assert_eq!(
            std::fs::read(outside.path().join("secret.docx")).unwrap(),
            b"no"
        );
        assert!(!outside.path().join("new.docx").exists());
    }

    #[test]
    fn a_write_changes_the_version_and_leaves_no_temporary_file() {
        let (dir, s) = store();
        let id = DirStore::id_for("Work/Plan.docx");
        let before = s.stat("u", &id).unwrap();
        let after = s.write("u", &id, b"version two").unwrap();
        assert_ne!(before.version, after.version);
        assert_eq!(after.size, 11);
        assert_eq!(s.read("u", &id).unwrap(), b"version two");
        let names: Vec<_> = std::fs::read_dir(dir.path().join("Work"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("Plan.docx")]);
    }

    #[test]
    fn a_read_only_file_is_not_writable() {
        let (dir, s) = store();
        let path = dir.path().join("Work/Plan.docx");
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();
        let id = DirStore::id_for("Work/Plan.docx");
        assert!(!s.stat("u", &id).unwrap().writable);
        assert_eq!(s.write("u", &id, b"x"), Err(StoreError::Forbidden));
    }

    #[test]
    fn create_lands_beside_the_file_and_numbers_a_taken_name() {
        let (_dir, s) = store();
        let beside = DirStore::id_for("Work/Plan.docx");
        let (id, info) = s
            .create("u", &beside, "Plan.pdf", b"pdf", CreateMode::Unique)
            .unwrap();
        assert_eq!(id, DirStore::id_for("Work/Plan.pdf"));
        assert_eq!(info.name, "Plan.pdf");
        let (id2, info2) = s
            .create("u", &beside, "Plan.pdf", b"pdf", CreateMode::Unique)
            .unwrap();
        assert_eq!(id2, DirStore::id_for("Work/Plan (2).pdf"));
        assert_eq!(info2.name, "Plan (2).pdf");
        assert_eq!(
            s.create("u", &beside, "Plan.pdf", b"x", CreateMode::Exact),
            Err(StoreError::Exists)
        );
        s.create("u", &beside, "Plan.pdf", b"new", CreateMode::Overwrite)
            .unwrap();
        assert_eq!(s.read("u", &id).unwrap(), b"new");
        assert_eq!(
            s.create("u", &beside, "../x.docx", b"x", CreateMode::Unique),
            Err(StoreError::InvalidName)
        );
    }

    #[test]
    fn rename_moves_the_id_with_the_name() {
        let (_dir, s) = store();
        let id = DirStore::id_for("Work/Plan.docx");
        let (new_id, info) = s.rename("u", &id, "Budget.docx").unwrap();
        assert_eq!(new_id, DirStore::id_for("Work/Budget.docx"));
        assert_eq!(info.name, "Budget.docx");
        assert_eq!(s.stat("u", &id), Err(StoreError::NotFound));
        s.create("u", &new_id, "Taken.docx", b"", CreateMode::Exact)
            .unwrap();
        assert_eq!(
            s.rename("u", &new_id, "Taken.docx"),
            Err(StoreError::Exists)
        );
        assert_eq!(
            s.rename("u", &new_id, "a/b.docx"),
            Err(StoreError::InvalidName)
        );
    }

    #[test]
    fn names() {
        assert!(valid_name("Report (final).docx"));
        assert!(valid_name("Zpráva.odt"));
        for bad in [
            "",
            ".x",
            "a/b",
            "a\\b",
            "a\nb",
            " a",
            "a ",
            &"x".repeat(256),
        ] {
            assert!(!valid_name(bad), "{bad:?}");
        }
    }
}
