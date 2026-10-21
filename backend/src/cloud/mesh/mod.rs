//! Sharing files with people on other boxes, and editing them together.
//!
//! A person here shares a file or folder with `bob@boxb.example`. This box
//! (the owner's) sends boxb an offer over a [`link`]; bob sees it under
//! "Shared with you" and accepts or declines. From then on boxb reads the
//! files through the link, and a text file opened in the editor on boxb is
//! a copy of the owner's document (`collab::hub::DocKey::Copy`) kept in step
//! over the same link, so bob and the owner's people see each other type.
//!
//! The owner's box is the only one that holds the file and decides who may
//! do what: every request from another box names a share, and is answered
//! only if that share was sent to that box. A copy keeps working while the
//! owner's box is out of reach, and what was typed on it merges in when the
//! two meet again ([`Mesh::run`] retries).

pub mod db;
pub mod dial;
pub mod identity;
pub mod link;
pub mod routes;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use self::db::{Db, Incoming, OutState, Outgoing};
use self::identity::{split_address, Identity};
use self::link::{doc_name, Link, Wire};
use crate::cloud::collab::hub::{Doc, DocKey, Hub, OpenError};
use crate::cloud::collab::room::Access;
use crate::cloud::collab::store::{FileId, FileInfo, Perm};

/// The largest file another box may read through a link in one piece.
pub const MAX_READ: u64 = 16 * 1024 * 1024;

/// Why something between boxes did not work. `code` is what crosses the
/// wire and what the web app shows a sentence for.
#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    #[error("the connection closed")]
    Closed,
    #[error("the other box did not answer in time")]
    Timeout,
    #[error("the other box spoke something else")]
    Protocol,
    #[error("another box answered at that name")]
    WrongBox,
    #[error("a signature did not check out")]
    BadSignature,
    #[error("{0} answers with a different key than before")]
    KeyChanged(String),
    #[error("the other box refused: {0}")]
    Refused(String),
    #[error("the other box said: {0}")]
    Remote(String),
    #[error("{0} could not be reached")]
    Unreachable(String),
    #[error("the box's records could not be read or written")]
    Store,
    #[error("no such share or file")]
    NotFound,
    #[error("not allowed")]
    Forbidden,
    #[error("that is not an address like name@box")]
    BadAddress,
    #[error("the file cannot be edited together")]
    Open(#[from] OpenError),
    #[error("the file is too large to send")]
    TooLarge,
}

impl MeshError {
    pub fn code(&self) -> &'static str {
        match self {
            MeshError::Closed => "closed",
            MeshError::Timeout => "timeout",
            MeshError::Protocol => "protocol",
            MeshError::WrongBox => "wrong-box",
            MeshError::BadSignature => "bad-signature",
            MeshError::KeyChanged(_) => "key-changed",
            MeshError::Refused(_) => "refused",
            MeshError::Remote(_) => "remote",
            MeshError::Unreachable(_) => "unreachable",
            MeshError::Store => "store",
            MeshError::NotFound => "not-found",
            MeshError::Forbidden => "forbidden",
            MeshError::BadAddress => "bad-address",
            MeshError::Open(OpenError::NotText) => "not-text",
            MeshError::Open(OpenError::TooLarge) | MeshError::TooLarge => "too-large",
            MeshError::Open(OpenError::Folder) => "folder",
            MeshError::Open(_) => "not-found",
        }
    }
}

/// A request one box makes of another.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Op {
    /// Owner's box to recipient's: a share for someone there.
    Offer {
        share: String,
        owner: String,
        owner_name: String,
        recipient: String,
        name: String,
        folder: bool,
        size: u64,
        perm: Perm,
    },
    /// Owner's box: the share is gone.
    Revoke { share: String },
    /// Owner's box: the share now allows this.
    Perm { share: String, perm: Perm },
    /// Recipient's box: the person took it.
    Accept { share: String },
    /// Recipient's box: the person turned it down.
    Decline { share: String },
    /// Recipient's box: carry this text file's document on the link.
    Open { share: String, file: FileId },
    /// Recipient's box: stop carrying it.
    Close { share: String, file: FileId },
    /// Recipient's box: what is in the shared folder (or a folder in it).
    List {
        share: String,
        folder: Option<FileId>,
    },
    /// Recipient's box: a file's bytes.
    Read { share: String, file: FileId },
    /// Recipient's box: a file's name and size.
    Stat { share: String, file: FileId },
}

