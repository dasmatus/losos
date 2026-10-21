//! LosOS Reviewer against a real `losos-git` and real git, with a stand-in
//! for Ollama that answers the way Qwen2.5-Coder 1.5B answered the same
//! kind of question: one true finding, one about a line that is not there,
//! a sentence per file.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Server {
    child: Child,
    port: u16,
    admin: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start(dir: &Path, model_url: Option<&str>) -> Server {
    let port = free_port();
    let state = dir.join("state");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_losos-git"));
    cmd.args(["serve", "--state-dir"])
        .arg(&state)
        .args(["--listen", &format!("127.0.0.1:{port}")])
        .stderr(Stdio::null());
    cmd.env_remove("LOSOS_REVIEWER_URL");
    if let Some(url) = model_url {
        cmd.env("LOSOS_REVIEWER_URL", url)
            .env("LOSOS_REVIEWER_MODEL", "qwen2.5-coder:1.5b")
            .env("LOSOS_REVIEWER_CONTEXT", "12288");
    }
    let child = cmd.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "losos-git did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    let admin = std::fs::read_to_string(state.join("admin-token")).unwrap();
    Server {
        child,
        port,
        admin: admin.trim().to_string(),
    }
}

/// Ollama's two routes, answering by which question was asked. Every chat
/// request is kept for the test to read.
fn stand_in(asked: Arc<Mutex<Vec<serde_json::Value>>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let asked = asked.clone();
            std::thread::spawn(move || answer(stream, &asked));
        }
    });
    port
}

