//! LosOS Git against real git: the `losos-git` binary on a free port, a
//! throwaway state directory, and `git clone`/`git push` as a person would
//! run them.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Server {
    child: Child,
    port: u16,
    state: PathBuf,
    admin: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start(dir: &Path, extra: &[&str]) -> Server {
    let port = free_port();
    let state = dir.join("state");
    let child = Command::new(env!("CARGO_BIN_EXE_losos-git"))
        .args(["serve", "--state-dir"])
        .arg(&state)
        .args(["--listen", &format!("127.0.0.1:{port}")])
        .args(extra)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "losos-git did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    let admin = std::fs::read_to_string(state.join("admin-token")).unwrap();
    Server {
        child,
        port,
        state,
        admin: admin.trim().to_string(),
    }
}

/// One HTTP/1.0 request; status and body.
fn http(port: u16, method: &str, path: &str, auth: Option<&str>, body: Option<&str>) -> (u16, String) {
    let mut req = format!("{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n");
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: {a}\r\n"));
    }
    if let Some(b) = body {
        req.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            b.len()
        ));
    }
    req.push_str("\r\n");
    if let Some(b) = body {
        req.push_str(b);
    }
    raw(port, req.as_bytes())
}

fn raw(port: u16, request: &[u8]) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(request).unwrap();
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

fn json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or_else(|_| panic!("not JSON: {s}"))
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@localhost")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@localhost")
        .args(args)
        .output()
        .unwrap()
}

