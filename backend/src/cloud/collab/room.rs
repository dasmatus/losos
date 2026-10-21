//! One shared document and the people (or boxes) connected to it.
//!
//! A room holds a Yjs document with one text, `content`, and speaks the y-sync
//! protocol (the one `y-websocket` speaks) with every peer that joined it. A
//! peer is anything that sends and receives encoded [`Message`]s: a browser
//! over a WebSocket, or another box's copy of the same document over the mesh
//! link. The room does not know which; it only knows what each peer may do.
//!
//! What the room enforces, because a peer can be another person's box:
//!
//!   * a read-only peer's edits are dropped, its sync answers included;
//!   * an update must decode and stay under [`MAX_MESSAGE`] bytes, and the
//!     text under [`MAX_TEXT`] bytes after it, or it is refused whole;
//!   * a presence entry (cursor, name) belongs to the peer that first sent
//!     it, so one peer cannot move or remove another's cursor;
//!   * a peer whose queue is full is dropped and resyncs when it comes back,
//!     instead of the room buffering for it without limit.
//!
//! Updates are idempotent and commute, so the room forwards what it applied
//! as it came, and a message seen twice or out of order does no harm.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::{mpsc, watch};
use yrs::encoding::read::Cursor;
use yrs::sync::awareness::AwarenessUpdateEntry;
use yrs::sync::{AwarenessUpdate, Message, MessageReader, SyncMessage};
use yrs::updates::decoder::{Decode, DecoderV1};
use yrs::updates::encoder::Encode;
use yrs::{
    ClientID, Doc, GetString, OffsetKind, Options, ReadTxn, StateVector, Text, TextRef, Transact,
    Update,
};

/// The name of the one text in every document, the same in the browser.
pub const TEXT: &str = "content";
/// The largest message a peer may send. An update of a whole 2 MiB file fits.
pub const MAX_MESSAGE: usize = 4 * 1024 * 1024;
/// The largest text a document may grow to, in UTF-8 bytes.
pub const MAX_TEXT: usize = 2 * 1024 * 1024;
/// Messages queued for one peer before it counts as gone.
pub const QUEUE: usize = 256;

/// What a peer may do in a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

/// A peer's number in its room.
pub type PeerId = u64;

/// Why a message was not taken.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Refused {
    #[error("message too large")]
    TooLarge,
    #[error("message does not decode")]
    Malformed,
    #[error("the text would grow past its limit")]
    TextTooLarge,
    #[error("no such peer")]
    Gone,
}

struct Peer {
    tx: mpsc::Sender<Vec<u8>>,
    access: Access,
}

struct Presence {
    clock: u32,
    json: std::sync::Arc<str>,
    owner: PeerId,
}

struct Inner {
    doc: Doc,
    text: TextRef,
    peers: HashMap<PeerId, Peer>,
    presence: HashMap<u64, Presence>,
    next: PeerId,
}

/// One document and its peers.
pub struct Room {
    inner: Mutex<Inner>,
    /// Counts applied changes; a saver waits on it.
    revision: watch::Sender<u64>,
}

impl Default for Room {
    fn default() -> Self {
        Self::new()
    }
}

impl Room {
    /// An empty document.
    pub fn new() -> Self {
        // Yjs in the browser counts text positions in UTF-16 code units; the
        // box must count the same way or its own edits land off by a few.
        let doc = Doc::with_options(Options {
            offset_kind: OffsetKind::Utf16,
            ..Options::default()
        });
        let text = doc.get_or_insert_text(TEXT);
        let (revision, _) = watch::channel(0);
        Room {
            inner: Mutex::new(Inner {
                doc,
                text,
                peers: HashMap::new(),
                presence: HashMap::new(),
                next: 1,
            }),
            revision,
        }
    }

    /// A document from its saved state (`encode_state`), or `None` if the
    /// bytes do not decode.
    pub fn from_state(state: &[u8]) -> Option<Self> {
        let room = Room::new();
        let update = Update::decode_v1(state).ok()?;
        {
            let inner = room.lock();
            let mut txn = inner.doc.transact_mut();
            txn.apply_update(update).ok()?;
        }
        Some(room)
    }

