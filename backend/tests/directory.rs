//! LosOS Desktop computers joined to the box, as a real `lososd` meets them
//! (`src/directory.rs`): an administrator joins one and gets its token
//! once, the token reads the people list and nothing else, and a computer
//! taken out stops reading. A person who is not an administrator cannot
//! join a computer or read the list.

use assert_fs::TempDir;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Daemon {
    child: Child,
    port: u16,
    dir: TempDir,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Daemon {
    /// The spare key, which lososd makes on its first start.
    fn spare_key(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("token"))
            .unwrap()
            .trim()
            .to_string()
    }

    fn start() -> Daemon {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("users.yml"),
            json!({"users": {
                "ada": {"displayname": "Ada Lovelace", "email": "ada@example.com",
                        "password": "$argon2id$secret-hash", "groups": ["losos-admins"]},
                "bob": {"displayname": "Bob", "password": "$argon2id$bobs-hash",
                        "groups": ["staff"], "disabled": true},
            }})
            .to_string(),
        )
        .unwrap();
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_lososd"))
            .env("LOSOS_NO_DBUS", "1")
            .env("LOSOS_STATE_DIR", dir.path().join("state"))
            .env("LOSOS_OVERRIDES", dir.path().join("overrides.nix"))
            .env("LOSOS_ADMIN_TOKEN_FILE", dir.path().join("token"))
            .env("LOSOS_ADMIN_PORT", port.to_string())
            .env("LOSOS_PEOPLE_FILE", dir.path().join("users.yml"))
            .env("LOSOS_PEOPLE_DIR", dir.path())
            .env("LOSOS_IDENTITY", "authelia")
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let d = Daemon { child, port, dir };
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if d.try_send("GET", "/api/health", &[], "").is_some() {
                return d;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("lososd never answered");
    }

    fn try_send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Option<(u16, Value)> {
        let mut request =
            format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ));
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

    /// A call as `who`: a Bearer token, or a person as nginx names them
    /// after LosOS Login let them through.
    fn call(&self, method: &str, path: &str, who: Who, body: &str) -> (u16, Value) {
        let bearer;
        let headers: Vec<(&str, &str)> = match who {
            Who::Bearer(t) => {
                bearer = format!("Bearer {t}");
                vec![("Authorization", bearer.as_str())]
            }
            Who::Person(name) => vec![("Remote-User", name)],
        };
        self.try_send(method, path, &headers, body)
            .unwrap_or_else(|| panic!("no answer to {method} {path}"))
    }
}

#[derive(Clone, Copy)]
enum Who<'a> {
    Bearer(&'a str),
    Person(&'a str),
}

fn join(d: &Daemon, who: Who, device: &str) -> (u16, Value) {
    d.call(
        "POST",
        "/api/devices/directory",
        who,
        &json!({ "device": device }).to_string(),
    )
}

#[test]
fn a_joined_computer_reads_the_people_list_and_nothing_else() {
    let d = Daemon::start();
    let (status, joined) = join(&d, Who::Person("ada"), "reception-pc");
    assert_eq!(status, 200, "{joined}");
    let token = joined["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("losos_dir_"), "{token}");
    assert_eq!(joined["computer"]["device"], "reception-pc");
    assert_eq!(joined["computer"]["joinedBy"], "ada");

    let (status, list) = d.call("GET", "/api/directory/people", Who::Bearer(&token), "");
    assert_eq!(status, 200, "{list}");
    let people = list["people"].as_array().unwrap();
    let ada = people.iter().find(|p| p["name"] == "ada").unwrap();
    assert_eq!(ada["displayName"], "Ada Lovelace");
    assert_eq!(ada["email"], "ada@example.com");
    assert_eq!(ada["groups"], json!(["losos-admins"]));
    let bob = people.iter().find(|p| p["name"] == "bob").unwrap();
    assert_eq!(bob["disabled"], true);
    assert!(
        !list.to_string().contains("argon2id"),
        "no password hash leaves the box: {list}"
    );

    for (method, path) in [
        ("GET", "/api/me"),
        ("GET", "/api/people"),
        ("GET", "/api/state"),
        ("GET", "/api/devices/directory"),
        ("POST", "/api/devices/directory"),
    ] {
        let (status, body) = d.call(method, path, Who::Bearer(&token), "{}");
        assert_eq!(status, 403, "{method} {path}: {body}");
    }

    // The token is kept as a hash only.
    let file = std::fs::read_to_string(d.dir.path().join("state/directory.json")).unwrap();
    assert!(!file.contains(&token));
}

#[test]
fn a_computer_taken_out_or_a_made_up_token_reads_nothing() {
    let d = Daemon::start();
    let (status, joined) = join(&d, Who::Bearer(&d.spare_key()), "laptop-7");
    assert_eq!(status, 200, "{joined}");
    let token = joined["token"].as_str().unwrap().to_string();
    let id = joined["computer"]["id"].as_str().unwrap().to_string();

    let (status, listed) = d.call("GET", "/api/devices/directory", Who::Person("ada"), "");
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["computers"][0]["device"], "laptop-7");
    assert!(listed["computers"][0].get("sha256").is_none());

    let (status, _) = d.call(
        "DELETE",
        &format!("/api/devices/directory/{id}"),
        Who::Person("ada"),
        "",
    );
    assert_eq!(status, 200);
    let (status, _) = d.call("GET", "/api/directory/people", Who::Bearer(&token), "");
    assert_eq!(status, 401);

    let made_up = format!("losos_dir_{}_{}", "0".repeat(8), "0".repeat(40));
    let (status, _) = d.call("GET", "/api/directory/people", Who::Bearer(&made_up), "");
    assert_eq!(status, 401);
}

#[test]
fn only_an_administrator_joins_a_computer() {
    let d = Daemon::start();
    let mut users: Value =
        serde_json::from_str(&std::fs::read_to_string(d.dir.path().join("users.yml")).unwrap())
            .unwrap();
    users["users"]["carol"] = json!({"displayname": "Carol", "password": "x", "groups": []});
    std::fs::write(d.dir.path().join("users.yml"), users.to_string()).unwrap();

    let (status, body) = join(&d, Who::Person("carol"), "laptop");
    assert_eq!(status, 403, "{body}");
    let (status, body) = d.call("GET", "/api/directory/people", Who::Person("carol"), "");
    assert_eq!(status, 403, "{body}");
    let (status, body) = join(&d, Who::Person("ada"), "no/slashes");
    assert_eq!(status, 400, "{body}");
}