/// The person signed in, as the mesh needs them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// Whose files these are in the cloud (the owner's are `notshared`).
    pub login: String,
    /// Their LosOS Login name: what goes before the `@` in their address.
    /// Shares from other boxes are addressed to this.
    pub public: String,
    /// The name people see.
    pub name: String,
}

impl Person {
    /// Someone whose cloud login is their LosOS Login name.
    pub fn named(login: &str, name: &str) -> Self {
        Person {
            login: login.into(),
            public: login.into(),
            name: name.into(),
        }
    }
}

/// Offers from one box that may wait unanswered at once. Offers are taken
/// for anyone, known here or not, so that a share does not tell another box
/// who has an account here; this keeps a box from filling the table.
const UNANSWERED: u64 = 500;

/// Opens a connection to the box at a name. The real one is
/// [`dial::Dialer`]; tests connect boxes in memory.
pub type DialFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<Wire, MeshError>> + Send>> + Send + Sync,
>;

/// The mesh side of LosOS cloud.
pub struct Mesh {
    /// This box's name, as other boxes reach it.
    pub name: String,
    pub identity: Identity,
    pub db: Db,
    pub hub: Arc<Hub>,
    dial: DialFn,
    links: Mutex<HashMap<String, Arc<Link>>>,
    /// Every live link, also a second one to a box both dialled at once:
    /// a document may be carried on either.
    every: Mutex<Vec<std::sync::Weak<Link>>>,
    dialing: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    keepers: Mutex<HashSet<String>>,
    /// How often [`Mesh::run`] retries what did not get through.
    pub retry: Duration,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn store_err<E>(_: E) -> MeshError {
    MeshError::Store
}

impl Mesh {
    pub fn new(name: String, identity: Identity, db: Db, hub: Arc<Hub>, dial: DialFn) -> Arc<Mesh> {
        Arc::new(Mesh {
            name,
            identity,
            db,
            hub,
            dial,
            links: Mutex::new(HashMap::new()),
            every: Mutex::new(Vec::new()),
            dialing: Mutex::new(HashMap::new()),
            keepers: Mutex::new(HashSet::new()),
            retry: Duration::from_secs(15),
        })
    }

    // ── Links ──────────────────────────────────────────────────────────

    fn open_link(&self, name: &str) -> Option<Arc<Link>> {
        lock(&self.links)
            .get(name)
            .filter(|l| !l.is_closed())
            .cloned()
    }

    /// The link to `name`, dialling it if there is none.
    pub async fn link(self: &Arc<Self>, name: &str) -> Result<Arc<Link>, MeshError> {
        if let Some(l) = self.open_link(name) {
            return Ok(l);
        }
        let gate = lock(&self.dialing)
            .entry(name.to_string())
            .or_default()
            .clone();
        let _held = gate.lock().await;
        if let Some(l) = self.open_link(name) {
            return Ok(l);
        }
        let mut wire = (self.dial)(name.to_string()).await?;
        link::client_handshake(self, name, &mut wire).await?;
        let l = Link::start(self, name.to_string(), wire);
        lock(&self.links).insert(name.to_string(), l.clone());
        self.remember(&l);
        tracing::info!(peer = name, "mesh: linked (dialled)");
        Ok(l)
    }

    /// Take a connection another box opened.
    pub async fn accept(self: &Arc<Self>, mut wire: Wire) -> Result<Arc<Link>, MeshError> {
        let peer = link::server_handshake(self, &mut wire).await?;
        let l = Link::start(self, peer.clone(), wire);
        self.remember(&l);
        let mut links = lock(&self.links);
        if links.get(&peer).is_none_or(|x| x.is_closed()) {
            links.insert(peer.clone(), l.clone());
        }
        drop(links);
        tracing::info!(peer = %peer, "mesh: linked (answered)");
        Ok(l)
    }

    fn remember(&self, l: &Arc<Link>) {
        let mut every = lock(&self.every);
        every.retain(|w| w.upgrade().is_some_and(|l| !l.is_closed()));
        every.push(Arc::downgrade(l));
    }

