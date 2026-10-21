//! Live editing over the cloud's own store: the files people see in the web
//! app, the sync apps and WebDAV. A write from the editor is an ordinary
//! commit with `If-Match`, so it keeps a version and reaches the sync apps
//! like any other.

use std::io::{self, Write};
use std::sync::{Arc, Weak};

use super::store::{CollabStore, FileId, FileInfo, Perm, WriteError};
use crate::cloud::store::{parent_of, Entry, StoreError};
use crate::cloud::Cloud;

/// The cloud's store, as live editing sees it. Weak, because the cloud
/// holds the mesh that holds this.
pub struct CloudStore(pub Weak<Cloud>);

fn io_err(e: StoreError) -> io::Error {
    match e {
        StoreError::NotFound => io::Error::from(io::ErrorKind::NotFound),
        StoreError::Io(e) => e,
        e => io::Error::other(e.to_string()),
    }
}

impl CloudStore {
    fn cloud(&self) -> io::Result<Arc<Cloud>> {
        self.0
            .upgrade()
            .ok_or_else(|| io::Error::other("the cloud is shutting down"))
    }

    fn info(e: Entry, parent: Option<FileId>) -> FileInfo {
        FileInfo {
            id: e.id,
            owner: e.owner,
            name: e.name,
            parent,
            folder: e.dir,
            size: e.size,
            etag: e.etag,
        }
    }
}

impl CollabStore for CloudStore {
    fn stat(&self, file: FileId) -> io::Result<Option<FileInfo>> {
        let cloud = self.cloud()?;
        let e = match cloud.store.by_id_any(file) {
            Ok(e) => e,
            Err(StoreError::NotFound) => return Ok(None),
            Err(e) => return Err(io_err(e)),
        };
        // The owner's root has no folder above it.
        let parent = match parent_of(&e.path) {
            Some(p) => Some(cloud.store.stat(&e.owner, p).map_err(io_err)?.id),
            None => None,
        };
        Ok(Some(Self::info(e, parent)))
    }

    fn access(&self, login: &str, file: FileId) -> Perm {
        let Ok(cloud) = self.cloud() else {
            return Perm::None;
        };
        let Ok(e) = cloud.store.by_id_any(file) else {
            return Perm::None;
        };
        if e.owner == login {
            return Perm::Edit;
        }
        if login.contains('@') {
            if let Some(mesh) = cloud.mesh.get() {
                return mesh.guest_access(login, file);
            }
        }
        Perm::None
    }

    fn read(&self, file: FileId) -> io::Result<Vec<u8>> {
        let cloud = self.cloud()?;
        let e = cloud.store.by_id_any(file).map_err(io_err)?;
        cloud.store.read_bytes(&e.owner, &e.path).map_err(io_err)
    }

    fn write(
        &self,
        file: FileId,
        bytes: &[u8],
        if_match: &str,
        _by: &str,
    ) -> Result<String, WriteError> {
        let cloud = self.cloud()?;
        let e = cloud.store.by_id_any(file).map_err(io_err)?;
        if e.dir {
            return Err(io::Error::other("a folder").into());
        }
        let mut temp = cloud.store.temp_file(&e.owner).map_err(io_err)?;
        temp.write_all(bytes)?;
        let sum = crate::cloud::util::sha1_hex(bytes);
        match cloud.store.commit_file(
            &e.owner,
            &e.path,
            temp,
            None,
            Some(if_match),
            false,
            Some(format!("SHA1:{sum}")),
        ) {
            Ok(w) => {
                cloud.changed(&e.owner, &w.entry);
                Ok(w.entry.etag)
            }
            Err(StoreError::Precondition) => {
                let now = cloud.store.by_id_any(file).map_err(io_err)?;
                Err(WriteError::Conflict(now.etag))
            }
            Err(e) => Err(io_err(e).into()),
        }
    }

    fn list(&self, folder: FileId) -> io::Result<Vec<FileInfo>> {
        let cloud = self.cloud()?;
        let dir = cloud.store.by_id_any(folder).map_err(io_err)?;
        let items = cloud.store.list(&dir.owner, &dir.path).map_err(io_err)?;
        Ok(items
            .into_iter()
            .map(|e| Self::info(e, Some(folder)))
            .collect())
    }
}
