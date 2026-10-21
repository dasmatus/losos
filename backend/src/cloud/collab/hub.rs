//! The documents open on this box, and keeping each one in step with its
//! file.
//!
//! A document is open while someone is in it, and for [`Hub::idle`] after
//! the last person leaves. While it is open a task writes its text back to
//! the file a short pause after the last keystroke ([`Hub::pause`]), through
//! the store with the ETag it last saw, so the sync apps and WebDAV see the
//! file change like any other write. A write from another door shows up as
//! a [`Hub::file_changed`] notice and is merged in (`Room::merge_external`).
//!
//! The Yjs state is saved next to the text in `<dir>/f<id>.ydoc`. It has to
//! be: another box holding a copy of the document knows its content by Yjs
//! item ids, and a document rebuilt from the plain text after a restart
//! would have new ids, so the other box's copy would merge in as a second
//! copy of the whole text. `<dir>/f<id>.base` is the state whose text the
//! file held at the ETag in `<dir>/f<id>.etag`, the base an outside write
//! is merged from.
//!
//! A copy of a document that lives on another box has no file here; its
//! state alone is saved, as `<dir>/c<share>-<file>.ydoc`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;

use super::room::{Room, MAX_TEXT};
use super::store::{CollabStore, FileId, WriteError};

/// Which document.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DocKey {
    /// A file on this box.
    File(FileId),
    /// This box's copy of a file shared from another box: the share's id
    /// and the file's id there.
    Copy(String, FileId),
}

impl DocKey {
    fn stem(&self) -> String {
        match self {
            DocKey::File(id) => format!("f{id}"),
            // Share ids are random letters and digits (mesh::share_id), so
            // they are safe in a file name; anything else is refused.
            DocKey::Copy(share, file) => format!("c{share}-{file}"),
        }
    }

    fn valid(&self) -> bool {
        match self {
            DocKey::File(_) => true,
            DocKey::Copy(share, _) => {
                !share.is_empty()
                    && share.len() <= 64
                    && share.bytes().all(|b| b.is_ascii_alphanumeric())
            }
        }
    }
}

/// Why a document did not open.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("no such file")]
    NotFound,
    #[error("this is a folder")]
    Folder,
    #[error("the file is not text")]
    NotText,
    #[error("the file is too large to edit together")]
    TooLarge,
    #[error("not a valid document")]
    BadKey,
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// An open document.
pub struct Doc {
    pub key: DocKey,
    pub room: Room,
    /// Notices for the document's task: the file changed elsewhere.
    notices: mpsc::UnboundedSender<String>,
}

/// What the task knows about the file behind a document.
struct FileSync {
    etag: String,
    text: String,
    base: Vec<u8>,
}

/// The open documents.
pub struct Hub {
    store: Arc<dyn CollabStore>,
    dir: PathBuf,
    docs: Mutex<HashMap<DocKey, Arc<Doc>>>,
    /// Quiet time after an edit before the text is written back.
    pub pause: Duration,
    /// How long a document with nobody in it stays open.
    pub idle: Duration,
}

impl Hub {
    pub fn new(store: Arc<dyn CollabStore>, dir: PathBuf) -> Arc<Self> {
        Arc::new(Hub {
            store,
            dir,
            docs: Mutex::new(HashMap::new()),
            pause: Duration::from_millis(800),
            idle: Duration::from_secs(30),
        })
    }

    /// The same hub with other timings, for tests.
    pub fn with_timing(
        store: Arc<dyn CollabStore>,
        dir: PathBuf,
        pause: Duration,
        idle: Duration,
    ) -> Arc<Self> {
        Arc::new(Hub {
            store,
            dir,
            docs: Mutex::new(HashMap::new()),
            pause,
            idle,
        })
    }

    pub fn store(&self) -> &Arc<dyn CollabStore> {
        &self.store
    }