    /// A document holding `text` and nothing else.
    pub fn from_text(text: &str) -> Self {
        let room = Room::new();
        {
            let inner = room.lock();
            let mut txn = inner.doc.transact_mut();
            inner.text.insert(&mut txn, 0, text);
        }
        room
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The text as it stands.
    pub fn text(&self) -> String {
        let inner = self.lock();
        let txn = inner.doc.transact();
        inner.text.get_string(&txn)
    }

    /// The whole document, to save and to load with [`Room::from_state`].
    pub fn encode_state(&self) -> Vec<u8> {
        let inner = self.lock();
        let txn = inner.doc.transact();
        txn.encode_state_as_update_v1(&StateVector::default())
    }

    /// A receiver that changes every time the text may have changed.
    pub fn watch(&self) -> watch::Receiver<u64> {
        self.revision.subscribe()
    }

    /// How many peers are connected.
    pub fn peers(&self) -> usize {
        self.lock().peers.len()
    }

    /// Connect a peer. It is sent this side's state vector, so it answers
    /// with what the room lacks, and everyone's presence.
    pub fn join(&self, access: Access) -> (PeerId, mpsc::Receiver<Vec<u8>>) {
        let (tx, rx) = mpsc::channel(QUEUE);
        let mut inner = self.lock();
        let id = inner.next;
        inner.next += 1;
        let sv = inner.doc.transact().state_vector();
        let _ = tx.try_send(Message::Sync(SyncMessage::SyncStep1(sv)).encode_v1());
        if !inner.presence.is_empty() {
            let clients = inner
                .presence
                .iter()
                .map(|(c, p)| {
                    (
                        ClientID::new(*c),
                        AwarenessUpdateEntry {
                            clock: p.clock,
                            json: p.json.clone(),
                        },
                    )
                })
                .collect();
            let _ = tx.try_send(Message::Awareness(AwarenessUpdate { clients }).encode_v1());
        }
        inner.peers.insert(id, Peer { tx, access });
        (id, rx)
    }

    /// Disconnect everyone, as when the document is taken away.
    pub fn close_all(&self) {
        let mut inner = self.lock();
        inner.peers.clear();
        inner.presence.clear();
    }

    /// Change what a connected peer may do (a share went read-only).
    pub fn set_access(&self, peer: PeerId, access: Access) {
        if let Some(p) = self.lock().peers.get_mut(&peer) {
            p.access = access;
        }
    }

    /// Disconnect a peer and clear its cursors for everyone else.
    pub fn leave(&self, peer: PeerId) {
        let mut inner = self.lock();
        inner.peers.remove(&peer);
        let gone: Vec<u64> = inner
            .presence
            .iter()
            .filter(|(_, p)| p.owner == peer)
            .map(|(c, _)| *c)
            .collect();
        if gone.is_empty() {
            return;
        }
        let mut clients = HashMap::new();
        for c in gone {
            if let Some(p) = inner.presence.remove(&c) {
                clients.insert(
                    ClientID::new(c),
                    AwarenessUpdateEntry {
                        clock: p.clock + 1,
                        json: "null".into(),
                    },
                );
            }
        }
        let msg = Message::Awareness(AwarenessUpdate { clients }).encode_v1();
        Self::broadcast(&mut inner, None, &msg);
    }

    /// Take one frame from a peer: one or more encoded messages.
    pub fn receive(&self, peer: PeerId, frame: &[u8]) -> Result<(), Refused> {
        if frame.len() > MAX_MESSAGE {
            return Err(Refused::TooLarge);
        }
        let messages = decode_all(frame).ok_or(Refused::Malformed)?;
        let mut inner = self.lock();
        let access = inner.peers.get(&peer).ok_or(Refused::Gone)?.access;
        let mut changed = false;
        for message in messages {
            match message {
                Message::Sync(SyncMessage::SyncStep1(sv)) => {
                    let diff = inner.doc.transact().encode_diff_v1(&sv);
                    let reply = Message::Sync(SyncMessage::SyncStep2(diff)).encode_v1();
                    Self::send(&mut inner, peer, reply);
                }
                Message::Sync(SyncMessage::SyncStep2(update))
                | Message::Sync(SyncMessage::Update(update)) => {
                    if access == Access::Read {
                        continue;
                    }
                    if Self::apply(&mut inner, &update)? {
                        changed = true;
                        let out = Message::Sync(SyncMessage::Update(update)).encode_v1();
                        Self::broadcast(&mut inner, Some(peer), &out);
                    }
                }
                Message::Awareness(update) => {
                    let mut clients = HashMap::new();
                    for (client, entry) in update.clients {
                        let c = client.get();
                        match inner.presence.get(&c) {
                            Some(p) if p.owner != peer => continue,
                            Some(p) if p.clock >= entry.clock => continue,
                            _ => {}
                        }
                        if &*entry.json == "null" {
                            inner.presence.remove(&c);
                        } else {
                            inner.presence.insert(
                                c,
                                Presence {
                                    clock: entry.clock,
                                    json: entry.json.clone(),
                                    owner: peer,
                                },
                            );
                        }
                        clients.insert(client, entry);
                    }
                    if !clients.is_empty() {
                        let out = Message::Awareness(AwarenessUpdate { clients }).encode_v1();
                        Self::broadcast(&mut inner, Some(peer), &out);
                    }
                }
                Message::AwarenessQuery | Message::Auth(_) | Message::Custom(..) => {}
            }
        }
        drop(inner);
        if changed {
            self.revision.send_modify(|r| *r += 1);
        }
        Ok(())
    }

    /// Make the text read `new`, as the smallest edits a line diff finds,
    /// and send them to every peer. The update, or `None` if the text
    /// already read `new`.
    pub fn replace_text(&self, new: &str) -> Option<Vec<u8>> {
        let mut inner = self.lock();
        let update = {
            let inner = &*inner;
            let mut txn = inner.doc.transact_mut();
            let old = inner.text.get_string(&txn);
            if old == new {
                return None;
            }
            for edit in super::diff::edits(&old, new).into_iter().rev() {
                if edit.delete > 0 {
                    inner.text.remove_range(&mut txn, edit.at, edit.delete);
                }
                if !edit.insert.is_empty() {
                    inner.text.insert(&mut txn, edit.at, &edit.insert);
                }
            }
            txn.encode_update_v1()
        };
        let out = Message::Sync(SyncMessage::Update(update.clone())).encode_v1();
        Self::broadcast(&mut inner, None, &out);
        drop(inner);
        self.revision.send_modify(|r| *r += 1);
        Some(update)
    }

    /// Fold in a version of the file written outside the editor.
    ///
    /// `base` is the state this room had when its text last matched the
    /// file. The new version is applied as edits to a copy of that state,
    /// and the copy's update merged here, so it lands as one more person's
    /// concurrent edit and whatever was typed since `base` survives.
    ///
    /// Returns the copy's state, whose text is `theirs`: the caller's next
    /// base. Merging the same version twice from the same base would be two
    /// people making the same edit. `None` if the merge was refused (the
    /// text would pass its limit).
    pub fn merge_external(&self, base: &[u8], theirs: &str) -> Option<Vec<u8>> {
        let fork = Room::from_state(base).unwrap_or_default();
        let update = fork.replace_text(theirs);
        let fork_state = fork.encode_state();
        let Some(update) = update else {
            return Some(fork_state);
        };
        let mut inner = self.lock();
        if Self::apply(&mut inner, &update).is_err() {
            return None;
        }
        let out = Message::Sync(SyncMessage::Update(update)).encode_v1();
        Self::broadcast(&mut inner, None, &out);
        drop(inner);
        self.revision.send_modify(|r| *r += 1);
        Some(fork_state)
    }

    /// The saved state and its text, read at one instant.
    pub fn snapshot(&self) -> (Vec<u8>, String) {
        let inner = self.lock();
        let txn = inner.doc.transact();
        (
            txn.encode_state_as_update_v1(&StateVector::default()),
            inner.text.get_string(&txn),
        )
    }

    /// This side's state vector, encoded.
    pub fn state_vector(&self) -> Vec<u8> {
        self.lock().doc.transact().state_vector().encode_v1()
    }

    /// Apply an update; `Ok(false)` when it changed nothing.
    fn apply(inner: &mut Inner, bytes: &[u8]) -> Result<bool, Refused> {
        let update = Update::decode_v1(bytes).map_err(|_| Refused::Malformed)?;
        if update.is_empty() {
            return Ok(false);
        }
        // An update cannot add more text than its own length, so this
        // bound refuses everything that could pass the limit, before any of
        // it is applied; there is no undoing half an update.
        let before = inner.doc.transact().state_vector();
        let len = inner.text.get_string(&inner.doc.transact()).len();
        if len + bytes.len() > MAX_TEXT {
            return Err(Refused::TextTooLarge);
        }
        let mut txn = inner.doc.transact_mut();
        txn.apply_update(update).map_err(|_| Refused::Malformed)?;
        drop(txn);
        Ok(inner.doc.transact().state_vector() != before)
    }

    fn send(inner: &mut Inner, peer: PeerId, msg: Vec<u8>) {
        let full = match inner.peers.get(&peer) {
            Some(p) => p.tx.try_send(msg).is_err(),
            None => false,
        };
        if full {
            inner.peers.remove(&peer);
        }
    }

    fn broadcast(inner: &mut Inner, except: Option<PeerId>, msg: &[u8]) {
        let mut dead = Vec::new();
        for (id, p) in &inner.peers {
            if Some(*id) != except && p.tx.try_send(msg.to_vec()).is_err() {
                dead.push(*id);
            }
        }
        for id in dead {
            inner.peers.remove(&id);
        }
    }
}

/// Every message in a frame, or `None` if any of it fails to decode.
fn decode_all(frame: &[u8]) -> Option<Vec<Message>> {
    let mut decoder = DecoderV1::new(Cursor::new(frame));
    let out = MessageReader::new(&mut decoder)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if out.is_empty() {
        return None;
    }
    Some(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Two rooms joined the way a box joins another's document: `down`
    /// is a peer of `up` with `access`, and `up` is a peer of `down` with
    /// write access (the copy takes whatever the owner sends).
    pub(crate) struct Link {
        pub up_peer: PeerId,
        pub down_peer: PeerId,
        up_rx: mpsc::Receiver<Vec<u8>>,
        down_rx: mpsc::Receiver<Vec<u8>>,
    }

    pub(crate) fn link(up: &Room, down: &Room, access: Access) -> Link {
        let (up_peer, up_rx) = up.join(access);
        let (down_peer, down_rx) = down.join(Access::Write);
        Link {
            up_peer,
            down_peer,
            up_rx,
            down_rx,
        }
    }

    /// Carry messages both ways until nothing is left in flight.
    pub(crate) fn pump(links: &mut [(&Room, &Room, &mut Link)]) {
        loop {
            let mut moved = false;
            for (up, down, l) in links.iter_mut() {
                while let Ok(m) = l.up_rx.try_recv() {
                    // What `up` sends to its peer arrives at `down`.
                    let _ = down.receive(l.down_peer, &m);
                    moved = true;
                }
                while let Ok(m) = l.down_rx.try_recv() {
                    let _ = up.receive(l.up_peer, &m);
                    moved = true;
                }
            }
            if !moved {
                return;
            }
        }
    }

    fn type_at_end(room: &Room, s: &str) {
        let t = room.text();
        room.replace_text(&format!("{t}{s}"));
    }

    #[test]
    fn three_editors_on_two_boxes_converge() {
        let a = Room::from_text("# Notes\n");
        let b = Room::new();
        let alice = Room::new();
        let bob = Room::new();
        let mut ab = link(&a, &b, Access::Write);
        let mut a_alice = link(&a, &alice, Access::Write);
        let mut b_bob = link(&b, &bob, Access::Write);
        pump(&mut [
            (&a, &b, &mut ab),
            (&a, &alice, &mut a_alice),
            (&b, &bob, &mut b_bob),
        ]);
        assert_eq!(bob.text(), "# Notes\n");

        type_at_end(&alice, "from alice\n");
        type_at_end(&bob, "from bob\n");
        pump(&mut [
            (&a, &b, &mut ab),
            (&a, &alice, &mut a_alice),
            (&b, &bob, &mut b_bob),
        ]);
        for r in [&a, &b, &alice, &bob] {
            assert_eq!(r.text(), a.text());
        }
        assert!(a.text().contains("from alice\n") && a.text().contains("from bob\n"));
    }

    #[test]
    fn edits_made_apart_merge_when_the_boxes_meet_again() {
        let a = Room::from_text("one\ntwo\nthree\n");
        let b = Room::new();
        let mut ab = link(&a, &b, Access::Write);
        pump(&mut [(&a, &b, &mut ab)]);
        // The link goes down.
        a.leave(ab.up_peer);
        b.leave(ab.down_peer);
        drop(ab);
        a.replace_text("ONE\ntwo\nthree\n");
        b.replace_text("one\ntwo\nTHREE\n");
        // Both kept their copy and come back with what they did.
        let state = b.encode_state();
        let b = Room::from_state(&state).expect("saved state loads");
        let mut ab = link(&a, &b, Access::Write);
        pump(&mut [(&a, &b, &mut ab)]);
        assert_eq!(a.text(), "ONE\ntwo\nTHREE\n");
        assert_eq!(b.text(), a.text());
    }

    #[test]
    fn a_read_only_box_cannot_change_the_owners_text() {
        let a = Room::from_text("hands off\n");
        let b = Room::new();
        let mut ab = link(&a, &b, Access::Read);
        pump(&mut [(&a, &b, &mut ab)]);
        assert_eq!(b.text(), "hands off\n");
        b.replace_text("vandalised\n");
        pump(&mut [(&a, &b, &mut ab)]);
        assert_eq!(a.text(), "hands off\n");
        // And it still gets the owner's edits.
        a.replace_text("hands off\nstill\n");
        pump(&mut [(&a, &b, &mut ab)]);
        assert!(b.text().contains("still"));
    }

    fn presence(client: u64, clock: u32, json: &str) -> Vec<u8> {
        let mut clients = HashMap::new();
        clients.insert(
            ClientID::new(client),
            AwarenessUpdateEntry {
                clock,
                json: json.into(),
            },
        );
        Message::Awareness(AwarenessUpdate { clients }).encode_v1()
    }

    fn drain(rx: &mut mpsc::Receiver<Vec<u8>>) -> Vec<Message> {
        let mut out = Vec::new();
        while let Ok(m) = rx.try_recv() {
            out.extend(decode_all(&m).unwrap());
        }
        out
    }

    #[test]
    fn a_cursor_belongs_to_the_peer_that_sent_it() {
        let a = Room::new();
        let (p1, mut rx1) = a.join(Access::Write);
        let (p2, mut rx2) = a.join(Access::Read);
        drain(&mut rx1);
        drain(&mut rx2);
        a.receive(p1, &presence(7, 1, r#"{"user":{"name":"alice"}}"#))
            .unwrap();
        // Read-only peers see cursors too.
        assert!(matches!(drain(&mut rx2)[..], [Message::Awareness(_)]));
        // Peer 2 tries to move and then remove alice's cursor.
        a.receive(p2, &presence(7, 5, r#"{"user":{"name":"mallory"}}"#))
            .unwrap();
        a.receive(p2, &presence(7, 6, "null")).unwrap();
        assert!(drain(&mut rx1).is_empty());
        // A newcomer still sees alice.
        let (_, mut rx3) = a.join(Access::Write);
        let got = drain(&mut rx3);
        let Message::Awareness(u) = &got[1] else {
            panic!("{got:?}")
        };
        assert!(u.clients[&ClientID::new(7)].json.contains("alice"));
        // When alice's connection goes, her cursor goes for everyone.
        a.leave(p1);
        let got = drain(&mut rx2);
        let Message::Awareness(u) = &got[0] else {
            panic!("{got:?}")
        };
        assert_eq!(&*u.clients[&ClientID::new(7)].json, "null");
    }

    #[test]
    fn oversized_and_malformed_messages_are_refused() {
        let a = Room::new();
        let (p, _rx) = a.join(Access::Write);
        assert_eq!(
            a.receive(p, &vec![0u8; MAX_MESSAGE + 1]),
            Err(Refused::TooLarge)
        );
        assert_eq!(a.receive(p, &[0, 9, 9, 9]), Err(Refused::Malformed));
        assert_eq!(a.receive(p, &[]), Err(Refused::Malformed));
        // An update that would push the text past its limit.
        let big = Room::from_text(&"x".repeat(MAX_TEXT + 10));
        let update = Message::Sync(SyncMessage::Update(big.encode_state())).encode_v1();
        assert_eq!(a.receive(p, &update), Err(Refused::TextTooLarge));
        assert_eq!(a.text(), "");
    }

    #[test]
    fn a_queue_that_fills_drops_its_peer() {
        let a = Room::new();
        let (_slow, _rx) = a.join(Access::Write);
        for i in 0..QUEUE + 5 {
            a.replace_text(&format!("{i}"));
        }
        assert_eq!(a.peers(), 0);
    }

    #[test]
    fn an_outside_write_merges_with_what_was_typed_since() {
        let a = Room::from_text("title\n\nbody\n");
        let (base, _) = a.snapshot();
        // Someone types in the editor...
        a.replace_text("title\n\nbody, more\n");
        // ...while a sync app uploads a version made from the old file.
        let next = a.merge_external(&base, "New title\n\nbody\n").unwrap();
        assert_eq!(
            Room::from_state(&next).unwrap().text(),
            "New title\n\nbody\n"
        );
        assert_eq!(a.text(), "New title\n\nbody, more\n");
    }
}
