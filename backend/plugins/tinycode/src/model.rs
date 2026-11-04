//! Agentic work on the box, through tinycode: what an ask is, the command
//! line it becomes and how the CLI's NDJSON is read. Plain functions on
//! data, so the host build tests them without lososd or the CLI.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The longest task prompt, in characters. The route's `maxBody` is the
/// real ceiling; this is what the model reads.
pub const MAX_TASK_CHARS: usize = 120_000;
/// Model turns one task may take. tinycode defaults to 24; an ask may
/// raise it, within reason.
pub const MAX_MAX_TURNS: u32 = 96;
/// How long an ask is kept for the page to read. An agent task may take
/// well over the assistant's hour, so this is a day.
pub const KEEP_SECS: u32 = 86_400;

/// One task: the prompt, whether the agent may change anything, and
/// whether it continues one of its earlier sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub task: String,
    /// With `auto` the agent's write/edit/run tools work — as lososd's
    /// user, root on a box. Without it the run is read-only and the agent
    /// is told to ask instead.
    pub auto: bool,
    pub max_turns: u32,
    /// Continue a session a previous ask made: the `session` it answered
    /// with, the file name inside the workspace's `.tinycode/sessions/`.
    pub resume: Option<String>,
}

/// The sentence a refusal carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(pub String);

fn invalid<T>(why: impl Into<String>) -> Result<T, Invalid> {
    Err(Invalid(why.into()))
}

/// `POST /api/tinycode/asks`: `{"task", "auto"? (default false), "maxTurns"?,
/// "resume"?}`.
pub fn parse_ask(doc: &Value) -> Result<Ask, Invalid> {
    let task = doc
        .get("task")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if task.is_empty() {
        return invalid("give the agent a task");
    }
    if task.chars().count() > MAX_TASK_CHARS {
        return invalid(format!("a task is at most {MAX_TASK_CHARS} characters"));
    }
    if !matches!(doc.get("auto"), None | Some(Value::Bool(_))) {
        return invalid("auto is true or false");
    }
    let max_turns = match doc.get("maxTurns") {
        None | Some(Value::Null) => 24,
        Some(Value::Number(n)) => n
            .as_u64()
            .filter(|n| (1..=u64::from(MAX_MAX_TURNS)).contains(n))
            .ok_or_else(|| Invalid(format!("maxTurns is 1…{MAX_MAX_TURNS}")))?
            as u32,
        Some(_) => return invalid("maxTurns is a number"),
    };
    let resume = match doc.get("resume") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if session_file(s) => Some(s.clone()),
        Some(_) => return invalid("resume names a session file a previous ask answered with"),
    };
    Ok(Ask {
        task,
        auto: doc.get("auto").and_then(Value::as_bool).unwrap_or(false),
        max_turns,
        resume,
    })
}

/// A session file name a previous ask answered with: the file is under the
/// plugin's own `files` root and this leaves it there.
pub fn session_file(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-._".contains(c))
}

/// The session named by a previous ask, as the CLI writes it: beside the
/// workspace root it was given.
pub fn session_path(workspace: &str, resume: &str) -> String {
    fs_join(workspace, &format!(".tinycode/sessions/{resume}"))
}