    fn docs(&self) -> std::sync::MutexGuard<'_, HashMap<DocKey, Arc<Doc>>> {
        self.docs.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The document if it is open.
    pub fn get(&self, key: &DocKey) -> Option<Arc<Doc>> {
        self.docs().get(key).cloned()
    }

    /// Open a document, loading it if nobody has it open. Must be called on
    /// a tokio runtime: an open document has a task.
    pub async fn open(self: &Arc<Self>, key: DocKey) -> Result<Arc<Doc>, OpenError> {
        if !key.valid() {
            return Err(OpenError::BadKey);
        }
        if let Some(doc) = self.get(&key) {
            return Ok(doc);
        }
        let hub = self.clone();
        let k = key.clone();
        let (room, sync) = tokio::task::spawn_blocking(move || hub.load(&k))
            .await
            .map_err(|e| io::Error::other(e.to_string()))??;
        let mut docs = self.docs();
        // Someone else may have opened it while this one loaded.
        if let Some(doc) = docs.get(&key) {
            return Ok(doc.clone());
        }
        let (tx, rx) = mpsc::unbounded_channel();
        // Watch from before anyone can join, so no edit slips past the task.
        let changes = room.watch();
        let doc = Arc::new(Doc {
            key: key.clone(),
            room,
            notices: tx,
        });
        docs.insert(key, doc.clone());
        drop(docs);
        tokio::spawn(self.clone().run(doc.clone(), sync, changes, rx));
        Ok(doc)
    }

    /// A file changed through some other door. If it is open, the new
    /// version is merged in.
    pub fn file_changed(&self, file: FileId, etag: &str) {
        if let Some(doc) = self.get(&DocKey::File(file)) {
            let _ = doc.notices.send(etag.to_string());
        }
    }

    fn path(&self, key: &DocKey, ext: &str) -> PathBuf {
        self.dir.join(format!("{}.{ext}", key.stem()))
    }

    fn load(&self, key: &DocKey) -> Result<(Room, Option<FileSync>), OpenError> {
        let saved = std::fs::read(self.path(key, "ydoc")).ok();
        let DocKey::File(id) = key else {
            let room = saved.and_then(|s| Room::from_state(&s)).unwrap_or_default();
            return Ok((room, None));
        };
        let info = self.store.stat(*id)?.ok_or(OpenError::NotFound)?;
        if info.folder {
            return Err(OpenError::Folder);
        }
        if info.size as usize > MAX_TEXT {
            return Err(OpenError::TooLarge);
        }
        let bytes = self.store.read(*id)?;
        let text = String::from_utf8(bytes).map_err(|_| OpenError::NotText)?;
        if text.contains('\0') {
            return Err(OpenError::NotText);
        }
        let base = std::fs::read(self.path(key, "base")).ok();
        let etag = std::fs::read_to_string(self.path(key, "etag")).ok();
        if let (Some(room), Some(base), Some(etag)) =
            (saved.as_deref().and_then(Room::from_state), base, etag)
        {
            if etag == info.etag {
                return Ok((room, Some(FileSync { etag, text, base })));
            }
            // The file was written while nobody had it open. The merge marks
            // the room changed, so the task writes the result back.
            if let Some(next) = room.merge_external(&base, &text) {
                return Ok((
                    room,
                    Some(FileSync {
                        etag: info.etag,
                        text,
                        base: next,
                    }),
                ));
            }
        }
        // A new document: its state is saved at once, because from now on
        // other boxes may know its text by these Yjs ids.
        let room = Room::from_text(&text);
        let base = room.encode_state();
        self.write_state(key, &base, Some((&base, &info.etag)))?;
        Ok((
            room,
            Some(FileSync {
                etag: info.etag,
                text,
                base,
            }),
        ))
    }

