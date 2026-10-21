//! One connection between two boxes, and the handshake that opens it.
//!
//! A link is a WebSocket at `/mesh/v1/link`, dialled by whichever box has
//! something to say. Both sides then use it the same way: JSON requests
//! with an id and their answers in text frames, and the y-sync messages of
//! every document the two share in binary frames, each with the document's
//! name in front. A box keeps one link per other box.
//!
//! The handshake, in text frames, before anything else is read:
//!
//! ```text
//!   dialler  -> hello   { v: 1, from, to, key, nonce: cn }
//!   answerer -> welcome { from: to, key, nonce: sn, sig: S("server", cn, sn, from, to) }
//!   dialler  -> auth    { sig: S("client", cn, sn, from, to) }
//!   answerer -> ready
//! ```
//!
//! Each side signs both nonces and both names with its box key, so neither
//! signature can be replayed on another connection or toward another box.
//! The dialler checks the answer against the key it pinned for the name it
//! dialled (pinning it on first contact). The answerer has only the
//! dialler's word for its name, so the first time it hears from a box it
//! dials that name itself and takes the key it finds there; a box that
//! claims another box's name fails that check.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, watch};

use super::db::Pin;
use super::identity::{self, valid_box_name};
use super::{Mesh, MeshError, Op};
use crate::cloud::collab::hub::Doc;
use crate::cloud::collab::room::{Access, PeerId};

/// A frame on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Text(String),
    Binary(Vec<u8>),
}

/// The two ends of a connection, whatever carries it.
pub type Wire = (mpsc::Sender<Frame>, mpsc::Receiver<Frame>);

/// How long a handshake step or a request may take.
pub const STEP: Duration = Duration::from_secs(15);

/// How often each side says it is still there, and how long a silent link
/// stays open. A connection whose other end vanished (a cable pulled, a
/// box asleep) never closes on its own: TCP would keep it for many minutes.
pub const BEAT: Duration = Duration::from_secs(10);
pub const SILENCE: Duration = Duration::from_secs(30);

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum Handshake {
    Hello {
        v: u32,
        from: String,
        to: String,
        key: String,
        nonce: String,
    },
    Welcome {
        from: String,
        key: String,
        nonce: String,
        sig: String,
    },
    Auth {
        sig: String,
    },
    Ready,
    Refused {
        reason: String,
    },
}

async fn send(tx: &mpsc::Sender<Frame>, h: &Handshake) -> Result<(), MeshError> {
    let text = serde_json::to_string(h).map_err(|_| MeshError::Protocol)?;
    tx.send(Frame::Text(text))
        .await
        .map_err(|_| MeshError::Closed)
}

async fn recv(rx: &mut mpsc::Receiver<Frame>) -> Result<Handshake, MeshError> {
    match tokio::time::timeout(STEP, rx.recv()).await {
        Err(_) => Err(MeshError::Timeout),
        Ok(None) => Err(MeshError::Closed),
        Ok(Some(Frame::Text(t))) => serde_json::from_str(&t).map_err(|_| MeshError::Protocol),
        Ok(Some(Frame::Binary(_))) => Err(MeshError::Protocol),
    }
}

async fn refuse(tx: &mpsc::Sender<Frame>, reason: &str) {
    let _ = send(
        tx,
        &Handshake::Refused {
            reason: reason.into(),
        },
    )
    .await;
}

