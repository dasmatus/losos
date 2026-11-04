//! LosOS Tag over the wire: a real `lososd`, a real socket, the tag's own
//! routes with the spare key. The queue's runner and the chat listener stay
//! off (`LOSOS_TAG` unset), so an ask sits queued and a test reads the
//! file's own answers.

use assert_fs::TempDir;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BOOT_TIMEOUT: Duration = Duration::from_secs(20);

struct Daemon {
    child: Child,
    port: u16,
    _dir: TempDir,
    token_file: std::path::PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    probe.local_addr().unwrap().port()
}

impl Daemon {
    fn start() -> Daemon {
        let dir = TempDir::new().unwrap();
        let token_file = dir.path().join("token");
        let port = free_port();
        let child = Command::new(env!("CARGO_BIN_EXE_lososd"))
            .env("LOSOS_NO_DBUS", "1")
            .env("LOSOS_STATE_DIR", dir.path().join("state"))
            .env("LOSOS_OVERRIDES", dir.path().join("overrides.nix"))
            .env("LOSOS_ADMIN_TOKEN_FILE", &token_file)
            .env("LOSOS_ADMIN_PORT", port.to_string())
            .env("LOSOS_PEOPLE_DIR", dir.path())
            .env("LOSOS_PEOPLE_FILE", dir.path().join("users.yml"))
            // The tag's own file, in the throwaway directory. No
            // LOSOS_TAG: the runner and the listener stay off.
            .env("LOSOS_TAG_FILE", dir.path().join("tag.json"))
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let daemon = Daemon {
            child,
            port,
            _dir: dir,
            token_file,
        };
        daemon.wait_until_up();
        daemon
    }