    /// The document's task: write back after a pause, merge outside
    /// writes, close when idle.
    async fn run(
        self: Arc<Self>,
        doc: Arc<Doc>,
        mut sync: Option<FileSync>,
        mut changes: tokio::sync::watch::Receiver<u64>,
        mut notices: mpsc::UnboundedReceiver<String>,
    ) {
        // A merge made while loading has not reached the file yet.
        if sync.as_ref().is_some_and(|s| s.text != doc.room.text()) {
            self.save(&doc, &mut sync).await;
        }
        loop {
            let idle = tokio::time::sleep(self.idle);
            tokio::select! {
                r = changes.changed() => {
                    if r.is_err() {
                        break;
                    }
                    // Wait for a pause in the typing, but not forever.
                    let deadline = tokio::time::Instant::now() + self.pause * 6;
                    loop {
                        let quiet = tokio::time::sleep(self.pause);
                        tokio::select! {
                            r = changes.changed() => if r.is_err() { break },
                            _ = quiet => break,
                        }
                        if tokio::time::Instant::now() >= deadline {
                            break;
                        }
                    }
                    self.save(&doc, &mut sync).await;
                }
                Some(etag) = notices.recv() => {
                    self.merge_outside(&doc, &mut sync, &etag).await;
                }
                _ = idle => {
                    if doc.room.peers() == 0 {
                        let mut docs = self.docs();
                        // Nobody joined between the check and the lock.
                        if doc.room.peers() == 0 {
                            docs.remove(&doc.key);
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Save the state, and write the text back if it changed.
    async fn save(self: &Arc<Self>, doc: &Arc<Doc>, sync: &mut Option<FileSync>) {
        let (state, text) = doc.room.snapshot();
        let hub = self.clone();
        let key = doc.key.clone();
        let Some(s) = sync.as_mut() else {
            let _ = tokio::task::spawn_blocking(move || hub.write_state(&key, &state, None)).await;
            return;
        };
        if text == s.text {
            let base = s.base.clone();
            let etag = s.etag.clone();
            let _ = tokio::task::spawn_blocking(move || {
                hub.write_state(&key, &state, Some((&base, &etag)))
            })
            .await;
            return;
        }
        let DocKey::File(id) = doc.key else { return };
        let store = self.store.clone();
        let if_match = s.etag.clone();
        let t = text.clone();
        let written =
            tokio::task::spawn_blocking(move || store.write(id, t.as_bytes(), &if_match, "collab"))
                .await;
        match written {
            Ok(Ok(etag)) => {
                *s = FileSync {
                    etag: etag.clone(),
                    text,
                    base: state.clone(),
                };
                let _ = tokio::task::spawn_blocking(move || {
                    hub.write_state(&key, &state, Some((&state, &etag)))
                })
                .await;
            }
            Ok(Err(WriteError::Conflict(etag))) => {
                // Someone wrote first: take their version in, and the
                // change this causes writes the merge back.
                self.merge_outside(doc, sync, &etag).await;
            }
            Ok(Err(WriteError::Io(e))) => {
                tracing::warn!(file = id, error = %e, "collab: could not write the file back");
            }
            Err(e) => tracing::warn!(file = id, error = %e, "collab: write-back task failed"),
        }
    }

    async fn merge_outside(
        self: &Arc<Self>,
        doc: &Arc<Doc>,
        sync: &mut Option<FileSync>,
        etag: &str,
    ) {
        let Some(s) = sync.as_mut() else { return };
        if s.etag == etag {
            return;
        }
        let DocKey::File(id) = doc.key else { return };
        let store = self.store.clone();
        let read = tokio::task::spawn_blocking(move || -> io::Result<_> {
            let info = store.stat(id)?;
            let bytes = store.read(id)?;
            Ok((info, bytes))
        })
        .await;
        let Ok(Ok((Some(info), bytes))) = read else {
            return;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            // It is not text any more; the people in it keep their copy,
            // and the next write-back will fail its ETag and land here again.
            return;
        };
        if let Some(next) = doc.room.merge_external(&s.base, &text) {
            *s = FileSync {
                etag: info.etag,
                text,
                base: next,
            };
        }
    }

    fn write_state(
        &self,
        key: &DocKey,
        state: &[u8],
        file: Option<(&[u8], &str)>,
    ) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        atomic_write(&self.path(key, "ydoc"), state)?;
        if let Some((base, etag)) = file {
            atomic_write(&self.path(key, "base"), base)?;
            atomic_write(&self.path(key, "etag"), etag.as_bytes())?;
        }
        Ok(())
    }

    /// The owner's file ids of every copy this box keeps for a share.
    pub fn copies_of(&self, share: &str) -> Vec<FileId> {
        let prefix = format!("c{share}-");
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                name.strip_prefix(&prefix)?
                    .strip_suffix(".ydoc")?
                    .parse()
                    .ok()
            })
            .collect()
    }

    /// Forget every copy of a share: it was taken back. What was in them is
    /// gone from this box.
    pub fn drop_copies(&self, share: &str) {
        let mut files = self.copies_of(share);
        files.extend(self.docs().keys().filter_map(|k| match k {
            DocKey::Copy(s, f) if s == share => Some(*f),
            _ => None,
        }));
        for f in files {
            let key = DocKey::Copy(share.to_string(), f);
            if let Some(doc) = self.docs().remove(&key) {
                doc.room.close_all();
            }
            let _ = std::fs::remove_file(self.path(&key, "ydoc"));
        }
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::collab::room::Access;
    use crate::cloud::collab::store::MemStore;

    fn hub(store: &Arc<MemStore>, dir: &Path) -> Arc<Hub> {
        Hub::with_timing(
            store.clone() as Arc<dyn CollabStore>,
            dir.to_path_buf(),
            Duration::from_millis(20),
            Duration::from_millis(200),
        )
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    #[tokio::test]
    async fn typing_is_written_back_to_the_file() {
        let dir = tempdir();
        let store = Arc::new(MemStore::new());
        let id = store.add("alice", None, "notes.md", Some(b"# Notes\n"));
        let hub = hub(&store, &dir);
        let doc = hub.open(DocKey::File(id)).await.unwrap();
        let (_p, _rx) = doc.room.join(Access::Write);
        doc.room.replace_text("# Notes\nfirst line\n");
        settle().await;
        assert_eq!(store.content(id), "# Notes\nfirst line\n");
    }

    #[tokio::test]
    async fn an_outside_write_reaches_the_open_document() {
        let dir = tempdir();
        let store = Arc::new(MemStore::new());
        let id = store.add("alice", None, "notes.md", Some(b"a\nb\n"));
        let hub = hub(&store, &dir);
        let doc = hub.open(DocKey::File(id)).await.unwrap();
        let (_p, _rx) = doc.room.join(Access::Write);
        doc.room.replace_text("a\nb\nc\n");
        // The phone app uploads a version made from the old file before the
        // write-back went out.
        let etag = store.overwrite(id, b"A\nb\n");
        hub.file_changed(id, &etag);
        settle().await;
        assert_eq!(doc.room.text(), "A\nb\nc\n");
        assert_eq!(store.content(id), "A\nb\nc\n");
    }

    #[tokio::test]
    async fn the_document_keeps_its_identity_across_a_restart() {
        let dir = tempdir();
        let store = Arc::new(MemStore::new());
        let id = store.add("alice", None, "notes.md", Some(b"hello\n"));
        let first = hub(&store, &dir);
        let doc = first.open(DocKey::File(id)).await.unwrap();
        let copy = Room::new();
        let (p, mut rx) = doc.room.join(Access::Write);
        let (q, mut crx) = copy.join(Access::Write);
        // Bring the other box's copy up to date.
        for _ in 0..3 {
            while let Ok(m) = rx.try_recv() {
                copy.receive(q, &m).unwrap();
            }
            while let Ok(m) = crx.try_recv() {
                doc.room.receive(p, &m).unwrap();
            }
        }
        doc.room.leave(p);
        settle().await;
        drop(doc);
        drop(first);
        // The box restarts; the copy reconnects with what it knew.
        let second = hub(&store, &dir);
        let doc = second.open(DocKey::File(id)).await.unwrap();
        let (p, mut rx) = doc.room.join(Access::Write);
        let (q, mut crx) = copy.join(Access::Write);
        for _ in 0..3 {
            while let Ok(m) = rx.try_recv() {
                copy.receive(q, &m).unwrap();
            }
            while let Ok(m) = crx.try_recv() {
                doc.room.receive(p, &m).unwrap();
            }
        }
        // One copy of the text, not two.
        assert_eq!(doc.room.text(), "hello\n");
        assert_eq!(copy.text(), "hello\n");
    }

    #[tokio::test]
    async fn a_write_while_closed_is_merged_on_open() {
        let dir = tempdir();
        let store = Arc::new(MemStore::new());
        let id = store.add("alice", None, "n.txt", Some(b"one\n"));
        let h = hub(&store, &dir);
        let doc = h.open(DocKey::File(id)).await.unwrap();
        doc.room.replace_text("one\ntwo\n");
        settle().await;
        drop(doc);
        settle().await;
        assert!(h.get(&DocKey::File(id)).is_none(), "closed when idle");
        store.overwrite(id, b"zero\none\n");
        let doc = h.open(DocKey::File(id)).await.unwrap();
        doc.room.replace_text("zero\none\nthree\n");
        settle().await;
        drop(doc);
        settle().await;
        // Typed while the file was edited elsewhere and nobody had it open.
        let mut text = std::fs::read(dir.join(format!("f{id}.ydoc"))).unwrap();
        let typed = Room::from_state(&text).unwrap();
        typed.replace_text("zero\none\nthree\nfour\n");
        text = typed.encode_state();
        std::fs::write(dir.join(format!("f{id}.ydoc")), text).unwrap();
        store.overwrite(id, b"ZERO\none\nthree\n");
        let doc = h.open(DocKey::File(id)).await.unwrap();
        assert_eq!(doc.room.text(), "ZERO\none\nthree\nfour\n");
        settle().await;
        assert_eq!(store.content(id), "ZERO\none\nthree\nfour\n");
    }

    #[tokio::test]
    async fn only_text_opens() {
        let dir = tempdir();
        let store = Arc::new(MemStore::new());
        let bin = store.add("alice", None, "x.bin", Some(&[0xff, 0xfe, 0x00]));
        let folder = store.add("alice", None, "docs", None);
        let big = store.add("alice", None, "big.txt", Some(&vec![b'x'; MAX_TEXT + 1]));
        let h = hub(&store, &dir);
        assert!(matches!(
            h.open(DocKey::File(bin)).await,
            Err(OpenError::NotText)
        ));
        assert!(matches!(
            h.open(DocKey::File(folder)).await,
            Err(OpenError::Folder)
        ));
        assert!(matches!(
            h.open(DocKey::File(big)).await,
            Err(OpenError::TooLarge)
        ));
        assert!(matches!(
            h.open(DocKey::File(999)).await,
            Err(OpenError::NotFound)
        ));
        assert!(matches!(
            h.open(DocKey::Copy("../x".into(), 1)).await,
            Err(OpenError::BadKey)
        ));
    }

    #[tokio::test]
    async fn a_copy_keeps_its_state_with_no_file() {
        let dir = tempdir();
        let store = Arc::new(MemStore::new());
        let h = hub(&store, &dir);
        let key = DocKey::Copy("abc123".into(), 7);
        let doc = h.open(key.clone()).await.unwrap();
        doc.room.replace_text("typed offline\n");
        settle().await;
        drop(doc);
        settle().await;
        let doc = h.open(key).await.unwrap();
        assert_eq!(doc.room.text(), "typed offline\n");
    }

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "losos-collab-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
