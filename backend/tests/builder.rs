//! The builder end to end, widgets and apps: lososd, `curl` and a stand-in for Ollama on
//! loopback that streams a recorded answer the way Ollama does.
//!
//! The stand-in checks what lososd sends (the model, the context window, the
//! prompt and the owner's request) and answers with chunks of NDJSON. It
//! cannot say whether a real model writes a working widget; that is measured
//! by hand against the real one (the handbook's widget builder page says
//! how well it does).

use assert_fs::TempDir;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BOOT_TIMEOUT: Duration = Duration::from_secs(20);

const ANSWER: &str = "```html\n<p id=\"t\">…</p>\n<script>\nsetInterval(function () { document.getElementById(\"t\").textContent = new Date().toLocaleTimeString(losos.lang); }, 1000);\n</script>\n```\nSummary: A clock.";

/// A clock whose script looks up an element the markup does not have.
const MISTAKE: &str = "```html\n<p id=\"t\">…</p>\n<script>\ndocument.getElementById(\"clock\").textContent = new Date().toLocaleTimeString(losos.lang);\n</script>\n```\nSummary: A clock.";

/// An app, the way the model answers.
const APP: &str = "```jsx\nfunction App() {\n  const items = useCollection(\"items\");\n  return (\n    <Page title=\"Nákup\">\n      {items.items.map(i => <Text key={i.id}>{i.name}</Text>)}\n    </Page>\n  );\n}\n```\nName: Nákupný zoznam\nSummary: Zoznam nákupov.";

/// An app with a component the frame does not have.
const APP_MISTAKE: &str = "```jsx\nfunction App() {\n  return <Page><DataGrid /></Page>;\n}\n```\nName: Grid\nSummary: A grid.";

/// What the stand-in does with a chat: answer at once, or trickle the answer
/// out slowly enough for a test to stop it halfway.
#[derive(Clone, Copy)]
enum Pace {
    Quick,
    Slow,
    /// Refuse the way Ollama does when the box has too little free memory.
    NoMemory,
}

/// A stand-in for Ollama: `GET /api/tags` lists `qwen2.5-coder:1.5b`, `POST
/// /api/chat` streams an answer in pieces, [`ANSWER`] unless told others.
/// Every chat body is kept.
struct Ollama {
    port: u16,
    chats: Arc<Mutex<Vec<Value>>>,
}

impl Ollama {
    fn start(pace: Pace) -> Ollama {
        Ollama::answering(pace, vec![ANSWER.to_string()])
    }

    /// Answer the first chat with `answers[0]`, the second with
    /// `answers[1]`, and so on; the last one from then on.
    fn answering(pace: Pace, answers: Vec<String>) -> Ollama {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let chats = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&chats);
        let answers = Arc::new(answers);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = Arc::clone(&seen);
                let answers = Arc::clone(&answers);
                std::thread::spawn(move || serve(stream, pace, &seen, &answers));
            }
        });
        Ollama { port, chats }
    }
}

fn serve(mut stream: TcpStream, pace: Pace, seen: &Mutex<Vec<Value>>, answers: &[String]) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = v.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    if first.starts_with("GET /api/tags") {
        let doc = json!({ "models": [{ "name": "qwen2.5-coder:1.5b" }] }).to_string();
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{doc}",
            doc.len()
        );
        return;
    }
    let answer = {
        let mut seen = seen.lock().unwrap();
        seen.push(serde_json::from_slice(&body).unwrap());
        answers[(seen.len() - 1).min(answers.len() - 1)].clone()
    };
    if let Pace::NoMemory = pace {
        let doc = json!({ "error": "model requires more system memory (4.1 GiB) than is available (2.0 GiB)" })
            .to_string();
        let _ = write!(
            stream,
            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{doc}",
            doc.len()
        );
        return;
    }
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n"
    );
    for piece in answer.split_inclusive('\n') {
        let chunk = json!({ "message": { "role": "assistant", "content": piece }, "done": false });
        if writeln!(stream, "{chunk}").is_err() {
            return;
        }
        let _ = stream.flush();
        if let Pace::Slow = pace {
            std::thread::sleep(Duration::from_millis(400));
        }
    }
    let end = json!({ "message": { "role": "assistant", "content": "" }, "done": true,
                      "done_reason": "stop", "prompt_eval_count": 2000, "eval_count": 120 });
    let _ = writeln!(stream, "{end}");
}