    /// Every open link to `remote`.
    fn links_to(&self, remote: &str) -> Vec<Arc<Link>> {
        lock(&self.every)
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .filter(|l| l.peer == remote && !l.is_closed())
            .collect()
    }

    /// A box's key, by reaching it at its name.
    async fn learn_key(self: &Arc<Self>, name: &str) -> Result<String, MeshError> {
        self.link(name).await?;
        self.db.pinned(name).ok_or(MeshError::Store)
    }

    fn forget_link(&self, l: &Arc<Link>) {
        let mut links = lock(&self.links);
        if links.get(&l.peer).is_some_and(|x| Arc::ptr_eq(x, l)) {
            links.remove(&l.peer);
        }
    }

    // ── The owner's side ──────────────────────────────────────────────

    /// Share `file` (owned by or editable by `owner`) with `address`.
    pub fn share(
        self: &Arc<Self>,
        owner: &Person,
        file: FileId,
        address: &str,
        perm: Perm,
    ) -> Result<Outgoing, MeshError> {
        let (_, remote) = split_address(address).ok_or(MeshError::BadAddress)?;
        if remote == self.name || perm == Perm::None {
            return Err(MeshError::BadAddress);
        }
        let store = self.hub.store();
        let info = store
            .stat(file)
            .map_err(store_err)?
            .ok_or(MeshError::NotFound)?;
        if store.access(&owner.login, file) != Perm::Edit {
            return Err(MeshError::Forbidden);
        }
        let s = Outgoing {
            id: identity::random_id(),
            owner: owner.login.clone(),
            owner_address: format!("{}@{}", owner.public, self.name),
            owner_name: owner.name.chars().take(120).collect(),
            file_id: file,
            name: info.name,
            folder: info.folder,
            recipient: address.into(),
            remote: remote.into(),
            perm,
            state: OutState::Unsent,
            created: db::now(),
        };
        self.db.add_outgoing(&s).map_err(store_err)?;
        let me = self.clone();
        let remote = remote.to_string();
        tokio::spawn(async move { me.deliver(&remote).await });
        Ok(s)
    }

    /// Send every share for `remote` it has not heard of.
    pub async fn deliver(self: &Arc<Self>, remote: &str) {
        let pending = self.db.unsent_to(remote);
        if pending.is_empty() {
            return;
        }
        let l = match self.link(remote).await {
            Ok(l) => l,
            Err(e) => {
                tracing::info!(peer = remote, error = %e, "mesh: shares wait for the box");
                return;
            }
        };
        for s in pending {
            let size = self
                .hub
                .store()
                .stat(s.file_id)
                .ok()
                .flatten()
                .map_or(0, |i| i.size);
            let op = Op::Offer {
                share: s.id.clone(),
                owner: s.owner_address.clone(),
                owner_name: s.owner_name.clone(),
                recipient: s.recipient.clone(),
                name: s.name.clone(),
                folder: s.folder,
                size,
                perm: s.perm,
            };
            match l.request(op).await {
                Ok(_) => {
                    let _ = self.db.set_out_state(&s.id, OutState::Pending);
                }
                Err(e) => {
                    tracing::info!(peer = remote, error = %e, "mesh: offer not delivered");
                }
            }
        }
    }

    /// Take a share back.
    pub fn unshare(self: &Arc<Self>, owner: &str, id: &str) -> Result<(), MeshError> {
        let s = self
            .db
            .outgoing(id)
            .filter(|s| s.owner == owner)
            .ok_or(MeshError::NotFound)?;
        self.db.remove_outgoing(id).map_err(store_err)?;
        for l in self.links_to(&s.remote) {
            l.detach_share(id);
        }
        let me = self.clone();
        let share = id.to_string();
        tokio::spawn(async move {
            if let Ok(l) = me.link(&s.remote).await {
                let _ = l.request(Op::Revoke { share }).await;
            }
        });
        Ok(())
    }

