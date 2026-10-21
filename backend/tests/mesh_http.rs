//! Two boxes on real sockets: the mesh link over WebSockets between two
//! HTTP servers, and two editors connected to them the way the web app is,
//! one per box, typing into one file.

use std::sync::Arc;
use std::time::Duration;

use actix_web::{App, HttpServer};
use losos_ctl::cloud::collab::room::{Access, Room};
use losos_ctl::cloud::mesh::dial::{pump, Dialer};
use losos_ctl::cloud::mesh::link::Frame;
use losos_ctl::cloud::mesh::{self, Mesh};
use losos_ctl::cloud::{server, Cloud, Config};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

struct TestBox {
    mesh: Arc<Mesh>,
    cloud: Arc<Cloud>,
    port: u16,
}

fn tempdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("losos-mesh-http-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Start a box's cloud on a free port; `peers` maps other box names to
/// their ports.
fn start(name: &str, listener: std::net::TcpListener, peers: &[(&str, u16)]) -> TestBox {
    let port = listener.local_addr().unwrap().port();
    let root = tempdir(name);
    let cloud = Arc::new(Cloud::open(&root, Config::default()).unwrap());
    let mut dialer = Dialer::default();
    for (n, p) in peers {
        dialer
            .urls
            .insert(n.to_string(), format!("ws://127.0.0.1:{p}/mesh/v1/link"));
    }
    let mesh = mesh::start(
        &cloud,
        mesh::Options {
            name: name.into(),
            dir: root.join(".mesh"),
            dialer,
        },
    )
    .unwrap();
    let routes = server::configure(cloud.clone(), true);
    let server = HttpServer::new(move || App::new().configure(routes.clone()))
        .workers(1)
        .listen(listener)
        .unwrap()
        .run();
    actix_web::rt::spawn(server);
    TestBox { mesh, cloud, port }
}

/// Who nginx says is asking: the cloud login and the LosOS Login name.
#[derive(Clone, Copy)]
struct As(&'static str, &'static str);

const MATUS: As = As("notshared", "matus");
const EVA: As = As("eva", "eva");

/// An editor: its own Yjs document joined to a box's WebSocket.
async fn editor(port: u16, who: As, query: &str) -> Arc<Room> {
    let url = format!("ws://127.0.0.1:{port}/cloud/api/collab/ws?{query}");
    let mut req = url.into_client_request().unwrap();
    req.headers_mut()
        .insert("x-losos-cloud-user", who.0.parse().unwrap());
    req.headers_mut()
        .insert("x-losos-cloud-login", who.1.parse().unwrap());
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
    who: Option<As>,
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
    let who = who.map_or(String::new(), |w| {
        format!(
            "X-Losos-Cloud-User: {}\r\nX-Losos-Cloud-Login: {}\r\nX-Losos-Cloud-Name: {}\r\n",
            w.0,
            w.1,
            title(w.1)
        )
    });
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: x\r\n{who}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

fn title(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

#[actix_web::test]
async fn two_boxes_over_websockets() {
    let la = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let lb = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let (pa, pb) = (
        la.local_addr().unwrap().port(),
        lb.local_addr().unwrap().port(),
    );
    let a = start("mattbox", la, &[("kitchenbox", pb)]);
    let b = start("kitchenbox", lb, &[("mattbox", pa)]);
    // The owner of mattbox: files under `notshared`, signed in as matus.
    a.cloud.store.ensure_home("notshared").unwrap();
    let file = a
        .cloud
        .store
        .write_bytes("notshared", "Holiday 2026.md", b"# Holiday 2026\n")
        .unwrap()
        .entry
        .id;
    let content = |c: &Cloud| {
        String::from_utf8(c.store.read_bytes("notshared", "Holiday 2026.md").unwrap()).unwrap()
    };

    // Not signed in.
    let (status, _) = json(a.port, None, "GET", "/cloud/api/mesh/incoming", None).await;
    assert_eq!(status, 401);
    // The box answers to its own name, and says what the owner's address is.
    let (_, me) = json(a.port, Some(MATUS), "GET", "/cloud/api/mesh/box", None).await;
    assert_eq!(me["me"], "matus@mattbox");

    let (status, share) = json(
        a.port,
        Some(MATUS),
        "POST",
        "/cloud/api/mesh/shares",
        Some(serde_json::json!({ "fileId": file, "to": "Eva@KitchenBox", "perm": "edit" })),
    )
    .await;
    assert_eq!(status, 201, "{share}");
    let id = share["id"].as_str().unwrap().to_string();
    // eva cannot share matus's file on to someone else.
    a.cloud.store.ensure_home("eva").unwrap();
    let (status, _) = json(
        a.port,
        Some(EVA),
        "POST",
        "/cloud/api/mesh/shares",
        Some(serde_json::json!({ "fileId": file, "to": "eve@kitchenbox", "perm": "edit" })),
    )
    .await;
    assert_eq!(status, 403);

    let mut incoming = serde_json::Value::Null;
    for _ in 0..100 {
        incoming = json(b.port, Some(EVA), "GET", "/cloud/api/mesh/incoming", None)
            .await
            .1;
        if incoming.as_array().is_some_and(|a| !a.is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert_eq!(incoming[0]["name"], "Holiday 2026.md");
    assert_eq!(incoming[0]["ownerRemote"], "matus@mattbox");
    assert_eq!(incoming[0]["ownerName"], "Matus");

    // A page on another site cannot answer for her.
    let (status, _) = json_from(
        b.port,
        EVA,
        "https://evil.example",
        "POST",
        &format!("/cloud/api/mesh/incoming/{id}/accept"),
    )
    .await;
    assert_eq!(status, 403);
    let (status, _) = json(
        b.port,
        Some(EVA),
        "POST",
        &format!("/cloud/api/mesh/incoming/{id}/accept"),
        None,
    )
    .await;
    assert_eq!(status, 200);

    // What the editor page asks first.
    let (status, meta) = json(
        b.port,
        Some(EVA),
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
            meta["remote"].as_bool(),
            meta["me"]["address"].as_str()
        ),
        (
            Some("mattbox"),
            Some("edit"),
            Some(true),
            Some("eva@kitchenbox")
        )
    );
    // Someone else on kitchenbox cannot open eva's share.
    let mallory = As("mallory", "mallory");
    let (status, _) = json(
        b.port,
        Some(mallory),
        "GET",
        &format!("/cloud/api/collab/doc?share={id}&file={file}"),
        None,
    )
    .await;
    assert_eq!(status, 404);
    // Nor can eva, on mattbox, open matus's file directly.
    let (status, _) = json(
        a.port,
        Some(EVA),
        "GET",
        &format!("/cloud/api/collab/doc?file={file}"),
        None,
    )
    .await;
    assert_eq!(status, 404);

    let matus = editor(a.port, MATUS, &format!("file={file}")).await;
    let eva = editor(b.port, EVA, &format!("share={id}&file={file}")).await;
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
        let c = content(&a.cloud);
        c.contains("Vienna") && c.contains("Tuesday")
    })
    .await;
    // The editor's writes are versions like any other.
    assert!(!a
        .cloud
        .store
        .versions("notshared", "Holiday 2026.md")
        .unwrap()
        .is_empty());

    // A sync app writes the file; both editors take it in.
    let now = content(&a.cloud);
    let w = a
        .cloud
        .store
        .write_bytes(
            "notshared",
            "Holiday 2026.md",
            format!("{now}Pack the boots.\n").as_bytes(),
        )
        .unwrap();
    a.cloud.changed("notshared", &w.entry);
    eventually("the outside edit reaches eva", || {
        eva.text().contains("boots") && eva.text().contains("Vienna")
    })
    .await;

    // The owner turns the share read-only; eva's next edit stays hers.
    let (status, _) = json(
        a.port,
        Some(MATUS),
        "PATCH",
        &format!("/cloud/api/mesh/shares/{id}"),
        Some(serde_json::json!({ "perm": "read" })),
    )
    .await;
    assert_eq!(status, 204);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let t = eva.text();
    eva.replace_text(&format!("{t}Sneaky line.\n"));
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(!matus.text().contains("Sneaky"));
    assert!(!content(&a.cloud).contains("Sneaky"));

    // And takes it back.
    let (status, _) = json(
        a.port,
        Some(MATUS),
        "DELETE",
        &format!("/cloud/api/mesh/shares/{id}"),
        None,
    )
    .await;
    assert_eq!(status, 204);
    eventually("gone from kitchenbox", || b.mesh.incoming("eva").is_empty()).await;
    let _ = Access::Read;
}

/// A request a page on `origin` makes with eva's session.
async fn json_from(port: u16, who: As, origin: &str, method: &str, path: &str) -> (u16, ()) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: x\r\nOrigin: {origin}\r\nX-Losos-Cloud-User: {}\r\nX-Losos-Cloud-Login: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        who.0, who.1
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    (String::from_utf8_lossy(&out)[9..12].parse().unwrap(), ())
}
