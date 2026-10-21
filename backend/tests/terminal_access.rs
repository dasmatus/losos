//! Terminal access end to end: a real `lososd`, a seeded key, a token made
//! through the web UI's route, and calls sealed as the `losos` client seals
//! them (`docs/terminal-access.md`).

use assert_fs::TempDir;
use losos_access::{open_answer, seal_request, HostKey, Keyring, Request, SeededKey};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Daemon {
    child: Child,
    port: u16,
    _dir: TempDir,
    admin: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Response {
    status: u16,
    content_type: String,
    body: String,
}

fn request(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Option<Response> {
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    let mut socket = TcpStream::connect(("127.0.0.1", port)).ok()?;
    socket
        .set_read_timeout(Some(Duration::from_secs(20)))
        .ok()?;
    socket.write_all(req.as_bytes()).ok()?;
    let mut raw = Vec::new();
    socket.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status = head.split_whitespace().nth(1)?.parse().ok()?;
    let content_type = head
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_default();
    Some(Response {
        status,
        content_type,
        body: body.to_string(),
    })
}

impl Daemon {
    fn start(seeded: &str) -> Daemon {
        let dir = TempDir::new().unwrap();
        let keys = dir.path().join("access-keys.asc");
        std::fs::write(&keys, seeded).unwrap();
        let token_file = dir.path().join("token");
        let port = TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut child = Command::new(env!("CARGO_BIN_EXE_lososd"))
            .env("LOSOS_NO_DBUS", "1")
            .env("LOSOS_STATE_DIR", dir.path().join("state"))
            .env("LOSOS_OVERRIDES", dir.path().join("overrides.nix"))
            .env("LOSOS_ADMIN_TOKEN_FILE", &token_file)
            .env("LOSOS_ADMIN_PORT", port.to_string())
            .env("LOSOS_ACCESS_DIR", dir.path().join("access"))
            .env("LOSOS_ACCESS_KEYS_FILE", &keys)
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if request(port, "GET", "/api/health", &[], "").is_some_and(|r| r.status == 200) {
                let admin = std::fs::read_to_string(&token_file).unwrap();
                return Daemon {
                    child,
                    port,
                    _dir: dir,
                    admin,
                };
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
        panic!("lososd never came up");
    }

    fn admin(&self, method: &str, path: &str, body: &str) -> Response {
        let bearer = format!("Bearer {}", self.admin);
        request(
            self.port,
            method,
            path,
            &[
                ("Authorization", &bearer),
                ("Content-Type", "application/json"),
            ],
            body,
        )
        .unwrap()
    }

    fn post_envelope(&self, armored: &str) -> Response {
        request(
            self.port,
            "POST",
            "/api/access/call",
            &[("Content-Type", "application/pgp-encrypted")],
            armored,
        )
        .unwrap()
    }
}

fn nonce() -> String {
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut b)
        .unwrap();
    b.iter().map(|x| format!("{x:02x}")).collect()
}

struct Client<'a> {
    key: &'a HostKey,
    server: SeededKey,
    fingerprint: String,
    token: String,
}

impl Client<'_> {
    fn seal(&self, method: &str, path: &str, body: serde_json::Value) -> (String, String) {
        let n = nonce();
        let req = Request {
            v: 1,
            host: self.fingerprint.clone(),
            token: self.token.clone(),
            method: method.into(),
            path: path.into(),
            ts: losos_access::now_secs(),
            nonce: n.clone(),
            body,
            delegation: None,
        };
        (seal_request(self.key, &self.server, &req).unwrap(), n)
    }
}