struct Daemon {
    child: Child,
    port: u16,
    _dir: TempDir,
    token: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Daemon {
    fn start(ollama: Option<&Ollama>) -> Daemon {
        // No page tries the apps here, unless a test says it will.
        Daemon::start_trying(ollama, 1)
    }

    /// A box whose builds wait `try_secs` for the page to try an app.
    fn start_trying(ollama: Option<&Ollama>, try_secs: u64) -> Daemon {
        let dir = TempDir::new().unwrap();
        let token_file = dir.path().join("token");
        let port = TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lososd"));
        cmd.env("LOSOS_NO_DBUS", "1")
            .env("LOSOS_STATE_DIR", dir.path().join("state"))
            .env("LOSOS_OVERRIDES", dir.path().join("overrides.nix"))
            .env("LOSOS_ADMIN_TOKEN_FILE", &token_file)
            .env("LOSOS_ADMIN_PORT", port.to_string())
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(o) = ollama {
            cmd.env("LOSOS_BUILDER_URL", format!("http://127.0.0.1:{}", o.port))
                .env("LOSOS_BUILDER_MODEL", "qwen2.5-coder:1.5b")
                .env("LOSOS_BUILDER_CONTEXT", "12288")
                .env("LOSOS_BUILDER_TRY_SECS", try_secs.to_string());
        }
        let child = cmd.spawn().unwrap();
        let deadline = Instant::now() + BOOT_TIMEOUT;
        let token = loop {
            if let Ok(token) = std::fs::read_to_string(&token_file) {
                if !token.trim().is_empty() {
                    break token.trim().to_string();
                }
            }
            assert!(Instant::now() < deadline, "lososd wrote no token");
            std::thread::sleep(Duration::from_millis(50));
        };
        let daemon = Daemon {
            child,
            port,
            _dir: dir,
            token,
        };
        while daemon.try_call("GET", "/api/health", None).is_none() {
            assert!(Instant::now() < deadline, "lososd did not come up");
            std::thread::sleep(Duration::from_millis(50));
        }
        daemon
    }

    fn try_call(&self, method: &str, path: &str, body: Option<&Value>) -> Option<(u16, Value)> {
        let body = body.map(Value::to_string).unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            self.token,
            body.len()
        );
        let mut socket = TcpStream::connect(("127.0.0.1", self.port)).ok()?;
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        socket.write_all(request.as_bytes()).ok()?;
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).ok()?;
        let text = String::from_utf8_lossy(&raw);
        let status = text.split_whitespace().nth(1)?.parse().ok()?;
        let body = text.split_once("\r\n\r\n").map_or("", |(_, b)| b);
        Some((status, serde_json::from_str(body).unwrap_or(Value::Null)))
    }

    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> (u16, Value) {
        self.try_call(method, path, body)
            .unwrap_or_else(|| panic!("no response to {method} {path}"))
    }

    /// Poll a build until it is no longer running.
    fn settle(&self, id: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let (status, build) = self.call("GET", &format!("/api/builder/builds/{id}"), None);
            assert_eq!(status, 200, "{build}");
            if build["status"] != "running" {
                return build;
            }
            assert!(Instant::now() < deadline, "the build never finished");
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[test]
fn a_box_without_a_model_says_the_builder_is_off() {
    let daemon = Daemon::start(None);
    assert_eq!(
        daemon.call("GET", "/api/builder", None),
        (200, json!({ "available": false, "reason": "off" }))
    );
    let (status, _) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "prompt": "a clock" })),
    );
    assert_eq!(status, 409);
}

