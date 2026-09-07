//! The admin API as an attacker meets it: a real `lososd`, a real socket, and
//! hand-written requests.
//!
//! `actix-web` is not in the loop for the unit tests of `ensure_token`, so the
//! header parsing and the Bearer check are only ever exercised here. Requests
//! are written by hand rather than through a client library so that the exact
//! bytes on the wire — `Bearer` with nothing after it, a lowercase scheme — are
//! what the daemon sees.

use assert_fs::TempDir;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// How long to wait for the daemon to answer on `/api/health`.
const BOOT_TIMEOUT: Duration = Duration::from_secs(20);

/// A running `lososd`, killed when the test drops it.
struct Daemon {
    child: Child,
    port: u16,
    /// Held only so the daemon's throwaway directory outlives it.
    _dir: TempDir,
    token_file: std::path::PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A loopback port nothing is listening on. Racy in principle; the window
/// between the probe closing and `lososd` binding is microseconds.
fn free_port() -> u16 {
    let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    probe.local_addr().unwrap().port()
}

impl Daemon {
    /// Start `lososd` against a throwaway directory, with `token_file_seed`
    /// written to the token file first when given.
    fn start(token_file_seed: Option<&str>) -> Daemon {
        let dir = TempDir::new().unwrap();
        let token_file = dir.path().join("token");
        if let Some(seed) = token_file_seed {
            std::fs::write(&token_file, seed).unwrap();
        }
        let port = free_port();

        let child = Command::new(env!("CARGO_BIN_EXE_lososd"))
            // No system bus in the test environment, and none needed: this is
            // the HTTP surface under test.
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
            if let Some(response) = self.try_request("GET", "/api/health", None, None) {
                if response.status == 200 {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("lososd never answered on 127.0.0.1:{}", self.port);
    }

    /// The daemon's exit status, if it has stopped by now.
    fn try_status(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().unwrap()
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

    /// Like [`Self::try_request`], but a connection failure is a test failure.
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
fn health_is_open_and_every_other_route_needs_the_token() {
    // An empty token file is what a write interrupted by the nightly reboot
    // leaves behind. The daemon must not adopt it as the secret.
    let daemon = Daemon::start(Some(""));

    let token = daemon.token();
    assert_eq!(
        token.len(),
        64,
        "empty token file was not re-minted: {token:?}"
    );
    assert!(token
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));

    // Reachability before anyone has pasted a token: deliberately public.
    let health = daemon.request("GET", "/api/health", None, None);
    assert_eq!(health.status, 200);
    assert!(health.body.contains("\"ok\""), "{}", health.body);

    let cases: [(&str, Option<String>); 7] = [
        ("no header at all", None),
        ("the bare scheme", Some("Bearer".to_string())),
        // The empty-credentials cases are the ones that mattered: against a
        // token file taken at face value, an empty file made these the
        // password.
        ("the scheme and one space", Some("Bearer ".to_string())),
        ("the scheme and two spaces", Some("Bearer  ".to_string())),
        ("the wrong scheme", Some(format!("Basic {token}"))),
        (
            "a token one byte off",
            Some(format!("Bearer {}", flip_last(&token))),
        ),
        (
            "a prefix of the token",
            Some(format!("Bearer {}", &token[..32])),
        ),
    ];
    for (label, authorization) in cases {
        let response = daemon.request("GET", "/api/state", authorization.as_deref(), None);
        assert_eq!(response.status, 401, "{label} was accepted");
        assert!(response.body.contains("unauthorized"), "{label}");
    }

    // The real token opens the door, with either spelling of the scheme —
    // RFC 7235 makes it case-insensitive.
    for scheme in ["Bearer", "bearer", "BEARER"] {
        let response = daemon.request(
            "GET",
            "/api/state",
            Some(&format!("{scheme} {token}")),
            None,
        );
        assert_eq!(response.status, 200, "{scheme} was rejected");
        assert!(response.body.contains("\"mode\""), "{}", response.body);
    }
}

/// One byte of `token` changed, keeping the length: what a constant-time
/// comparison must still reject.
fn flip_last(token: &str) -> String {
    let mut flipped = token.to_string();
    let last = if flipped.ends_with('0') { '1' } else { '0' };
    flipped.pop();
    flipped.push(last);
    flipped
}

#[test]
fn a_well_formed_token_is_kept_across_a_restart() {
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let daemon = Daemon::start(Some(token));
    assert_eq!(daemon.token(), token);
    assert_eq!(
        daemon
            .request("GET", "/api/state", Some(&format!("Bearer {token}")), None)
            .status,
        200
    );
}

#[test]
fn a_malformed_admin_port_stops_the_daemon_instead_of_binding_the_default() {
    // Binding 8082 anyway would leave Nginx proxying to a port nothing is on:
    // a dead admin UI, and not a word about why anywhere.
    let dir = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lososd"))
        .env("LOSOS_NO_DBUS", "1")
        .env("LOSOS_STATE_DIR", dir.path().join("state"))
        .env("LOSOS_ADMIN_TOKEN_FILE", dir.path().join("token"))
        .env("LOSOS_ADMIN_PORT", "http")
        .env("RUST_LOG", "error")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "lososd survived a malformed LOSOS_ADMIN_PORT; systemd would never restart it"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("LOSOS_ADMIN_PORT"), "stderr was: {stderr}");
}

#[test]
fn sigterm_stops_the_daemon_cleanly() {
    // systemd stops lososd with SIGTERM on every `nixos-rebuild switch`. Without
    // a handler the runtime just kept blocking, and the unit was killed on the
    // stop timeout instead of exiting.
    let mut daemon = Daemon::start(None);
    let pid = daemon.child.id();
    assert!(Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap()
        .success());

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match daemon.try_status() {
            Some(status) => {
                assert!(status.success(), "lososd exited {status} on SIGTERM");
                return;
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    panic!("lososd ignored SIGTERM");
}
