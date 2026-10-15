//! Text inside files, end to end: `losos-search index` builds the cloud index,
//! `losos-search cloud` and `losos-search git` answer on unix sockets the way
//! systemd runs them (one process per connection, the socket as stdin and
//! stdout), and lososd relays `GET /api/cloud/text` and `GET /api/git/text`.

use assert_fs::TempDir;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SEARCH: &str = env!("CARGO_BIN_EXE_losos-search");

/// Serve `args` on a socket at `path` like `Accept=yes` does.
fn listen(path: &Path, args: Vec<String>) {
    let listener = UnixListener::bind(path).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fd = OwnedFd::from(stream);
            let input = fd.try_clone().unwrap();
            let _ = Command::new(SEARCH)
                .args(&args)
                .stdin(Stdio::from(input))
                .stdout(Stdio::from(fd))
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut c| c.wait());
        }
    });
}

struct Daemon {
    child: Child,
    port: u16,
    token: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Daemon {
    fn start(dir: &Path, env: &[(&str, PathBuf)]) -> Daemon {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let token_file = dir.join("token");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lososd"));
        cmd.env("LOSOS_NO_DBUS", "1")
            .env("LOSOS_STATE_DIR", dir.join("state"))
            .env("LOSOS_OVERRIDES", dir.join("overrides.nix"))
            .env("LOSOS_ADMIN_TOKEN_FILE", &token_file)
            .env("LOSOS_ADMIN_PORT", port.to_string())
            .env_remove("LOSOS_SEARCH_CLOUD_SOCKET")
            .env_remove("LOSOS_SEARCH_GIT_SOCKET")
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let child = cmd.spawn().unwrap();
        let mut daemon = Daemon {
            child,
            port,
            token: String::new(),
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        while daemon.get("/api/health", false).is_none() {
            assert!(Instant::now() < deadline, "lososd did not come up");
            std::thread::sleep(Duration::from_millis(50));
        }
        daemon.token = std::fs::read_to_string(token_file)
            .unwrap()
            .trim()
            .to_string();
        daemon
    }

    fn get(&self, path: &str, auth: bool) -> Option<(u16, serde_json::Value)> {
        let mut request =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
        if auth {
            request.push_str(&format!("Authorization: Bearer {}\r\n", self.token));
        }
        request.push_str("\r\n");
        let mut socket = TcpStream::connect(("127.0.0.1", self.port)).ok()?;
        socket
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        socket.write_all(request.as_bytes()).ok()?;
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).ok()?;
        let text = String::from_utf8_lossy(&raw);
        let status = text.split_whitespace().nth(1)?.parse().ok()?;
        let body = text.split_once("\r\n\r\n").map_or("", |(_, b)| b);
        Some((status, serde_json::from_str(body).unwrap_or_default()))
    }

    fn ask(&self, path: &str) -> (u16, serde_json::Value) {
        self.get(path, true)
            .unwrap_or_else(|| panic!("no answer to {path}"))
    }
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn text_in_cloud_files_and_git_repositories_reaches_the_palette() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();

    // LosOS cloud: two files, one of them matching, then an index pass.
    let files = d.join("files");
    std::fs::create_dir_all(files.join("Documents")).unwrap();
    std::fs::write(
        files.join("Documents/Daňové priznanie.md"),
        "# 2025\nThe tax return is due in March.\n",
    )
    .unwrap();
    std::fs::write(files.join("list.txt"), "milk, bread").unwrap();
    let db = d.join("index.sqlite");
    let status = Command::new(SEARCH)
        .args(["index", "--root"])
        .arg(&files)
        .arg("--db")
        .arg(&db)
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    // LosOS Git: one repository with a matching line.
    let work = d.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("taxes.py"), "RATE = 0.19\ndef tax_due(x):\n").unwrap();
    git(d, &["init", "-q", "work"]);
    git(&work, &["add", "."]);
    git(&work, &["commit", "-qm", "first"]);
    std::fs::create_dir_all(d.join("repositories/matus")).unwrap();
    git(
        d,
        &[
            "clone",
            "-q",
            "--bare",
            "work",
            "repositories/matus/accounts.git",
        ],
    );

    let cloud = d.join("cloud.sock");
    let repos = d.join("git.sock");
    listen(
        &cloud,
        vec!["cloud".into(), "--db".into(), db.display().to_string()],
    );
    listen(
        &repos,
        vec![
            "git".into(),
            "--repos".into(),
            d.join("repositories").display().to_string(),
        ],
    );
    let daemon = Daemon::start(
        d,
        &[
            ("LOSOS_SEARCH_CLOUD_SOCKET", cloud),
            ("LOSOS_SEARCH_GIT_SOCKET", repos),
        ],
    );

    assert_eq!(daemon.get("/api/cloud/text?q=tax", false).unwrap().0, 401);

    let (status, body) = daemon.ask("/api/cloud/text?q=tax%20retu");
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["available"], true);
    assert_eq!(body["partial"], false);
    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{body}");
    assert_eq!(hits[0]["path"], "Documents/Daňové priznanie.md");
    assert_eq!(hits[0]["snippet"][1], "tax");

    let (status, body) = daemon.ask("/api/git/text?q=TAX_due");
    assert_eq!(status, 200, "{body}");
    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{body}");
    assert_eq!(hits[0]["owner"], "matus");
    assert_eq!(hits[0]["repo"], "accounts");
    assert_eq!(hits[0]["branch"], "main");
    assert_eq!(hits[0]["path"], "taxes.py");
    assert_eq!(hits[0]["line"], 2);

    // A query with nothing to search for is a 400, never a 404: the palette
    // latches a 404 as "not served".
    for path in [
        "/api/cloud/text",
        "/api/cloud/text?q=--",
        "/api/git/text?q=",
    ] {
        assert_eq!(daemon.ask(path).0, 400, "{path}");
    }
}

#[test]
fn a_box_without_the_searches_says_so_without_a_404() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    // The cloud socket is configured but nothing listens yet; Git is off.
    let daemon = Daemon::start(d, &[("LOSOS_SEARCH_CLOUD_SOCKET", d.join("absent.sock"))]);
    let (status, body) = daemon.ask("/api/cloud/text?q=tax");
    assert_eq!((status, body["available"].clone()), (200, false.into()));
    assert_eq!(body["reason"], "notUp");
    let (status, body) = daemon.ask("/api/git/text?q=tax");
    assert_eq!((status, body["available"].clone()), (200, false.into()));
    assert_eq!(body["reason"], "noGit");
}