/// The command the ask becomes, against one `tinycode` binary on lososd's
/// `PATH`. Runtime paths come in through the manifest's `config` table: the
/// state directory (which holds the workspace and the agent's own HOME) and
/// the two OpenCode config paths an administrator may point it at; the ones
/// the manifest leaves empty stay unset and the CLI falls back to its own
/// defaults.
pub fn command(
    ask: &Ask,
    dir: &str,
    opencode_config: Option<&str>,
    opencode_auth: Option<&str>,
) -> ExecSpec {
    let workspace = fs_join(dir, "work");
    let home = fs_join(dir, "home");
    let mut args = vec![
        "run".into(),
        "--json".into(),
        "--max-turns".into(),
        ask.max_turns.to_string(),
        "--root".into(),
        workspace.clone(),
        // The prompt on stdin ("--task-file -"): it can hold anything, and
        // no shell ever sees it.
        "--task-file".into(),
        "-".into(),
    ];
    if ask.auto {
        args.insert(1, "--auto".into());
    }
    if let Some(resume) = ask.resume.as_deref() {
        args.push("-s".into());
        args.push(session_path(&workspace, resume));
    }
    let mut env = vec![
        ("HOME".into(), home.clone()),
        ("XDG_CONFIG_HOME".into(), fs_join(&home, ".config")),
        ("XDG_DATA_HOME".into(), fs_join(&home, ".local/share")),
    ];
    if let Some(path) = opencode_config.filter(|p| !p.is_empty()) {
        env.push(("TINYCODE_OPENCODE_CONFIG".into(), path.into()));
    }
    if let Some(path) = opencode_auth.filter(|p| !p.is_empty()) {
        env.push(("TINYCODE_OPENCODE_AUTH".into(), path.into()));
    }
    ExecSpec {
        program: "tinycode",
        args,
        env,
        cwd: workspace.clone(),
        stdin: ask.task.clone().into_bytes(),
    }
}

/// What `tinycode run --json` runs as: the exec import's command, marked
/// `debug` only because it may carry model names an ask chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecSpec {
    pub program: &'static str,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: String,
    pub stdin: Vec<u8>,
}

#[must_use]
pub fn fs_join(dir: &str, rest: &str) -> String {
    format!("{}/{}", dir.trim_end_matches('/'), rest)
}

/// What an agent task came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub id: String,
    /// queued, running, done, max_turns, failed or stopped.
    pub state: String,
    /// The answer, when the CLI said it was done.
    pub answer: String,
    pub turns: Option<u32>,
    /// The session a `resume` continues: the file name inside the
    /// workspace's `.tinycode/sessions/`, so nothing outside there leaks.
    pub session: Option<String>,
    pub error: Option<String>,
    /// The CLI's exit code, when it left.
    pub exit: Option<i32>,
}

/// One ask, as the cache keeps it and the person may read it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// The person or key that asked.
    pub owner: String,
    pub state: State,
}

impl State {
    /// The agent said it was done: nothing to stop, nothing to resume.
    #[must_use]
    pub fn completed(&self) -> bool {
        self.state == "done"
    }
}

/// Whether an id is one this plugin made: hex, twelve bytes.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    id.len() == 24 && id.chars().all(|c| c.is_ascii_hexdigit())
}

/// What the CLI's NDJSON stdout said.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// The `done` event's text.
    pub answer: Option<String>,
    pub turns: Option<u32>,
    /// The session file, from `start` or `done`.
    pub session: Option<String>,
    /// The session of a run that used its turns up.
    pub max_turns: Option<String>,
    /// A terminal `error` event, the CLI's own.
    pub error: Option<String>,
    /// Failed tool results, so the journal line says more than "failed".
    pub failed_tools: u32,
    /// Lines that were neither: bytes the journal may want.
    pub unparsed: Vec<String>,
}

impl Outcome {
    /// True when the agent said it was done; a run that used its turns up
    /// is one that ran out, not one that failed — its session is resumable.
    #[must_use]
    pub fn completed(&self) -> bool {
        self.answer.is_some()
    }

    /// How the run ends: its shape in the cache.
    pub fn to_state(&self) -> String {
        match (&self.answer, &self.error) {
            (Some(_), _) => "done".into(),
            (None, Some(_)) => "failed".into(),
            _ if self.max_turns.is_some() => "max_turns".into(),
            _ => "failed".into(),
        }
    }
}

/// Parse the NDJSON events off the CLI's stdout. A line it does not know —
/// the CLI's narration escaping onto stdout, a shell's echo — is kept, not
/// dropped: the last few end up in the journal.
pub fn parse_events(stdout: &str) -> Outcome {
    let mut out = Outcome::default();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(doc) = serde_json::from_str::<Value>(line) else {
            out.unparsed.push(line.to_string());
            continue;
        };
        match doc.get("type").and_then(Value::as_str) {
            Some("start") => out.session = doc["session"].as_str().map(String::from),
            Some("done") => {
                out.answer = doc["text"].as_str().map(String::from);
                out.turns = doc["turns"].as_u64().map(|n| n as u32);
                out.session = doc["session"].as_str().map(String::from);
            }
            Some("max_turns") => out.max_turns = doc["session"].as_str().map(String::from),
            Some("error") => out.error = doc["message"].as_str().map(String::from),
            Some("tool_result") if doc["ok"].as_bool() == Some(false) => out.failed_tools += 1,
            _ => {}
        }
    }
    out
}

