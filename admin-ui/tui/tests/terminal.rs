//! The built `losos` in a real pseudo-terminal (util-linux `script`), for
//! what a buffer cannot show: raw-mode keys on the login screen, the hidden
//! prompt, the full-screen view opening and closing, and the exit statuses.
//! LOSOS_BIN runs it against another build, such as the static binary CI
//! publishes, in place of the one cargo just made.

mod common;

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use common::{fake_bin, mock, owner, TOKEN};
use serde_json::json;

fn binary() -> String {
    std::env::var("LOSOS_BIN").unwrap_or_else(|_| env!("CARGO_BIN_EXE_losos").to_owned())
}

fn have_script() -> bool {
    Command::new("script")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

struct Term {
    store: tempfile::TempDir,
}

impl Term {
    fn new() -> Term {
        Term {
            store: tempfile::tempdir().unwrap(),
        }
    }

    /// `losos <args>` on a 120x36 terminal, with what is typed sent later.
    fn spawn(&self, args: &str) -> Child {
        let line = format!("stty cols 120 rows 36; exec {} {args}", binary());
        Command::new("script")
            .args(["-qefc", &line, "/dev/null"])
            .env("LOSOS_SECRET_TOOL", fake_bin("secret-tool"))
            .env("FAKE_SECRET_STORE", self.store.path())
            .env("LANG", "C.UTF-8")
            .env("TERM", "xterm-256color")
            .env("COLORTERM", "truecolor")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    /// Run to the end, typing `keys` one burst at a time with a pause
    /// before each, the way a person would.
    fn run(&self, args: &str, keys: &[&str]) -> (i32, String) {
        let mut child = self.spawn(args);
        let mut stdin = child.stdin.take().unwrap();
        for k in keys {
            std::thread::sleep(Duration::from_millis(700));
            stdin.write_all(k.as_bytes()).unwrap();
            stdin.flush().unwrap();
        }
        std::thread::sleep(Duration::from_millis(300));
        drop(stdin);
        let out = child.wait_with_output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    }

    fn plain(&self, args: &[&str], stdin: &str) -> (i32, String) {
        let mut child = Command::new(binary())
            .args(args)
            .env("LOSOS_SECRET_TOOL", fake_bin("secret-tool"))
            .env("FAKE_SECRET_STORE", self.store.path())
            .env("LANG", "C.UTF-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
        )
    }
}

#[test]
fn logs_in_on_the_screen_opens_the_view_and_takes_a_hidden_value() {
    if !have_script() {
        eprintln!("skipped: no util-linux script for a pseudo-terminal");
        return;
    }
    let me = owner();
    let edge = mock(
        true,
        me.key.armored_public(),
        Box::new(|method, path, body| match (method, path) {
            ("GET", "/overview") => (200, json!({ "health": true, "tenants": [] })),
            ("POST", "/secrets") => (200, json!({ "sealed": true, "name": body["name"] })),
            _ => (200, json!([])),
        }),
    );
    let term = Term::new();
    let (code, out) = term.plain(&["key", "import", me.file.path().to_str().unwrap()], "");
    assert_eq!(code, 0, "{out}");

    // Refusing the fingerprint changes nothing.
    let (code, screen) = term.run(&format!("login {}", edge.url), &["n"]);
    assert_eq!(code, 130, "{screen}");
    let (_, hosts) = term.plain(&["hosts"], "");
    assert!(!hosts.contains(&edge.url), "{hosts}");

    // y, then the token pasted with its newline; one key, so no picker.
    let (code, screen) = term.run(
        &format!("login {}", edge.url),
        &["y", &format!("{TOKEN}\r")],
    );
    assert_eq!(code, 0, "{screen}");
    let grouped = losos::vault::group_fingerprint(&edge.fingerprint());
    assert!(
        screen.contains(&grouped[..24]),
        "the fingerprint was on screen:\n{screen}"
    );
    assert!(!screen.contains(TOKEN), "the token was echoed");
    let (_, hosts) = term.plain(&["hosts"], "");
    assert!(hosts.contains(&edge.url), "{hosts}");

    // The full-screen view: in the alternate screen, out again on q.
    let (code, screen) = term.run("", &["q"]);
    assert_eq!(code, 0, "{screen}");
    assert!(
        screen.contains("\x1b[?1049h") && screen.contains("\x1b[?1049l"),
        "{screen}"
    );
    assert!(screen.contains("LosOS"), "{screen}");

    // A value typed without echo.
    let (code, screen) = term.run(
        "edge secret set stripe-webhook-secret",
        &["whsec_hidden42\r"],
    );
    assert_eq!(code, 0, "{screen}");
    assert!(!screen.contains("whsec_hidden42"), "{screen}");
    let calls = edge.calls.lock().unwrap().clone();
    assert_eq!(
        calls.last().unwrap().2,
        json!({ "name": "stripe-webhook-secret", "value": "whsec_hidden42" })
    );

    // Ctrl-C at the prompt is a cancel.
    let (code, _) = term.run("edge secret set stripe-webhook-secret", &["\x03"]);
    assert_eq!(code, 130);
}
