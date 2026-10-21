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
fn http(
    port: u16,
    method: &str,
    path: &str,
    auth: Option<&str>,
    body: Option<&str>,
) -> (u16, String) {
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
        Some(&format!(
            r#"{{"name":"{name}","scopes":[{scopes}],"replace":true}}"#
        )),
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
        Some(
            r#"{"description":"This box's NixOS configuration.","private":true,"default_branch":"main"}"#,
        ),
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
    assert!(s
        .state
        .join("repositories/notshared/losos-config.git/HEAD")
        .exists());
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
    let lososd = format!(
        "http://losos:{}@127.0.0.1:{}/notshared/losos-config.git",
        s.admin, s.port
    );
    ok(git(&work, &["push", "-q", &lososd, "main"]));

    // The owner's token clones it, with protocol 2 and with protocol 0.
    let mine = format!(
        "http://notshared:{token}@127.0.0.1:{}/notshared/losos-config.git",
        s.port
    );
    ok(git(&dir, &["clone", "-q", &mine, "laptop"]));
    ok(git(
        &dir,
        &["-c", "protocol.version=0", "clone", "-q", &mine, "laptop0"],
    ));
    let laptop = dir.join("laptop");
    assert_eq!(
        std::fs::read_to_string(laptop.join("overrides.nix")).unwrap(),
        "{ ... }:\n{\n}\n"
    );

    // A change pushed from the laptop is on the server.
    std::fs::write(
        laptop.join("overrides.nix"),
        "{ ... }:\n{\n  losos.sharingMyStorage = true;\n}\n",
    )
    .unwrap();
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
    ok(git(
        &work,
        &["push", "-q", &url(&write, "notshared/fresh"), "trunk"],
    ));
    let (status, doc) = http(
        s.port,
        "GET",
        "/api/v1/repos/notshared/fresh",
        Some(&format!("token {write}")),
        None,
    );
    assert_eq!(status, 200);
    let doc = json(&doc);
    assert_eq!(
        doc["private"], true,
        "new repositories are private by default"
    );
    assert_eq!(
        doc["default_branch"], "trunk",
        "the first pushed branch becomes the default"
    );

    // A read token clones and cannot push.
    ok(git(
        &dir,
        &["clone", "-q", &url(&read, "notshared/fresh"), "clone"],
    ));
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
    let (status, _) = http(
        s.port,
        "GET",
        "/api/v1/user",
        Some("token 0123456789"),
        None,
    );
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
    let (status, _) = http(
        s.port,
        "DELETE",
        "/api/v1/admin/users/notshared/tokens/read",
        Some(&admin),
        None,
    );
    assert_eq!(status, 204);
    let (status, _) = http(
        s.port,
        "GET",
        "/api/v1/repos/notshared/fresh",
        Some(&format!("token {read}")),
        None,
    );
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
    let (status, _) = http(
        s.port,
        "POST",
        "/api/v1/user/repos",
        Some(&auth),
        Some(r#"{"name":"notes"}"#),
    );
    assert_eq!(status, 409, "names are unique whatever the case");
    let (status, _) = http(
        s.port,
        "POST",
        "/api/v1/user/repos",
        Some(&auth),
        Some(r#"{"name":"../escape"}"#),
    );
    assert_eq!(status, 422);

    // Anybody clones a public repository.
    let anon = format!("http://127.0.0.1:{}/notshared/Notes.git", s.port);
    ok(git(&dir, &["clone", "-q", &anon, "anon"]));
    assert_eq!(
        std::fs::read_to_string(dir.join("anon/README.md")).unwrap(),
        "# Notes\n\nOpen notes\n"
    );

    // Renamed, it is found under the new name only.
    let (status, body) = http(
        s.port,
        "PATCH",
        "/api/v1/repos/notshared/Notes",
        Some(&auth),
        Some(r#"{"name":"notebook","private":true}"#),
    );
    assert_eq!(status, 200, "{body}");
    assert!(s
        .state
        .join("repositories/notshared/notebook.git/HEAD")
        .exists());
    let (status, _) = http(
        s.port,
        "GET",
        "/api/v1/repos/notshared/Notes",
        Some(&auth),
        None,
    );
    assert_eq!(status, 404);
    let out = git(
        &dir,
        &[
            "clone",
            "-q",
            &format!("http://127.0.0.1:{}/notshared/notebook.git", s.port),
            "gone",
        ],
    );
    assert!(!out.status.success(), "private now");

    // A collaborator sees it; before that, nothing.
    http(
        s.port,
        "PUT",
        "/api/v1/admin/users/jana",
        Some(&admin),
        Some("{}"),
    );
    let (_, body) = http(
        s.port,
        "POST",
        "/api/v1/admin/users/jana/tokens",
        Some(&admin),
        Some(r#"{"name":"t","scopes":["write:repository"]}"#),
    );
    let jana = format!("token {}", json(&body)["token"].as_str().unwrap());
    let (status, _) = http(
        s.port,
        "GET",
        "/api/v1/repos/notshared/notebook",
        Some(&jana),
        None,
    );
    assert_eq!(status, 404);
    let (status, _) = http(
        s.port,
        "PUT",
        "/api/v1/repos/notshared/notebook/collaborators/jana",
        Some(&auth),
        Some(r#"{"permission":"read"}"#),
    );
    assert_eq!(status, 204);
    let (status, body) = http(
        s.port,
        "GET",
        "/api/v1/repos/notshared/notebook",
        Some(&jana),
        None,
    );
    assert_eq!(status, 200);
    assert_eq!(json(&body)["permission"], "read");
    let (status, _) = http(
        s.port,
        "DELETE",
        "/api/v1/repos/notshared/notebook",
        Some(&jana),
        None,
    );
    assert_eq!(status, 403);
    let (status, _) = http(
        s.port,
        "DELETE",
        "/api/v1/repos/notshared/notebook",
        Some(&auth),
        None,
    );
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
    let sha = ok(git(
        &s.state.join("repositories/notshared/z.git"),
        &["rev-parse", "main"],
    ));
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
    ok(git(
        &dir,
        &[
            "init",
            "-q",
            "--bare",
            "-b",
            "master",
            repo.to_str().unwrap(),
        ],
    ));
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
    assert!(
        !hooks.join("pre-receive").exists(),
        "Forgejo's hook is gone"
    );
    assert!(!hooks.join("pre-receive.d").exists());
    assert!(
        hooks.join("update.sample").exists(),
        "git's own samples stay"
    );
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

#[test]
fn a_forgejo_export_is_imported_and_served() {
    let dir = scratch("import");
    let repo = dir.join("state/repositories/matus/notes.git");
    std::fs::create_dir_all(repo.parent().unwrap()).unwrap();
    ok(git(
        &dir,
        &["init", "-q", "--bare", "-b", "main", repo.to_str().unwrap()],
    ));
    let from = dir.join("forgejo");
    std::fs::create_dir_all(from.join("tables")).unwrap();
    let token = "0123456789abcdef0123456789abcdef5678abcd";
    let salt = "Q7rT1x";
    let hash = losos_ctl::forge::db::forgejo_token_hash(token, salt);
    let tables = [
        ("user", r#"[{"id":1,"name":"matus","type":0,"is_admin":true,"email":"m@b.local"},
                     {"id":2,"name":"eva","type":0}]"#.to_string()),
        ("repository", r#"[{"id":3,"owner_id":1,"name":"notes","is_private":true,"default_branch":"main"}]"#.to_string()),
        ("access_token", format!(
            r#"[{{"id":4,"uid":1,"name":"laptop","token_hash":"{hash}","token_salt":"{salt}","token_last_eight":"5678abcd","scope":"write:repository,write:issue,read:user"}}]"#
        )),
        ("issue", r#"[{"id":5,"repo_id":3,"index":12,"poster_id":2,"name":"Fix the backup script","content":"Body","is_closed":false,"is_pull":false,"created_unix":1760000000}]"#.to_string()),
        ("comment", r#"[{"id":6,"type":0,"poster_id":1,"issue_id":5,"content":"On it."}]"#.to_string()),
        ("attachment", r#"[{"id":7,"uuid":"cafe0000-0000-4000-8000-000000000007","issue_id":5,"comment_id":0,"name":"log.txt","size":3}]"#.to_string()),
    ];
    for (t, body) in tables {
        std::fs::write(from.join("tables").join(format!("{t}.json")), body).unwrap();
    }
    let att = from.join("attachments/c/a");
    std::fs::create_dir_all(&att).unwrap();
    std::fs::write(att.join("cafe0000-0000-4000-8000-000000000007"), "log").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_losos-git"))
        .args(["import-forgejo", "--state-dir"])
        .arg(dir.join("state"))
        .arg("--from")
        .arg(&from)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = json(&std::fs::read_to_string(from.join("import.json")).unwrap());
    assert_eq!(report["complete"], true);
    assert_eq!(report["imported"]["issue"], 1);

    let s = start(&dir, &[]);
    let auth = format!("token {token}");
    let (status, body) = http(s.port, "GET", "/api/v1/user", Some(&auth), None);
    assert_eq!(status, 200, "Forgejo's token still opens the box: {body}");
    assert_eq!(json(&body)["login"], "matus");
    let (status, body) = http(
        s.port,
        "GET",
        "/api/v1/repos/matus/notes/issues/12",
        Some(&auth),
        None,
    );
    assert_eq!(status, 200, "{body}");
    let issue = json(&body);
    assert_eq!(issue["title"], "Fix the backup script");
    assert_eq!(issue["user"]["login"], "eva");
    assert_eq!(issue["state"], "open");
    let (_, body) = http(
        s.port,
        "GET",
        "/api/v1/repos/matus/notes/issues/12/comments",
        Some(&auth),
        None,
    );
    assert_eq!(json(&body)[0]["body"], "On it.");
    let (status, body) = http(
        s.port,
        "GET",
        "/api/v1/attachments/cafe0000-0000-4000-8000-000000000007",
        Some(&auth),
        None,
    );
    assert_eq!((status, body.as_str()), (200, "log"));
    // Nobody else sees a private repository's files.
    let (status, _) = http(
        s.port,
        "GET",
        "/api/v1/attachments/cafe0000-0000-4000-8000-000000000007",
        None,
        None,
    );
    assert_eq!(status, 404);
}

/// A second person on the box, with a token, made the way lososd does.
fn person_token(s: &Server, login: &str) -> String {
    let admin = format!("token {}", s.admin);
    let (status, body) = http(
        s.port,
        "PUT",
        &format!("/api/v1/admin/users/{login}"),
        Some(&admin),
        Some(&format!(
            r#"{{"full_name":"{login}","email":"{login}@example.org"}}"#
        )),
    );
    assert_eq!(status, 200, "{body}");
    let (status, body) = http(
        s.port,
        "POST",
        &format!("/api/v1/admin/users/{login}/tokens"),
        Some(&admin),
        Some(r#"{"name":"laptop","scopes":["all"],"replace":true}"#),
    );
    assert_eq!(status, 201, "{body}");
    json(&body)["token"].as_str().unwrap().to_string()
}

#[test]
fn a_pull_request_from_a_marked_worktree_is_reviewed_and_merged() {
    let dir = scratch("review");
    let s = start(&dir, &[]);
    let owner = format!("token {}", owner_token(&s, r#""all""#, "all"));
    let jana_token = person_token(&s, "jana");
    let jana = format!("token {jana_token}");
    let api = |method: &str, path: &str, auth: &str, body: Option<&str>| {
        let (status, text) = http(
            s.port,
            method,
            &format!("/api/v1/repos/notshared/app{path}"),
            Some(auth),
            body,
        );
        let doc = if text.trim().is_empty() {
            serde_json::Value::Null
        } else {
            json(&text)
        };
        (status, doc)
    };

    let (status, body) = http(
        s.port,
        "POST",
        "/api/v1/user/repos",
        Some(&owner),
        Some(r#"{"name":"app","private":true,"auto_init":true}"#),
    );
    assert_eq!(status, 201, "{body}");
    let (status, _) = api(
        "PUT",
        "/collaborators/jana",
        &owner,
        Some(r#"{"permission":"write"}"#),
    );
    assert!(status == 204 || status == 200, "{status}");

    // Jana works in a worktree her pushes name.
    let url = format!(
        "http://jana:{jana_token}@127.0.0.1:{}/notshared/app.git",
        s.port
    );
    ok(git(&dir, &["clone", "-q", &url, "jana"]));
    let main = dir.join("jana");
    ok(git(
        &main,
        &["worktree", "add", "-q", "-b", "fix-backup", "../wt"],
    ));
    let wt = dir.join("wt");
    ok(git(&main, &["config", "extensions.worktreeConfig", "true"]));
    ok(git(
        &wt,
        &[
            "config",
            "--worktree",
            "push.pushOption",
            "worktree=backup fix",
        ],
    ));
    ok(git(
        &wt,
        &[
            "config",
            "--worktree",
            "--add",
            "push.pushOption",
            "computer=laptop",
        ],
    ));
    std::fs::write(
        wt.join("README.md"),
        "# app\n\nBack up every night.\nKeep a week.\n",
    )
    .unwrap();
    ok(git(&wt, &["commit", "-qam", "Back up every night"]));
    ok(git(&wt, &["push", "-q", "-u", "origin", "fix-backup"]));

    let (status, list) = api("GET", "/worktrees", &owner, None);
    assert_eq!(status, 200);
    assert_eq!(list[0]["name"], "backup fix");
    assert_eq!(list[0]["computer"], "laptop");
    assert_eq!(list[0]["branch"], "fix-backup");
    assert_eq!(list[0]["owner"]["login"], "jana");

    // She opens a pull request; the same one twice is refused.
    let new_pr = r#"{"title":"Back up every night","body":"Fixes the backup.","head":"fix-backup","base":"main"}"#;
    let (status, pr) = api("POST", "/pulls", &jana, Some(new_pr));
    assert_eq!(status, 201, "{pr}");
    let n = pr["number"].as_i64().unwrap();
    assert_eq!(pr["mergeable"], true);
    assert_eq!(pr["ahead_by"], 1);
    assert_eq!(pr["worktree"]["name"], "backup fix");
    let (status, _) = api("POST", "/pulls", &jana, Some(new_pr));
    assert_eq!(status, 409);

    let (status, files) = api("GET", &format!("/pulls/{n}/files"), &owner, None);
    assert_eq!(status, 200);
    assert_eq!(files[0]["filename"], "README.md");
    assert_eq!(files[0]["additions"], 3);
    assert!(files[0]["patch"].as_str().unwrap().starts_with("@@ "));
    let (_, commits) = api("GET", &format!("/pulls/{n}/commits"), &owner, None);
    assert_eq!(commits[0]["subject"], "Back up every night");

    // Jana asks the owner; she cannot approve her own.
    let (status, doc) = api(
        "POST",
        &format!("/pulls/{n}/requested_reviewers"),
        &jana,
        Some(r#"{"reviewers":["notshared"]}"#),
    );
    assert_eq!(status, 200, "{doc}");
    assert_eq!(doc["reviewers"][0]["state"], "requested");
    let (status, _) = api(
        "POST",
        &format!("/pulls/{n}/reviews"),
        &jana,
        Some(r#"{"event":"APPROVE"}"#),
    );
    assert_eq!(status, 422);

    // The owner asks for a change on line 4, which is in the diff, and not
    // on line 40, which is not.
    let (status, _) = api(
        "POST",
        &format!("/pulls/{n}/reviews"),
        &owner,
        Some(
            r#"{"event":"REQUEST_CHANGES","comments":[{"path":"README.md","line":40,"body":"?"}]}"#,
        ),
    );
    assert_eq!(status, 422);
    let (status, review) = api(
        "POST",
        &format!("/pulls/{n}/reviews"),
        &owner,
        Some(
            r#"{"event":"REQUEST_CHANGES","body":"Almost.","comments":[{"path":"README.md","line":4,"body":"Keep two weeks."}]}"#,
        ),
    );
    assert_eq!(status, 201, "{review}");
    let (_, doc) = api("GET", &format!("/pulls/{n}"), &owner, None);
    assert_eq!(doc["reviewers"][0]["login"], "notshared");
    assert_eq!(doc["reviewers"][0]["state"], "changes_requested");

    let (_, lines) = api("GET", &format!("/pulls/{n}/comments"), &jana, None);
    let first = lines[0]["id"].as_i64().unwrap();
    assert_eq!(lines[0]["line"], 4);
    assert_eq!(lines[0]["side"], "new");
    let (status, reply) = api(
        "POST",
        &format!("/pulls/{n}/comments"),
        &jana,
        Some(&format!(r#"{{"in_reply_to":{first},"body":"Done."}}"#)),
    );
    assert_eq!(status, 201, "{reply}");
    assert_eq!(reply["in_reply_to"], first);
    let (status, resolved) = api(
        "PATCH",
        &format!("/pulls/comments/{first}"),
        &jana,
        Some(r#"{"resolved":true}"#),
    );
    assert_eq!(status, 200, "{resolved}");
    assert_eq!(resolved["resolved"], true);
    assert_eq!(resolved["resolved_by"]["login"], "jana");

    // She pushes the fix; a review of the old head is refused, of the new
    // one taken.
    let old_head = doc["head"]["sha"].as_str().unwrap().to_string();
    std::fs::write(
        wt.join("README.md"),
        "# app\n\nBack up every night.\nKeep two weeks.\n",
    )
    .unwrap();
    ok(git(&wt, &["commit", "-qam", "Keep two weeks"]));
    ok(git(&wt, &["push", "-q"]));
    let (status, _) = api(
        "POST",
        &format!("/pulls/{n}/reviews"),
        &owner,
        Some(&format!(
            r#"{{"event":"APPROVE","commit_id":"{old_head}"}}"#
        )),
    );
    assert_eq!(status, 409);
    let (_, doc) = api("GET", &format!("/pulls/{n}"), &owner, None);
    let new_head = doc["head"]["sha"].as_str().unwrap().to_string();
    assert_ne!(new_head, old_head);
    let (status, _) = api(
        "POST",
        &format!("/pulls/{n}/reviews"),
        &owner,
        Some(&format!(
            r#"{{"event":"APPROVE","body":"Thanks.","commit_id":"{new_head}"}}"#
        )),
    );
    assert_eq!(status, 201);
    let (_, doc) = api("GET", &format!("/pulls/{n}"), &owner, None);
    assert_eq!(doc["reviewers"][0]["state"], "approved");
    let (_, list) = api("GET", "/worktrees", &owner, None);
    assert_eq!(list[0]["head"], new_head.as_str());

    // A conversation comment, then the merge: a merge commit on main with
    // both parents, the branch gone.
    let (status, _) = api(
        "POST",
        &format!("/issues/{n}/comments"),
        &owner,
        Some(r#"{"body":"Merging."}"#),
    );
    assert_eq!(status, 201);
    let (status, merged) = api(
        "POST",
        &format!("/pulls/{n}/merge"),
        &owner,
        Some(&format!(
            r#"{{"style":"merge","head_commit_id":"{new_head}","delete_branch":true}}"#
        )),
    );
    assert_eq!(status, 200, "{merged}");
    assert_eq!(merged["merged"], true);
    assert_eq!(merged["state"], "closed");
    assert_eq!(merged["merged_by"]["login"], "notshared");
    let bare = s.state.join("repositories/notshared/app.git");
    let parents = ok(git(&bare, &["log", "-1", "--format=%P", "main"]));
    assert_eq!(parents.split_whitespace().count(), 2);
    assert!(parents.contains(&new_head));
    assert!(!git(
        &bare,
        &["rev-parse", "--verify", "-q", "refs/heads/fix-backup"]
    )
    .status
    .success());
    // The merged pull request still shows its diff.
    let (_, files) = api("GET", &format!("/pulls/{n}/files"), &owner, None);
    assert_eq!(files[0]["filename"], "README.md");
    let (status, _) = api("POST", &format!("/pulls/{n}/merge"), &owner, Some("{}"));
    assert_eq!(status, 409);

    // Two branches that change the same line: the second cannot merge.
    ok(git(&main, &["pull", "-q", "origin", "main"]));
    for (branch, text) in [("left", "Keep a month.\n"), ("right", "Keep a year.\n")] {
        ok(git(&main, &["checkout", "-q", "-b", branch, "main"]));
        let readme = std::fs::read_to_string(main.join("README.md")).unwrap();
        std::fs::write(
            main.join("README.md"),
            readme.replace("Keep two weeks.\n", text),
        )
        .unwrap();
        ok(git(&main, &["commit", "-qam", text.trim()]));
        ok(git(&main, &["push", "-q", "origin", branch]));
    }
    let (_, left) = api(
        "POST",
        "/pulls",
        &jana,
        Some(r#"{"title":"A month","head":"left","base":"main"}"#),
    );
    let (_, right) = api(
        "POST",
        "/pulls",
        &jana,
        Some(r#"{"title":"A year","head":"right","base":"main"}"#),
    );
    let (status, _) = api(
        "POST",
        &format!("/pulls/{}/merge", left["number"]),
        &owner,
        Some(r#"{"style":"squash"}"#),
    );
    assert_eq!(status, 200);
    let squashed = ok(git(&bare, &["log", "-1", "--format=%P%n%s", "main"]));
    assert_eq!(
        squashed.lines().next().unwrap().split_whitespace().count(),
        1
    );
    assert!(squashed.contains("A month (#"));
    let (_, doc) = api("GET", &format!("/pulls/{}", right["number"]), &owner, None);
    assert_eq!(doc["mergeable"], false);
    assert_eq!(doc["conflicts"][0], "README.md");
    let (status, _) = api(
        "POST",
        &format!("/pulls/{}/merge", right["number"]),
        &owner,
        Some("{}"),
    );
    assert_eq!(status, 409);
    let (status, _) = api(
        "POST",
        &format!("/pulls/{}/merge", right["number"]),
        &owner,
        Some(r#"{"style":"fast-forward"}"#),
    );
    assert_eq!(status, 422);

    // A merge made on a computer and pushed marks its pull request merged.
    ok(git(&main, &["checkout", "-q", "-b", "small", "main"]));
    std::fs::write(main.join("NOTES.md"), "notes\n").unwrap();
    ok(git(&main, &["add", "NOTES.md"]));
    ok(git(&main, &["commit", "-qm", "Notes"]));
    ok(git(&main, &["push", "-q", "origin", "small"]));
    let (_, small) = api(
        "POST",
        "/pulls",
        &jana,
        Some(r#"{"title":"Notes","head":"small","base":"main"}"#),
    );
    ok(git(&main, &["fetch", "-q", "origin"]));
    ok(git(&main, &["checkout", "-q", "-B", "main", "origin/main"]));
    ok(git(&main, &["merge", "-q", "--no-edit", "small"]));
    ok(git(&main, &["push", "-q", "origin", "main"]));
    let (_, doc) = api("GET", &format!("/pulls/{}", small["number"]), &owner, None);
    assert_eq!(doc["merged"], true);

    // The code as the pages read it.
    let (status, tree) = api("GET", "/contents", &owner, None);
    assert_eq!(status, 200);
    let names: Vec<&str> = tree["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["NOTES.md", "README.md"]);
    let (_, readme) = api("GET", "/readme", &owner, None);
    assert!(readme["text"].as_str().unwrap().contains("Keep a month."));
    let (status, _) = api("GET", "/contents/missing.txt", &owner, None);
    assert_eq!(status, 404);
    let (_, branches) = api("GET", "/branches", &owner, None);
    assert_eq!(branches[0]["name"], "main");
    assert_eq!(branches[0]["default"], true);
    let (_, log) = api("GET", "/commits?limit=2", &owner, None);
    assert_eq!(log.as_array().unwrap().len(), 2);
}