#[test]
fn a_seeded_key_and_a_token_reach_the_api_and_nothing_else() {
    let owner = HostKey::generate("Owner <owner@example.com>").unwrap();
    let stranger = HostKey::generate("Stranger").unwrap();
    let daemon = Daemon::start(owner.armored_public());

    // The host key is public; the terminal pins it.
    let hk = request(daemon.port, "GET", "/api/access/host-key", &[], "").unwrap();
    assert_eq!(hk.status, 200, "{}", hk.body);
    let hk: serde_json::Value = serde_json::from_str(&hk.body).unwrap();
    let fingerprint = hk["fingerprint"].as_str().unwrap().to_string();
    let server = Keyring::parse(hk["publicKey"].as_str().unwrap())
        .unwrap()
        .keys()[0]
        .clone();
    assert_eq!(server.fingerprint, fingerprint);

    // The web UI's view: one seeded key, no tokens, and the routes need the
    // Bearer like every other.
    assert_eq!(
        request(daemon.port, "GET", "/api/access", &[], "")
            .unwrap()
            .status,
        401
    );
    let view = daemon.admin("GET", "/api/access", "");
    assert_eq!(view.status, 200, "{}", view.body);
    let view: serde_json::Value = serde_json::from_str(&view.body).unwrap();
    assert_eq!(view["hostKey"]["fingerprint"], fingerprint.as_str());
    assert_eq!(view["keys"][0]["fingerprint"], owner.fingerprint());
    assert_eq!(view["tokens"].as_array().unwrap().len(), 0);

    // A key that was not seeded gets no token.
    let refused = daemon.admin(
        "POST",
        "/api/access/tokens",
        &serde_json::json!({"name": "x", "fingerprint": stranger.fingerprint(), "scope": "full", "expiresInDays": null}).to_string(),
    );
    assert_eq!(refused.status, 400, "{}", refused.body);

    let made = daemon.admin(
        "POST",
        "/api/access/tokens",
        &serde_json::json!({"name": "laptop", "fingerprint": owner.fingerprint(), "scope": "full", "expiresInDays": 90}).to_string(),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    let made: serde_json::Value = serde_json::from_str(&made.body).unwrap();
    let token = made["token"].as_str().unwrap().to_string();
    let id = made["record"]["id"].as_str().unwrap().to_string();

    let client = Client {
        key: &owner,
        server: server.clone(),
        fingerprint: fingerprint.clone(),
        token: token.clone(),
    };

    // A call runs the web UI's own route and comes back sealed to the owner.
    let (sealed, n) = client.seal("GET", "/api/state", serde_json::Value::Null);
    let answer = daemon.post_envelope(&sealed);
    assert_eq!(answer.status, 200, "{}", answer.body);
    assert_eq!(answer.content_type, "application/pgp-encrypted");
    assert!(
        !answer.body.contains("\"mode\""),
        "the answer is in the clear"
    );
    let opened = open_answer(&owner, &server, answer.body.as_bytes()).unwrap();
    assert_eq!(opened.nonce, n);
    assert_eq!(opened.status, 200);
    assert!(opened.body.is_object(), "{:?}", opened.body);

    // The same envelope again is a replay.
    assert_eq!(daemon.post_envelope(&sealed).status, 409);

    // A token never reaches the token routes, even with full scope.
    let (sealed, _) = client.seal(
        "POST",
        "/api/access/tokens",
        serde_json::json!({"name": "more", "fingerprint": owner.fingerprint(), "scope": "full"}),
    );
    assert_eq!(daemon.post_envelope(&sealed).status, 403);

    // The right token signed by the wrong key is refused, as is a wrong token.
    let thief = Client {
        key: &stranger,
        server: server.clone(),
        fingerprint: fingerprint.clone(),
        token: token.clone(),
    };
    let (sealed, _) = thief.seal("GET", "/api/state", serde_json::Value::Null);
    let r = daemon.post_envelope(&sealed);
    assert_eq!(r.status, 401);
    assert_eq!(r.content_type, "application/json");
    let last = if token.ends_with('0') { "1" } else { "0" };
    let guess = Client {
        key: &owner,
        server: server.clone(),
        fingerprint: fingerprint.clone(),
        token: format!("{}{last}", &token[..token.len() - 1]),
    };
    let (sealed, _) = guess.seal("GET", "/api/state", serde_json::Value::Null);
    assert_eq!(daemon.post_envelope(&sealed).status, 401);

    // The web UI saw it used, and revoking it ends it.
    let view: serde_json::Value =
        serde_json::from_str(&daemon.admin("GET", "/api/access", "").body).unwrap();
    assert!(view["tokens"][0]["lastUsed"].is_i64());
    assert!(view["tokens"][0].get("sha256").is_none());
    let revoked = daemon.admin(
        "POST",
        "/api/access/tokens/revoke",
        &serde_json::json!({ "id": id }).to_string(),
    );
    assert_eq!(revoked.status, 200, "{}", revoked.body);
    let (sealed, _) = client.seal("GET", "/api/state", serde_json::Value::Null);
    assert_eq!(daemon.post_envelope(&sealed).status, 401);
}

#[test]
fn a_box_with_no_seeded_key_makes_no_token() {
    let daemon = Daemon::start("");
    let view: serde_json::Value =
        serde_json::from_str(&daemon.admin("GET", "/api/access", "").body).unwrap();
    assert_eq!(view["keys"].as_array().unwrap().len(), 0);
    let made = daemon.admin(
        "POST",
        "/api/access/tokens",
        r#"{"name": "x", "fingerprint": "00", "scope": "read", "expiresInDays": null}"#,
    );
    assert_eq!(made.status, 400);
}
