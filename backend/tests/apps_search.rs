//! `GET /api/apps/search` at the HTTP boundary: auth, and the validation that
//! happens before anything reaches the network.
//!
//! **Nothing here searches.** A test that let the daemon run `curl` would pass
//! or fail on whether the machine running it has a route to the internet, which
//! is not a property of this code — CI's runner has one, a nix sandbox does
//! not, and the environment this was written in refuses the host outright. The
//! two halves that *are* deterministic are tested where they live: the
//! response mapping in `backend/src/catalogue.rs`'s own unit tests, and the
//! command's behaviour against a recorded body in `backend/src/losos.rs`.
//!
//! What is left, and what this file is for, is the boundary: that the route is
//! behind the same Bearer gate as everything else, and that a query the box
//! will not act on comes back as a 400 rather than a 404.

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
            .env("LOSOS_CONFIG", dir.path().join("defaults.nix"))
            .env("LOSOS_OVERRIDES", dir.path().join("overrides.nix"))
            .env("LOSOS_ADMIN_TOKEN_FILE", &token_file)
            .env("LOSOS_ADMIN_PORT", port.to_string())
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
            if self.try_request("GET", "/api/health", None).is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("lososd did not answer /api/health within {BOOT_TIMEOUT:?}");
    }

    fn token(&self) -> String {
        std::fs::read_to_string(&self.token_file)
            .unwrap()
            .trim()
            .to_string()
    }

    fn try_request(
        &self,
        method: &str,
        path: &str,
        authorization: Option<&str>,
    ) -> Option<Response> {
        let mut request =
            format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        if let Some(value) = authorization {
            request.push_str(&format!("Authorization: {value}\r\n"));
        }
        request.push_str("Content-Length: 0\r\n\r\n");

        let mut socket = TcpStream::connect(("127.0.0.1", self.port)).ok()?;
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        socket.write_all(request.as_bytes()).ok()?;
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).ok()?;
        Response::parse(&raw)
    }

    fn get(&self, path: &str, authorization: Option<&str>) -> Response {
        self.try_request("GET", path, authorization)
            .unwrap_or_else(|| panic!("no response to GET {path}"))
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
}

#[test]
fn the_search_route_is_behind_the_same_bearer_gate_as_everything_else() {
    let daemon = Daemon::start();
    let response = daemon.get("/api/apps/search?q=nextcloud", None);
    assert_eq!(response.status, 401);
    assert!(response.body.contains("unauthorized"), "{}", response.body);
}

/// The property this route most needs, and the one that is invisible when it
/// breaks.
///
/// `admin-ui/app/src/screens/settings/catalogue.ts` latches a 404 as "this box
/// does not serve the route" and never asks again for the rest of the session.
/// So a query the box declines has to come back as a 400: answering it the
/// other way would disable the search field until the owner reloaded the page,
/// and nothing would say why.
#[test]
fn a_query_the_box_will_not_act_on_is_a_400_never_a_404() {
    let daemon = Daemon::start();
    let token = format!("Bearer {}", daemon.token());

    // Derived from the constant rather than written out, so the case cannot
    // drift away from the rule it is checking.
    let too_long = format!(
        "/api/apps/search?q={}",
        "n".repeat(losos_ctl::catalogue::MAX_QUERY_CHARS + 1)
    );
    let cases: [(&str, &str); 6] = [
        ("no q at all", "/api/apps/search"),
        ("an empty q", "/api/apps/search?q="),
        ("a q of one character", "/api/apps/search?q=n"),
        ("a q that is only spaces", "/api/apps/search?q=%20%20%20"),
        // %0A is a newline. It never reaches an argv — the fetch execs curl
        // directly — but the refusal must not depend on that staying true.
        ("a q carrying a newline", "/api/apps/search?q=next%0Acloud"),
        ("a q past the length cap", &too_long),
    ];
    for (label, path) in cases {
        let response = daemon.get(path, Some(&token));
        assert_eq!(
            response.status, 400,
            "{label} answered {} — a 404 here latches the search off in the SPA",
            response.status
        );
        assert!(
            response.body.contains("error"),
            "{label}: {}",
            response.body
        );
    }
}

/// A path the daemon really does not serve still 404s, so the SPA's latch
/// keeps working for the case it was written for — an older lososd in front of
/// a newer UI.
#[test]
fn an_unserved_path_under_the_same_prefix_is_still_a_404() {
    let daemon = Daemon::start();
    let token = format!("Bearer {}", daemon.token());
    let response = daemon.get("/api/apps/install", Some(&token));
    assert_eq!(response.status, 404, "{}", response.body);
}
