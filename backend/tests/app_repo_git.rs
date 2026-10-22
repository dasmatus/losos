//! The owner's apps in repositories of their own, against a real `git` and a
//! real `losos-git`: lososd's side ([`IoLosos`] and
//! `user_apps::cmd_sync`) makes the private repository, pushes each save,
//! takes a version a clone pushes and answers one the checks turn down,
//! the way `src/user_apps.rs` tests it over the fake.
//!
//! One test, because the daemon finds LosOS Git through its environment
//! (`$LOSOS_CONFIG_REPO`, `$LOSOS_GIT_URL`, `$LOSOS_GIT_TOKEN_FILE`), which is
//! the process's, and this file is a process of its own.

use assert_fs::TempDir;
use losos_ctl::io_backend::{IoLosos, Paths};
use losos_ctl::losos::Losos;
use losos_ctl::user_apps::{cmd_put, cmd_sync, Draft};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `losos-git` on a free port; the port and the server.
fn start(state: &Path) -> (u16, Server) {
    for _ in 0..5 {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut child = Command::new(env!("CARGO_BIN_EXE_losos-git"))
            .args(["serve", "--state-dir"])
            .arg(state)
            .args(["--listen", &format!("127.0.0.1:{port}")])
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "losos-git did not start");
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(200));
        if child.try_wait().unwrap().is_none() {
            return (port, Server(child));
        }
    }
    panic!("losos-git did not start on any of five ports");
}

fn http(port: u16, method: &str, path: &str, token: &str, body: Option<&str>) -> (u16, String) {
    let mut req = format!(
        "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nAuthorization: token {token}\r\n"
    );
    if let Some(b) = body {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            b.len()
        ));
    }
    req.push_str("\r\n");
    req.push_str(body.unwrap_or(""));
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(req.as_bytes()).unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).unwrap();
    let text = String::from_utf8_lossy(&out).into_owned();
    let status = text[9..12].parse().unwrap();
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

/// `git` as the owner runs it in their clone.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["-c", "user.name=Owner", "-c", "user.email=owner@localhost"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

const SOURCE: &str = "function App() {\n  const chores = useCollection('chores');\n  return <Page title=\"Chores\">{chores.items.length}</Page>;\n}\n";

#[test]
fn an_app_lives_in_a_private_repository_and_takes_what_its_owner_pushes() {
    let dir = TempDir::new().unwrap();
    let git_state = dir.path().join("losos-git");
    let (port, _server) = start(&git_state);
    let admin = std::fs::read_to_string(git_state.join("admin-token"))
        .unwrap()
        .trim()
        .to_string();
    let (status, body) = http(
        port,
        "PUT",
        "/api/v1/admin/users/notshared",
        &admin,
        Some(r#"{"full_name": "Owner", "email": ""}"#),
    );
    assert!(status < 300, "{status} {body}");

    std::env::set_var("LOSOS_CONFIG_REPO", "notshared/losos-config");
    std::env::set_var("LOSOS_GIT_URL", format!("http://127.0.0.1:{port}"));
    std::env::set_var("LOSOS_GIT_TOKEN_FILE", git_state.join("admin-token"));
    std::env::remove_var("LOSOS_CONFIG_REMOTE_URL");
    let state_dir = dir.path().join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    let mut l = IoLosos::new(Paths {
        state_dir,
        overrides_file: dir.path().join("overrides.nix"),
        flake_ref: "/etc/nixos#install".to_string(),
        config_dir: dir.path().join("etc-nixos"),
        options_file: dir.path().join("no-such-options.json"),
    });

    // A save commits; the sync makes the repository and pushes.
    cmd_put(
        &mut l,
        &Draft {
            id: None,
            name: "Chores".into(),
            summary: Some("Who does what.".into()),
            prompt: Some("a chore list for the kids".into()),
            source: SOURCE.into(),
        },
    )
    .unwrap();
    let out = cmd_sync(&mut l).unwrap();
    assert_eq!(out["apps"][0]["repo"]["state"], "synced", "{out}");
    assert_eq!(out["apps"][0]["repo"]["name"], "notshared/app-chores");
    let (status, body) = http(
        port,
        "GET",
        "/api/v1/repos/notshared/app-chores",
        &admin,
        None,
    );
    assert_eq!(status, 200, "{body}");
    let repo: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(repo["private"], true, "{repo}");

    // The owner clones it.
    let url = format!("http://losos:{admin}@127.0.0.1:{port}/notshared/app-chores.git");
    git(dir.path(), &["clone", "-q", &url, "clone"]);
    let clone = dir.path().join("clone");
    assert_eq!(
        std::fs::read_to_string(clone.join("app.jsx")).unwrap(),
        SOURCE
    );
    assert!(std::fs::read_to_string(clone.join("README.md"))
        .unwrap()
        .starts_with("# Chores"));
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(clone.join("losos-app.json")).unwrap())
            .unwrap();
    assert_eq!(meta["name"], "Chores");
    assert_eq!(git(&clone, &["log", "-1", "--format=%s"]), "Make Chores");

    // A change pushed from the clone becomes the app.
    let changed = SOURCE.replace("title=\"Chores\"", "title=\"House jobs\"");
    std::fs::write(clone.join("app.jsx"), &changed).unwrap();
    std::fs::write(clone.join("notes.md"), "mine\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "Call them house jobs"]);
    git(&clone, &["push", "-q", "origin", "main"]);
    let out = cmd_sync(&mut l).unwrap();
    assert_eq!(out["apps"][0]["repo"]["state"], "synced", "{out}");
    let apps = l.load_user_apps().unwrap();
    assert_eq!(apps.apps[0].source, changed);
    assert!(apps.apps[0].code.contains("House jobs"));

    // A save on the box after that keeps the owner's file.
    cmd_put(
        &mut l,
        &Draft {
            id: Some("u1".into()),
            name: "Chores".into(),
            summary: None,
            prompt: None,
            source: SOURCE.into(),
        },
    )
    .unwrap();
    cmd_sync(&mut l).unwrap();
    git(&clone, &["pull", "-q", "--ff-only"]);
    assert_eq!(
        std::fs::read_to_string(clone.join("app.jsx")).unwrap(),
        SOURCE
    );
    assert_eq!(
        std::fs::read_to_string(clone.join("notes.md")).unwrap(),
        "mine\n"
    );
    assert_eq!(git(&clone, &["log", "-1", "--format=%s"]), "Change Chores");

    // A push that does not compile is answered in the history, and the app
    // stays as it was.
    std::fs::write(clone.join("app.jsx"), "function App() { return <p>; }\n").unwrap();
    git(&clone, &["commit", "-qam", "Break it"]);
    git(&clone, &["push", "-q", "origin", "main"]);
    let out = cmd_sync(&mut l).unwrap();
    assert_eq!(out["apps"][0]["repo"]["state"], "refused", "{out}");
    assert!(out["apps"][0]["repo"]["detail"]
        .as_str()
        .unwrap()
        .contains("does not compile"));
    assert_eq!(l.load_user_apps().unwrap().apps[0].source, SOURCE);
    git(&clone, &["pull", "-q", "--ff-only"]);
    assert_eq!(
        git(&clone, &["log", "-1", "--format=%s"]),
        "Keep Chores as it was"
    );
    assert_eq!(
        std::fs::read_to_string(clone.join("app.jsx")).unwrap(),
        SOURCE
    );

    // The box's copy is a bare repository beside the list.
    assert!(l.paths.app_repo_dir("u1").join("HEAD").exists());
}
