//! What live editing needs from the cloud's file store.
//!
//! The cloud's `Store` implements this in `store_impl.rs`; tests use
//! [`MemStore`]. Ids are the cloud's integer file ids, and an ETag changes on
//! every write.

use std::collections::HashMap;
use std::io;
use std::sync::Mutex;

/// A file or folder's integer id in the cloud.
pub type FileId = i64;

/// What the store knows about a file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub id: FileId,
    /// The LosOS Login name of the person whose files these are.
    pub owner: String,
    pub name: String,
    /// The folder it is in, if any.
    pub parent: Option<FileId>,
    pub folder: bool,
    pub size: u64,
    pub etag: String,
}

/// What a person may do with a file.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Perm {
    None,
    Read,
    Edit,
}

/// Why a write did not happen.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// Someone else wrote first; the file's ETag is now this.
    #[error("the file changed (now {0})")]
    Conflict(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// The store, as live editing sees it. Calls block; callers run them off
/// the async runtime.
pub trait CollabStore: Send + Sync + 'static {
    fn stat(&self, file: FileId) -> io::Result<Option<FileInfo>>;
    /// What `login` may do with `file`. A login of the form `name@box` is a
    /// person on another box; the mesh share decides for them.
    fn access(&self, login: &str, file: FileId) -> Perm;
    fn read(&self, file: FileId) -> io::Result<Vec<u8>>;
    /// Write the file if its ETag is still `if_match`; the new ETag.
    fn write(
        &self,
        file: FileId,
        bytes: &[u8],
        if_match: &str,
        by: &str,
    ) -> Result<String, WriteError>;
    /// The files and folders directly inside a folder.
    fn list(&self, folder: FileId) -> io::Result<Vec<FileInfo>>;
}

/// An in-memory store, for tests.
#[derive(Default)]
pub struct MemStore {
    inner: Mutex<MemInner>,
}

#[derive(Default)]
struct MemInner {
    files: HashMap<FileId, (FileInfo, Vec<u8>, Option<FileId>)>,
    grants: HashMap<(String, FileId), Perm>,
    next: FileId,
    writes: u64,
}

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MemInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Add a file (or, with `None` content, a folder) and return its id.
    pub fn add(
        &self,
        owner: &str,
        parent: Option<FileId>,
        name: &str,
        content: Option<&[u8]>,
    ) -> FileId {
        let mut s = self.lock();
        s.next += 1;
        s.writes += 1;
        let id = s.next;
        let info = FileInfo {
            id,
            owner: owner.into(),
            name: name.into(),
            parent,
            folder: content.is_none(),
            size: content.map_or(0, |c| c.len() as u64),
            etag: format!("e{}", s.writes),
        };
        s.files
            .insert(id, (info, content.unwrap_or_default().to_vec(), parent));
        s.grants.insert((owner.into(), id), Perm::Edit);
        id
    }

    /// Let `login` do `perm` with `file`.
    pub fn grant(&self, login: &str, file: FileId, perm: Perm) {
        self.lock().grants.insert((login.into(), file), perm);
    }

    /// Write as some other door would (a sync app), ignoring ETags.
    pub fn overwrite(&self, file: FileId, bytes: &[u8]) -> String {
        let mut s = self.lock();
        s.writes += 1;
        let etag = format!("e{}", s.writes);
        if let Some((info, content, _)) = s.files.get_mut(&file) {
            *content = bytes.to_vec();
            info.size = bytes.len() as u64;
            info.etag = etag.clone();
        }
        etag
    }

    /// The bytes as they stand, for assertions.
    pub fn content(&self, file: FileId) -> String {
        String::from_utf8_lossy(&self.lock().files[&file].1).into_owned()
    }
}

impl CollabStore for MemStore {
    fn stat(&self, file: FileId) -> io::Result<Option<FileInfo>> {
        Ok(self.lock().files.get(&file).map(|f| f.0.clone()))
    }

    fn access(&self, login: &str, file: FileId) -> Perm {
        let s = self.lock();
        // A grant on a folder covers what is inside it.
        let mut at = Some(file);
        while let Some(id) = at {
            if let Some(p) = s.grants.get(&(login.to_string(), id)) {
                return *p;
            }
            at = s.files.get(&id).and_then(|f| f.2);
        }
        Perm::None
    }

    fn read(&self, file: FileId) -> io::Result<Vec<u8>> {
        self.lock()
            .files
            .get(&file)
            .map(|f| f.1.clone())
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn write(
        &self,
        file: FileId,
        bytes: &[u8],
        if_match: &str,
        _by: &str,
    ) -> Result<String, WriteError> {
        let mut s = self.lock();
        s.writes += 1;
        let etag = format!("e{}", s.writes);
        let entry = s
            .files
            .get_mut(&file)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        if entry.0.etag != if_match {
            return Err(WriteError::Conflict(entry.0.etag.clone()));
        }
        entry.1 = bytes.to_vec();
        entry.0.size = bytes.len() as u64;
        entry.0.etag = etag.clone();
        Ok(etag)
    }

    fn list(&self, folder: FileId) -> io::Result<Vec<FileInfo>> {
        let s = self.lock();
        let mut out: Vec<FileInfo> = s
            .files
            .values()
            .filter(|f| f.2 == Some(folder))
            .map(|f| f.0.clone())
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }
}
