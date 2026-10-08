//! `losos-registrar lab` end to end, with a fake `virsh`.
//!
//! The helper runs for real on a real socket; only libvirt is a shell script
//! that logs its arguments, keeps the XML it was handed and a list of
//! "running" domains. The test then plays QEMU's part: it connects to the
//! serial port the XML names and binds the UDP port QEMU would, so the
//! console and NIC bridges are exercised byte for byte.

mod common;

use std::path::Path;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use losos_registrar::lab::{serve, LabOpts};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

const ORIGIN: &str = "https://lab.example.org";

fn fake_virsh(dir: &Path) -> String {
    let d = dir.display();
    let script = format!(
        r#"#!/bin/sh
# -q -c URI COMMAND ARGS...
shift 3
echo "$*" >> "{d}/log"
case "$1" in
  uri) echo "qemu:///session" ;;
  domcapabilities) echo "error: no kvm here" >&2; exit 1 ;;
  create)
    n=$(ls "{d}" | grep -c '^xml-')
    cat "$2" > "{d}/xml-$n"
    sed -n 's:.*<name>\(.*\)</name>.*:\1:p' "{d}/xml-$n" >> "{d}/running" ;;
  destroy)
    grep -vx "$2" "{d}/running" > "{d}/running.new"; mv "{d}/running.new" "{d}/running" ;;
  list) cat "{d}/running" ;;
esac
"#
    );
    let path = dir.join("virsh");
    std::fs::write(&path, script).expect("write fake virsh");
    let status = std::process::Command::new("chmod")
        .arg("+x")
        .arg(&path)
        .status()
        .expect("chmod");
    assert!(status.success());
    path.display().to_string()
}

struct Helper {
    base: String,
    ws: String,
    dir: common::TempDir,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<miette::Result<()>>,
}

async fn helper(idle: Duration) -> Helper {
    let dir = common::TempDir::new("lab");
    let images = dir.path().join("guest");
    std::fs::create_dir_all(&images).expect("images");
    std::fs::write(images.join("bzImage"), b"kernel").expect("kernel");
    std::fs::write(images.join("rootfs.bin"), b"rootfs").expect("rootfs");
    // A leftover of a killed helper, and a domain that is not ours.
    std::fs::write(
        dir.path().join("running"),
        "losos-lab-old-abcdef\nsomeone-elses-vm\n",
    )
    .expect("running");
    let virsh = fake_virsh(dir.path());
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let opts = LabOpts {
        listen: addr.to_string(),
        images,
        origins: vec![ORIGIN.to_string()],
        virsh,
        idle,
        ..LabOpts::defaults()
    };
    let (tx, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(serve(listener, opts, async {
        let _ = rx.await;
    }));
    let base = format!("http://{addr}/lab/v1/");
    // Wait for the router.
    for _ in 0..50 {
        if reqwest::get(format!("{base}hello")).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Helper {
        ws: format!("ws://{addr}/lab/v1/guests/"),
        base,
        dir,
        stop: Some(tx),
        task,
    }
}

impl Helper {
    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(name)).unwrap_or_default()
    }

    async fn create(&self, body: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}guests", self.base))
            .header("Origin", ORIGIN)
            .json(&body)
            .send()
            .await
            .expect("post")
    }

    async fn stop(mut self) -> Self {
        let _ = self.stop.take().expect("running").send(());
        (&mut self.task)
            .await
            .expect("join")
            .expect("served cleanly");
        self
    }
}

fn guest(id: &str, role: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": "losos",
        "role": role,
        "cmdline": "console=ttyS0 root=/dev/vda ro losos.host=losos losos.ip=10.0.1.20/24",
        "macs": ["52:54:00:4C:03:20"],
    })
}

/// The number after `attr='` on the line that contains `marker`.
fn port_in(xml: &str, marker: &str, attr: &str) -> u16 {
    let line = xml
        .lines()
        .find(|l| l.contains(marker))
        .unwrap_or_else(|| panic!("no {marker} in {xml}"));
    let rest = &line[line.find(&format!("{attr}='")).expect("attr") + attr.len() + 2..];
    rest[..rest.find('\'').expect("quote")]
        .parse()
        .expect("port")
}