    /// Change what a share allows.
    pub fn set_perm(self: &Arc<Self>, owner: &str, id: &str, perm: Perm) -> Result<(), MeshError> {
        if perm == Perm::None {
            return self.unshare(owner, id);
        }
        let s = self
            .db
            .outgoing(id)
            .filter(|s| s.owner == owner)
            .ok_or(MeshError::NotFound)?;
        self.db.set_out_perm(id, perm).map_err(store_err)?;
        let access = if perm == Perm::Edit {
            Access::Write
        } else {
            Access::Read
        };
        for l in self.links_to(&s.remote) {
            // Every document of this share the other box has open.
            for f in self.files_open_by(&l, id) {
                l.set_access(&f, access);
            }
        }
        let me = self.clone();
        let share = id.to_string();
        tokio::spawn(async move {
            if let Ok(l) = me.link(&s.remote).await {
                let _ = l.request(Op::Perm { share, perm }).await;
            }
        });
        Ok(())
    }

    fn files_open_by(&self, l: &Arc<Link>, share: &str) -> Vec<String> {
        let Some(s) = self.db.outgoing(share) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut candidates = vec![s.file_id];
        if s.folder {
            candidates.extend(self.descendants(s.file_id));
        }
        for f in candidates {
            let n = doc_name(&s.id, f);
            if l.carries(&n) {
                out.push(n);
            }
        }
        out
    }

    fn descendants(&self, folder: FileId) -> Vec<FileId> {
        let mut out = Vec::new();
        let mut todo = vec![folder];
        while let Some(f) = todo.pop() {
            if out.len() > 10_000 {
                break;
            }
            for c in self.hub.store().list(f).unwrap_or_default() {
                if c.folder {
                    todo.push(c.id);
                }
                out.push(c.id);
            }
        }
        out
    }

    /// Whether `file` is the shared file or inside the shared folder.
    fn within(&self, s: &Outgoing, file: FileId) -> bool {
        if file == s.file_id {
            return true;
        }
        if !s.folder {
            return false;
        }
        let mut at = file;
        for _ in 0..64 {
            match self
                .hub
                .store()
                .stat(at)
                .ok()
                .flatten()
                .and_then(|i| i.parent)
            {
                Some(p) if p == s.file_id => return true,
                Some(p) => at = p,
                None => return false,
            }
        }
        false
    }

    /// What `login` may do with `file` when `login` is a person on another
    /// box (`bob@boxb`). The cloud's store asks this for such logins.
    pub fn guest_access(&self, login: &str, file: FileId) -> Perm {
        let Some((_, remote)) = split_address(login) else {
            return Perm::None;
        };
        self.db
            .outgoing_of_recipient(login, remote)
            .into_iter()
            .filter(|s| s.state == OutState::Accepted && self.within(s, file))
            .map(|s| s.perm)
            .max()
            .unwrap_or(Perm::None)
    }

    /// Shares `owner` sent.
    pub fn outgoing(&self, owner: &str) -> Vec<Outgoing> {
        self.db.outgoing_of(owner)
    }

    // ── The recipient's side ──────────────────────────────────────────

    /// Shares sent to `login` from other boxes.
    pub fn incoming(&self, login: &str) -> Vec<Incoming> {
        self.db.incoming_for(login)
    }

    fn incoming_of(&self, login: &str, id: &str) -> Result<Incoming, MeshError> {
        self.db
            .incoming(id)
            .filter(|s| s.recipient == login)
            .ok_or(MeshError::NotFound)
    }

    /// Accept or decline a share sent to `login`.
    pub async fn answer(
        self: &Arc<Self>,
        login: &str,
        id: &str,
        accept: bool,
    ) -> Result<Incoming, MeshError> {
        let s = self.incoming_of(login, id)?;
        let state = if accept { "accepted" } else { "declined" };
        self.db
            .set_in_state(&s.remote, id, state)
            .map_err(store_err)?;
        let op = if accept {
            Op::Accept { share: id.into() }
        } else {
            Op::Decline { share: id.into() }
        };
        // If the owner's box is out of reach, it learns at the first Open.
        if let Ok(l) = self.link(&s.remote).await {
            let _ = l.request(op).await;
        }
        self.incoming_of(login, id)
    }

    /// The files in a folder shared with `login` (or a folder inside it).
    pub async fn list(
        self: &Arc<Self>,
        login: &str,
        id: &str,
        folder: Option<FileId>,
    ) -> Result<Vec<FileInfo>, MeshError> {
        let s = self.incoming_of(login, id)?;
        if s.state != "accepted" {
            return Err(MeshError::Forbidden);
        }
        let l = self.link(&s.remote).await?;
        let v = l
            .request(Op::List {
                share: id.into(),
                folder,
            })
            .await?;
        serde_json::from_value(v).map_err(|_| MeshError::Protocol)
    }

