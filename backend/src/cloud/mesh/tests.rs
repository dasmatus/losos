//! Boxes talking to each other in memory: the handshake, shares, and two
//! people on two boxes editing one file, with the cable between them cut
//! and put back.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use tokio::sync::{mpsc, watch};

use super::link::{Frame, Wire};
use super::*;
use crate::cloud::collab::hub::Hub;
use crate::cloud::collab::room::Room;
use crate::cloud::collab::store::{CollabStore, MemStore};

/// Someone signed in whose cloud login is their LosOS Login name.
fn p(login: &str) -> Person {
    let mut c = login.chars();
    let name: String = c
        .next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default();
    Person::named(login, &name)
}

/// The network: boxes by name, and whether the cable is plugged in.
#[derive(Default)]
struct Net {
    boxes: Mutex<HashMap<String, Weak<Mesh>>>,
    up: Mutex<Option<watch::Sender<bool>>>,
}

impl Net {
    fn new() -> Arc<Net> {
        let n = Arc::new(Net::default());
        *n.up.lock().unwrap() = Some(watch::channel(true).0);
        n
    }

    fn plugged(&self) -> watch::Receiver<bool> {
        self.up.lock().unwrap().as_ref().unwrap().subscribe()
    }

    /// Pull the cable: every link drops and no dial gets through.
    fn cut(&self) {
        self.up
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .send_replace(false);
    }

    fn plug(&self) {
        self.up.lock().unwrap().as_ref().unwrap().send_replace(true);
    }

    fn dialer(self: &Arc<Self>) -> DialFn {
        let net = self.clone();
        Arc::new(move |name: String| {
            let net = net.clone();
            Box::pin(async move {
                if !*net.plugged().borrow() {
                    return Err(MeshError::Unreachable(name));
                }
                let target = net.boxes.lock().unwrap().get(&name).and_then(Weak::upgrade);
                let Some(target) = target else {
                    return Err(MeshError::Unreachable(name));
                };
                let (client, server) = net.cable();
                tokio::spawn(async move {
                    let _ = target.accept(server).await;
                });
                Ok(client)
            })
        })
    }

    /// Two wires joined through relays that stop when the cable is cut.
    fn cable(&self) -> (Wire, Wire) {
        let (a_out, a_out_rx) = mpsc::channel::<Frame>(512);
        let (a_in_tx, a_in) = mpsc::channel::<Frame>(512);
        let (b_out, b_out_rx) = mpsc::channel::<Frame>(512);
        let (b_in_tx, b_in) = mpsc::channel::<Frame>(512);
        for (mut from, to) in [(a_out_rx, b_in_tx), (b_out_rx, a_in_tx)] {
            let mut up = self.plugged();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        f = from.recv() => match f {
                            Some(f) => if to.send(f).await.is_err() { return },
                            None => return,
                        },
                        r = up.changed() => if r.is_err() || !*up.borrow() { return },
                    }
                }
            });
        }
        ((a_out, a_in), (b_out, b_in))
    }

    fn add(self: &Arc<Self>, name: &str, _people: Vec<&'static str>) -> (Arc<Mesh>, Arc<MemStore>) {
        let store = Arc::new(MemStore::new());
        let hub = Hub::with_timing(
            store.clone() as Arc<dyn CollabStore>,
            tempdir(name),
            Duration::from_millis(20),
            Duration::from_millis(300),
        );
        let mut mesh = Mesh::new(
            name.into(),
            Identity::ephemeral(),
            Db::memory().unwrap(),
            hub,
            self.dialer(),
        );
        Arc::get_mut(&mut mesh).unwrap().retry = Duration::from_millis(200);
        self.boxes
            .lock()
            .unwrap()
            .insert(name.into(), Arc::downgrade(&mesh));
        (mesh, store)
    }
}