/// The dialler's side. `to` is the name that was dialled.
pub async fn client_handshake(mesh: &Mesh, to: &str, wire: &mut Wire) -> Result<(), MeshError> {
    let (tx, rx) = wire;
    let me = mesh.name.as_str();
    let cn = identity::nonce();
    send(
        tx,
        &Handshake::Hello {
            v: 1,
            from: me.into(),
            to: to.into(),
            key: mesh.identity.public(),
            nonce: cn.clone(),
        },
    )
    .await?;
    let (key, sn) = match recv(rx).await? {
        Handshake::Welcome {
            from,
            key,
            nonce,
            sig,
        } => {
            if from != to {
                return Err(MeshError::WrongBox);
            }
            if !identity::verify(
                &key,
                &[
                    b"server",
                    cn.as_bytes(),
                    nonce.as_bytes(),
                    me.as_bytes(),
                    to.as_bytes(),
                ],
                &sig,
            ) {
                return Err(MeshError::BadSignature);
            }
            (key, nonce)
        }
        Handshake::Refused { reason } => return Err(MeshError::Refused(reason)),
        _ => return Err(MeshError::Protocol),
    };
    match mesh.db.pin(to, &key).map_err(|_| MeshError::Store)? {
        Pin::New | Pin::Same => {}
        Pin::Changed(_) => return Err(MeshError::KeyChanged(to.into())),
    }
    let sig = mesh.identity.sign(&[
        b"client",
        cn.as_bytes(),
        sn.as_bytes(),
        me.as_bytes(),
        to.as_bytes(),
    ]);
    send(tx, &Handshake::Auth { sig }).await?;
    match recv(rx).await? {
        Handshake::Ready => Ok(()),
        Handshake::Refused { reason } => Err(MeshError::Refused(reason)),
        _ => Err(MeshError::Protocol),
    }
}

/// The answerer's side; the dialler's box name once it checks out.
pub async fn server_handshake(mesh: &Arc<Mesh>, wire: &mut Wire) -> Result<String, MeshError> {
    let (tx, rx) = wire;
    let me = mesh.name.clone();
    let (from, key, cn) = match recv(rx).await? {
        Handshake::Hello {
            v: 1,
            from,
            to,
            key,
            nonce,
        } => {
            if to != me {
                refuse(tx, "wrong-box").await;
                return Err(MeshError::WrongBox);
            }
            if !valid_box_name(&from) || from == me {
                refuse(tx, "bad-name").await;
                return Err(MeshError::Protocol);
            }
            (from, key, nonce)
        }
        _ => {
            refuse(tx, "protocol").await;
            return Err(MeshError::Protocol);
        }
    };
    let sn = identity::nonce();
    let sig = mesh.identity.sign(&[
        b"server",
        cn.as_bytes(),
        sn.as_bytes(),
        from.as_bytes(),
        me.as_bytes(),
    ]);
    send(
        tx,
        &Handshake::Welcome {
            from: me.clone(),
            key: mesh.identity.public(),
            nonce: sn.clone(),
            sig,
        },
    )
    .await?;
    match recv(rx).await? {
        Handshake::Auth { sig } => {
            if !identity::verify(
                &key,
                &[
                    b"client",
                    cn.as_bytes(),
                    sn.as_bytes(),
                    from.as_bytes(),
                    me.as_bytes(),
                ],
                &sig,
            ) {
                refuse(tx, "bad-signature").await;
                return Err(MeshError::BadSignature);
            }
        }
        _ => {
            refuse(tx, "protocol").await;
            return Err(MeshError::Protocol);
        }
    }
    match mesh.db.pinned(&from) {
        Some(k) if k == key => {}
        Some(_) => {
            refuse(tx, "key-changed").await;
            return Err(MeshError::KeyChanged(from));
        }
        None => {
            // Never met: ask the box at that name for its key.
            match mesh.learn_key(&from).await {
                Ok(k) if k == key => {}
                Ok(_) => {
                    refuse(tx, "not-that-box").await;
                    return Err(MeshError::WrongBox);
                }
                Err(e) => {
                    refuse(tx, "unreachable").await;
                    return Err(e);
                }
            }
        }
    }
    send(tx, &Handshake::Ready).await?;
    Ok(from)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum Packet {
    Req {
        id: u64,
        op: Op,
    },
    Res {
        id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ok: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        err: Option<String>,
    },
    /// Nothing to say, but still here.
    Beat,
}

struct Attached {
    doc: Arc<Doc>,
    peer: PeerId,
    access: Access,
    task: tokio::task::AbortHandle,
}

/// An open link to another box.
pub struct Link {
    /// The other box's name, checked by the handshake.
    pub peer: String,
    out: mpsc::Sender<Frame>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>,
    next: AtomicU64,
    docs: Mutex<HashMap<String, Attached>>,
    /// Frames for documents this side asked to open and has not attached
    /// yet, by name.
    early: Mutex<HashMap<String, Vec<Vec<u8>>>>,
    closed: watch::Sender<bool>,
}

/// Frames held for one document that is about to be attached.
const EARLY: usize = 64;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A document's name on the link: the share and the file's id on the
/// owner's box.
pub fn doc_name(share: &str, file: i64) -> String {
    format!("{share}/{file}")
}

fn encode_doc_frame(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + name.len() + payload.len());
    out.push(name.len() as u8);
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(payload);
    out
}