    fn wait_until_up(&self) {
        let deadline = Instant::now() + BOOT_TIMEOUT;
        while Instant::now() < deadline {
            if let Some(response) = self.try_request("GET", "/api/health", None, None) {
                if response.status == 200 {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("lososd never answered on 127.0.0.1:{}", self.port);
    }

    fn token(&self) -> String {
        std::fs::read_to_string(&self.token_file).unwrap()
    }

    fn try_request(
        &self,
        method: &str,
        path: &str,
        authorization: Option<&str>,
        body: Option<&str>,
    ) -> Option<Response> {
        let mut request =
            format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        if let Some(value) = authorization {
            request.push_str(&format!("Authorization: {value}\r\n"));
        }
        if body.is_some() {
            request.push_str("Content-Type: application/json\r\n");
        }
        let body = body.unwrap_or_default();
        request.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
        request.push_str(body);
        let mut socket = TcpStream::connect(("127.0.0.1", self.port)).ok()?;
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        socket.write_all(request.as_bytes()).ok()?;
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).ok()?;
        Response::parse(&raw)
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        authorization: Option<&str>,
        body: Option<&str>,
    ) -> Response {
        self.try_request(method, path, authorization, body)
            .unwrap_or_else(|| panic!("no response to {method} {path}"))
    }

    /// One call with the spare key.
    fn admin(&self, method: &str, path: &str, body: Option<&str>) -> Response {
        let token = self.token();
        self.request(method, path, Some(&format!("Bearer {token}")), body)
    }
}

struct Response {
    status: u16,
    body: String,
}

impl Response {
    fn parse(raw: &[u8]) -> Option<Response> {
        let text = String::from_utf8_lossy(raw);
        let status = text.split_whitespace().nth(1)?.parse().ok()?;
        let body = text.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        Some(Response {
            status,
            body: body.to_string(),
        })
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| panic!("{}: {}", e, self.body))
    }
}

#[test]
fn the_tag_route_needs_the_token() {
    let daemon = Daemon::start();
    let refused = daemon.request("GET", "/api/tag", None, None);
    assert_eq!(refused.status, 401);
    let open = daemon.admin("GET", "/api/tag", None);
    assert_eq!(open.status, 200, "{}", open.body);
    let view = open.json();
    // The seeds: one level, the built-in assistant, the four skills.
    assert_eq!(view["levels"][0]["name"], "everyday");
    assert_eq!(view["assistants"][0]["slug"], "losos-tag");
    assert!(view["skills"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["slug"] == "find"));
}

#[test]
fn the_levels_areas_and_assistants_land_in_one_file() {
    let daemon = Daemon::start();

    let level = daemon.admin(
        "POST",
        "/api/tag/levels",
        Some(r#"{"name":"personal","description":"mine"}"#),
    );
    assert_eq!(level.status, 200, "{}", level.body);

    // An area at a level the box does not have is refused.
    let bad = daemon.admin(
        "POST",
        "/api/tag/areas",
        Some(r#"{"kind":"git","ref":"eva/wallet","level":"secret"}"#),
    );
    assert_eq!(bad.status, 400, "{}", bad.body);

    let area = daemon.admin(
        "POST",
        "/api/tag/areas",
        Some(r#"{"kind":"git","ref":"eva/wallet","level":"personal"}"#),
    );
    assert_eq!(area.status, 200, "{}", area.body);

    let assistant = daemon.admin(
        "POST",
        "/api/tag/assistants",
        Some(r#"{"slug":"gardener","display":"Gardener","clearance":"personal","prompt":"tend the plants"}"#),
    );
    assert_eq!(assistant.status, 200, "{}", assistant.body);

    let view = daemon.admin("GET", "/api/tag", None).json();
    let levels: Vec<&str> = view["levels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(levels, ["everyday", "personal"]);
    assert!(view["areas"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["ref"] == "eva/wallet" && a["level"] == "personal"));
    assert!(view["assistants"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["slug"] == "gardener" && a["clearance"] == "personal"));

    // A level name that is not letters, digits and - is refused.
    let bad = daemon.admin("POST", "/api/tag/levels", Some(r#"{"name":"top secret!"}"#));
    assert_eq!(bad.status, 400, "{}", bad.body);
}

#[test]
fn an_ask_queues_and_stops_and_the_search_answers() {
    let daemon = Daemon::start();

    let asked = daemon.admin(
        "POST",
        "/api/tag/ask",
        Some(r#"{"question":"what is due?"}"#),
    );
    assert_eq!(asked.status, 200, "{}", asked.body);
    let id = asked.json()["id"].as_str().unwrap().to_string();
    assert_eq!(id.len(), 24);

    let view = daemon.admin("GET", "/api/tag", None).json();
    let job = view["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["id"] == id)
        .expect("the ask on the queue");
    assert_eq!(job["state"], "queued");
    assert_eq!(job["origin"], "suggestion");

    let stopped = daemon.admin("DELETE", &format!("/api/tag/asks/{id}"), None);
    assert_eq!(stopped.status, 200, "{}", stopped.body);
    let view = daemon.admin("GET", "/api/tag", None).json();
    let job = view["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["id"] == id)
        .unwrap();
    assert_eq!(job["state"], "stopped");

    // An empty question is refused; the unified search answers with no
    // sources up (no sockets in the test) and never fails.
    let empty = daemon.admin("POST", "/api/tag/ask", Some(r#"{"question":"  "}"#));
    assert_eq!(empty.status, 400, "{}", empty.body);
    let search = daemon.admin("GET", "/api/search?q=tax", None);
    assert_eq!(search.status, 200, "{}", search.body);
    assert_eq!(search.json()["hits"].as_array().unwrap().len(), 0);
    let bad = daemon.admin("GET", "/api/search?q=%00", None);
    assert_eq!(bad.status, 400, "{}", bad.body);

    // The suggestion needs a room, and says no on an empty tail.
    let no_room = daemon.admin("GET", "/api/tag/suggest", None);
    assert_eq!(no_room.status, 400);
    let suggest = daemon.admin("GET", "/api/tag/suggest?room=!x:box", None);
    assert_eq!(suggest.status, 200, "{}", suggest.body);
    assert_eq!(suggest.json()["suggest"], false);
}

#[test]
fn a_voice_ask_runs_as_a_mention_and_its_state_can_be_read() {
    let daemon = Daemon::start();

    // A call's agent asks for a voice answer: no room to post into, and
    // none of the asker's own material unless the page says they allowed it.
    let asked = daemon.admin(
        "POST",
        "/api/tag/ask",
        Some(r#"{"question":"what is due?","voice":true,"room":"!x:box"}"#),
    );
    assert_eq!(asked.status, 200, "{}", asked.body);
    let id = asked.json()["id"].as_str().unwrap().to_string();

    let view = daemon.admin("GET", "/api/tag", None).json();
    let job = view["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["id"] == id)
        .expect("the ask on the queue");
    assert_eq!(job["origin"], "mention");
    assert!(job.get("room").is_none_or(|r| r.is_null()), "{job}");
    assert!(job.get("material").is_none_or(|m| m.is_null()), "{job}");

    // The page reads it back by id while the answer grows.
    let read = daemon.admin("GET", &format!("/api/tag/asks/{id}"), None);
    assert_eq!(read.status, 200, "{}", read.body);
    let state = read.json();
    assert_eq!(state["id"], id);
    assert!(state["answer"].is_string() || state["answer"].is_null());

    // An ask nobody made is not found.
    let missing = daemon.admin("GET", "/api/tag/asks/nosuchask", None);
    assert_eq!(missing.status, 404, "{}", missing.body);

    // Without the token the route is closed like the rest.
    let refused = daemon.request("GET", &format!("/api/tag/asks/{id}"), None, None);
    assert_eq!(refused.status, 401);
}
