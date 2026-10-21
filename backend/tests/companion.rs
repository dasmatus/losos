//! The LosOS companion's tokens, as a real `lososd` meets them: a Bearer
//! header with a LosOS Login access token, asked about at a stand-in for
//! LosOS Login's introspection endpoint (`src/companion.rs`).
//!
//! The stand-in knows three tokens: one LosOS Login issued to the
//! companion for `ada`, one it issued to some other client, and one it
//! no longer counts. It records each question, so the test can see the
//! host the token was asked about at and the client lososd asked as.

use assert_fs::TempDir;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ADA: &str = "authelia_at_ada.companion";
const OTHER_CLIENT: &str = "authelia_at_ada.forgejo";
const REVOKED: &str = "authelia_at_ada.revoked";

/// A stand-in LosOS Login: answers each introspection from the table and
/// keeps the questions.
fn login() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = asked.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let (mut head, mut length) = (String::new(), 0);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let body = String::from_utf8(body).unwrap();
            seen.lock().unwrap().push(format!("{head}{body}"));
            let exp = chrono_now() + 3600;
            let answer = if body.contains(&format!("token={ADA}&")) {
                json!({ "active": true, "client_id": "losos-companion", "exp": exp, "username": "ada" })
            } else if body.contains(&format!("token={OTHER_CLIENT}&")) {
                json!({ "active": true, "client_id": "forgejo", "exp": exp, "username": "ada" })
            } else {
                json!({ "active": false })
            }
            .to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{answer}",
                answer.len()
            );
        }
    });
    (addr, asked)
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

struct Daemon {
    child: Child,
    port: u16,
    _dir: TempDir,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Daemon {
    fn start(login: &str, identity: bool) -> Daemon {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("users.yml"),
            json!({"users": {
                "ada": {"displayname": "Ada", "password": "x", "groups": ["losos-admins"]},
            }})
            .to_string(),
        )
        .unwrap();
        std::fs::write(dir.path().join("oidc-secret"), "s3cret\n").unwrap();
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lososd"));
        cmd.env("LOSOS_NO_DBUS", "1")
            .env("LOSOS_STATE_DIR", dir.path().join("state"))
            .env("LOSOS_OVERRIDES", dir.path().join("overrides.nix"))
            .env("LOSOS_ADMIN_TOKEN_FILE", dir.path().join("token"))
            .env("LOSOS_ADMIN_PORT", port.to_string())
            .env("LOSOS_PEOPLE_FILE", dir.path().join("users.yml"))
            .env("LOSOS_PEOPLE_DIR", dir.path())
            .env("LOSOS_LOGIN_ADDR", login)
            .env("LOSOS_OIDC_SECRET_FILE", dir.path().join("oidc-secret"))
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if identity {
            cmd.env("LOSOS_IDENTITY", "authelia");
        }
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        let d = Daemon {
            child: cmd.spawn().unwrap(),
            port,
            _dir: dir,
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if d.try_get("/api/health", None).is_some() {
                return d;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("lososd never answered");
    }

    fn try_get(&self, path: &str, token: Option<&str>) -> Option<(u16, Value)> {
        self.try_call("GET", path, token)
    }

    fn try_call(&self, method: &str, path: &str, token: Option<&str>) -> Option<(u16, Value)> {
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Forwarded-Host: mattbox.local\r\nConnection: close\r\n"
        );
        if let Some(t) = token {
            request.push_str(&format!("Authorization: Bearer {t}\r\n"));
        }
        request.push_str("Content-Length: 0\r\n\r\n");
        let mut socket = TcpStream::connect(("127.0.0.1", self.port)).ok()?;
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        socket.write_all(request.as_bytes()).ok()?;
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).ok()?;
        let raw = String::from_utf8_lossy(&raw);
        let status = raw.split_whitespace().nth(1)?.parse().ok()?;
        let body = raw.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        Some((
            status,
            serde_json::from_str(body).unwrap_or(Value::String(body.into())),
        ))
    }

    fn call(&self, method: &str, path: &str, token: Option<&str>) -> (u16, Value) {
        self.try_call(method, path, token)
            .unwrap_or_else(|| panic!("no answer to {method} {path}"))
    }
}

#[test]
fn a_companion_token_reads_as_its_person_and_does_nothing_else() {
    let (addr, asked) = login();
    let d = Daemon::start(&addr, true);

    let (status, me) = d.call("GET", "/api/me", Some(ADA));
    assert_eq!(status, 200, "{me}");
    assert_eq!(me["name"], "ada");
    assert_eq!(me["companion"], true);
    assert_eq!(
        me["admin"], false,
        "an administrator's companion is no administrator"
    );
    let question = asked.lock().unwrap()[0].clone();
    assert!(
        question.contains("X-Forwarded-Host: mattbox.local\r\n"),
        "{question}"
    );
    assert!(
        question.contains("Authorization: Basic bG9zb3NkOnMzY3JldA==\r\n"),
        "{question}"
    );

    assert_eq!(d.call("GET", "/api/status", Some(ADA)).0, 200);
    // Anything off its routes, or not a read, is refused, administrator or not.
    for (method, path) in [
        ("GET", "/api/settings"),
        ("GET", "/api/access"),
        ("GET", "/api/people"),
        ("POST", "/api/me"),
        ("POST", "/api/apply"),
        ("POST", "/api/factory-reset"),
    ] {
        let (status, answer) = d.call(method, path, Some(ADA));
        assert!(
            status == 403 || status == 404 || status == 405,
            "{method} {path}: {status} {answer}"
        );
        assert_ne!(status / 100, 2, "{method} {path}");
    }
    let (status, answer) = d.call("GET", "/api/settings", Some(ADA));
    assert_eq!(status, 403);
    assert!(
        answer["error"]
            .as_str()
            .unwrap()
            .contains("LosOS companion"),
        "{answer}"
    );

    // Kept for the minute: the calls above asked LosOS Login once.
    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[test]
fn another_clients_token_a_dead_one_or_one_without_login_is_nobody() {
    let (addr, _) = login();
    let d = Daemon::start(&addr, true);
    for token in [
        OTHER_CLIENT,
        REVOKED,
        "authelia_rt_ada.refresh",
        "not-a-token",
    ] {
        assert_eq!(d.call("GET", "/api/me", Some(token)).0, 401, "{token}");
    }
    // Without LosOS Login (`LOSOS_IDENTITY` unset) no token is asked about.
    let (addr, asked) = login();
    let d = Daemon::start(&addr, false);
    assert_eq!(d.call("GET", "/api/me", Some(ADA)).0, 401);
    assert!(asked.lock().unwrap().is_empty());
}