#[test]
fn a_build_asks_the_model_on_the_box_and_hands_back_the_widget() {
    let ollama = Ollama::start(Pace::Quick);
    let daemon = Daemon::start(Some(&ollama));
    assert_eq!(
        daemon.call("GET", "/api/builder", None),
        (
            200,
            json!({ "available": true, "model": "qwen2.5-coder:1.5b", "builds": [] })
        )
    );

    let (status, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "prompt": "a clock", "lang": "sk" })),
    );
    assert_eq!(status, 200, "{started}");
    assert_eq!(started["status"], "running");
    let id = started["id"].as_str().unwrap().to_string();

    let done = daemon.settle(&id);
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["files"][0]["name"], "index.html");
    assert!(done["files"][0]["content"]
        .as_str()
        .unwrap()
        .starts_with("<p id=\"t\">"));
    assert_eq!(done["notes"], "A clock.");
    assert_eq!(done["kind"], "widget");
    assert_eq!(done["problems"], json!([]));
    assert_eq!(done["fault"], Value::Null);
    assert!(done["lines"].as_u64().unwrap() >= 5);

    let chats = ollama.chats.lock().unwrap();
    assert_eq!(chats.len(), 1);
    let chat = &chats[0];
    assert_eq!(chat["model"], "qwen2.5-coder:1.5b");
    assert_eq!(chat["stream"], true);
    assert_eq!(chat["options"]["num_ctx"], 12288);
    let messages = chat["messages"].as_array().unwrap();
    assert_eq!(
        messages[..7],
        losos_ctl::builder::prompt_messages(losos_ctl::builder::WIDGET_PROMPT)[..]
    );
    let request = messages[7]["content"].as_str().unwrap();
    assert!(request.starts_with("a clock\n"), "{request}");
    assert!(
        request.ends_with("Write the summary in Slovak."),
        "{request}"
    );

    // The list shows it, without its files.
    let (_, view) = daemon.call("GET", "/api/builder", None);
    assert_eq!(view["builds"][0]["id"], id.as_str());
    assert_eq!(view["builds"][0]["has_source"], true);
    assert!(view["builds"][0].get("files").is_none());
}

#[test]
fn a_running_build_can_be_stopped_and_one_runs_at_a_time() {
    let ollama = Ollama::start(Pace::Slow);
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "prompt": "a clock" })),
    );
    let id = started["id"].as_str().unwrap().to_string();
    let (status, second) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "prompt": "another" })),
    );
    assert_eq!(status, 409, "{second}");
    assert!(second["error"]
        .as_str()
        .unwrap()
        .contains("already being built"));

    let (status, stopped) = daemon.call("DELETE", &format!("/api/builder/builds/{id}"), None);
    assert_eq!(status, 200, "{stopped}");
    assert_eq!(stopped["status"], "stopped");
    let after = daemon.settle(&id);
    assert_eq!(after["status"], "stopped");
    assert_eq!(after["files"], Value::Null);

    // Stopped, the box takes the next build.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (status, _) = daemon.call(
            "POST",
            "/api/builder/builds",
            Some(&json!({ "prompt": "again" })),
        );
        if status == 200 {
            break;
        }
        assert!(Instant::now() < deadline, "the stopped build kept its slot");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        daemon.call("DELETE", "/api/builder/builds/b0-0", None).0,
        404
    );
}

#[test]
fn a_request_the_builder_will_not_act_on_is_a_400_with_a_sentence() {
    let ollama = Ollama::start(Pace::Quick);
    let daemon = Daemon::start(Some(&ollama));
    let too_big = "x".repeat(losos_ctl::builder::MAX_BASE_BYTES + 1);
    for body in [
        json!({}),
        json!({ "prompt": "  " }),
        json!({ "prompt": "x", "lang": "fr" }),
        json!({ "prompt": "x", "base": too_big }),
    ] {
        let (status, reply) = daemon.call("POST", "/api/builder/builds", Some(&body));
        assert_eq!(status, 400, "{body} -> {reply}");
        assert!(reply["error"].as_str().is_some_and(|e| !e.is_empty()));
    }
    assert!(ollama.chats.lock().unwrap().is_empty());
}

#[test]
fn a_box_short_of_memory_says_so() {
    let ollama = Ollama::start(Pace::NoMemory);
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "prompt": "a clock" })),
    );
    let failed = daemon.settle(started["id"].as_str().unwrap());
    assert_eq!(failed["status"], "failed", "{failed}");
    assert_eq!(failed["fault"], "memory");
}