fn decode_doc_frame(frame: &[u8]) -> Option<(&str, &[u8])> {
    let n = *frame.first()? as usize;
    let name = std::str::from_utf8(frame.get(1..1 + n)?).ok()?;
    Some((name, &frame[1 + n..]))
}

impl Link {
    /// A link over an open, handshaken wire. Its reader runs until the
    /// wire closes.
    pub fn start(mesh: &Arc<Mesh>, peer: String, wire: Wire) -> Arc<Link> {
        let (out, rx) = wire;
        let (closed, _) = watch::channel(false);
        let link = Arc::new(Link {
            peer,
            out,
            pending: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            docs: Mutex::new(HashMap::new()),
            early: Mutex::new(HashMap::new()),
            closed,
        });
        tokio::spawn(link.clone().read(Arc::downgrade(mesh), rx));
        tokio::spawn(link.clone().beat());
        link
    }

    async fn beat(self: Arc<Self>) {
        let Ok(text) = serde_json::to_string(&Packet::Beat) else {
            return;
        };
        let mut closed = self.closed.subscribe();
        loop {
            tokio::select! {
                _ = tokio::time::sleep(BEAT) => {}
                _ = closed.changed() => return,
            }
            if self.is_closed() || self.out.send(Frame::Text(text.clone())).await.is_err() {
                return;
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.borrow() || self.out.is_closed()
    }

    /// Wait until the link closes.
    pub async fn wait_closed(&self) {
        let mut rx = self.closed.subscribe();
        while !*rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// Ask the other box something and wait for its answer.
    pub async fn request(&self, op: Op) -> Result<Value, MeshError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        lock(&self.pending).insert(id, tx);
        let text =
            serde_json::to_string(&Packet::Req { id, op }).map_err(|_| MeshError::Protocol)?;
        if self.out.send(Frame::Text(text)).await.is_err() {
            lock(&self.pending).remove(&id);
            return Err(MeshError::Closed);
        }
        match tokio::time::timeout(STEP, rx).await {
            Err(_) => {
                lock(&self.pending).remove(&id);
                Err(MeshError::Timeout)
            }
            Ok(Err(_)) => Err(MeshError::Closed),
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(MeshError::Remote(e)),
        }
    }

    /// Hold frames for `name` until it is attached: this side asked the
    /// other to open it.
    pub fn expect(&self, name: &str) {
        lock(&self.early).entry(name.to_string()).or_default();
    }

    /// Stop holding frames for `name`.
    pub fn unexpect(&self, name: &str) {
        lock(&self.early).remove(name);
    }

    /// Carry a document over this link: the room's messages go out under
    /// `name`, and what arrives under `name` goes into the room as a peer
    /// with `access`.
    pub fn attach(self: &Arc<Self>, name: &str, doc: Arc<Doc>, access: Access) {
        let mut docs = lock(&self.docs);
        if docs.contains_key(name) {
            return;
        }
        let (peer, rx) = doc.room.join(access);
        for frame in lock(&self.early).remove(name).unwrap_or_default() {
            let _ = doc.room.receive(peer, &frame);
        }
        let task = tokio::spawn(self.clone().forward(name.to_string(), rx)).abort_handle();
        docs.insert(
            name.to_string(),
            Attached {
                doc,
                peer,
                access,
                task,
            },
        );
    }

    /// Whether a document is carried.
    pub fn carries(&self, name: &str) -> bool {
        lock(&self.docs).contains_key(name)
    }

    /// Change what the other box may do in a document it has open.
    pub fn set_access(&self, name: &str, access: Access) {
        if let Some(a) = lock(&self.docs).get_mut(name) {
            a.access = access;
            a.doc.room.set_access(a.peer, access);
        }
    }

    /// Stop carrying a document.
    pub fn detach(&self, name: &str) {
        if let Some(a) = lock(&self.docs).remove(name) {
            a.task.abort();
            a.doc.room.leave(a.peer);
        }
    }

    /// Every document whose name starts with `share/`.
    pub fn detach_share(&self, share: &str) {
        let prefix = format!("{share}/");
        let names: Vec<String> = lock(&self.docs)
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        for n in names {
            self.detach(&n);
        }
    }

    async fn forward(self: Arc<Self>, name: String, mut rx: mpsc::Receiver<Vec<u8>>) {
        loop {
            while let Some(m) = rx.recv().await {
                if self
                    .out
                    .send(Frame::Binary(encode_doc_frame(&name, &m)))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            // The room dropped this peer (its queue filled). Join again; the
            // fresh sync step brings the other side back up to date.
            let mut docs = lock(&self.docs);
            let Some(a) = docs.get_mut(&name) else { return };
            if self.is_closed() {
                return;
            }
            let (peer, next) = a.doc.room.join(a.access);
            a.peer = peer;
            rx = next;
        }
    }

    async fn read(self: Arc<Self>, mesh: std::sync::Weak<Mesh>, mut rx: mpsc::Receiver<Frame>) {
        loop {
            let frame = match tokio::time::timeout(SILENCE, rx.recv()).await {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(_) => {
                    tracing::info!(peer = %self.peer, "mesh: the other box went silent");
                    break;
                }
            };
            match frame {
                Frame::Binary(b) => {
                    let Some((name, payload)) = decode_doc_frame(&b) else {
                        continue;
                    };
                    let target = lock(&self.docs).get(name).map(|a| (a.doc.clone(), a.peer));
                    if let Some((doc, peer)) = target {
                        if let Err(e) = doc.room.receive(peer, payload) {
                            tracing::warn!(peer = %self.peer, doc = name, error = %e, "mesh: refused a document message");
                        }
                    } else if let Some(held) = lock(&self.early).get_mut(name) {
                        if held.len() < EARLY {
                            held.push(payload.to_vec());
                        }
                    }
                }
                Frame::Text(t) => match serde_json::from_str::<Packet>(&t) {
                    Ok(Packet::Res { id, ok, err }) => {
                        if let Some(tx) = lock(&self.pending).remove(&id) {
                            let _ = tx.send(match err {
                                Some(e) => Err(e),
                                None => Ok(ok.unwrap_or(Value::Null)),
                            });
                        }
                    }
                    Ok(Packet::Beat) => {}
                    Ok(Packet::Req { id, op }) => {
                        let Some(mesh) = mesh.upgrade() else { break };
                        let link = self.clone();
                        tokio::spawn(async move {
                            let res = match mesh.handle(&link, op).await {
                                Ok(v) => Packet::Res {
                                    id,
                                    ok: Some(v),
                                    err: None,
                                },
                                Err(e) => Packet::Res {
                                    id,
                                    ok: None,
                                    err: Some(e.code().into()),
                                },
                            };
                            if let Ok(text) = serde_json::to_string(&res) {
                                let _ = link.out.send(Frame::Text(text)).await;
                            }
                        });
                    }
                    Err(_) => {
                        tracing::warn!(peer = %self.peer, "mesh: a frame that does not parse");
                    }
                },
            }
        }
        self.close();
        if let Some(mesh) = mesh.upgrade() {
            mesh.forget_link(&self);
        }
    }

    /// Leave every room and fail every waiting request.
    pub fn close(&self) {
        self.closed.send_replace(true);
        let names: Vec<String> = lock(&self.docs).keys().cloned().collect();
        for n in names {
            self.detach(&n);
        }
        lock(&self.pending).clear();
        lock(&self.early).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_frames_round_trip() {
        let f = encode_doc_frame("abc/12", &[1, 2, 3]);
        assert_eq!(decode_doc_frame(&f), Some(("abc/12", &[1u8, 2, 3][..])));
        assert_eq!(decode_doc_frame(&[5, b'a']), None);
        assert_eq!(decode_doc_frame(&[]), None);
    }
}