fn ok(out: std::process::Output) -> String {
    assert!(
        out.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("losos-forge-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The owner's account and a token, made the way lososd makes them.
fn owner_token(s: &Server, scopes: &str, name: &str) -> String {
    let admin = format!("token {}", s.admin);
    let (status, _) = http(
        s.port,
        "PUT",
        "/api/v1/admin/users/notshared",
        Some(&admin),
        Some(r#"{"full_name":"Owner","email":"owner@example.org","admin":true}"#),
    );
    assert_eq!(status, 200);
    let (status, body) = http(
        s.port,
        "POST",
        "/api/v1/admin/users/notshared/tokens",
        Some(&admin),
        Some(&format!(r#"{{"name":"{name}","scopes":[{scopes}],"replace":true}}"#)),
    );
    assert_eq!(status, 201, "{body}");
    json(&body)["token"].as_str().unwrap().to_string()
}

#[test]
fn the_configuration_repository_round_trip() {
    let dir = scratch("config");
    let s = start(&dir, &[]);
    let admin = format!("token {}", s.admin);
    let token = owner_token(&s, r#""write:repository","read:user""#, "LosOS on laptop");

    // What lososd's reconciler does: make sure the private repository is
    // there, twice, and get the same one back.
    let (status, body) = http(
        s.port,
        "PUT",
        "/api/v1/admin/users/notshared/repos/losos-config",
        Some(&admin),
        Some(r#"{"description":"This box's NixOS configuration.","private":true,"default_branch":"main"}"#),
    );
    assert_eq!(status, 201, "{body}");
    let (status, again) = http(
        s.port,
        "PUT",
        "/api/v1/admin/users/notshared/repos/losos-config",
        Some(&admin),
        Some("{}"),
    );
    assert_eq!(status, 200);
    assert_eq!(json(&body)["id"], json(&again)["id"]);
    assert!(s.state.join("repositories/notshared/losos-config.git/HEAD").exists());
    assert_eq!(json(&body)["empty"], true);

    // Nobody is asked to sign in, never told the repository exists.
    let url = format!("http://127.0.0.1:{}/notshared/losos-config.git", s.port);
    let out = git(&dir, &["clone", &url, "nobody"]);
    assert!(!out.status.success());
    let (status, _) = http(
        s.port,
        "GET",
        "/notshared/losos-config.git/info/refs?service=git-upload-pack",
        None,
        None,
    );
    assert_eq!(status, 401);

    // The admin token pushes, as lososd does.
    let work = dir.join("work");
    ok(git(&dir, &["init", "-q", "-b", "main", "work"]));
    std::fs::write(work.join("overrides.nix"), "{ ... }:\n{\n}\n").unwrap();
    ok(git(&work, &["add", "."]));
    ok(git(&work, &["commit", "-q", "-m", "Install"]));
    let lososd = format!("http://losos:{}@127.0.0.1:{}/notshared/losos-config.git", s.admin, s.port);
    ok(git(&work, &["push", "-q", &lososd, "main"]));

    // The owner's token clones it, with protocol 2 and with protocol 0.
    let mine = format!("http://notshared:{token}@127.0.0.1:{}/notshared/losos-config.git", s.port);
    ok(git(&dir, &["clone", "-q", &mine, "laptop"]));
    ok(git(&dir, &["-c", "protocol.version=0", "clone", "-q", &mine, "laptop0"]));
    let laptop = dir.join("laptop");
    assert_eq!(
        std::fs::read_to_string(laptop.join("overrides.nix")).unwrap(),
        "{ ... }:\n{\n}\n"
    );

    // A change pushed from the laptop is on the server.
    std::fs::write(laptop.join("overrides.nix"), "{ ... }:\n{\n  losos.sharingMyStorage = true;\n}\n").unwrap();
    ok(git(&laptop, &["commit", "-qam", "Share storage"]));
    ok(git(&laptop, &["push", "-q", "origin", "main"]));
    let head = ok(git(&laptop, &["rev-parse", "HEAD"]));
    let server = ok(git(
        &s.state.join("repositories/notshared/losos-config.git"),
        &["rev-parse", "main"],
    ));
    assert_eq!(head, server);
    let (_, doc) = http(
        s.port,
        "GET",
        "/api/v1/repos/notshared/losos-config",
        Some(&format!("token {token}")),
        None,
    );
    assert_eq!(json(&doc)["empty"], false);
    assert_eq!(json(&doc)["private"], true);
}

#[test]
fn tokens_hold_their_scopes_and_owners_their_namespace() {
    let dir = scratch("scopes");
    let s = start(&dir, &["--push-create"]);
    let admin = format!("token {}", s.admin);
    let write = owner_token(&s, r#""write:repository""#, "write");
    let read = owner_token(&s, r#""read:repository""#, "read");

    // Push-to-create in one's own namespace.
    let work = dir.join("work");
    ok(git(&dir, &["init", "-q", "-b", "trunk", "work"]));
    std::fs::write(work.join("a"), "a").unwrap();
    ok(git(&work, &["add", "."]));
    ok(git(&work, &["commit", "-q", "-m", "a"]));
    let url = |t: &str, repo: &str| format!("http://notshared:{t}@127.0.0.1:{}/{repo}.git", s.port);
    ok(git(&work, &["push", "-q", &url(&write, "notshared/fresh"), "trunk"]));
    let (status, doc) = http(s.port, "GET", "/api/v1/repos/notshared/fresh", Some(&format!("token {write}")), None);
    assert_eq!(status, 200);
    let doc = json(&doc);
    assert_eq!(doc["private"], true, "new repositories are private by default");
    assert_eq!(doc["default_branch"], "trunk", "the first pushed branch becomes the default");

    // A read token clones and cannot push.
    ok(git(&dir, &["clone", "-q", &url(&read, "notshared/fresh"), "clone"]));
    let clone = dir.join("clone");
    std::fs::write(clone.join("b"), "b").unwrap();
    ok(git(&clone, &["add", "."]));
    ok(git(&clone, &["commit", "-q", "-m", "b"]));
    let out = git(&clone, &["push", "-q", "origin", "trunk"]);
    assert!(!out.status.success(), "a read token pushed");

    // Nobody else's namespace is created by a push.
    let (status, _) = http(
        s.port,
        "PUT",
        "/api/v1/admin/users/jana",
        Some(&admin),
        Some(r#"{"full_name":"Jana"}"#),
    );
    assert_eq!(status, 200);
    let out = git(&work, &["push", "-q", &url(&write, "jana/stolen"), "trunk"]);
    assert!(!out.status.success());
    assert!(!s.state.join("repositories/jana/stolen.git").exists());

    // A wrong token is a 401, not "nobody".
    let (status, _) = http(s.port, "GET", "/api/v1/user", Some("token 0123456789"), None);
    assert_eq!(status, 401);
    // A token cannot mint another.
    let (status, _) = http(
        s.port,
        "POST",
        "/api/v1/user/tokens",
        Some(&format!("token {write}")),
        Some(r#"{"name":"more","scopes":["all"]}"#),
    );
    assert_eq!(status, 403);
    // The administrator removes a computer's token; it stops working.
    let (status, _) = http(s.port, "DELETE", "/api/v1/admin/users/notshared/tokens/read", Some(&admin), None);
    assert_eq!(status, 204);
    let (status, _) = http(s.port, "GET", "/api/v1/repos/notshared/fresh", Some(&format!("token {read}")), None);
    assert_eq!(status, 401);
}

#[test]
fn public_repositories_rename_and_collaborators() {
    let dir = scratch("public");
    let s = start(&dir, &[]);
    let admin = format!("token {}", s.admin);
    let owner = owner_token(&s, r#""all""#, "all");
    let auth = format!("token {owner}");

    let (status, body) = http(
        s.port,
        "POST",
        "/api/v1/user/repos",
        Some(&auth),
        Some(r#"{"name":"Notes","description":"Open notes","private":false,"auto_init":true}"#),
    );
    assert_eq!(status, 201, "{body}");
    let doc = json(&body);
    assert_eq!(doc["empty"], false);
    assert_eq!(doc["html_url"], "/git/notshared/Notes");
    assert!(doc["clone_url"]
        .as_str()
        .unwrap()
        .ends_with("/git/notshared/Notes.git"));
    let (status, _) = http(s.port, "POST", "/api/v1/user/repos", Some(&auth), Some(r#"{"name":"notes"}"#));
    assert_eq!(status, 409, "names are unique whatever the case");
    let (status, _) = http(s.port, "POST", "/api/v1/user/repos", Some(&auth), Some(r#"{"name":"../escape"}"#));
    assert_eq!(status, 422);

    // Anybody clones a public repository.
    let anon = format!("http://127.0.0.1:{}/notshared/Notes.git", s.port);
    ok(git(&dir, &["clone", "-q", &anon, "anon"]));
    assert_eq!(std::fs::read_to_string(dir.join("anon/README.md")).unwrap(), "# Notes\n\nOpen notes\n");

    // Renamed, it is found under the new name only.
    let (status, body) = http(
        s.port,
        "PATCH",
        "/api/v1/repos/notshared/Notes",
        Some(&auth),
        Some(r#"{"name":"notebook","private":true}"#),
    );
    assert_eq!(status, 200, "{body}");
    assert!(s.state.join("repositories/notshared/notebook.git/HEAD").exists());
    let (status, _) = http(s.port, "GET", "/api/v1/repos/notshared/Notes", Some(&auth), None);
    assert_eq!(status, 404);
    let out = git(&dir, &["clone", "-q", &format!("http://127.0.0.1:{}/notshared/notebook.git", s.port), "gone"]);
    assert!(!out.status.success(), "private now");

    // A collaborator sees it; before that, nothing.
    http(s.port, "PUT", "/api/v1/admin/users/jana", Some(&admin), Some("{}"));
    let (_, body) = http(
        s.port,
        "POST",
        "/api/v1/admin/users/jana/tokens",
        Some(&admin),
        Some(r#"{"name":"t","scopes":["write:repository"]}"#),
    );
    let jana = format!("token {}", json(&body)["token"].as_str().unwrap());
    let (status, _) = http(s.port, "GET", "/api/v1/repos/notshared/notebook", Some(&jana), None);
    assert_eq!(status, 404);
    let (status, _) = http(
        s.port,
        "PUT",
        "/api/v1/repos/notshared/notebook/collaborators/jana",
        Some(&auth),
        Some(r#"{"permission":"read"}"#),
    );
    assert_eq!(status, 204);
    let (status, body) = http(s.port, "GET", "/api/v1/repos/notshared/notebook", Some(&jana), None);
    assert_eq!(status, 200);
    assert_eq!(json(&body)["permission"], "read");
    let (status, _) = http(s.port, "DELETE", "/api/v1/repos/notshared/notebook", Some(&jana), None);
    assert_eq!(status, 403);
    let (status, _) = http(s.port, "DELETE", "/api/v1/repos/notshared/notebook", Some(&auth), None);
    assert_eq!(status, 204);
    assert!(!s.state.join("repositories/notshared/notebook.git").exists());
}

#[test]
fn a_gzipped_fetch_request_is_unzipped_for_git() {
    let dir = scratch("gzip");
    let s = start(&dir, &["--public-by-default"]);
    let owner = owner_token(&s, r#""all""#, "all");
    let (status, body) = http(
        s.port,
        "POST",
        "/api/v1/user/repos",
        Some(&format!("token {owner}")),
        Some(r#"{"name":"z","auto_init":true}"#),
    );
    assert_eq!(status, 201, "{body}");
    let sha = ok(git(&s.state.join("repositories/notshared/z.git"), &["rev-parse", "main"]));
    let sha = sha.trim();

    let want = format!("want {sha}\n");
    let mut plain = format!("{:04x}{want}", want.len() + 4).into_bytes();
    plain.extend(b"0000");
    plain.extend(b"0009done\n");
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&plain).unwrap();
    let gz = gz.finish().unwrap();
    let mut req = format!(
        "POST /notshared/z.git/git-upload-pack HTTP/1.0\r\nHost: x\r\n\
         Content-Type: application/x-git-upload-pack-request\r\n\
         Content-Encoding: gzip\r\nContent-Length: {}\r\n\r\n",
        gz.len()
    )
    .into_bytes();
    req.extend(gz);
    let (status, body) = raw(s.port, &req);
    assert_eq!(status, 200);
    assert!(body.contains("PACK"), "no pack in the answer: {body:?}");
}

#[test]
fn repositories_on_disk_without_a_record_are_taken_in() {
    let dir = scratch("adopt");
    // A bare repository where Forgejo kept it, moved over before the start.
    let repo = dir.join("state/repositories/notshared/old-project.git");
    std::fs::create_dir_all(repo.parent().unwrap()).unwrap();
    ok(git(&dir, &["init", "-q", "--bare", "-b", "master", repo.to_str().unwrap()]));
    // Forgejo's hooks call a forgejo binary that is gone: left in place,
    // they would refuse every push.
    let hooks = repo.join("hooks");
    std::fs::create_dir_all(hooks.join("pre-receive.d")).unwrap();
    for hook in ["pre-receive", "update", "post-receive"] {
        std::fs::write(hooks.join(hook), "#!/bin/sh\nexit 1\n").unwrap();
        let f = hooks.join(hook);
        let mut p = std::fs::metadata(&f).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut p, 0o755);
        std::fs::set_permissions(&f, p).unwrap();
    }
    std::fs::write(hooks.join("pre-receive.d/gitea"), "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::write(hooks.join("update.sample"), "#!/bin/sh\n").unwrap();
    let s = start(&dir, &[]);
    assert!(!hooks.join("pre-receive").exists(), "Forgejo's hook is gone");
    assert!(!hooks.join("pre-receive.d").exists());
    assert!(hooks.join("update.sample").exists(), "git's own samples stay");
    let token = owner_token(&s, r#""read:repository""#, "r");
    let (status, body) = http(
        s.port,
        "GET",
        "/api/v1/repos/notshared/old-project",
        Some(&format!("token {token}")),
        None,
    );
    assert_eq!(status, 200, "{body}");
    let doc = json(&body);
    assert_eq!(doc["private"], true);
    assert_eq!(doc["default_branch"], "master");
}