async fn ws_connect(
    url: &str,
    ticket: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    tokio_tungstenite::tungstenite::Error,
> {
    let mut req = url.into_client_request().expect("request");
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("losos-lab, ticket.{ticket}")
            .parse()
            .expect("header"),
    );
    req.headers_mut()
        .insert("Origin", ORIGIN.parse().expect("origin"));
    tokio_tungstenite::connect_async(req)
        .await
        .map(|(ws, _)| ws)
}

#[tokio::test]
async fn a_guest_is_created_bridged_and_destroyed_through_virsh() {
    let h = helper(Duration::from_secs(60)).await;

    // The start-up sweep took our leftover and left the other VM alone.
    let running = h.read("running");
    assert!(!running.contains("losos-lab-old-abcdef"), "{running}");
    assert!(running.contains("someone-elses-vm"));

    let hello: serde_json::Value = reqwest::get(format!("{}hello", h.base))
        .await
        .expect("hello")
        .json()
        .await
        .expect("json");
    assert_eq!(hello["available"], true, "{hello}");
    assert_eq!(hello["domainType"], "qemu", "the fake has no kvm");
    assert_eq!(hello["images"]["gear"], false);

    // An origin the helper was not given is refused before anything runs.
    let refused = reqwest::Client::new()
        .post(format!("{}guests", h.base))
        .header("Origin", "https://evil.example")
        .json(&guest("d3", "box"))
        .send()
        .await
        .expect("post");
    assert_eq!(refused.status(), 403);
    assert!(!h.read("log").contains("create"));

    // No gear.bin: a router cannot start here, and says why.
    let router = h.create(guest("d4", "router")).await;
    assert_eq!(router.status(), 503);

    let created = h.create(guest("d3", "box")).await;
    assert_eq!(created.status(), 201);
    assert_eq!(
        created
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some(ORIGIN)
    );
    assert_eq!(
        created
            .headers()
            .get("cross-origin-resource-policy")
            .and_then(|v| v.to_str().ok()),
        Some("cross-origin")
    );
    let g: serde_json::Value = created.json().await.expect("json");
    let key = g["guest"].as_str().expect("key").to_string();
    let ticket = g["ticket"].as_str().expect("ticket").to_string();
    assert!(key.starts_with("d3-"));
    assert_eq!(g["domain"], format!("losos-lab-{key}"));

    let xml = h.read("xml-0");
    assert!(xml.starts_with("<domain type='qemu'>"), "{xml}");
    assert!(xml.contains(&format!("<name>losos-lab-{key}</name>")));
    assert!(xml.contains("<mac address='52:54:00:4c:03:20'/>"));
    assert!(xml.contains("<cmdline>console=ttyS0 root=/dev/vda ro losos.host=losos"));
    assert!(xml.contains("/guest/rootfs.bin'/>"));

    // Play QEMU: connect the serial port and print something.
    let serial_port = port_in(&xml, "<source mode='connect'", "service");
    let mut serial = TcpStream::connect(("127.0.0.1", serial_port))
        .await
        .expect("serial");
    serial
        .write_all(b"losos is ready\r\n# ")
        .await
        .expect("print");

    // A wrong ticket gets no console.
    assert!(ws_connect(&format!("{}{key}/console", h.ws), "00ff")
        .await
        .is_err());

    let mut console = ws_connect(&format!("{}{key}/console", h.ws), &ticket)
        .await
        .expect("console");
    let mut seen = Vec::new();
    while !String::from_utf8_lossy(&seen).contains("is ready") {
        match tokio::time::timeout(Duration::from_secs(5), console.next()).await {
            Ok(Some(Ok(Message::Binary(b)))) => seen.extend_from_slice(&b),
            other => panic!("console said {other:?}"),
        }
    }
    console
        .send(Message::binary(b"uname\r".to_vec()))
        .await
        .expect("type");
    let mut typed = [0u8; 6];
    tokio::time::timeout(Duration::from_secs(5), serial.read_exact(&mut typed))
        .await
        .expect("in time")
        .expect("read");
    assert_eq!(&typed, b"uname\r");

    // Play QEMU's network card: bind its end of the tunnel.
    let qemu_port = port_in(&xml, "<local address=", "port");
    let helper_port = port_in(&xml, "<source address=", "port");
    let qemu = UdpSocket::bind(("127.0.0.1", qemu_port))
        .await
        .expect("qemu end");
    let mut nic = ws_connect(&format!("{}{key}/nic/0", h.ws), &ticket)
        .await
        .expect("nic");
    let frame: Vec<u8> = (0u8..60).collect();
    // The session attaches just after the handshake; resend until it does.
    let got = loop {
        qemu.send_to(&frame, ("127.0.0.1", helper_port))
            .await
            .expect("send");
        match tokio::time::timeout(Duration::from_millis(200), nic.next()).await {
            Ok(Some(Ok(Message::Binary(b)))) => break b,
            Ok(other) => panic!("nic said {other:?}"),
            Err(_) => continue,
        }
    };
    assert_eq!(&got[..], &frame[..]);
    let back: Vec<u8> = (100u8..164).collect();
    nic.send(Message::binary(back.clone())).await.expect("send");
    let mut buf = [0u8; 2048];
    let (n, from) = tokio::time::timeout(Duration::from_secs(5), qemu.recv_from(&mut buf))
        .await
        .expect("in time")
        .expect("recv");
    assert_eq!(&buf[..n], &back[..]);
    assert_eq!(from.port(), helper_port);
    // A runt frame is not forwarded.
    nic.send(Message::binary(vec![1u8; 4])).await.expect("send");

    // DELETE destroys the domain and ends both sockets.
    let gone = reqwest::Client::new()
        .delete(format!("{}guests/{key}", h.base))
        .send()
        .await
        .expect("delete");
    assert_eq!(gone.status(), 204);
    assert!(h.read("log").contains(&format!("destroy losos-lab-{key}")));
    assert!(!h.read("running").contains(&key));

    // A second guest is left running for the shutdown to take.
    let second: serde_json::Value = h
        .create(guest("d5", "edge-local"))
        .await
        .json()
        .await
        .expect("json");
    let second = second["guest"].as_str().expect("key").to_string();
    assert!(h.read("running").contains(&second));
    let h = h.stop().await;
    assert!(
        h.read("log")
            .contains(&format!("destroy losos-lab-{second}")),
        "{}",
        h.read("log")
    );
    assert!(!h.read("running").contains("losos-lab-"));
}