#[test]
fn a_widget_with_a_mistake_the_box_can_see_goes_back_to_the_model_once() {
    let ollama = Ollama::answering(Pace::Quick, vec![MISTAKE.to_string(), ANSWER.to_string()]);
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "prompt": "a clock" })),
    );
    let done = daemon.settle(started["id"].as_str().unwrap());
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["fixing"], false);
    assert!(done["files"][0]["content"]
        .as_str()
        .unwrap()
        .contains("setInterval"));

    let chats = ollama.chats.lock().unwrap();
    assert_eq!(chats.len(), 2);
    let turn = chats[1]["messages"].as_array().unwrap();
    assert_eq!(turn.len(), 10);
    assert_eq!(turn[8]["role"], "assistant");
    assert_eq!(turn[8]["content"], MISTAKE);
    assert_eq!(turn[9]["role"], "user");
    let note = turn[9]["content"].as_str().unwrap();
    assert!(note.contains("getElementById(\"clock\")"), "{note}");
}

#[test]
fn a_model_that_repeats_itself_is_cut_off_and_asked_again() {
    let looping = format!(
        "```html\n<div>\n{}",
        "<span class=\"day\"></span>\n".repeat(200)
    );
    let ollama = Ollama::answering(Pace::Quick, vec![looping, ANSWER.to_string()]);
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "prompt": "uptime as squares" })),
    );
    let done = daemon.settle(started["id"].as_str().unwrap());
    assert_eq!(done["status"], "done", "{done}");
    assert!(done["files"][0]["content"]
        .as_str()
        .unwrap()
        .starts_with("<p id=\"t\">"));

    let chats = ollama.chats.lock().unwrap();
    assert_eq!(chats.len(), 2);
    let turn = chats[1]["messages"].as_array().unwrap();
    // The model is shown where it went round in circles, not 200 lines of it.
    let shown = turn[8]["content"].as_str().unwrap();
    assert!(shown.lines().count() < 30, "{shown}");
    assert!(turn[9]["content"].as_str().unwrap().contains("repeated"));
}

#[test]
fn a_second_answer_that_is_worse_than_the_first_is_dropped() {
    let ollama = Ollama::answering(
        Pace::Quick,
        vec![MISTAKE.to_string(), "Sorry, I cannot help.".to_string()],
    );
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "prompt": "a clock" })),
    );
    let done = daemon.settle(started["id"].as_str().unwrap());
    assert_eq!(done["status"], "done", "{done}");
    assert!(done["files"][0]["content"]
        .as_str()
        .unwrap()
        .contains("getElementById(\"clock\")"));
    assert_eq!(ollama.chats.lock().unwrap().len(), 2);
}

#[test]
fn an_app_is_built_compiled_saved_and_keeps_its_data() {
    let ollama = Ollama::answering(Pace::Quick, vec![APP.to_string()]);
    let daemon = Daemon::start(Some(&ollama));
    let (status, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "kind": "app", "prompt": "nákupný zoznam", "lang": "sk" })),
    );
    assert_eq!(status, 200, "{started}");
    assert_eq!(started["kind"], "app");
    let done = daemon.settle(started["id"].as_str().unwrap());
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["files"][0]["name"], "app.jsx");
    assert_eq!(done["name"], "Nákupný zoznam");
    assert_eq!(done["notes"], "Zoznam nákupov.");
    assert_eq!(done["problems"], json!([]));
    let code = done["code"].as_str().unwrap();
    assert!(code.contains("React.createElement(Page"), "{code}");
    {
        let chats = ollama.chats.lock().unwrap();
        assert_eq!(chats.len(), 1, "a clean app gets no second turn");
        let messages = chats[0]["messages"].as_array().unwrap();
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .starts_with("You write small apps"));
        assert!(messages[7]["content"]
            .as_str()
            .unwrap()
            .ends_with("Write the app's words and the summary in Slovak."));
    }

    // Saved as the owner keeps it, and listed without its code.
    let (status, saved) = daemon.call(
        "POST",
        "/api/user-apps",
        Some(&json!({ "name": done["name"], "summary": done["notes"], "source": done["files"][0]["content"] })),
    );
    assert_eq!(status, 200, "{saved}");
    let id = saved["app"]["id"].as_str().unwrap().to_string();
    assert_eq!(saved["app"]["code"], done["code"]);
    let (_, list) = daemon.call("GET", "/api/user-apps", None);
    assert_eq!(list["apps"][0]["name"], "Nákupný zoznam");
    assert!(list["apps"][0].get("source").is_none());

    // Its data: added, changed, read back, removed.
    let data = format!("/api/user-apps/{id}/data");
    let (status, added) = daemon.call(
        "POST",
        &data,
        Some(&json!({ "op": "add", "collection": "items", "value": { "name": "mlieko" } })),
    );
    assert_eq!(status, 200, "{added}");
    let item = added["item"]["id"].as_str().unwrap().to_string();
    let (status, _) = daemon.call(
        "POST",
        &data,
        Some(&json!({ "op": "update", "collection": "items", "id": item, "value": { "done": true } })),
    );
    assert_eq!(status, 200);
    let (_, kept) = daemon.call("GET", &data, None);
    assert_eq!(kept["collections"]["items"][0]["name"], "mlieko");
    assert_eq!(kept["collections"]["items"][0]["done"], true);
    let (status, refused) = daemon.call("POST", &data, Some(&json!({ "op": "drop" })));
    assert_eq!(status, 400, "{refused}");

    // An app the box does not have is a 404, and a deleted one takes its
    // data with it.
    assert_eq!(daemon.call("GET", "/api/user-apps/u99/data", None).0, 404);
    assert_eq!(daemon.call("GET", "/api/user-apps/u99", None).0, 404);
    let (_, gone) = daemon.call("DELETE", &format!("/api/user-apps/{id}"), None);
    assert_eq!(gone["deleted"], true);
    assert_eq!(daemon.call("GET", &data, None).0, 404);
}