fn answer(mut stream: TcpStream, asked: &Mutex<Vec<serde_json::Value>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    let mut length = 0;
    loop {
        let mut h = String::new();
        reader.read_line(&mut h).unwrap();
        if h.trim().is_empty() {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
            length = v.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let reply = if first.starts_with("GET /api/tags") {
        serde_json::json!({ "models": [{ "name": "qwen2.5-coder:1.5b" }] })
    } else {
        let req: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let system = req["messages"][0]["content"].as_str().unwrap().to_string();
        let user = req["messages"][1]["content"].as_str().unwrap().to_string();
        asked.lock().unwrap().push(req);
        let text = if system.starts_with("You are a careful code reviewer") {
            if user.contains("backup.py") && user.contains("shutil.rmtree(target)") {
                // A true finding on line 9, and one on a line the diff
                // does not have.
                "L9: `shutil.rmtree(target)` removes the folder even when the copy failed; check the result of the copy first.\n\nL40: `return None` is never reached."
            } else {
                "LGTM"
            }
        } else if system.starts_with("You describe one file") {
            if user.contains("backup.py") {
                "Adds a backup that copies the data folder and keeps the newest copies."
            } else {
                "Says how the backup works."
            }
        } else {
            "Adds a nightly backup of the data folder and documents it."
        };
        serde_json::json!({
            "message": { "role": "assistant", "content": text },
            "done": true, "prompt_eval_count": 300, "eval_count": 20,
        })
    };
    let data = reply.to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{data}",
        data.len()
    );
}

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

fn json(s: &str) -> serde_json::Value {
    if s.trim().is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::from_str(s).unwrap_or_else(|_| panic!("not JSON: {s}"))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
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
        .unwrap();
    assert!(
        out.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("losos-reviewer-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An account and a token, made the way lososd makes them.
fn account(s: &Server, login: &str, admin: bool) -> String {
    let a = format!("token {}", s.admin);
    let (status, body) = http(
        s.port,
        "PUT",
        &format!("/api/v1/admin/users/{login}"),
        Some(&a),
        Some(&format!(
            r#"{{"full_name":"{login}","email":"{login}@example.org","admin":{admin}}}"#
        )),
    );
    assert_eq!(status, 200, "{body}");
    let (status, body) = http(
        s.port,
        "POST",
        &format!("/api/v1/admin/users/{login}/tokens"),
        Some(&a),
        Some(r#"{"name":"laptop","scopes":["all"],"replace":true}"#),
    );
    assert_eq!(status, 201, "{body}");
    json(&body)["token"].as_str().unwrap().to_string()
}

const BACKUP: &str = "import shutil\n\n\ndef backup(source, target, keep=[]):\n    \"\"\"Copy source to target, keeping the newest copies.\"\"\"\n    keep.append(target)\n    copied = shutil.copytree(source, target + \".new\")\n    if copied:\n        shutil.rmtree(target)\n    return keep\n";

#[test]
fn a_repository_that_asks_for_it_gets_a_review_from_the_box() {
    let dir = scratch("review");
    let asked = Arc::new(Mutex::new(Vec::new()));
    let ollama = stand_in(asked.clone());
    let s = start(&dir, Some(&format!("http://127.0.0.1:{ollama}")));
    let owner = format!("token {}", account(&s, "notshared", true));
    let jana_token = account(&s, "jana", false);
    let jana = format!("token {jana_token}");
    let api = |method: &str, path: &str, auth: &str, body: Option<&str>| {
        let (status, text) = http(
            s.port,
            method,
            &format!("/api/v1/repos/notshared/app{path}"),
            Some(auth),
            body,
        );
        (status, json(&text))
    };
    let (status, body) = http(
        s.port,
        "POST",
        "/api/v1/user/repos",
        Some(&owner),
        Some(r#"{"name":"app","private":true,"auto_init":true}"#),
    );
    assert_eq!(status, 201, "{body}");
    assert_eq!(
        json(&body)["ai_review"],
        false,
        "off until someone turns it on"
    );
    let (status, _) = api(
        "PUT",
        "/collaborators/jana",
        &owner,
        Some(r#"{"permission":"write"}"#),
    );
    assert!(status == 204 || status == 200, "{status}");

    // Only the repository's administrators turn it on.
    let (status, _) = api("PUT", "/ai-review", &jana, Some(r#"{"enabled":true}"#));
    assert_eq!(status, 403);
    let (status, setting) = api("PUT", "/ai-review", &owner, Some(r#"{"enabled":true}"#));
    assert_eq!(status, 200, "{setting}");
    assert_eq!(setting["enabled"], true);
    assert_eq!(setting["available"], true);
    assert_eq!(setting["model"], "qwen2.5-coder:1.5b");
    let (_, setting) = api("GET", "/ai-review", &jana, None);
    assert_eq!(setting["canChange"], false);

    // Jana pushes a branch and opens a pull request.
    let url = format!(
        "http://jana:{jana_token}@127.0.0.1:{}/notshared/app.git",
        s.port
    );
    git(&dir, &["clone", "-q", &url, "work"]);
    let work = dir.join("work");
    git(&work, &["checkout", "-q", "-b", "backup"]);
    std::fs::write(work.join("backup.py"), BACKUP).unwrap();
    std::fs::write(work.join("BACKUP.md"), "Runs every night.\n").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-qm", "Back up the data folder"]);
    git(&work, &["push", "-q", "-u", "origin", "backup"]);
    let (status, pr) = api(
        "POST",
        "/pulls",
        &jana,
        Some(r#"{"title":"Back up the data folder","head":"backup","base":"main"}"#),
    );
    assert_eq!(status, 201, "{pr}");
    let n = pr["number"].as_i64().unwrap();

    let wait_done = |reviews: usize| {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let (_, doc) = api("GET", &format!("/pulls/{n}/reviews"), &jana, None);
            let by_bot = doc
                .as_array()
                .map(|all| {
                    all.iter()
                        .filter(|r| r["user"]["login"] == "losos-reviewer")
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if by_bot.len() >= reviews {
                return by_bot;
            }
            assert!(Instant::now() < deadline, "no review came: {doc}");
            std::thread::sleep(Duration::from_millis(100));
        }
    };

    let reviews = wait_done(1);
    let review = &reviews[0];
    assert_eq!(review["state"], "commented", "it never approves");
    let body = review["body"].as_str().unwrap();
    assert!(body.starts_with("## Pull request overview"), "{body}");
    assert!(
        body.contains("Adds a nightly backup of the data folder and documents it."),
        "{body}"
    );
    assert!(
        body.contains("looked at 2 of 2 changed files and left 2 comments"),
        "{body}"
    );
    assert!(
        body.contains("| `backup.py` | Adds a backup that copies"),
        "{body}"
    );
    assert!(body.contains("Reviewed on this box by qwen2.5-coder:1.5b"));

    // The box's check on line 4 and the model's finding on line 9; the
    // model's line 40 is not in the diff and is gone.
    let (_, lines) = api("GET", &format!("/pulls/{n}/comments"), &jana, None);
    let lines = lines.as_array().unwrap().clone();
    let at: Vec<(String, i64)> = lines
        .iter()
        .map(|c| {
            (
                c["path"].as_str().unwrap().to_string(),
                c["line"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        at,
        vec![("backup.py".to_string(), 4), ("backup.py".to_string(), 9)]
    );
    assert!(lines.iter().all(|c| c["user"]["login"] == "losos-reviewer"));
    assert!(lines[0]["body"]
        .as_str()
        .unwrap()
        .contains("Default to `None`"));
    assert!(lines[1]["body"]
        .as_str()
        .unwrap()
        .contains("check the result of the copy"));

    // Progress and the result are on the head commit, for the Checks box.
    let head = pr["head"]["sha"].as_str().unwrap().to_string();
    let (_, statuses) = api("GET", &format!("/commits/{head}/statuses"), &jana, None);
    let mine = statuses["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["context"] == "LosOS Reviewer")
        .cloned()
        .unwrap();
    assert_eq!(mine["state"], "success");
    assert_eq!(mine["description"], "Left 2 comments");
    assert_eq!(
        mine["targetUrl"],
        format!("/git/notshared/app/pulls/{n}/files")
    );
    let (_, progress) = api("GET", &format!("/pulls/{n}/ai-review"), &jana, None);
    assert_eq!(progress["state"], "done");
    assert_eq!(progress["filesTotal"], 2);
    assert_eq!(progress["comments"], 2);

    // It was among the reviewers while it worked; the review answers that.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let (_, doc) = api("GET", &format!("/pulls/{n}"), &jana, None);
        let bot = doc["reviewers"]
            .as_array()
            .and_then(|all| all.iter().find(|r| r["login"] == "losos-reviewer"))
            .cloned()
            .unwrap_or_default();
        if bot["requested"] == false {
            assert_eq!(bot["state"], "commented", "{doc}");
            break;
        }
        assert!(Instant::now() < deadline, "still asked: {doc}");
        std::thread::sleep(Duration::from_millis(50));
    }

    // People answer and resolve its comments like anyone's.
    let first = lines[0]["id"].as_i64().unwrap();
    let (status, _) = api(
        "POST",
        &format!("/pulls/{n}/comments"),
        &jana,
        Some(&format!(
            r#"{{"in_reply_to":{first},"body":"Right, fixed."}}"#
        )),
    );
    assert_eq!(status, 201);
    let (status, resolved) = api(
        "PATCH",
        &format!("/pulls/comments/{first}"),
        &jana,
        Some(r#"{"resolved":true}"#),
    );
    assert_eq!(status, 200, "{resolved}");

    // Anyone asks it again, and it reviews the whole pull request again.
    let (status, doc) = api(
        "POST",
        &format!("/pulls/{n}/requested_reviewers"),
        &jana,
        Some(r#"{"reviewers":["losos-reviewer"]}"#),
    );
    assert_eq!(status, 200, "{doc}");
    let reviews = wait_done(2);
    assert!(!reviews[1]["body"]
        .as_str()
        .unwrap()
        .contains("since the last review"));

    // A push is reviewed from the last reviewed head.
    std::fs::write(work.join("BACKUP.md"), "Runs every night at two.\n").unwrap();
    git(&work, &["commit", "-qam", "Say when"]);
    git(&work, &["push", "-q"]);
    let reviews = wait_done(3);
    let body = reviews[2]["body"].as_str().unwrap();
    assert!(
        body.contains("These are the commits pushed since the last review."),
        "{body}"
    );
    assert!(body.contains("looked at 1 of 1 changed file"), "{body}");

    // Every question kept to the builder's context size and to loopback.
    let all = asked.lock().unwrap().clone();
    assert!(all.iter().all(|r| r["options"]["num_ctx"] == 12288));
    let asked_before = all.len();

    // Turned off, a push gets no review.
    let (status, _) = api("PUT", "/ai-review", &owner, Some(r#"{"enabled":false}"#));
    assert_eq!(status, 200);
    std::fs::write(work.join("BACKUP.md"), "Runs every night at three.\n").unwrap();
    git(&work, &["commit", "-qam", "Later"]);
    git(&work, &["push", "-q"]);
    std::thread::sleep(Duration::from_secs(2));
    let (_, progress) = api("GET", &format!("/pulls/{n}/ai-review"), &jana, None);
    assert_ne!(progress["state"], "queued");
    assert_ne!(progress["state"], "reviewing");
    assert_eq!(asked.lock().unwrap().len(), asked_before);
    let (status, _) = api(
        "POST",
        &format!("/pulls/{n}/requested_reviewers"),
        &jana,
        Some(r#"{"reviewers":["losos-reviewer"]}"#),
    );
    assert_eq!(status, 422, "it cannot be asked while it is off");
}

#[test]
fn without_a_code_model_the_setting_cannot_be_turned_on() {
    let dir = scratch("nomodel");
    let s = start(&dir, None);
    let owner = format!("token {}", account(&s, "notshared", true));
    let (status, _) = http(
        s.port,
        "POST",
        "/api/v1/user/repos",
        Some(&owner),
        Some(r#"{"name":"app","private":true,"auto_init":true}"#),
    );
    assert_eq!(status, 201);
    let (status, body) = http(
        s.port,
        "GET",
        "/api/v1/repos/notshared/app/ai-review",
        Some(&owner),
        None,
    );
    assert_eq!(status, 200);
    assert_eq!(json(&body)["available"], false);
    let (status, body) = http(
        s.port,
        "PUT",
        "/api/v1/repos/notshared/app/ai-review",
        Some(&owner),
        Some(r#"{"enabled":true}"#),
    );
    assert_eq!(status, 422, "{body}");
    assert!(json(&body)["message"]
        .as_str()
        .unwrap()
        .contains("Build with AI"));
}