    /// A file's name in a share sent to `login`, if the owner's box answers.
    pub async fn list_name(
        self: &Arc<Self>,
        login: &str,
        id: &str,
        file: FileId,
    ) -> Option<String> {
        let s = self.incoming_of(login, id).ok()?;
        let l = self.link(&s.remote).await.ok()?;
        let v = l
            .request(Op::Stat {
                share: id.into(),
                file,
            })
            .await
            .ok()?;
        v["name"].as_str().map(str::to_string)
    }

    /// A shared file's name and bytes.
    pub async fn read(
        self: &Arc<Self>,
        login: &str,
        id: &str,
        file: FileId,
    ) -> Result<(String, Vec<u8>), MeshError> {
        let s = self.incoming_of(login, id)?;
        if s.state != "accepted" {
            return Err(MeshError::Forbidden);
        }
        let l = self.link(&s.remote).await?;
        let v = l
            .request(Op::Read {
                share: id.into(),
                file,
            })
            .await?;
        let name = v["name"].as_str().unwrap_or("file").to_string();
        let data = B64
            .decode(v["data"].as_str().unwrap_or(""))
            .map_err(|_| MeshError::Protocol)?;
        Ok((name, data))
    }

    /// Open this box's copy of a shared text file for `login`, and keep it
    /// in step with the owner's box while anyone has it open.
    pub async fn open_copy(
        self: &Arc<Self>,
        login: &str,
        id: &str,
        file: FileId,
    ) -> Result<(Arc<Doc>, Access), MeshError> {
        let s = self.incoming_of(login, id)?;
        if s.state != "accepted" {
            return Err(MeshError::Forbidden);
        }
        let doc = self.hub.open(DocKey::Copy(id.into(), file)).await?;
        let access = if s.perm == Perm::Edit {
            Access::Write
        } else {
            Access::Read
        };
        self.keep(s, file, doc.clone());
        Ok((doc, access))
    }

    /// Start the task that keeps a copy linked, unless one runs.
    fn keep(self: &Arc<Self>, s: Incoming, file: FileId, doc: Arc<Doc>) {
        let name = doc_name(&s.id, file);
        if !lock(&self.keepers).insert(name.clone()) {
            return;
        }
        let me = self.clone();
        tokio::spawn(async move {
            me.keeper(&s, file, doc).await;
            lock(&me.keepers).remove(&name);
        });
    }