#[test]
fn an_app_with_a_component_that_does_not_exist_goes_back_to_the_model_once() {
    let ollama = Ollama::answering(Pace::Quick, vec![APP_MISTAKE.to_string(), APP.to_string()]);
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "kind": "app", "prompt": "a grid" })),
    );
    let done = daemon.settle(started["id"].as_str().unwrap());
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["problems"], json!([]));
    let chats = ollama.chats.lock().unwrap();
    assert_eq!(chats.len(), 2);
    let note = chats[1]["messages"][9]["content"].as_str().unwrap();
    assert!(
        note.contains("there is no component called DataGrid"),
        "{note}"
    );
    assert!(
        note.ends_with("then the Name: and Summary: lines. It is still the app for: a grid"),
        "{note}"
    );
}

#[test]
fn a_fix_that_sends_an_example_back_goes_back_to_the_model_with_what_was_asked() {
    // The last example answer in the instructions, the disk, sent for a
    // shopping list: it compiles and checks clean, and is the wrong app.
    let turns = losos_ctl::builder::prompt_messages(losos_ctl::builder::APP_PROMPT);
    let disk = turns.last().unwrap()["content"]
        .as_str()
        .unwrap()
        .to_string();
    let ollama = Ollama::answering(Pace::Quick, vec![disk, APP.to_string()]);
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "kind": "app", "prompt": "a shopping list" })),
    );
    let done = daemon.settle(started["id"].as_str().unwrap());
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(
        done["name"], "Nákupný zoznam",
        "the shopping list is kept: {done}"
    );
    let chats = ollama.chats.lock().unwrap();
    assert_eq!(chats.len(), 2);
    let note = chats[1]["messages"][9]["content"].as_str().unwrap();
    assert!(
        note.contains(
            "This is the example from the instructions, the app for \"Show how full the disk is.\""
        ),
        "{note}"
    );
    assert!(
        note.ends_with("It is still the app for: a shopping list"),
        "{note}"
    );
}

