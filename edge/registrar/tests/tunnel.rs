//! The QUIC tunnel end to end in one process: a real edge endpoint on
//! loopback UDP, a real client, TCP services on both sides.

use losos_registrar::tunnel::{self, ClientFile, ClientService, ServerFile, ServerService};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let d = std::env::temp_dir().join(format!("losos-tunnel-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Self(d)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn free_port() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
}

/// A TCP service that answers each connection with `tag`, then everything
/// it read, once the peer has finished writing.
async fn echo(tag: &'static str) -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                s.read_to_end(&mut buf).await.unwrap();
                s.write_all(tag.as_bytes()).await.unwrap();
                s.write_all(&buf).await.unwrap();
                s.shutdown().await.unwrap();
            });
        }
    });
    addr
}

fn write_json<T: serde::Serialize>(path: &Path, v: &T) {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(v).unwrap()).unwrap();
    std::fs::rename(tmp, path).unwrap();
}

/// Start an edge whose file is at `dir/server.json`; returns its UDP address
/// and pin.
async fn edge(dir: &Path) -> (SocketAddr, String) {
    edge_with(dir, true).await
}

/// An edge that answers on TCP alone, as one behind a network that drops
/// UDP looks to its boxes.
async fn edge_with(dir: &Path, quic: bool) -> (SocketAddr, String) {
    let id = tunnel::load_or_make_identity(&dir.join("id")).unwrap();
    let pin = id.pin();
    let mut bound = tunnel::bind_server("127.0.0.1:0".parse().unwrap(), &id).unwrap();
    if !quic {
        bound = bound.without_quic();
    }
    let addr = bound.local_addr().unwrap();
    tokio::spawn(tunnel::serve_on(bound, dir.join("server.json")));
    (addr, pin)
}

