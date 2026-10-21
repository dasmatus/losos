//! Two boxes on real sockets: the mesh link over WebSockets between two
//! HTTP servers, and two editors connected to them the way the web app is,
//! one per box, typing into one file.

use std::sync::Arc;
use std::time::Duration;

use actix_web::{web, App, HttpServer};
use losos_ctl::cloud::collab::hub::Hub;
use losos_ctl::cloud::collab::room::{Access, Room};
use losos_ctl::cloud::collab::routes as collab_routes;
use losos_ctl::cloud::collab::store::{CollabStore, MemStore, Perm};
use losos_ctl::cloud::mesh::db::Db;
use losos_ctl::cloud::mesh::dial::{pump, Dialer};
use losos_ctl::cloud::mesh::identity::Identity;
use losos_ctl::cloud::mesh::link::Frame;
use losos_ctl::cloud::mesh::{routes as mesh_routes, Mesh, People};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

struct Folk(&'static [&'static str]);

impl People for Folk {
    fn exists(&self, login: &str) -> bool {
        self.0.contains(&login)
    }
    fn display_name(&self, login: &str) -> String {
        login.to_string()
    }
}

struct TestBox {
    mesh: Arc<Mesh>,
    store: Arc<MemStore>,
    port: u16,
}

fn tempdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("losos-mesh-http-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Start a box on a free port; `peers` maps other box names to their ports.
fn start(
    name: &str,
    people: &'static [&'static str],
    listener: std::net::TcpListener,
    peers: &[(&str, u16)],
) -> TestBox {
    let port = listener.local_addr().unwrap().port();
    let store = Arc::new(MemStore::new());
    let hub = Hub::with_timing(
        store.clone() as Arc<dyn CollabStore>,
        tempdir(name),
        Duration::from_millis(50),
        Duration::from_secs(2),
    );
    let mut dialer = Dialer::default();
    for (n, p) in peers {
        dialer
            .urls
            .insert(n.to_string(), format!("ws://127.0.0.1:{p}/mesh/v1/link"));
    }
    let mesh = Mesh::new(
        name.into(),
        Identity::ephemeral(),
        Db::memory().unwrap(),
        hub,
        Arc::new(Folk(people)),
        dialer.into_fn(),
    );
    let data = web::Data::new(mesh.clone());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .service(collab_routes::scope())
            .service(mesh_routes::scope())
            .service(mesh_routes::link_scope())
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    actix_web::rt::spawn(server);
    TestBox { mesh, store, port }
}

/// An editor: its own Yjs document joined to a box's WebSocket.
async fn editor(port: u16, user: &str, query: &str) -> Arc<Room> {
    let url = format!("ws://127.0.0.1:{port}/cloud/api/collab/ws?{query}");
    let mut req = url.into_client_request().unwrap();
    req.headers_mut()
        .insert("x-losos-cloud-user", user.parse().unwrap());
    let (ws, _) = tokio_tungstenite::connect_async(req)
        .await
        .expect("editor connects");
    let (tx, mut rx) = pump(ws);
    let room = Arc::new(Room::new());
    let (me, mut out) = room.join(Access::Write);
    let r = room.clone();
    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                f = rx.recv() => match f {
                    Some(Frame::Binary(b)) => { let _ = r.receive(me, &b); }
                    Some(_) => {}
                    None => return,
                },
                m = out.recv() => match m {
                    Some(m) => if tx.send(Frame::Binary(m)).await.is_err() { return },
                    None => return,
                },
            }
        }
    });
    room
}

async fn eventually(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..300 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    panic!("never happened: {what}");
}

async fn json(
    port: u16,
    user: &str,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    // A tiny HTTP/1.1 client: one request per connection.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: x\r\nX-Losos-Cloud-User: {user}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    let text = String::from_utf8_lossy(&out);
    let status: u16 = text[9..12].parse().unwrap();
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("");
    // Chunked bodies: take the JSON between the size lines.
    let json_text = body
        .find(['{', '['])
        .map(|i| &body[i..body.rfind(['}', ']']).map_or(body.len(), |j| j + 1)])
        .unwrap_or("null");
    (
        status,
        serde_json::from_str(json_text).unwrap_or(serde_json::Value::Null),
    )
}