/// The line the journal gets about a finished run.
#[must_use]
pub fn journal_line(outcome: &Outcome, exit: i32, stderr_last: &str) -> String {
    let mut l = if outcome.completed() {
        format!(
            "the task ended in {} turn(s) (exit {exit})",
            outcome.turns.unwrap_or(0)
        )
    } else if outcome.max_turns.is_some() {
        format!("the task used its turns up (exit {exit})")
    } else {
        format!("the task failed (exit {exit})")
    };
    if outcome.failed_tools > 0 {
        l = format!("{l}, {} tool(s) came back refused", outcome.failed_tools);
    }
    if let Some(session) = outcome.session.as_deref() {
        l = format!("{l}; session {session}");
    }
    if !stderr_last.is_empty() {
        l = format!("{l}: {stderr_last}");
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_ask_is_a_task_and_how_wide_the_agent_may_go() {
        let a = parse_ask(&json!({ "task": "  update the CHANGELOG ", "auto": true })).unwrap();
        assert_eq!(
            (a.task.as_str(), a.auto, a.max_turns),
            ("update the CHANGELOG", true, 24)
        );
        assert!(parse_ask(&json!({ "task": "x", "maxTurns": 96 })).is_ok());
        assert!(!parse_ask(&json!({ "task": "x" })).unwrap().auto);
        assert!(parse_ask(&json!({ "task": "" })).is_err());
        assert!(parse_ask(&json!({ "task": "x", "auto": "yes" })).is_err());
        assert!(parse_ask(&json!({ "task": "x", "maxTurns": 0 })).is_err());
        assert!(parse_ask(&json!({ "task": "x", "maxTurns": 97 })).is_err());
        assert!(parse_ask(&json!({ "task": "x", "maxTurns": "24" })).is_err());
        assert!(parse_ask(&json!({ "task": "x", "resume": "../../etc/shadow" })).is_err());
        let long = "a".repeat(MAX_TASK_CHARS + 1);
        assert!(parse_ask(&json!({ "task": long })).is_err());
    }

    #[test]
    fn the_command_is_built_out_of_the_directories_the_manifest_names() {
        let ask = parse_ask(&json!({ "task": "find the failing test", "maxTurns": 8 })).unwrap();
        let spec = command(&ask, "/nix/store/…-tinycode", None, None);
        assert_eq!(spec.program, "tinycode");
        assert_eq!(
            spec.args,
            vec![
                "run",
                "--json",
                "--max-turns",
                "8",
                "--root",
                "/nix/store/…-tinycode/work",
                "--task-file",
                "-"
            ]
        );
        assert_eq!(spec.cwd, "/nix/store/…-tinycode/work");
        // The task is on stdin, never on the command line.
        assert_eq!(spec.stdin, b"find the failing test");
        let env = spec.env.to_vec();
        assert!(env.contains(&("HOME".into(), "/nix/store/…-tinycode/home".into())));
        assert!(env.contains(&(
            "XDG_CONFIG_HOME".into(),
            "/nix/store/…-tinycode/home/.config".into()
        )));
        // No OpenCode override the manifest left empty.
        assert!(env
            .iter()
            .all(|(k, _)| !k.starts_with("TINYCODE_OPENCODE_")));
    }

    #[test]
    fn a_wide_resume_runs_wide_and_continues_its_session() {
        let ask = parse_ask(&json!({
            "task": "update the CHANGELOG",
            "auto": true,
            "resume": "tc-18a-7.json",
        }))
        .unwrap();
        let spec = command(&ask, "/var/lib/losos/tinycode", None, None);
        assert_eq!(
            spec.args,
            vec![
                "run",
                "--auto",
                "--json",
                "--max-turns",
                "24",
                "--root",
                "/var/lib/losos/tinycode/work",
                "--task-file",
                "-",
                "-s",
                "/var/lib/losos/tinycode/work/.tinycode/sessions/tc-18a-7.json"
            ]
        );
    }

    #[test]
    fn the_overrides_go_through_when_the_manifest_names_them() {
        let ask = parse_ask(&json!({ "task": "x" })).unwrap();
        let spec = command(&ask, "/nix/store/…", Some("/nix/store/…/out.json"), None);
        let env = spec.env.to_vec();
        assert!(env.contains(&(
            "TINYCODE_OPENCODE_CONFIG".into(),
            "/nix/store/…/out.json".into()
        )));
        assert!(env.iter().all(|(k, _)| k != "TINYCODE_OPENCODE_AUTH"));
        let none = command(&ask, "/nix/store/…", Some(""), Some(""));
        assert!(none
            .env
            .iter()
            .all(|(k, _)| !k.starts_with("TINYCODE_OPENCODE_")));
    }

    #[test]
    fn the_events_of_a_run_read_off_its_stdout() {
        let raw = concat!(
            r#"{"type":"start","session":"/nix/store/…/work/.tinycode/sessions/tc-1.json"}"#,
            "\n",
            r#"{"type":"turn","n":1}"#,
            "\n",
            "narration on stdout is kept\n",
            r#"{"type":"tool","name":"grep","input":"{}"}"#,
            "\n",
            r#"{"type":"tool_result","name":"grep","ok":true,"bytes":80}"#,
            "\n",
            r#"{"type":"tool_result","name":"edit","ok":false,"bytes":8}"#,
            "\n",
            r#"{"type":"done","turns":2,"text":"Fixed it.","session":"/nix/store/…/sessions/tc-1.json"}"#,
            "\n"
        );
        let o = parse_events(raw);
        assert_eq!(o.answer.as_deref(), Some("Fixed it."));
        assert_eq!(o.turns, Some(2));
        assert_eq!(o.failed_tools, 1);
        assert_eq!(o.unparsed, ["narration on stdout is kept"]);
        assert_eq!(o.to_state(), "done");
        assert!(o.completed());

        // A run that used its turns up is not a failure, and the session is
        // what a resume carries on with.
        let o = parse_events(r#"{ "type": "max_turns", "session": "tc-2.json" }"#);
        assert!(!o.completed());
        assert_eq!(o.to_state(), "max_turns");
        assert_eq!(o.max_turns.as_deref(), Some("tc-2.json"));

        // The CLI's own error: an ask the provider refused.
        let o = parse_events(r#"{ "type": "error", "message": "401: bad key" }"#);
        assert_eq!(o.error.as_deref(), Some("401: bad key"));
        assert!(!o.completed());
        assert_eq!(o.to_state(), "failed");

        // The journal line names what it can.
        let o = parse_events(raw);
        let l = journal_line(&o, 0, "a warning");
        assert!(l.contains("2 turn"), "{l}");
        assert!(l.contains("session /nix/store/…/sessions/tc-1.json"), "{l}");
        assert!(l.ends_with(": a warning"), "{l}");
    }

    #[test]
    fn an_id_is_hex_and_a_session_file_name_still_where_it_was() {
        assert!(valid_id(&"0f".repeat(12)));
        assert!(!valid_id("../escape"));
        assert!(!valid_id(""));
        assert!(!valid_id("short"));
        assert!(session_file("tc-18a-7.json"));
        assert!(!session_file("tc a.json"));
        assert!(!session_file("../x"));
        assert!(!session_file("."));
        assert_eq!(
            session_path("/var/lib/losos/tinycode/work", "tc-18a-7.json"),
            "/var/lib/losos/tinycode/work/.tinycode/sessions/tc-18a-7.json"
        );
    }

    #[test]
    fn the_page_never_reads_the_owner() {
        let s = State {
            id: "a".repeat(24),
            state: "done".into(),
            answer: "Hi".into(),
            turns: Some(1),
            session: Some("tc-1.json".into()),
            error: None,
            exit: Some(0),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert!(v.get("owner").is_none());
        assert_eq!(v["state"], "done");
    }
}