/// Send `body`, half-close, and read the whole answer, retrying while the
/// tunnel comes up.
async fn ask(addr: SocketAddr, body: &[u8]) -> Vec<u8> {
    for _ in 0..100 {
        if let Ok(mut s) = TcpStream::connect(addr).await {
            if s.write_all(body).await.is_ok() && s.shutdown().await.is_ok() {
                let mut out = Vec::new();
                if s.read_to_end(&mut out).await.is_ok() && !out.is_empty() {
                    return out;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("nothing answered on {addr}");
}

fn server_file(services: &[(&str, &str, SocketAddr)]) -> ServerFile {
    ServerFile {
        services: services
            .iter()
            .map(|(n, t, b)| {
                (
                    n.to_string(),
                    ServerService {
                        token: t.to_string(),
                        bind: *b,
                    },
                )
            })
            .collect(),
    }
}

fn client_file(remote: SocketAddr, pin: &str, services: &[(&str, &str, SocketAddr)]) -> ClientFile {
    ClientFile {
        remote: remote.to_string(),
        pin: pin.to_string(),
        services: services
            .iter()
            .map(|(n, t, l)| {
                (
                    n.to_string(),
                    ClientService {
                        token: t.to_string(),
                        local: l.to_string(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn requests_cross_the_tunnel_both_ways_with_half_close() {
    let dir = Dir::new("basic");
    let (remote, pin) = edge(&dir.0).await;
    let web = echo("web:").await;
    let llm = echo("llm:").await;
    let (pub_web, pub_llm) = (free_port().await, free_port().await);
    write_json(
        &dir.0.join("server.json"),
        &server_file(&[("box", "t1", pub_web), ("box:llm", "t1", pub_llm)]),
    );
    write_json(
        &dir.0.join("client.json"),
        &client_file(remote, &pin, &[("box", "t1", web), ("box:llm", "t1", llm)]),
    );
    tokio::spawn(tunnel::run_client(dir.0.join("client.json")));

    assert_eq!(ask(pub_web, b"GET /").await, b"web:GET /");
    assert_eq!(ask(pub_llm, b"ping").await, b"llm:ping");

    // Many at once share the one connection.
    let big = vec![7u8; 4 << 20];
    let mut tasks = Vec::new();
    for i in 0..32u8 {
        let body = if i == 0 { big.clone() } else { vec![i; 100] };
        tasks.push(tokio::spawn(async move {
            (body.clone(), ask(pub_web, &body).await)
        }));
    }
    for t in tasks {
        let (body, got) = t.await.unwrap();
        assert_eq!(&got[..4], b"web:");
        assert_eq!(got[4..], body[..]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_token_or_pin_gets_nothing() {
    let dir = Dir::new("refuse");
    let (remote, pin) = edge(&dir.0).await;
    let web = echo("web:").await;
    let public = free_port().await;
    write_json(
        &dir.0.join("server.json"),
        &server_file(&[("box", "right", public)]),
    );
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let wrong = client_file(remote, &pin, &[("box", "wrong", web)]);
    let e = tunnel::connect(&wrong).await.unwrap_err();
    assert!(format!("{e:?}").contains("took none"), "{e:?}");

    let other_pin = "ab".repeat(32);
    let pinned = client_file(remote, &other_pin, &[("box", "right", web)]);
    assert!(tunnel::connect(&pinned).await.is_err());

    // Nothing holds the service, so its port is closed: Traefik's dial is
    // refused and it shows the offline page.
    assert!(TcpStream::connect(public).await.is_err());

    // The right token opens it, and a link that leaves closes it again.
    let right = client_file(remote, &pin, &[("box", "right", web)]);
    let up = tunnel::connect(&right).await.unwrap();
    assert_eq!(ask(public, b"x").await, b"web:x");
    up.close().await;
    let mut refused = false;
    for _ in 0..50 {
        if TcpStream::connect(public).await.is_err() {
            refused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(refused, "{public} still open after the link left");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_service_in_the_file_is_served_within_seconds_and_a_newer_link_takes_over() {
    let dir = Dir::new("reload");
    let (remote, pin) = edge(&dir.0).await;
    let first = echo("first:").await;
    let second = echo("second:").await;
    let public = free_port().await;

    // The client is up before the edge knows the service.
    write_json(&dir.0.join("server.json"), &ServerFile::default());
    write_json(
        &dir.0.join("a.json"),
        &client_file(remote, &pin, &[("box", "t", first)]),
    );
    tokio::spawn(tunnel::run_client(dir.0.join("a.json")));
    write_json(
        &dir.0.join("server.json"),
        &server_file(&[("box", "t", public)]),
    );
    assert_eq!(ask(public, b"x").await, b"first:x");

    // Another link with the same token takes the service over.
    write_json(
        &dir.0.join("b.json"),
        &client_file(remote, &pin, &[("box", "t", second)]),
    );
    tokio::spawn(tunnel::run_client(dir.0.join("b.json")));
    let mut got = Vec::new();
    for _ in 0..50 {
        got = ask(public, b"y").await;
        if got == b"second:y" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(got, b"second:y");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_udp_the_tunnel_falls_back_to_tcp_and_carries_the_same() {
    let dir = Dir::new("tcp");
    let (remote, pin) = edge_with(&dir.0, false).await;
    let web = echo("web:").await;
    let public = free_port().await;
    write_json(
        &dir.0.join("server.json"),
        &server_file(&[("box", "t1", public)]),
    );
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let file = client_file(remote, &pin, &[("box", "t1", web)]);
    let up = tunnel::connect(&file).await.unwrap();
    assert!(up.is_tcp(), "QUIC cannot have answered");
    drop(up);

    write_json(&dir.0.join("client.json"), &file);
    tokio::spawn(tunnel::run_client(dir.0.join("client.json")));
    assert_eq!(ask(public, b"GET /").await, b"web:GET /");

    let big = vec![9u8; 8 << 20];
    let mut tasks = Vec::new();
    for i in 0..32u8 {
        let body = if i == 0 { big.clone() } else { vec![i; 100] };
        tasks.push(tokio::spawn(async move {
            (body.clone(), ask(public, &body).await)
        }));
    }
    for t in tasks {
        let (body, got) = t.await.unwrap();
        assert_eq!(&got[..4], b"web:");
        assert_eq!(got[4..], body[..]);
    }

    // The pin holds on TCP as on QUIC, and so does the token.
    let other_pin = "cd".repeat(32);
    assert!(
        tunnel::connect(&client_file(remote, &other_pin, &[("box", "t1", web)]))
            .await
            .is_err()
    );
    let e = tunnel::connect(&client_file(remote, &pin, &[("box", "nope", web)]))
        .await
        .unwrap_err();
    assert!(format!("{e:?}").contains("took none"), "{e:?}");
}