/// The page's half of a try: wait for the build to ask for a try other than
/// `done`, then report.
fn report_try(daemon: &Daemon, id: &str, done: Option<&str>, report: &Value) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (_, build) = daemon.call("GET", &format!("/api/builder/builds/{id}"), None);
        if let Some(trial) = build["trial"]["id"].as_str().filter(|t| Some(*t) != done) {
            assert!(
                build["trial"]["code"]
                    .as_str()
                    .unwrap()
                    .contains("window.LosOS"),
                "{build}"
            );
            let mut body = report.clone();
            body["trial"] = json!(trial);
            let (status, taken) = daemon.call(
                "POST",
                &format!("/api/builder/builds/{id}/tried"),
                Some(&body),
            );
            assert_eq!(status, 200, "{taken}");
            return build;
        }
        assert_ne!(
            build["status"], "done",
            "the build finished without asking for a try: {build}"
        );
        assert!(Instant::now() < deadline, "the build never asked for a try");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn an_app_that_keeps_nothing_when_tried_goes_back_to_the_model_with_what_the_try_found() {
    let ollama = Ollama::answering(Pace::Quick, vec![APP.to_string(), APP.to_string()]);
    let daemon = Daemon::start_trying(Some(&ollama), 20);
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "kind": "app", "prompt": "a shopping list" })),
    );
    let id = started["id"].as_str().unwrap().to_string();

    // The first try: a field and an Add button, and nothing kept.
    let asking = report_try(
        &daemon,
        &id,
        None,
        &json!({ "shown": ["h1: Nákup", "input[text] \"Item\"", "button: Add"], "pressed": "Add",
                 "kept": [], "errors": [], "threwOnPress": [] }),
    );
    // The preview shows the app being tried.
    assert!(asking["code"]
        .as_str()
        .unwrap()
        .contains("React.createElement(Page"));
    // The second: it keeps the entry.
    let first = asking["trial"]["id"].as_str().unwrap();
    report_try(
        &daemon,
        &id,
        Some(first),
        &json!({ "shown": ["h1: Nákup", "input[text] \"Item\"", "button: Add"], "pressed": "Add",
                 "kept": ["items: 0 to 1"], "errors": [], "threwOnPress": [] }),
    );
    let done = daemon.settle(&id);
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["problems"], json!([]));
    assert_eq!(done["trial"], Value::Null);
    let kinds: Vec<&str> = done["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["write", "check", "try", "fix", "check", "try"]);
    assert!(done["steps"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["running"] == false));
    assert_eq!(done["steps"][2]["shown"][2], "button: Add");
    assert!(done["steps"][2]["problems"][0]
        .as_str()
        .unwrap()
        .contains("kept nothing"));

    let chats = ollama.chats.lock().unwrap();
    assert_eq!(chats.len(), 2);
    let note = chats[1]["messages"][9]["content"].as_str().unwrap();
    assert!(
        note.contains("\"Add\" was pressed, the app kept nothing"),
        "{note}"
    );

    // A report for a try the build is not waiting for is refused.
    let (status, _) = daemon.call(
        "POST",
        &format!("/api/builder/builds/{id}/tried"),
        Some(&json!({ "trial": "t0", "shown": [] })),
    );
    assert_eq!(status, 409);
}

#[test]
fn an_app_with_no_page_to_try_it_in_is_kept_untried_and_fixes_stop_at_three() {
    let ollama = Ollama::answering(Pace::Quick, vec![APP_MISTAKE.to_string()]);
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "kind": "app", "prompt": "a grid" })),
    );
    let done = daemon.settle(started["id"].as_str().unwrap());
    // The model never fixes it: one answer and three fixes, then the box
    // hands over what it has, with the mistake named.
    assert_eq!(ollama.chats.lock().unwrap().len(), 4);
    assert_eq!(done["status"], "done", "{done}");
    assert!(
        done["problems"][0].as_str().unwrap().contains("DataGrid"),
        "{done}"
    );

    let ollama = Ollama::answering(Pace::Quick, vec![APP.to_string()]);
    let daemon = Daemon::start(Some(&ollama));
    let (_, started) = daemon.call(
        "POST",
        "/api/builder/builds",
        Some(&json!({ "kind": "app", "prompt": "a list" })),
    );
    let done = daemon.settle(started["id"].as_str().unwrap());
    assert_eq!(done["status"], "done", "{done}");
    let last = done["steps"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["kind"], "try");
    assert_eq!(last["skipped"], true);
    assert_eq!(ollama.chats.lock().unwrap().len(), 1);
}

#[test]
fn source_that_does_not_compile_is_refused_and_the_check_says_why() {
    let daemon = Daemon::start(None);
    let (status, refused) = daemon.call(
        "POST",
        "/api/user-apps",
        Some(&json!({ "name": "Broken", "source": "function App() {\n  return <Page>;\n}" })),
    );
    assert_eq!(status, 400, "{refused}");
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("Line 3 has a syntax error"),
        "{refused}"
    );
    let (status, checked) = daemon.call(
        "POST",
        "/api/user-apps/check",
        Some(&json!({ "source": "function App() { return <Page><Nope /></Page>; }" })),
    );
    assert_eq!(status, 200);
    assert!(checked["code"].is_string());
    assert!(checked["problems"][0].as_str().unwrap().contains("Nope"));
}