#[actix_web::test]
async fn two_boxes_over_websockets() {
    let la = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let lb = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let (pa, pb) = (
        la.local_addr().unwrap().port(),
        lb.local_addr().unwrap().port(),
    );
    let a = start("mattbox", &["matus"], la, &[("kitchenbox", pb)]);
    let b = start("kitchenbox", &["eva"], lb, &[("mattbox", pa)]);
    let file = a
        .store
        .add("matus", None, "Holiday 2026.md", Some(b"# Holiday 2026\n"));

    // Not signed in.
    let (status, _) = json(a.port, "", "GET", "/cloud/api/mesh/incoming", None).await;
    assert_eq!(status, 401);

    let (status, share) = json(
        a.port,
        "matus",
        "POST",
        "/cloud/api/mesh/shares",
        Some(serde_json::json!({ "fileId": file, "to": "Eva@KitchenBox", "perm": "edit" })),
    )
    .await;
    assert_eq!(status, 201, "{share}");
    let id = share["id"].as_str().unwrap().to_string();

    let mut incoming = serde_json::Value::Null;
    for _ in 0..100 {
        incoming = json(b.port, "eva", "GET", "/cloud/api/mesh/incoming", None)
            .await
            .1;
        if incoming.as_array().is_some_and(|a| !a.is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert_eq!(incoming[0]["name"], "Holiday 2026.md");
    assert_eq!(incoming[0]["ownerRemote"], "matus@mattbox");
    let (status, _) = json(
        b.port,
        "eva",
        "POST",
        &format!("/cloud/api/mesh/incoming/{id}/accept"),
        None,
    )
    .await;
    assert_eq!(status, 200);

    // What the editor page asks first.
    let (status, meta) = json(
        b.port,
        "eva",
        "GET",
        &format!("/cloud/api/collab/doc?share={id}&file={file}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{meta}");
    assert_eq!(
        (
            meta["box"].as_str(),
            meta["perm"].as_str(),
            meta["remote"].as_bool()
        ),
        (Some("mattbox"), Some("edit"), Some(true))
    );
    // Someone else on kitchenbox cannot open eva's share.
    let (status, _) = json(
        b.port,
        "mallory",
        "GET",
        &format!("/cloud/api/collab/doc?share={id}&file={file}"),
        None,
    )
    .await;
    assert_eq!(status, 404);

    let matus = editor(a.port, "matus", &format!("file={file}")).await;
    let eva = editor(b.port, "eva", &format!("share={id}&file={file}")).await;
    eventually("eva's editor fills", || eva.text() == "# Holiday 2026\n").await;
    let t = matus.text();
    matus.replace_text(&format!("{t}Train to Vienna on Friday.\n"));
    let t = eva.text();
    eva.replace_text(&format!("{t}Book the flat by Tuesday.\n"));
    eventually("both editors agree", || {
        matus.text() == eva.text()
            && eva.text().contains("Vienna")
            && matus.text().contains("Tuesday")
    })
    .await;
    eventually("the file on mattbox", || {
        let c = a.store.content(file);
        c.contains("Vienna") && c.contains("Tuesday")
    })
    .await;

    // The owner turns the share read-only; eva's next edit stays hers.
    let (status, _) = json(
        a.port,
        "matus",
        "PATCH",
        &format!("/cloud/api/mesh/shares/{id}"),
        Some(serde_json::json!({ "perm": "read" })),
    )
    .await;
    assert_eq!(status, 204);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let t = eva.text();
    eva.replace_text(&format!("{t}Sneaky line.\n"));
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(!matus.text().contains("Sneaky"));
    assert!(!a.store.content(file).contains("Sneaky"));

    // And takes it back.
    let (status, _) = json(
        a.port,
        "matus",
        "DELETE",
        &format!("/cloud/api/mesh/shares/{id}"),
        None,
    )
    .await;
    assert_eq!(status, 204);
    eventually("gone from kitchenbox", || b.mesh.incoming("eva").is_empty()).await;
    let _ = Perm::Edit;
}