    /// Links the copy to the owner's document while someone here has it
    /// open, reconnecting when the link drops; marks it unsynced while it
    /// is edited apart.
    async fn keeper(self: &Arc<Self>, s: &Incoming, file: FileId, doc: Arc<Doc>) {
        let name = doc_name(&s.id, file);
        let mut changes = doc.room.watch();
        changes.borrow_and_update();
        let mut idle = 0;
        loop {
            let up = match self.upstream(s, file, &doc).await {
                Ok(l) => Some(l),
                Err(e) => {
                    tracing::debug!(share = %s.id, error = %e, "mesh: the owner's box is out of reach");
                    None
                }
            };
            let edited = changes.has_changed().unwrap_or(false);
            changes.borrow_and_update();
            match &up {
                Some(_) => {
                    let _ = self.db.set_unsynced(&s.id, false);
                }
                None if edited => {
                    let _ = self.db.set_unsynced(&s.id, true);
                }
                None => {}
            }
            let carried = up.as_ref().is_some_and(|l| l.carries(&name));
            let local = doc.room.peers().saturating_sub(usize::from(carried));
            idle = if local == 0 { idle + 1 } else { 0 };
            if idle >= 3 {
                if let Some(l) = up {
                    let _ = l
                        .request(Op::Close {
                            share: s.id.clone(),
                            file,
                        })
                        .await;
                    l.detach(&name);
                }
                return;
            }
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
    }

    async fn upstream(
        self: &Arc<Self>,
        s: &Incoming,
        file: FileId,
        doc: &Arc<Doc>,
    ) -> Result<Arc<Link>, MeshError> {
        let l = self.link(&s.remote).await?;
        let name = doc_name(&s.id, file);
        if !l.carries(&name) {
            // The owner's box starts sending as soon as it has the request,
            // possibly before its answer arrives; hold that until attached.
            l.expect(&name);
            if let Err(e) = l
                .request(Op::Open {
                    share: s.id.clone(),
                    file,
                })
                .await
            {
                l.unexpect(&name);
                return Err(e);
            }
            // The copy takes whatever the owner's box sends; the owner's
            // box decides what of this box's edits it takes.
            l.attach(&name, doc.clone(), Access::Write);
        }
        Ok(l)
    }

    // ── Requests from other boxes ─────────────────────────────────────

    /// Answer a request from the box at the other end of `l`.
    pub async fn handle(self: &Arc<Self>, l: &Arc<Link>, op: Op) -> Result<Value, MeshError> {
        let peer = l.peer.as_str();
        match op {
            Op::Offer {
                share,
                owner,
                owner_name,
                recipient,
                name,
                folder,
                size,
                perm,
            } => {
                let (login, host) = split_address(&recipient).ok_or(MeshError::BadAddress)?;
                let (_, owner_host) = split_address(&owner).ok_or(MeshError::BadAddress)?;
                // A box can only offer its own people's files, to this box.
                if host != self.name
                    || owner_host != peer
                    || share.len() > 64
                    || !share.bytes().all(|b| b.is_ascii_alphanumeric())
                {
                    return Err(MeshError::Forbidden);
                }
                if self.db.incoming(&share).is_none() && self.db.unanswered_from(peer) >= UNANSWERED
                {
                    return Err(MeshError::TooLarge);
                }
                let clip = |s: String, n: usize| s.chars().take(n).collect::<String>();
                self.db
                    .offer(&Incoming {
                        id: share,
                        remote: peer.into(),
                        owner_remote: owner,
                        owner_name: clip(owner_name, 120),
                        recipient: login.into(),
                        name: clip(name, 255),
                        folder,
                        size,
                        perm,
                        state: "pending".into(),
                        created: db::now(),
                        unsynced: false,
                    })
                    .map_err(store_err)?;
                Ok(json!({}))
            }
            Op::Revoke { share } => {
                if self.db.remove_incoming(peer, &share).map_err(store_err)? {
                    l.detach_share(&share);
                    self.hub.drop_copies(&share);
                }
                Ok(json!({}))
            }
            Op::Perm { share, perm } => {
                self.db.set_in_perm(peer, &share, perm).map_err(store_err)?;
                Ok(json!({}))
            }
            Op::Accept { share } | Op::Decline { share }
                if self.sent_to(&share, peer).is_none() =>
            {
                let _ = share;
                Err(MeshError::NotFound)
            }
            Op::Accept { share } => {
                self.db
                    .set_out_state(&share, OutState::Accepted)
                    .map_err(store_err)?;
                Ok(json!({}))
            }
            Op::Decline { share } => {
                self.db
                    .set_out_state(&share, OutState::Declined)
                    .map_err(store_err)?;
                l.detach_share(&share);
                Ok(json!({}))
            }
            Op::Open { share, file } => {
                let s = self.usable(&share, peer)?;
                if !self.within(&s, file) {
                    return Err(MeshError::NotFound);
                }
                let doc = self.hub.open(DocKey::File(file)).await?;
                let access = if s.perm == Perm::Edit {
                    Access::Write
                } else {
                    Access::Read
                };
                l.attach(&doc_name(&share, file), doc, access);
                Ok(json!({ "perm": s.perm }))
            }
            Op::Close { share, file } => {
                l.detach(&doc_name(&share, file));
                Ok(json!({}))
            }
            Op::List { share, folder } => {
                let s = self.usable(&share, peer)?;
                let folder = folder.unwrap_or(s.file_id);
                if !s.folder || !self.within(&s, folder) {
                    return Err(MeshError::NotFound);
                }
                let mut items = self.hub.store().list(folder).map_err(store_err)?;
                for i in &mut items {
                    // Another box has no use for this box's login names.
                    i.owner = String::new();
                }
                Ok(serde_json::to_value(items).map_err(store_err)?)
            }
            Op::Stat { share, file } => {
                let s = self.usable(&share, peer)?;
                if !self.within(&s, file) {
                    return Err(MeshError::NotFound);
                }
                let mut info = self
                    .hub
                    .store()
                    .stat(file)
                    .map_err(store_err)?
                    .ok_or(MeshError::NotFound)?;
                info.owner = String::new();
                Ok(serde_json::to_value(info).map_err(store_err)?)
            }
            Op::Read { share, file } => {
                let s = self.usable(&share, peer)?;
                if !self.within(&s, file) {
                    return Err(MeshError::NotFound);
                }
                let store = self.hub.store().clone();
                let read = tokio::task::spawn_blocking(
                    move || -> Result<(FileInfo, Vec<u8>), MeshError> {
                        let info = store
                            .stat(file)
                            .map_err(store_err)?
                            .ok_or(MeshError::NotFound)?;
                        if info.folder {
                            return Err(MeshError::NotFound);
                        }
                        if info.size > MAX_READ {
                            return Err(MeshError::TooLarge);
                        }
                        Ok((info, store.read(file).map_err(store_err)?))
                    },
                )
                .await
                .map_err(store_err)??;
                Ok(json!({ "name": read.0.name, "size": read.0.size, "data": B64.encode(read.1) }))
            }
        }
    }

    fn sent_to(&self, share: &str, peer: &str) -> Option<Outgoing> {
        self.db.outgoing(share).filter(|s| s.remote == peer)
    }

    /// A share `peer` may use now. Opening a share it never answered counts
    /// as accepting it: the answer may have been lost while out of reach.
    fn usable(&self, share: &str, peer: &str) -> Result<Outgoing, MeshError> {
        let mut s = self.sent_to(share, peer).ok_or(MeshError::NotFound)?;
        match s.state {
            OutState::Declined => Err(MeshError::Forbidden),
            OutState::Accepted => Ok(s),
            OutState::Pending | OutState::Unsent => {
                self.db
                    .set_out_state(share, OutState::Accepted)
                    .map_err(store_err)?;
                s.state = OutState::Accepted;
                Ok(s)
            }
        }
    }

    // ── Background ────────────────────────────────────────────────────

    /// Retry offers that did not get through and copies edited apart.
    pub async fn run(self: Arc<Self>) {
        loop {
            for remote in self.db.unsent_remotes() {
                self.deliver(&remote).await;
            }
            for s in self.db.unsynced() {
                for file in self.hub.copies_of(&s.id) {
                    if let Ok(doc) = self.hub.open(DocKey::Copy(s.id.clone(), file)).await {
                        self.keep(s.clone(), file, doc);
                    }
                }
            }
            tokio::time::sleep(self.retry).await;
        }
    }
}

/// How [`start`] sets the mesh up.
pub struct Options {
    /// This box's name as other boxes reach it.
    pub name: String,
    /// Where the key, the table and the documents are kept.
    pub dir: std::path::PathBuf,
    pub dialer: dial::Dialer,
}

/// Start sharing across boxes for `cloud`: the key, the table, the
/// documents, the retries, and the cloud's change notices passed on to the
/// documents open in the editor. Call it inside the runtime that serves.
pub fn start(cloud: &Arc<crate::cloud::Cloud>, opts: Options) -> std::io::Result<Arc<Mesh>> {
    use crate::cloud::collab::store_impl::CloudStore;
    use tokio::sync::broadcast::error::RecvError;

    if !identity::valid_box_name(&opts.name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{:?} is not a box name other boxes can reach", opts.name),
        ));
    }
    std::fs::create_dir_all(&opts.dir)?;
    let identity = Identity::load_or_create(&opts.dir.join("identity.pk8"))?;
    let db = Db::open(&opts.dir.join("mesh.db")).map_err(std::io::Error::other)?;
    let store = Arc::new(CloudStore(Arc::downgrade(cloud)));
    let hub = Hub::new(store, opts.dir.join("docs"));
    let mesh = Mesh::new(opts.name, identity, db, hub, opts.dialer.into_fn());
    cloud
        .mesh
        .set(mesh.clone())
        .map_err(|_| std::io::Error::other("the mesh is started already"))?;
    tokio::spawn(mesh.clone().run());
    let mut changes = cloud.subscribe();
    let hub = mesh.hub.clone();
    tokio::spawn(async move {
        loop {
            match changes.recv().await {
                Ok(c) => hub.file_changed(c.file_id, &c.etag),
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, "mesh: change notices dropped");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
    Ok(mesh)
}