#[tokio::test]
async fn a_guest_nobody_watches_is_destroyed() {
    let h = helper(Duration::from_secs(1)).await;
    let g: serde_json::Value = h
        .create(guest("d7", "box"))
        .await
        .json()
        .await
        .expect("json");
    let key = g["guest"].as_str().expect("key").to_string();
    for _ in 0..50 {
        if !h.read("running").contains(&key) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        !h.read("running").contains(&key),
        "still running after idle"
    );
    h.stop().await;
}

#[tokio::test]
async fn preflights_answer_private_network_access_and_foreign_hosts_are_refused() {
    let h = helper(Duration::from_secs(60)).await;
    let pre = reqwest::Client::new()
        .request(reqwest::Method::OPTIONS, format!("{}guests", h.base))
        .header("Origin", ORIGIN)
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Private-Network", "true")
        .send()
        .await
        .expect("preflight");
    assert_eq!(pre.status(), 204);
    assert_eq!(
        pre.headers()
            .get("access-control-allow-private-network")
            .and_then(|v| v.to_str().ok()),
        Some("true")
    );

    // A page that rebound its own name to 127.0.0.1 sends its own Host.
    let addr = h
        .base
        .trim_start_matches("http://")
        .split('/')
        .next()
        .expect("addr")
        .to_string();
    let mut s = TcpStream::connect(&addr).await.expect("connect");
    s.write_all(
        b"GET /lab/v1/hello HTTP/1.1\r\nHost: rebind.evil.example:8095\r\nOrigin: http://rebind.evil.example:8095\r\nConnection: close\r\n\r\n",
    )
    .await
    .expect("write");
    let mut out = String::new();
    s.read_to_string(&mut out).await.expect("read");
    assert!(out.starts_with("HTTP/1.1 403"), "{out}");
    h.stop().await;
}