fn tempdir(name: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!(
        "losos-mesh-{}-{}-{name}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Wait until `f` holds, or fail after a few seconds.
async fn eventually(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..200 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("never happened: {what}");
}

/// A browser: a room of its own, joined to a document as one peer.
struct Browser {
    room: Arc<Room>,
    task: tokio::task::JoinHandle<()>,
}

impl Browser {
    fn open(doc: &Arc<crate::cloud::collab::hub::Doc>, access: Access) -> Browser {
        let room = Arc::new(Room::new());
        let (doc_peer, mut from_doc) = doc.room.join(access);
        let (mine, mut from_me) = room.join(Access::Write);
        let (d, r) = (doc.clone(), room.clone());
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    m = from_doc.recv() => match m { Some(m) => { let _ = r.receive(mine, &m); } None => return },
                    m = from_me.recv() => match m { Some(m) => { let _ = d.room.receive(doc_peer, &m); } None => return },
                }
            }
        });
        Browser { room, task }
    }

    fn type_at_end(&self, s: &str) {
        let t = self.room.text();
        self.room.replace_text(&format!("{t}{s}"));
    }

    fn text(&self) -> String {
        self.room.text()
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn boxes_pin_each_others_keys() {
    let net = Net::new();
    let (a, _) = net.add("mattbox", vec!["matus"]);
    let (b, _) = net.add("kitchenbox", vec!["eva"]);
    a.link("kitchenbox").await.unwrap();
    assert_eq!(a.db.pinned("kitchenbox"), Some(b.identity.public()));
    eventually("kitchenbox pins mattbox", || {
        b.db.pinned("mattbox").is_some()
    })
    .await;
    assert_eq!(b.db.pinned("mattbox"), Some(a.identity.public()));
}

#[tokio::test]
async fn a_box_that_changes_its_key_is_refused() {
    let net = Net::new();
    let (a, _) = net.add("mattbox", vec!["matus"]);
    let (_b, _) = net.add("kitchenbox", vec!["eva"]);
    a.link("kitchenbox").await.unwrap();
    // Something else answers at kitchenbox now.
    net.cut();
    tokio::time::sleep(Duration::from_millis(50)).await;
    net.plug();
    let (_imposter, _) = net.add("kitchenbox", vec!["eva"]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let err = a.link("kitchenbox").await.err().expect("refused");
    assert!(
        matches!(err, MeshError::KeyChanged(ref n) if n == "kitchenbox"),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_box_cannot_dial_in_under_another_boxs_name() {
    let net = Net::new();
    let (a, _) = net.add("mattbox", vec!["matus"]);
    let (_b, _) = net.add("kitchenbox", vec!["eva"]);
    // A third box, not on the network, claims to be kitchenbox.
    let liar = Mesh::new(
        "kitchenbox".into(),
        Identity::ephemeral(),
        Db::memory().unwrap(),
        Hub::new(Arc::new(MemStore::new()), tempdir("liar")),
        net.dialer(),
    );
    let (client, server) = net.cable();
    let a2 = a.clone();
    let answer = tokio::spawn(async move { a2.accept(server).await });
    let mut client = client;
    let dialled = link::client_handshake(&liar, "mattbox", &mut client).await;
    assert!(dialled.is_err());
    assert!(matches!(answer.await.unwrap(), Err(MeshError::WrongBox)));
    // mattbox learnt the real kitchenbox's key on the way, not the liar's.
    assert_ne!(a.db.pinned("kitchenbox"), Some(liar.identity.public()));
}

#[tokio::test]
async fn two_people_on_two_boxes_edit_one_file() {
    let net = Net::new();
    let (a, store) = net.add("mattbox", vec!["matus"]);
    let (b, _) = net.add("kitchenbox", vec!["eva"]);
    let file = store.add("matus", None, "Holiday 2026.md", Some(b"# Holiday 2026\n"));

    let s = a
        .share(&p("matus"), file, "eva@kitchenbox", Perm::Edit)
        .unwrap();
    eventually("the offer arrives", || b.incoming("eva").len() == 1).await;
    let got = &b.incoming("eva")[0];
    assert_eq!(
        (
            got.owner_remote.as_str(),
            got.owner_name.as_str(),
            got.state.as_str()
        ),
        ("matus@mattbox", "Matus", "pending")
    );
    assert_eq!(a.outgoing("matus")[0].state, OutState::Pending);

    b.answer("eva", &s.id, true).await.unwrap();
    eventually("mattbox sees the answer", || {
        a.outgoing("matus")[0].state == OutState::Accepted
    })
    .await;

    let matus = Browser::open(
        &a.hub.open(DocKey::File(file)).await.unwrap(),
        Access::Write,
    );
    let (copy, access) = b.open_copy("eva", &s.id, file).await.unwrap();
    assert_eq!(access, Access::Write);
    let eva = Browser::open(&copy, access);
    eventually("eva sees the file", || eva.text() == "# Holiday 2026\n").await;

    matus.type_at_end("Train to Vienna on Friday.\n");
    eva.type_at_end("Book the flat by Tuesday.\n");
    eventually("both see both lines", || {
        matus.text() == eva.text()
            && matus.text().contains("Vienna")
            && matus.text().contains("Tuesday")
    })
    .await;
    eventually("the file on mattbox has both", || {
        let c = store.content(file);
        c.contains("Vienna") && c.contains("Tuesday")
    })
    .await;
    // Only mattbox has the file; kitchenbox never wrote one.
    assert_eq!(b.hub.store().stat(file).unwrap(), None);
}

#[tokio::test]
async fn edits_made_while_apart_merge_when_the_boxes_meet() {
    let net = Net::new();
    let (a, store) = net.add("mattbox", vec!["matus"]);
    let (b, _) = net.add("kitchenbox", vec!["eva"]);
    tokio::spawn(b.clone().run());
    let file = store.add("matus", None, "list.txt", Some(b"milk\n"));
    let s = a
        .share(&p("matus"), file, "eva@kitchenbox", Perm::Edit)
        .unwrap();
    eventually("offer", || b.incoming("eva").len() == 1).await;
    b.answer("eva", &s.id, true).await.unwrap();
    let matus = Browser::open(
        &a.hub.open(DocKey::File(file)).await.unwrap(),
        Access::Write,
    );
    let (copy, access) = b.open_copy("eva", &s.id, file).await.unwrap();
    let eva = Browser::open(&copy, access);
    eventually("in step", || eva.text() == "milk\n").await;

    net.cut();
    matus.type_at_end("bread\n");
    eva.type_at_end("eggs\n");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!eva.text().contains("bread"), "the cable is out");
    assert!(
        b.db.incoming(&s.id).unwrap().unsynced,
        "kitchenbox knows it has edits to send"
    );
    // Eva closes the editor while still apart.
    drop(eva);

    net.plug();
    eventually("mattbox gets the eggs", || matus.text().contains("eggs")).await;
    assert!(matus.text().contains("bread") && matus.text().contains("milk"));
    eventually("written to the file", || {
        store.content(file).contains("eggs")
    })
    .await;
    eventually("kitchenbox marks it synced", || {
        !b.db.incoming(&s.id).unwrap().unsynced
    })
    .await;
}

#[tokio::test]
async fn a_read_only_share_cannot_change_the_file() {
    let net = Net::new();
    let (a, store) = net.add("mattbox", vec!["matus"]);
    let (b, _) = net.add("kitchenbox", vec!["eva"]);
    let file = store.add("matus", None, "budget.txt", Some(b"rent 900\n"));
    let s = a
        .share(&p("matus"), file, "eva@kitchenbox", Perm::Read)
        .unwrap();
    eventually("offer", || b.incoming("eva").len() == 1).await;
    b.answer("eva", &s.id, true).await.unwrap();
    let (copy, access) = b.open_copy("eva", &s.id, file).await.unwrap();
    assert_eq!(access, Access::Read);
    let eva = Browser::open(&copy, access);
    eventually("eva sees it", || eva.text() == "rent 900\n").await;
    // A tampered kitchenbox writes into its copy anyway.
    copy.room.replace_text("rent 0\n");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(store.content(file), "rent 900\n");
    let owner = a.hub.open(DocKey::File(file)).await.unwrap();
    assert_eq!(owner.room.text(), "rent 900\n");
}

#[tokio::test]
async fn taking_a_share_back_removes_it_from_the_other_box() {
    let net = Net::new();
    let (a, store) = net.add("mattbox", vec!["matus"]);
    let (b, _) = net.add("kitchenbox", vec!["eva"]);
    let file = store.add("matus", None, "notes.md", Some(b"x\n"));
    let s = a
        .share(&p("matus"), file, "eva@kitchenbox", Perm::Edit)
        .unwrap();
    eventually("offer", || b.incoming("eva").len() == 1).await;
    b.answer("eva", &s.id, true).await.unwrap();
    let (copy, _) = b.open_copy("eva", &s.id, file).await.unwrap();
    eventually("copy filled", || copy.room.text() == "x\n").await;
    // Someone else cannot take it back.
    assert!(matches!(a.unshare("eva", &s.id), Err(MeshError::NotFound)));
    a.unshare("matus", &s.id).unwrap();
    eventually("gone from kitchenbox", || b.incoming("eva").is_empty()).await;
    eventually("copy dropped", || b.hub.copies_of(&s.id).is_empty()).await;
    assert!(matches!(
        b.open_copy("eva", &s.id, file).await,
        Err(MeshError::NotFound)
    ));
}

#[tokio::test]
async fn a_shared_folder_is_read_through_the_link_and_nothing_outside_it() {
    let net = Net::new();
    let (a, store) = net.add("mattbox", vec!["matus"]);
    let (b, _) = net.add("kitchenbox", vec!["eva"]);
    let folder = store.add("matus", None, "Recipes", None);
    let soup = store.add("matus", Some(folder), "soup.md", Some(b"## Soup\n"));
    let private = store.add("matus", None, "diary.md", Some(b"secret\n"));
    let s = a
        .share(&p("matus"), folder, "eva@kitchenbox", Perm::Edit)
        .unwrap();
    eventually("offer", || b.incoming("eva").len() == 1).await;
    assert!(b.incoming("eva")[0].folder);
    // Not before she accepts.
    assert!(matches!(
        b.list("eva", &s.id, None).await,
        Err(MeshError::Forbidden)
    ));
    b.answer("eva", &s.id, true).await.unwrap();
    let items = b.list("eva", &s.id, None).await.unwrap();
    assert_eq!(
        items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
        vec!["soup.md"]
    );
    assert!(items[0].owner.is_empty(), "no login names cross");
    let (name, bytes) = b.read("eva", &s.id, soup).await.unwrap();
    assert_eq!(
        (name.as_str(), bytes.as_slice()),
        ("soup.md", &b"## Soup\n"[..])
    );
    // A file outside the folder, asked for by its id.
    assert!(
        matches!(b.read("eva", &s.id, private).await, Err(MeshError::Remote(ref c)) if c == "not-found")
    );
    let (copy, _) = b.open_copy("eva", &s.id, private).await.unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(copy.room.text(), "", "the diary never reaches kitchenbox");
    // And a person only sees shares sent to them.
    assert!(matches!(
        b.list("someone", &s.id, None).await,
        Err(MeshError::NotFound)
    ));
    assert_eq!(a.guest_access("eva@kitchenbox", soup), Perm::Edit);
    assert_eq!(a.guest_access("eva@kitchenbox", private), Perm::None);
    assert_eq!(a.guest_access("eva@otherbox", soup), Perm::None);
}

#[tokio::test]
async fn a_share_for_someone_unknown_waits_for_them() {
    let net = Net::new();
    let (a, store) = net.add("mattbox", vec!["matus"]);
    let (_b, _) = net.add("kitchenbox", vec!["eva"]);
    let file = store.add("matus", None, "n.md", Some(b"x"));
    a.share(&p("matus"), file, "nobody@kitchenbox", Perm::Edit)
        .unwrap();
    // The other box takes it without saying whether "nobody" has an
    // account there.
    eventually("offered", || {
        a.outgoing("matus")[0].state == OutState::Pending
    })
    .await;
    assert!(matches!(
        a.share(&p("matus"), file, "eva", Perm::Edit),
        Err(MeshError::BadAddress)
    ));
    assert!(matches!(
        a.share(&p("matus"), file, "eva@mattbox", Perm::Edit),
        Err(MeshError::BadAddress)
    ));
    assert!(matches!(
        a.share(&p("eva"), file, "eva@kitchenbox", Perm::Edit),
        Err(MeshError::Forbidden)
    ));
}

#[tokio::test]
async fn a_share_offered_while_apart_arrives_later() {
    let net = Net::new();
    let (a, store) = net.add("mattbox", vec!["matus"]);
    let (b, _) = net.add("kitchenbox", vec!["eva"]);
    tokio::spawn(a.clone().run());
    net.cut();
    let file = store.add("matus", None, "n.md", Some(b"x"));
    a.share(&p("matus"), file, "eva@kitchenbox", Perm::Read)
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(a.outgoing("matus")[0].state, OutState::Unsent);
    net.plug();
    eventually("delivered", || b.incoming("eva").len() == 1).await;
}

#[tokio::test]
async fn a_box_cannot_offer_files_of_another_box() {
    let net = Net::new();
    let (a, _) = net.add("mattbox", vec!["matus"]);
    let (b, _) = net.add("kitchenbox", vec!["eva"]);
    let l = a.link("kitchenbox").await.unwrap();
    let forged = Op::Offer {
        share: "abc".into(),
        owner: "boss@bankbox".into(),
        owner_name: "Your bank".into(),
        recipient: "eva@kitchenbox".into(),
        name: "statement.txt".into(),
        folder: false,
        size: 1,
        perm: Perm::Read,
    };
    assert!(matches!(l.request(forged).await, Err(MeshError::Remote(ref c)) if c == "forbidden"));
    assert!(b.incoming("eva").is_empty());
}
