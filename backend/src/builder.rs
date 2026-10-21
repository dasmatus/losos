//! The builder: a language model on this box writes a widget or an app from
//! the owner's description.
//!
//! A hand-written widget (`crate::look`) is HTML, style and script that the
//! admin page draws in a sandboxed frame; an app (`crate::user_apps`) is one
//! React component in JSX that runs in a frame of its own and keeps its data
//! on the box. Writing either takes some knowledge of the frame, so the
//! builder lets the owner describe it in a sentence and has a model write it.
//! The result is text and nothing more: it lands in the same editor and the
//! same frame as code typed by hand, with no privilege typed code lacks, and
//! nothing is saved until the owner saves it.
//!
//! # Where it runs
//!
//! On the box. Ollama (`modules/builder.nix`) serves one small model on
//! loopback, and lososd talks to it with `curl`, like every other request it
//! makes. The owner's description, and the widget being changed, never leave
//! the box, and a build costs nothing but the box's own time.
//!
//! A model that small writes a widget in minutes on a mini PC's processor, so
//! a build runs on a thread of its own and the page polls it. The answer is
//! streamed, which gives the page a progress count and lets the owner stop a
//! build halfway. Builds live in memory only: a widget goes into the editor
//! the moment it is done, and a restart of lososd forgets the rest.
//!
//! # What the model is told
//!
//! `builder-widget-prompt.md` and `builder-app-prompt.md`, the two contracts
//! written for a small coding model: the answer's shape, the frame's limits,
//! what the box serves, and three worked examples. Each file is chat turns
//! (`<!-- system -->`, `<!-- user -->`, `<!-- assistant -->`): a model the
//! size of the default one copies the shape of an answer it has seen itself
//! give far more reliably than one described in the system turn. The answer
//! is a fenced code block and one or two labelled lines, the shape such
//! models are trained on. The prompt's size and [`MAX_ANSWER_TOKENS`] set the
//! context window (`LOSOS_BUILDER_CONTEXT`); a widget or app to change has to
//! fit in what is left, which is why a change has its own, smaller limit
//! ([`MAX_BASE_BYTES`]).
//!
//! # Checking the answer, and the loop
//!
//! A small model gets the shape right far more often than the details, so a
//! build is a loop of steps ([`Step`]) the page shows as they happen: write,
//! check, try, fix, until nothing is found wrong or the fixes run out.
//!
//! Before a widget goes to the editor, [`problems`] looks for the mistakes it
//! makes most: a script that looks up an element the markup does not have,
//! and a `losos` member that does not exist. An app goes through
//! [`crate::jsx::check`], which parses and compiles it and names syntax
//! errors, components and names that do not exist, and storage the frame
//! does not have. A model that repeats one line until it runs out of room is
//! cut off as soon as it does ([`looped`]).
//!
//! An app that passes the check is then tried, in the owner's browser: no
//! app code runs on the box, so the build hands the page the compiled app
//! ([`Trial`]), the page's preview frame fills every field, presses the
//! first button and says what it showed, kept and threw, and the page
//! reports that back (`POST /api/builder/builds/{id}/tried`). An app that
//! throws, or keeps nothing when a form is filled in, goes back to the model
//! with that named ([`try_problems`]). With no page open to try it in, the
//! build carries on untried.
//!
//! The box drives the loop and the model only writes and fixes, in the
//! fenced shape above. A model the size of the default one does not call
//! tools: given Ollama's tool calls it answered with none in six turns out
//! of six, so it is never asked to pick its next step. Each fix is one more
//! turn with the last answer and what was found wrong with it, and the best
//! answer of the build is kept: one that compiles over one that does not,
//! then the one with the fewest findings.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::look::{is_file_name, validate_files, WidgetFile, ENTRY_FILE, MAX_FILES};

/// What the model is told before a widget, as chat turns.
pub const WIDGET_PROMPT: &str = include_str!("builder-widget-prompt.md");
/// What the model is told before an app, as chat turns.
pub const APP_PROMPT: &str = include_str!("builder-app-prompt.md");
/// The model Ollama serves when the configuration names none: the coding
/// model that fits the smallest box LosOS runs on (`modules/builder.nix`).
pub const DEFAULT_MODEL: &str = "qwen2.5-coder:1.5b";
/// The context window when the configuration names none, in tokens. Either
/// prompt is about 2,300, an answer at most [`MAX_ANSWER_TOKENS`], and the
/// rest is the request and the widget or app to change.
pub const DEFAULT_CONTEXT: u32 = 12288;
/// The longest answer, in tokens. A widget the model writes is 1,000 to
/// 2,500; this stops a model that repeats itself.
pub const MAX_ANSWER_TOKENS: u32 = 4096;
/// Longest description, in characters.
pub const MAX_PROMPT_CHARS: usize = 2000;
/// The largest widget the builder will change, its files together, in
/// bytes. Code runs about 3.5 bytes to a token, so this is some 3,000
/// tokens: what the default context has room for beside the prompt and the
/// answer.
pub const MAX_BASE_BYTES: usize = 10 * 1024;
/// The languages the summary is written in; the admin page's own.
pub const LANGS: [&str; 3] = ["en", "sk", "de"];
/// Builds kept in memory, newest first.
const KEEP_BUILDS: usize = 10;
/// Longest summary kept, in characters.
const MAX_SUMMARY_CHARS: usize = 600;
/// A build that has run this long is stopped. At five tokens a second a
/// full answer takes under fifteen minutes.
const BUILD_TIMEOUT_SECS: u32 = 30 * 60;
/// How long the model's list may take to come back.
const PROBE_TIMEOUT_SECS: &str = "3";
/// Temperature for every build: low, so the model keeps to the contract.
const TEMPERATURE: f64 = 0.2;
/// The same line this many times in a row is a model going round in circles.
const LOOP_LINES: usize = 12;
/// The most of a first answer the model is shown again when it fixes it.
const MAX_REPAIR_CHARS: usize = 6000;
/// How many times an app goes back to the model after the first answer.
/// Each is a full answer, a minute or two on a small box.
pub const APP_FIXES: usize = 3;
/// A widget gets one: it cannot be tried, only checked.
pub const WIDGET_FIXES: usize = 1;
/// How long a build waits for the page to try an app.
pub const DEFAULT_TRY_SECS: u64 = 30;
/// What a try report may hold: lines, and characters a line.
const MAX_REPORT_LINES: usize = 40;
const MAX_REPORT_CHARS: usize = 200;
/// What the builder writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A tile for the board (`crate::look`).
    Widget,
    /// An app with a page of its own (`crate::user_apps`).
    App,
}

impl Kind {
    fn noun(self) -> &'static str {
        match self {
            Kind::Widget => "widget",
            Kind::App => "app",
        }
    }

    fn prompt(self) -> &'static str {
        match self {
            Kind::Widget => WIDGET_PROMPT,
            Kind::App => APP_PROMPT,
        }
    }

    /// The answer's shape, as the model is reminded of it.
    fn shape(self) -> &'static str {
        match self {
            Kind::Widget => "one ```html code block, then the Summary: line",
            Kind::App => "one ```jsx code block, then the Name: and Summary: lines",
        }
    }
}

/// The file an app's source is, in a build and in a request to change one.
pub const APP_FILE: &str = "app.jsx";

/// A prompt file as chat messages: the text after each `<!-- role -->`
/// marker is one turn.
#[must_use]
pub fn prompt_messages(prompt: &str) -> Vec<Value> {
    let mut messages: Vec<Value> = Vec::new();
    let mut role: Option<&str> = None;
    let mut text = String::new();
    let mut flush = |role: Option<&str>, text: &mut String| {
        if let Some(role) = role {
            messages.push(json!({ "role": role, "content": text.trim_end_matches('\n') }));
        }
        text.clear();
    };
    for line in prompt.split_inclusive('\n') {
        let marker = line
            .trim()
            .strip_prefix("<!-- ")
            .and_then(|l| l.strip_suffix(" -->"))
            .filter(|r| matches!(*r, "system" | "user" | "assistant"));
        if let Some(next) = marker {
            flush(role, &mut text);
            role = Some(next);
        } else {
            text.push_str(line);
        }
    }
    flush(role, &mut text);
    messages
}

/// What the frame's `losos` object has (`admin-ui/app/public/widget-frame/`).
const LOSOS_MEMBERS: [&str; 8] = [
    "metric", "lang", "resize", "theme", "onTheme", "files", "file", "asset",
];

/// Where the model is: `LOSOS_BUILDER_URL` (Ollama's address),
/// `LOSOS_BUILDER_MODEL` and `LOSOS_BUILDER_CONTEXT`, set by
/// `modules/builder.nix`, and `LOSOS_BUILDER_TRY_SECS`, how long a try
/// waits for the page. No URL, no builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub url: String,
    pub model: String,
    pub context: u32,
    pub try_wait: Duration,
}

impl Config {
    #[must_use]
    pub fn from_env() -> Option<Config> {
        let url = std::env::var("LOSOS_BUILDER_URL").ok()?;
        let url = url.trim().trim_end_matches('/').to_string();
        if !(url.starts_with("http://127.") || url.starts_with("http://localhost")) {
            // Loopback only: the description is the owner's, and so is the
            // widget being changed.
            tracing::warn!(%url, "LOSOS_BUILDER_URL is not on loopback; the builder is off");
            return None;
        }
        let model = std::env::var("LOSOS_BUILDER_MODEL")
            .ok()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let context = std::env::var("LOSOS_BUILDER_CONTEXT")
            .ok()
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or(DEFAULT_CONTEXT);
        let try_secs = std::env::var("LOSOS_BUILDER_TRY_SECS")
            .ok()
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or(DEFAULT_TRY_SECS);
        Some(Config {
            url,
            model: model.trim().to_string(),
            context,
            try_wait: Duration::from_secs(try_secs),
        })
    }
}

/// A request the builder turns down, with the sentence for the owner.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Refused {
    pub status: u16,
    pub message: String,
}

fn refuse<T>(status: u16, message: impl Into<String>) -> Result<T, Refused> {
    Err(Refused {
        status,
        message: message.into(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Done,
    Failed,
    Stopped,
}

/// Why a build produced no widget. A fixed set, so the page can say it in
/// the owner's language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Fault {
    /// The answer had no `index.html` in it.
    NoWidget,
    /// The answer had no JSX code block in it.
    NoApp,
    /// It had files the box would not keep: badly named, too many, too large.
    Unusable,
    /// The model did not answer.
    Upstream,
    /// Ollama would not load the model: the box has too little free memory.
    Memory,
}

/// What a step of a build is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepKind {
    /// The model writes the first answer.
    Write,
    /// The box checks what it wrote.
    Check,
    /// The page tries the app.
    Try,
    /// The model writes again, fixing what was found.
    Fix,
}

/// One step of a build, as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    pub kind: StepKind,
    /// Still under way.
    pub running: bool,
    /// What it found wrong, as sentences; none when it found nothing.
    pub problems: Vec<String>,
    /// For a try: what the app showed, a line per element.
    pub shown: Vec<String>,
    /// For a try: no page was open to try the app in.
    pub skipped: bool,
    pub started_at: u64,
}

impl Step {
    fn new(kind: StepKind) -> Step {
        Step {
            kind,
            running: true,
            problems: Vec::new(),
            shown: Vec::new(),
            skipped: false,
            started_at: now(),
        }
    }
}

/// An app the build asks the page to try: the compiled code, and the id the
/// page's report names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trial {
    pub id: String,
    pub code: String,
}

/// What the page's try of an app found (`admin-ui/app/src/app-frame/
/// runtime.tsx`): what it showed, the first button's words, which
/// collections changed when it was pressed, and, when nothing did, the
/// choices it changed before pressing again and what changed then, and
/// what it threw on start and on the press.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TryReport {
    pub shown: Vec<String>,
    pub pressed: String,
    pub kept: Vec<String>,
    pub chose: Vec<String>,
    pub kept_after_choosing: Vec<String>,
    pub errors: Vec<String>,
    pub threw_on_press: Vec<String>,
}

/// Read `{"trial", "shown", "pressed", "kept", "chose",
/// "keptAfterChoosing", "errors", "threwOnPress"}`
/// into the trial's id and its report, every list cut to
/// [`MAX_REPORT_LINES`] of [`MAX_REPORT_CHARS`].
///
/// # Errors
/// A sentence for a 400.
pub fn parse_report(doc: &Value) -> Result<(String, TryReport), Refused> {
    let Some(trial) = doc.get("trial").and_then(Value::as_str) else {
        return refuse(400, "the report names no trial");
    };
    let line = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_control())
            .take(MAX_REPORT_CHARS)
            .collect()
    };
    let lines = |key: &str| -> Vec<String> {
        doc.get(key)
            .and_then(Value::as_array)
            .map(|all| {
                all.iter()
                    .filter_map(Value::as_str)
                    .take(MAX_REPORT_LINES)
                    .map(line)
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok((
        trial.to_string(),
        TryReport {
            shown: lines("shown"),
            pressed: line(doc.get("pressed").and_then(Value::as_str).unwrap_or("")),
            kept: lines("kept"),
            chose: lines("chose"),
            kept_after_choosing: lines("keptAfterChoosing"),
            errors: lines("errors"),
            threw_on_press: lines("threwOnPress"),
        },
    ))
}

/// What a try found wrong, as sentences for the model and the owner: the
/// app stops on start, or on the press, keeps an entry only once a choice
/// is changed from the one it shows first, or has fields to fill in and
/// kept nothing when they were filled and the first button pressed.
#[must_use]
pub fn try_problems(report: &TryReport) -> Vec<String> {
    let mut found: Vec<String> = report
        .errors
        .iter()
        .map(|e| format!("When the app starts, it stops with: {e}"))
        .collect();
    if !found.is_empty() {
        return found;
    }
    found.extend(
        report
            .threw_on_press
            .iter()
            .map(|e| format!("Pressing \"{}\" stops the app with: {e}", report.pressed)),
    );
    let fields = report.shown.iter().any(|l| {
        (l.starts_with("input[")
            && !l.starts_with("input[checkbox]")
            && !l.starts_with("input[radio]"))
            || l.starts_with("textarea")
    });
    if found.is_empty() && report.kept.is_empty() && !report.kept_after_choosing.is_empty() {
        let choices = report
            .chose
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(" and ");
        found.push(format!(
            "With {choices} left at its first choice, pressing \"{}\" keeps nothing; it adds only once another choice is picked. The state behind {choices} starts empty while the list shows its first choice: start it at the first choice instead.",
            report.pressed
        ));
        return found;
    }
    if found.is_empty() && fields && !report.pressed.is_empty() && report.kept.is_empty() {
        found.push(format!(
            "When every field was filled in and \"{}\" was pressed, the app kept nothing. The button that adds an entry must call add on a useCollection with what was typed.",
            report.pressed
        ));
    }
    found
}

/// One build, as the page sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Build {
    pub id: String,
    pub kind: Kind,
    pub status: Status,
    pub prompt: String,
    /// Whether the owner asked to change a widget rather than start one.
    pub revision: bool,
    pub created_at: u64,
    pub finished_at: Option<u64>,
    /// Lines of the answer written so far.
    pub lines: u32,
    pub fault: Option<Fault>,
    /// The answer ran into [`MAX_ANSWER_TOKENS`]; the widget may be
    /// unfinished.
    pub at_limit: bool,
    pub files: Option<Vec<WidgetFile>>,
    /// The name the model gave an app.
    pub name: Option<String>,
    /// An app compiled for the frame (`crate::jsx`), so the page can show
    /// it before it is saved. `None` for a widget, or an app that does not
    /// compile.
    pub code: Option<String>,
    /// What the checks still find wrong with what was made, as sentences.
    pub problems: Vec<String>,
    /// What the model says it made.
    pub notes: Option<String>,
    /// The model is writing again, fixing what was found wrong.
    pub fixing: bool,
    /// What the build has done so far, in order.
    pub steps: Vec<Step>,
    /// An app the page is asked to try now.
    pub trial: Option<Trial>,
}

impl Build {
    /// The build without its files, for the list.
    fn summary(&self) -> Value {
        json!({
            "id": self.id,
            "kind": self.kind,
            "status": self.status,
            "prompt": self.prompt,
            "revision": self.revision,
            "created_at": self.created_at,
            "finished_at": self.finished_at,
            "lines": self.lines,
            "fixing": self.fixing,
            "steps": self.steps.len(),
            "fault": self.fault,
            "has_source": self.files.is_some(),
        })
    }
}

/// A checked request to start a build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub kind: Kind,
    pub prompt: String,
    pub base: Option<Vec<WidgetFile>>,
    pub lang: String,
}

/// Read `{"prompt", "kind"?, "base"?, "lang"?}` and check it: a description
/// of 1 to [`MAX_PROMPT_CHARS`] characters with no control characters but
/// newlines and tabs, a widget (`"widget"`, the default) or an app
/// (`"app"`), the widget or app to change, which the box would keep and
/// which fits [`MAX_BASE_BYTES`], and one of [`LANGS`]. An app to change is
/// its source as a string.
///
/// # Errors
/// A sentence for a 400.
pub fn parse_request(doc: &Value) -> Result<Request, Refused> {
    let kind = match doc.get("kind").and_then(Value::as_str) {
        None | Some("widget") => Kind::Widget,
        Some("app") => Kind::App,
        Some(_) => return refuse(400, "the builder writes widgets and apps"),
    };
    let noun = kind.noun();
    let Some(prompt) = doc.get("prompt").and_then(Value::as_str) else {
        return refuse(400, format!("describe the {noun} you want"));
    };
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return refuse(400, format!("describe the {noun} you want"));
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return refuse(
            400,
            format!("the description is longer than {MAX_PROMPT_CHARS} characters"),
        );
    }
    if prompt
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return refuse(400, "the description has a control character in it");
    }
    let base = match (kind, doc.get("base")) {
        (_, None | Some(Value::Null)) => None,
        (_, Some(Value::String(s))) if s.trim().is_empty() => None,
        (Kind::Widget, Some(Value::String(s))) => Some(vec![WidgetFile::entry(s)]),
        (Kind::App, Some(Value::String(s))) => Some(vec![WidgetFile {
            name: APP_FILE.into(),
            content: s.clone(),
        }]),
        (Kind::Widget, Some(files)) => match crate::look::parse_files(files) {
            Some(files) => Some(files),
            None => return refuse(400, "the widget to change is not a list of files"),
        },
        (Kind::App, Some(_)) => return refuse(400, "the app to change is not its source"),
    };
    if let Some(files) = &base {
        if kind == Kind::Widget {
            if let Err(why) = validate_files(files) {
                return refuse(400, why.0);
            }
        }
        let bytes: usize = files.iter().map(|f| f.content.len()).sum();
        if bytes > MAX_BASE_BYTES {
            return refuse(
                400,
                format!(
                    "this {noun} is too large for the builder to change; it changes {noun}s of up to {} KiB",
                    MAX_BASE_BYTES / 1024
                ),
            );
        }
    }
    let lang = match doc.get("lang").and_then(Value::as_str) {
        None => "en",
        Some(l) if LANGS.contains(&l) => l,
        Some(_) => return refuse(400, "the language is not one the page is in"),
    };
    Ok(Request {
        kind,
        prompt: prompt.to_string(),
        base,
        lang: lang.to_string(),
    })
}

fn language(lang: &str) -> &'static str {
    match lang {
        "sk" => "Slovak",
        "de" => "German",
        _ => "English",
    }
}

/// The fence a file is shown in, by its name.
fn fence_language(name: &str) -> &'static str {
    match name.rsplit('.').next() {
        Some("jsx") => "jsx",
        Some("css") => "css",
        Some("js" | "mjs") => "js",
        Some("json") => "json",
        Some("svg") => "svg",
        Some("md") => "md",
        Some("txt") => "text",
        _ => "html",
    }
}

/// The request as the model reads it, the way the prompt's worked examples
/// put it: the owner's words, the widget or app to change when there is
/// one, and the language to write in.
#[must_use]
pub fn user_message(req: &Request) -> String {
    let lang = language(&req.lang);
    let noun = req.kind.noun();
    let mut text = format!("{}\n", req.prompt);
    if let Some(base) = &req.base {
        text.push_str(&format!(
            "\nChange this {noun}, the one the owner has now, and keep what the request does not ask to change. Answer with the whole changed {noun}.\n\n"
        ));
        let named = base.len() > 1;
        for file in base {
            // The names are checked (`is_file_name`), so they need no quoting.
            if named {
                text.push_str(&format!("File: {}\n", file.name));
            }
            text.push_str(&format!(
                "```{}\n{}\n```\n",
                fence_language(&file.name),
                file.content.trim_end()
            ));
        }
    }
    text.push_str(&match req.kind {
        Kind::Widget => format!("\nWrite the summary in {lang}."),
        Kind::App => format!("\nWrite the app's words and the summary in {lang}."),
    });
    text
}

/// A first answer and what is wrong with it, for the model's second turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repair {
    pub answer: String,
    pub note: String,
}

/// The body of Ollama's `POST /api/chat` for one build, or for its second
/// turn when `repair` names what the first answer got wrong.
#[must_use]
pub fn chat_body(config: &Config, req: &Request, repair: Option<&Repair>) -> Value {
    let mut messages = prompt_messages(req.kind.prompt());
    messages.push(json!({ "role": "user", "content": user_message(req) }));
    if let Some(repair) = repair {
        let shown: String = repair.answer.chars().take(MAX_REPAIR_CHARS).collect();
        messages.push(json!({ "role": "assistant", "content": shown }));
        messages.push(json!({ "role": "user", "content": repair.note }));
    }
    json!({
        "model": config.model,
        "stream": true,
        "messages": messages,
        "options": {
            "num_ctx": config.context,
            "num_predict": MAX_ANSWER_TOKENS,
            "temperature": TEMPERATURE,
        },
    })
}

/// What the model answered, read into files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Answer {
    pub files: Vec<WidgetFile>,
    /// The `Name:` line, for an app.
    pub name: Option<String>,
    pub summary: Option<String>,
}

/// The file a fenced block with no name is, by the fence's language.
fn file_for_fence(kind: Kind, language: &str) -> Option<&'static str> {
    let language = language.trim().to_ascii_lowercase();
    match kind {
        Kind::Widget => match language.as_str() {
            "html" | "htm" | "" => Some(ENTRY_FILE),
            _ => None,
        },
        Kind::App => match language.as_str() {
            "jsx" | "js" | "javascript" | "tsx" | "react" | "" => Some(APP_FILE),
            _ => None,
        },
    }
}

/// The value of a labelled line, `Summary: …` or `**Name:** …`.
fn labelled(text: &str, label: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim().trim_start_matches(['*', '#', '-', ' ']);
        let rest = line.strip_prefix(label)?;
        let rest = rest.strip_prefix(':')?;
        let rest = rest.trim().trim_start_matches('*').trim();
        Some(rest.trim_matches('`').trim().to_string()).filter(|r| !r.is_empty())
    })
}

/// A file name named on the line before a fence: `File: style.css`.
fn named_before(before: &str) -> Option<String> {
    let line = before.trim_end().lines().last()?;
    let name = labelled(line, "File")?;
    let name = name.trim_matches(['`', '*', '"']).trim().to_string();
    (is_file_name(&name) || name == APP_FILE).then_some(name)
}

fn put(files: &mut Vec<WidgetFile>, name: &str, content: &str) {
    match files.iter_mut().find(|f| f.name == name) {
        // A name given twice: the later one is the model's correction.
        Some(same) => same.content = content.to_string(),
        None => files.push(WidgetFile {
            name: name.to_string(),
            content: content.to_string(),
        }),
    }
}

/// Read an answer: fenced code blocks, a `Name:` line and a `Summary:` line.
/// A small model drifts from the shape now and then, so this is forgiving
/// where the meaning is plain: a block the answer ran out before closing is
/// kept to where it stops, a block named on the line before it (`File:
/// style.css`) is that file, and `<file name="…">` blocks and a `<summary>`,
/// the shape the builder once asked for, are read too.
#[must_use]
pub fn parse_answer(text: &str, kind: Kind) -> Answer {
    let mut files: Vec<WidgetFile> = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<file") {
        let after = &rest[start..];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let tag = &after[..open_end];
        let body = &after[open_end + 1..];
        let (content, next) = match body.find("</file>") {
            Some(end) => (&body[..end], &body[end + "</file>".len()..]),
            None => (body.split("<summary>").next().unwrap_or(body), ""),
        };
        if let Some(name) = attribute(tag, "name") {
            let content = content.strip_prefix('\n').unwrap_or(content);
            let content = content.strip_suffix('\n').unwrap_or(content);
            put(&mut files, name, strip_fence(content));
        }
        rest = next;
    }
    if files.is_empty() {
        let mut rest = text;
        while let Some(at) = rest.find("```") {
            let before = &rest[..at];
            let after = &rest[at + 3..];
            let (language, body) = after.split_once('\n').unwrap_or((after, ""));
            let (content, next) = match body.find("```") {
                Some(end) => (&body[..end], &body[end + 3..]),
                None => (body, ""),
            };
            let name =
                named_before(before).or_else(|| file_for_fence(kind, language).map(str::to_string));
            if let Some(name) = name {
                put(&mut files, &name, content.trim_end());
            }
            rest = next;
        }
    }
    let summary = labelled(text, "Summary")
        .or_else(|| {
            text.split_once("<summary>")
                .map(|(_, s)| s.split("</summary>").next().unwrap_or(s).to_string())
        })
        .and_then(|s| clean_summary(&s));
    let name = match kind {
        Kind::App => labelled(text, "Name").and_then(|n| clean_name(&n)),
        Kind::Widget => None,
    };
    Answer {
        files,
        name,
        summary,
    }
}

/// An app's name, short and on one line.
fn clean_name(raw: &str) -> Option<String> {
    let name = clean_summary(raw)?;
    let name: String = name
        .trim_matches(['"', '\'', '.'])
        .chars()
        .take(crate::user_apps::MAX_NAME_CHARS)
        .collect();
    Some(name.trim().to_string()).filter(|n| !n.is_empty())
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.find(&format!("{name}="))? + name.len() + 1;
    let value = &tag[at..];
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let value = &value[1..];
    Some(value[..value.find(quote)?].trim())
}

/// A file a model wrapped in a Markdown fence inside the block, unwrapped.
fn strip_fence(content: &str) -> &str {
    let trimmed = content.trim();
    if let Some(open) = trimmed.strip_prefix("```") {
        let body = open.split_once('\n').map_or("", |(_, b)| b);
        return body.strip_suffix("```").unwrap_or(body).trim_end();
    }
    content
}

/// Keep the summary short and on one line.
#[must_use]
pub fn clean_summary(raw: &str) -> Option<String> {
    let text: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= MAX_SUMMARY_CHARS {
        return Some(text);
    }
    let cut: String = text.chars().take(MAX_SUMMARY_CHARS - 1).collect();
    Some(format!("{}…", cut.trim_end()))
}

/// The widget an answer makes, or why it makes none: files with names a
/// widget cannot have are dropped first (a model sometimes adds a
/// `README.md`), then what is left must be a widget the box would keep.
///
/// # Errors
/// The fault to show.
pub fn widget(answer: &Answer) -> Result<Vec<WidgetFile>, Fault> {
    let mut files: Vec<WidgetFile> = answer
        .files
        .iter()
        .filter(|f| is_file_name(&f.name))
        .cloned()
        .collect();
    if !files
        .iter()
        .any(|f| f.name == ENTRY_FILE && !f.content.trim().is_empty())
    {
        return Err(Fault::NoWidget);
    }
    files.truncate(MAX_FILES + 1);
    validate_files(&files).map_err(|_| Fault::Unusable)?;
    files.sort_by(|a, b| (a.name != ENTRY_FILE, &a.name).cmp(&(b.name != ENTRY_FILE, &b.name)));
    Ok(files)
}

/// The mistakes a small model makes most that can be seen without running
/// the widget, each as a sentence the model is told: an element looked up
/// by an id no file declares, and a `losos` member the frame does not have.
#[must_use]
pub fn problems(files: &[WidgetFile]) -> Vec<String> {
    let all: String = files
        .iter()
        .map(|f| f.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut found = Vec::new();
    let mut seen = Vec::new();
    for id in quoted_after(&all, "getElementById(") {
        let declared = ["id=\"", "id='", ".id = \"", ".id = '", ".id=\"", ".id='"]
            .iter()
            .any(|p| all.contains(&format!("{p}{id}{}", &p[p.len() - 1..])));
        if !declared && !seen.contains(&id) {
            found.push(format!(
                "The script calls document.getElementById(\"{id}\"), but no element has id=\"{id}\". Give the element it means that id, or look up an id the markup has."
            ));
            seen.push(id);
        }
    }
    let mut rest = all.as_str();
    while let Some(at) = rest.find("losos.") {
        let before = rest[..at].chars().next_back();
        let after = &rest[at + "losos.".len()..];
        let member: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let word =
            before.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'));
        if word
            && !member.is_empty()
            && !LOSOS_MEMBERS.contains(&member.as_str())
            && !seen.contains(&member)
        {
            found.push(format!(
                "The script uses losos.{member}, which does not exist. The losos object has only metric, lang and resize."
            ));
            seen.push(member);
        }
        rest = after;
    }
    if all.contains("document.resize(") {
        found.push("document.resize() does not exist. Call losos.resize() instead.".into());
    }
    found
}

/// The string literals right after each `marker`: `getElementById("x")` and
/// `getElementById('x')` give `x`.
fn quoted_after(text: &str, marker: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(marker) {
        rest = &rest[at + marker.len()..];
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        if let Some(end) = rest[1..].find(quote) {
            let id = &rest[1..=end];
            if !id.is_empty() && !id.contains(char::is_whitespace) {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// The app an answer makes: its source, or [`Fault::NoApp`].
///
/// # Errors
/// The fault to show.
pub fn app(answer: &Answer) -> Result<WidgetFile, Fault> {
    answer
        .files
        .iter()
        .find(|f| f.name == APP_FILE && !f.content.trim().is_empty())
        .cloned()
        .ok_or(Fault::NoApp)
}

/// Whether the answer ends in the same non-blank line [`LOOP_LINES`] times.
#[must_use]
pub fn looped(text: &str) -> bool {
    let mut lines = text.lines().rev().map(str::trim).filter(|l| !l.is_empty());
    let Some(last) = lines.next() else {
        return false;
    };
    lines.take(LOOP_LINES - 1).filter(|l| *l == last).count() == LOOP_LINES - 1
}

/// What the model is told when its first answer repeated itself.
fn loop_note(kind: Kind) -> String {
    let noun = kind.noun();
    let repeat = match kind {
        Kind::Widget => "make repeated elements in the script with a for loop",
        Kind::App => "make repeated elements with .map over an array",
    };
    format!(
        "Your answer repeated the same line over and over and was cut off. Write the whole {noun} again, short: {repeat}, never one line per element. Same shape: {}.",
        kind.shape()
    )
}

fn repair_note(kind: Kind, problems: &[String], request: &str) -> String {
    let noun = kind.noun();
    let mut note = format!("Your {noun} has these mistakes:\n");
    for p in problems {
        note.push_str("- ");
        note.push_str(p);
        note.push('\n');
    }
    // The request again, last: a small model asked for a fix three turns
    // in drifts towards the examples it was shown first.
    note.push_str(&format!(
        "Send the whole {noun} again with them fixed, in the same shape: {}. It is still the {noun} for: {}",
        kind.shape(),
        request.trim()
    ));
    note
}

/// The example in the instructions that `code` repeats, as that example's
/// request, when it does: four in five of its lines are lines of the
/// example's answer. A small model asked for a fix sends an example back
/// often, which checks clean and is not what the owner asked for. An owner
/// who asked for much the same as the example (two words of four letters or
/// more in common with its request) is given it without a word.
fn copied_example(kind: Kind, code: &str, request: &str) -> Option<String> {
    let lines = |text: &str| -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|l| l.chars().count() >= 4)
            .map(str::to_string)
            .collect()
    };
    let words = |text: &str| -> std::collections::BTreeSet<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() >= 4)
            .map(str::to_lowercase)
            .collect()
    };
    let answer = lines(code);
    if answer.is_empty() {
        return None;
    }
    let asked = words(request);
    let turns = prompt_messages(kind.prompt());
    turns.windows(2).find_map(|pair| {
        let (ask, said) = (&pair[0], &pair[1]);
        if ask["role"] != "user" || said["role"] != "assistant" {
            return None;
        }
        let example_request = ask["content"].as_str()?.split("\n\n").next()?.trim();
        let example: std::collections::HashSet<String> =
            lines(said["content"].as_str()?).into_iter().collect();
        let same = answer.iter().filter(|l| example.contains(*l)).count();
        let copied = same * 5 >= answer.len() * 4;
        let alike = words(example_request).intersection(&asked).count() >= 2;
        (copied && !alike).then(|| example_request.to_string())
    })
}

// ── the running builder ─────────────────────────────────────────────────────

#[derive(Default)]
struct Inner {
    builds: VecDeque<Build>,
    /// The `curl` of the build running now, so the owner can stop it.
    running: Option<(String, Arc<Mutex<Option<Child>>>)>,
    counter: u64,
    /// The page's report on the running build's trial: its id and what it
    /// found.
    report: Option<(String, String, TryReport)>,
}

/// The builder lososd serves `/api/builder*` from.
pub struct Builder {
    config: Option<Config>,
    inner: Arc<Mutex<Inner>>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn lock(inner: &Mutex<Inner>) -> std::sync::MutexGuard<'_, Inner> {
    inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Builder {
    #[must_use]
    pub fn new(config: Option<Config>) -> Builder {
        Builder {
            config,
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    #[must_use]
    pub fn from_env() -> Builder {
        Builder::new(Config::from_env())
    }

    /// `GET /api/builder`: whether the model is there to ask, and the recent
    /// builds. `reason` is `off` (the box runs no builder), `starting` (Ollama
    /// does not answer yet) or `downloading` (it has not got the model yet).
    #[must_use]
    pub fn view(&self) -> Value {
        let Some(config) = &self.config else {
            return json!({ "available": false, "reason": "off" });
        };
        let builds: Vec<Value> = lock(&self.inner)
            .builds
            .iter()
            .map(Build::summary)
            .collect();
        let ready = match probe(config) {
            Ok(models) => {
                if models.iter().any(|m| same_model(m, &config.model)) {
                    None
                } else {
                    Some("downloading")
                }
            }
            Err(e) => {
                tracing::debug!(error = ?e, "the builder's model does not answer");
                Some("starting")
            }
        };
        match ready {
            // A build that is running shows even while the list cannot be
            // read: Ollama answers one request at a time on a small box.
            Some(reason) if builds.iter().all(|b| b["status"] != "running") => {
                json!({ "available": false, "reason": reason, "model": config.model })
            }
            _ => json!({ "available": true, "model": config.model, "builds": builds }),
        }
    }

    /// `GET /api/builder/builds/{id}`.
    #[must_use]
    pub fn build(&self, id: &str) -> Option<Build> {
        lock(&self.inner)
            .builds
            .iter()
            .find(|b| b.id == id)
            .cloned()
    }

    /// `POST /api/builder/builds`: start a build on a thread of its own.
    ///
    /// # Errors
    /// 409 while the box has no model or a build is running already.
    pub fn start(&self, req: Request) -> Result<Build, Refused> {
        let Some(config) = self.config.clone() else {
            return refuse(409, "this box does not run the builder");
        };
        let mut inner = lock(&self.inner);
        if inner.running.is_some() {
            return refuse(
                409,
                "something is already being built; wait for it or stop it",
            );
        }
        inner.counter += 1;
        let id = format!("b{}-{}", now(), inner.counter);
        let build = Build {
            id: id.clone(),
            kind: req.kind,
            status: Status::Running,
            prompt: req.prompt.clone(),
            revision: req.base.is_some(),
            created_at: now(),
            finished_at: None,
            lines: 0,
            fault: None,
            at_limit: false,
            files: None,
            name: None,
            code: None,
            problems: Vec::new(),
            notes: None,
            fixing: false,
            steps: Vec::new(),
            trial: None,
        };
        inner.builds.push_front(build.clone());
        inner.builds.truncate(KEEP_BUILDS);
        let slot = Arc::new(Mutex::new(None));
        inner.running = Some((id.clone(), Arc::clone(&slot)));
        drop(inner);

        let shared = Arc::clone(&self.inner);
        std::thread::Builder::new()
            .name("losos-builder".into())
            .spawn(move || run(&config, &req, &id, &slot, &shared))
            .map_err(|e| {
                tracing::error!(error = ?e, "could not start a build thread");
                lock(&self.inner).running = None;
                Refused {
                    status: 500,
                    message: "the build could not be started".into(),
                }
            })?;
        Ok(build)
    }

    /// `POST /api/builder/builds/{id}/tried`: what the page found when it
    /// tried the app the build asked it to.
    ///
    /// # Errors
    /// 404 for a build the box does not know, 409 when it asks for no try
    /// or for another one.
    pub fn tried(&self, id: &str, trial: &str, report: TryReport) -> Result<Value, Refused> {
        let mut inner = lock(&self.inner);
        let Some(build) = inner.builds.iter().find(|b| b.id == id) else {
            return refuse(404, "no such build");
        };
        if build.trial.as_ref().map(|t| t.id.as_str()) != Some(trial)
            || matches!(&inner.report, Some((b, t, _)) if b == id && t == trial)
        {
            return refuse(409, "the build is not waiting for that try");
        }
        inner.report = Some((id.to_string(), trial.to_string(), report));
        Ok(json!({ "taken": true }))
    }

    /// `DELETE /api/builder/builds/{id}`: stop a running build. Stopping one
    /// that has finished is not an error; it is already stopped.
    ///
    /// # Errors
    /// 404 for a build the box does not know.
    pub fn stop(&self, id: &str) -> Result<Build, Refused> {
        let mut inner = lock(&self.inner);
        let Some(build) = inner.builds.iter_mut().find(|b| b.id == id) else {
            return refuse(404, "no such build");
        };
        if build.status == Status::Running {
            build.status = Status::Stopped;
            build.finished_at = Some(now());
        }
        let build = build.clone();
        if let Some((running, slot)) = &inner.running {
            if running == id {
                if let Some(child) = lock_child(slot).as_mut() {
                    let _ = child.kill();
                }
            }
        }
        Ok(build)
    }
}

fn lock_child(slot: &Mutex<Option<Child>>) -> std::sync::MutexGuard<'_, Option<Child>> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A model name as Ollama lists it, `llama3.2:3b`, matches the one asked for;
/// a name without a tag means `:latest`.
fn same_model(listed: &str, wanted: &str) -> bool {
    let full = |n: &str| {
        if n.contains(':') {
            n.to_string()
        } else {
            format!("{n}:latest")
        }
    };
    full(listed) == full(wanted)
}

/// The models Ollama has: `GET /api/tags`.
fn probe(config: &Config) -> anyhow::Result<Vec<String>> {
    let out = Command::new("curl")
        .args(["--silent", "--show-error", "--fail"])
        .args(["--max-time", PROBE_TIMEOUT_SECS])
        .arg(format!("{}/api/tags", config.url))
        .stdin(Stdio::null())
        .output()?;
    if !out.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let doc: Value = serde_json::from_slice(&out.stdout)?;
    Ok(doc["models"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(|m| m["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

/// One answer from the model.
struct Streamed {
    text: String,
    /// It ran into [`MAX_ANSWER_TOKENS`].
    at_limit: bool,
    /// It was cut off for repeating itself ([`looped`]).
    looped: bool,
}

/// What an answer makes: its files, the app compiled when it is one, and
/// what the checks found wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Work {
    files: Vec<WidgetFile>,
    code: Option<String>,
    problems: Vec<String>,
}

impl Work {
    /// Which of two answers is the better: one that compiles over one that
    /// does not, then the one with fewer mistakes.
    fn score(&self) -> (bool, std::cmp::Reverse<usize>) {
        (
            self.code.is_some() || self.files.iter().all(|f| f.name != APP_FILE),
            std::cmp::Reverse(self.problems.len()),
        )
    }
}

/// What an answer makes, or why it makes nothing.
type Made = Result<Work, Fault>;

fn judge(kind: Kind, answer: &Streamed) -> (Answer, Made) {
    let parsed = parse_answer(&answer.text, kind);
    let made = match kind {
        Kind::Widget => widget(&parsed).map(|files| {
            let problems = problems(&files);
            Work {
                files,
                code: None,
                problems,
            }
        }),
        Kind::App => app(&parsed).map(|file| {
            let checked = crate::jsx::check(&file.content);
            Work {
                files: vec![file],
                code: checked.code,
                problems: checked.problems,
            }
        }),
    };
    (parsed, made)
}

/// How good an answer is: one that makes something over one that does
/// not, one that compiles over one that does not, then fewer findings.
fn rank(made: &Made, findings: usize) -> (bool, bool, std::cmp::Reverse<usize>) {
    match made {
        Ok(work) => {
            let (compiles, _) = work.score();
            (true, compiles, std::cmp::Reverse(findings))
        }
        Err(_) => (false, false, std::cmp::Reverse(findings)),
    }
}

/// The best answer of a build so far, and what was found wrong with it.
struct Kept {
    answer: Streamed,
    made: Made,
    findings: Vec<String>,
}

/// One build, start to finish, on its own thread: write, check, try (an
/// app, in the page), and fix what was found, until nothing is or the fixes
/// run out ([`APP_FIXES`], [`WIDGET_FIXES`]).
fn run(
    config: &Config,
    req: &Request,
    id: &str,
    slot: &Arc<Mutex<Option<Child>>>,
    shared: &Arc<Mutex<Inner>>,
) {
    let update = |f: &mut dyn FnMut(&mut Build)| {
        let mut inner = lock(shared);
        if let Some(b) = inner.builds.iter_mut().find(|b| b.id == id) {
            f(b);
        }
    };
    let stopped = || {
        lock(shared)
            .builds
            .iter()
            .any(|b| b.id == id && b.status == Status::Stopped)
    };
    let begin = |kind: StepKind| {
        update(&mut |b| {
            b.steps.push(Step::new(kind));
            if matches!(kind, StepKind::Write | StepKind::Fix) {
                b.fixing = kind == StepKind::Fix;
                b.lines = 0;
            }
        });
    };
    let end = |problems: &[String], shown: Vec<String>, skipped: bool| {
        let mut shown = Some(shown);
        update(&mut |b| {
            if let Some(step) = b.steps.last_mut() {
                step.running = false;
                step.problems = problems.to_vec();
                step.shown = shown.take().unwrap_or_default();
                step.skipped = skipped;
            }
        });
    };
    let mut progress = |text: &str| {
        let lines = u32::try_from(text.lines().count()).unwrap_or(u32::MAX);
        update(&mut |b| b.lines = lines);
    };
    let fixes = match req.kind {
        Kind::Widget => WIDGET_FIXES,
        Kind::App => APP_FIXES,
    };

    let mut kept: Option<Kept> = None;
    let mut failure: Option<anyhow::Error> = None;
    let mut repair: Option<Repair> = None;
    for round in 0..=fixes {
        if stopped() {
            break;
        }
        begin(if round == 0 {
            StepKind::Write
        } else {
            StepKind::Fix
        });
        let answer = match stream(config, req, repair.as_ref(), slot, &mut progress) {
            Ok(answer) => answer,
            Err(e) => {
                end(&[], Vec::new(), false);
                if kept.is_none() {
                    failure = Some(e);
                } else {
                    tracing::warn!(error = ?e, "a build's fix failed");
                }
                break;
            }
        };
        end(&[], Vec::new(), false);
        if stopped() {
            break;
        }

        let (_, made) = judge(req.kind, &answer);
        begin(StepKind::Check);
        let mut findings = match &made {
            _ if answer.looped => Vec::new(),
            Ok(work) => {
                let mut found = work.problems.clone();
                let text: String = work
                    .files
                    .iter()
                    .map(|f| f.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                if let Some(example) = copied_example(req.kind, &text, &req.prompt) {
                    let noun = req.kind.noun();
                    found.push(format!(
                        "This is the example from the instructions, the {noun} for \"{example}\", not the {noun} that was asked for. Write the {noun} for what was asked."
                    ));
                }
                found
            }
            Err(Fault::NoApp) => {
                vec!["The answer has no ```jsx code block with the app in it.".to_string()]
            }
            Err(_) => Vec::new(),
        };
        end(&findings, Vec::new(), false);

        if let (
            Ok(Work {
                code: Some(code), ..
            }),
            true,
        ) = (&made, findings.is_empty() && !answer.looped)
        {
            begin(StepKind::Try);
            let trial = format!("t{round}");
            update(&mut |b| {
                b.code = Some(code.clone());
                b.trial = Some(Trial {
                    id: trial.clone(),
                    code: code.clone(),
                });
            });
            let report = wait_for_report(config, shared, id, &trial, &stopped);
            update(&mut |b| b.trial = None);
            match report {
                Some(report) => {
                    let found = try_problems(&report);
                    end(&found, report.shown, false);
                    findings.extend(found);
                }
                None => end(&[], Vec::new(), true),
            }
        }

        let better = kept
            .as_ref()
            .is_none_or(|k| rank(&made, findings.len()) >= rank(&k.made, k.findings.len()));
        let note = if answer.looped {
            Some(loop_note(req.kind))
        } else if findings.is_empty() {
            None
        } else {
            Some(repair_note(req.kind, &findings, &req.prompt))
        };
        let text = answer.text.clone();
        if better {
            kept = Some(Kept {
                answer,
                made,
                findings,
            });
        }
        // A widget answer that made no widget at all rarely does better a
        // second time; nothing found wrong needs no fix.
        let Some(note) = note else { break };
        if round == fixes {
            break;
        }
        tracing::info!(round, note = %note, "a build goes back to the model");
        repair = Some(Repair { answer: text, note });
    }

    let mut inner = lock(shared);
    inner.running = None;
    inner.report = None;
    let Some(build) = inner.builds.iter_mut().find(|b| b.id == id) else {
        return;
    };
    build.trial = None;
    if build.status == Status::Stopped {
        return;
    }
    build.finished_at = Some(now());
    build.fixing = false;
    match (kept, failure) {
        (
            Some(Kept {
                answer,
                made,
                findings,
            }),
            _,
        ) => {
            let (parsed, _) = judge(req.kind, &answer);
            build.at_limit = answer.at_limit || answer.looped;
            build.notes = parsed.summary.clone();
            build.name = parsed.name.clone();
            match made {
                Ok(work) => {
                    if !findings.is_empty() {
                        tracing::info!(problems = ?findings, "a build kept work with mistakes");
                    }
                    build.status = Status::Done;
                    build.files = Some(work.files);
                    build.code = work.code;
                    build.problems = findings;
                }
                Err(fault) => {
                    tracing::info!(?fault, "a build made nothing");
                    build.status = Status::Failed;
                    build.fault = Some(fault);
                    build.code = None;
                }
            }
        }
        (None, Some(e)) => {
            tracing::warn!(error = ?e, "a build failed");
            build.status = Status::Failed;
            // Ollama says "model requires more system memory (…) than is
            // available (…)" when the box has too little free.
            build.fault = Some(if format!("{e:#}").contains("memory") {
                Fault::Memory
            } else {
                Fault::Upstream
            });
        }
        (None, None) => {
            build.status = Status::Failed;
            build.fault = Some(Fault::Upstream);
        }
    }
}

/// Wait for the page's report on `trial`, until [`Config::try_wait`] runs
/// out or the build is stopped.
fn wait_for_report(
    config: &Config,
    shared: &Mutex<Inner>,
    id: &str,
    trial: &str,
    stopped: &dyn Fn() -> bool,
) -> Option<TryReport> {
    let deadline = Instant::now() + config.try_wait;
    loop {
        {
            let mut inner = lock(shared);
            if matches!(&inner.report, Some((b, t, _)) if b == id && t == trial) {
                return inner.report.take().map(|(_, _, r)| r);
            }
        }
        if stopped() || Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Ask the model and read its answer as it comes, calling `progress` with the
/// text so far. An answer that starts repeating itself is cut off there.
fn stream(
    config: &Config,
    req: &Request,
    repair: Option<&Repair>,
    slot: &Mutex<Option<Child>>,
    progress: &mut dyn FnMut(&str),
) -> anyhow::Result<Streamed> {
    let body = chat_body(config, req, repair).to_string();
    let mut child = Command::new("curl")
        .args(["--silent", "--show-error", "--no-buffer"])
        .args(["--max-time", &BUILD_TIMEOUT_SECS.to_string()])
        .args(["--header", "Content-Type: application/json"])
        .args(["--data-binary", "@-"])
        .arg(format!("{}/api/chat", config.url))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Ollama reads the whole body before it answers, so this cannot block on
    // a full pipe in the other direction.
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(body.as_bytes())?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("curl has no stdout"))?;
    *lock_child(slot) = Some(child);

    let mut text = String::new();
    let mut at_limit = false;
    let mut done = false;
    let mut cut = false;
    let mut error = None;
    for line in BufReader::new(stdout).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(chunk) = serde_json::from_str::<Value>(&line) else {
            error = Some(format!(
                "not a chunk: {}",
                line.chars().take(200).collect::<String>()
            ));
            continue;
        };
        if let Some(e) = chunk["error"].as_str() {
            error = Some(e.to_string());
            continue;
        }
        if let Some(piece) = chunk["message"]["content"].as_str() {
            text.push_str(piece);
            progress(&text);
            if piece.contains('\n') && looped(&text) {
                cut = true;
                if let Some(child) = lock_child(slot).as_mut() {
                    let _ = child.kill();
                }
                break;
            }
        }
        if chunk["done"].as_bool() == Some(true) {
            done = true;
            at_limit = chunk["done_reason"].as_str() == Some("length");
            tracing::info!(
                prompt_tokens = chunk["prompt_eval_count"].as_u64(),
                answer_tokens = chunk["eval_count"].as_u64(),
                "a build finished"
            );
        }
    }
    let child = lock_child(slot).take();
    if let Some(mut child) = child {
        let status = child.wait()?;
        if !status.success() && !done && !cut {
            let mut stderr = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = std::io::Read::read_to_string(&mut e, &mut stderr);
            }
            anyhow::bail!("curl: {status}: {}", stderr.trim());
        }
    }
    if let Some(e) = error {
        anyhow::bail!("ollama: {e}");
    }
    if !done && !cut {
        anyhow::bail!("the answer ended before the model said it was done");
    }
    Ok(Streamed {
        text,
        at_limit,
        looped: cut,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(prompt: &str) -> Value {
        json!({ "prompt": prompt })
    }

    #[test]
    fn a_try_names_what_the_app_did_wrong_and_nothing_when_it_worked() {
        let tried = |shown: &[&str], kept: &[&str], errors: &[&str], press: &[&str]| {
            try_problems(&TryReport {
                shown: shown.iter().map(|s| (*s).to_string()).collect(),
                pressed: "Add".into(),
                kept: kept.iter().map(|s| (*s).to_string()).collect(),
                errors: errors.iter().map(|s| (*s).to_string()).collect(),
                threw_on_press: press.iter().map(|s| (*s).to_string()).collect(),
                ..TryReport::default()
            })
        };
        let form = ["h1: Notes", "input[text] \"Title\"", "button: Add"];
        assert_eq!(
            tried(&form, &["notes: 0 to 1"], &[], &[]),
            Vec::<String>::new()
        );
        assert!(tried(&form, &[], &[], &[])[0].contains("kept nothing"));
        let empty_choice = try_problems(&TryReport {
            shown: vec![
                "select \"Child\" options: Ann, Bob".into(),
                "button: Add".into(),
            ],
            pressed: "Add".into(),
            chose: vec!["Child".into()],
            kept_after_choosing: vec!["chores: 0 to 1".into()],
            ..TryReport::default()
        });
        assert_eq!(empty_choice.len(), 1);
        assert!(
            empty_choice[0].starts_with(
                "With \"Child\" left at its first choice, pressing \"Add\" keeps nothing"
            ),
            "{empty_choice:?}"
        );
        // A start-up error is all that is said: nothing else was reached.
        let p = tried(&form, &[], &["x is not defined"], &[]);
        assert_eq!(p, ["When the app starts, it stops with: x is not defined"]);
        assert_eq!(
            tried(&form, &[], &[], &["boom"]),
            ["Pressing \"Add\" stops the app with: boom"]
        );
        // A timer has no field to fill in, so keeping nothing is no mistake;
        // nor is a checkbox a field.
        assert_eq!(
            tried(&["h1: Timer", "button: Add"], &[], &[], &[]),
            Vec::<String>::new()
        );
        assert_eq!(
            tried(&["input[checkbox] \"Done\"", "button: Add"], &[], &[], &[]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_example_sent_back_is_named_unless_it_was_asked_for() {
        let turns = prompt_messages(APP_PROMPT);
        let disk = turns.last().unwrap()["content"].as_str().unwrap();
        // A fix for a chore chart that came back as the disk example, as
        // the small model once sent it.
        assert_eq!(
            copied_example(Kind::App, disk, "A chore chart for the kids").as_deref(),
            Some("Show how full the disk is.")
        );
        assert_eq!(
            copied_example(Kind::App, disk, "How full is the disk?"),
            None
        );
        let chores = "function App() {\n  const chores = useCollection(\"chores\");\n  const [task, setTask] = useState(\"\");\n  return (\n    <Page title=\"Chores\">\n      <Input label=\"Task\" value={task} onChange={setTask} />\n      <Button onClick={() => chores.add({ task })}>Add</Button>\n    </Page>\n  );\n}";
        assert_eq!(
            copied_example(Kind::App, chores, "A chore chart for the kids"),
            None
        );
    }

    #[test]
    fn a_try_report_is_cut_to_size() {
        let long = "x".repeat(500);
        let many: Vec<String> = (0..100).map(|i| format!("li: {i}")).collect();
        let (trial, r) = parse_report(
            &json!({ "trial": "t1", "shown": many, "pressed": long, "errors": [1, "a\u{7}b"] }),
        )
        .unwrap();
        assert_eq!(trial, "t1");
        assert_eq!(r.shown.len(), MAX_REPORT_LINES);
        assert_eq!(r.pressed.chars().count(), MAX_REPORT_CHARS);
        assert_eq!(r.errors, ["ab"]);
        assert!(parse_report(&json!({ "shown": [] })).is_err());
    }

    #[test]
    fn a_request_is_a_description_and_maybe_a_widget_to_change() {
        let r = parse_request(&json!({ "prompt": "  a clock \n", "lang": "sk" })).unwrap();
        assert_eq!(r.prompt, "a clock");
        assert_eq!(r.lang, "sk");
        assert_eq!(r.base, None);
        assert_eq!(parse_request(&req("x")).unwrap().lang, "en");

        let r = parse_request(&json!({ "prompt": "bigger", "base": "<p>hi</p>" })).unwrap();
        assert_eq!(r.base, Some(vec![WidgetFile::entry("<p>hi</p>")]));
        let files = json!([{ "name": "index.html", "content": "<p>x</p>" }, { "name": "app.js", "content": "1" }]);
        let r = parse_request(&json!({ "prompt": "bigger", "base": files })).unwrap();
        assert_eq!(r.base.unwrap().len(), 2);
    }

    #[test]
    fn a_request_the_builder_would_not_act_on_is_refused_with_a_sentence() {
        let refused = |doc: Value| parse_request(&doc).unwrap_err().message;
        assert_eq!(refused(json!({})), "describe the widget you want");
        assert_eq!(
            refused(json!({ "kind": "app" })),
            "describe the app you want"
        );
        assert!(refused(json!({ "kind": "game", "prompt": "x" })).contains("widgets and apps"));
        assert!(
            refused(json!({ "kind": "app", "prompt": "x", "base": [] })).contains("not its source")
        );
        assert_eq!(refused(req("   ")), "describe the widget you want");
        assert!(refused(req(&"x".repeat(MAX_PROMPT_CHARS + 1))).contains("longer than"));
        assert!(refused(req("a\u{7}b")).contains("control character"));
        assert!(refused(json!({ "prompt": "x", "lang": "fr" })).contains("language"));
        assert!(refused(
            json!({ "prompt": "x", "base": [{ "name": "../x.html", "content": "a" }] })
        )
        .contains("not a file name"));
        let big = "x".repeat(MAX_BASE_BYTES + 1);
        assert!(refused(json!({ "prompt": "x", "base": big })).contains("too large"));
        assert!(refused(json!({ "prompt": "x", "base": 3 })).contains("list of files"));
    }

    #[test]
    fn the_model_reads_the_request_and_the_widget_file_by_file() {
        let r = Request {
            kind: Kind::Widget,
            prompt: "show the disk".into(),
            base: Some(vec![
                WidgetFile::entry("<div></div>"),
                WidgetFile {
                    name: "app.js".into(),
                    content: "go()".into(),
                },
            ]),
            lang: "de".into(),
        };
        let text = user_message(&r);
        assert!(
            text.starts_with("show the disk\n\nChange this widget"),
            "{text}"
        );
        assert!(
            text.contains("File: index.html\n```html\n<div></div>\n```\n"),
            "{text}"
        );
        assert!(text.contains("File: app.js\n```js\ngo()\n```\n"), "{text}");
        assert!(text.ends_with("\nWrite the summary in German."), "{text}");

        let config = Config {
            url: "http://127.0.0.1:11434".into(),
            model: DEFAULT_MODEL.into(),
            context: 8192,
            try_wait: Duration::from_secs(1),
        };
        let body = chat_body(&config, &r, None);
        assert_eq!(body["model"], "qwen2.5-coder:1.5b");
        assert_eq!(body["stream"], true);
        assert_eq!(body["options"]["num_ctx"], 8192);
        assert_eq!(body["options"]["num_predict"], MAX_ANSWER_TOKENS);
        let messages = body["messages"].as_array().unwrap();
        // The system turn, three worked examples, the request.
        assert_eq!(messages.len(), 8);
        assert_eq!(messages[0]["role"], "system");
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .starts_with("You write widgets"));
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert!(messages[2]["content"]
            .as_str()
            .unwrap()
            .starts_with("```html\n"));
        assert_eq!(messages[7]["content"], text);
    }

    #[test]
    fn an_app_request_is_its_own_prompt_and_its_source_to_change() {
        let r = parse_request(&json!({ "kind": "app", "prompt": "a to-do list", "base": "function App() {}", "lang": "sk" })).unwrap();
        assert_eq!(r.kind, Kind::App);
        assert_eq!(
            r.base,
            Some(vec![WidgetFile {
                name: APP_FILE.into(),
                content: "function App() {}".into()
            }])
        );
        let text = user_message(&r);
        assert!(text.contains("Change this app"), "{text}");
        assert!(text.contains("```jsx\nfunction App() {}\n```\n"), "{text}");
        assert!(!text.contains("File:"), "one file needs no name: {text}");
        assert!(
            text.ends_with("Write the app's words and the summary in Slovak."),
            "{text}"
        );
        let config = Config {
            url: "http://127.0.0.1:1".into(),
            model: "m".into(),
            context: 1,
            try_wait: Duration::from_secs(1),
        };
        let messages = chat_body(&config, &r, None)["messages"]
            .as_array()
            .unwrap()
            .clone();
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .starts_with("You write small apps"));
        assert!(messages[2]["content"]
            .as_str()
            .unwrap()
            .starts_with("```jsx\n"));
    }

    #[test]
    fn every_worked_example_is_an_answer_the_box_would_take() {
        for (kind, prompt) in [(Kind::Widget, WIDGET_PROMPT), (Kind::App, APP_PROMPT)] {
            let messages = prompt_messages(prompt);
            let examples: Vec<&Value> = messages
                .iter()
                .filter(|m| m["role"] == "assistant")
                .collect();
            assert_eq!(examples.len(), 3, "{kind:?}");
            for example in examples {
                let answer = parse_answer(example["content"].as_str().unwrap(), kind);
                assert!(answer.summary.is_some(), "{kind:?}: {answer:?}");
                match kind {
                    Kind::Widget => {
                        let files = widget(&answer).unwrap();
                        assert_eq!(problems(&files), Vec::<String>::new());
                    }
                    Kind::App => {
                        assert!(answer.name.is_some());
                        let checked = crate::jsx::check(&app(&answer).unwrap().content);
                        assert!(checked.ok(), "{:?}", checked.problems);
                    }
                }
            }
            for user in messages.iter().filter(|m| m["role"] == "user") {
                assert!(
                    user["content"].as_str().unwrap().ends_with("in English."),
                    "{user}"
                );
            }
        }
    }

    #[test]
    fn an_answer_is_read_into_files_a_name_and_a_summary() {
        let a = parse_answer(
            "Here it is.\n```html\n<p id=a></p>\n<script>x();</script>\n```\nSummary: A  thing.\n",
            Kind::Widget,
        );
        assert_eq!(
            a.files,
            vec![WidgetFile::entry("<p id=a></p>\n<script>x();</script>")]
        );
        assert_eq!(a.summary.as_deref(), Some("A thing."));
        assert_eq!(a.name, None);

        let a = parse_answer(
            "```jsx\nfunction App() {}\n```\n**Name:** \"Shopping list\"\n**Summary:** A list.",
            Kind::App,
        );
        assert_eq!(
            a.files,
            vec![WidgetFile {
                name: APP_FILE.into(),
                content: "function App() {}".into()
            }]
        );
        assert_eq!(a.name.as_deref(), Some("Shopping list"));
        assert_eq!(a.summary.as_deref(), Some("A list."));
    }

    #[test]
    fn an_answer_that_drifts_from_the_shape_is_read_where_its_meaning_is_plain() {
        // Cut off by the token limit: the block is kept to where it stops.
        let a = parse_answer("```html\n<p>half", Kind::Widget);
        assert_eq!(a.files, vec![WidgetFile::entry("<p>half")]);
        // Named blocks, and a name given twice.
        let a = parse_answer(
            "File: index.html\n```html\n<b>1</b>\n```\nFile: `style.css`\n```css\nb{}\n```\n```html\n<b>2</b>\n```",
            Kind::Widget,
        );
        assert_eq!(a.files.len(), 2);
        assert_eq!(a.files[0], WidgetFile::entry("<b>2</b>"));
        assert_eq!(a.files[1].name, "style.css");
        // A fence of another language is not the widget.
        assert_eq!(
            parse_answer("```python\nprint()\n```", Kind::Widget).files,
            vec![]
        );
        // An app in a fence of plain JavaScript, or one with no language.
        assert_eq!(
            parse_answer("```javascript\nfunction App(){}\n```", Kind::App).files[0].name,
            APP_FILE
        );
        assert_eq!(
            parse_answer("```\nfunction App(){}\n```", Kind::App).files[0].name,
            APP_FILE
        );
        // The shape the builder once asked for.
        let a = parse_answer(
            "<file name='index.html'>\n```html\n<b>1</b>\n```\n</file>\n<summary>Old.</summary>",
            Kind::Widget,
        );
        assert_eq!(a.files, vec![WidgetFile::entry("<b>1</b>")]);
        assert_eq!(a.summary.as_deref(), Some("Old."));
        assert_eq!(
            parse_answer("I cannot help with that.", Kind::App).files,
            vec![]
        );
        assert_eq!(
            app(&parse_answer("I cannot help with that.", Kind::App)),
            Err(Fault::NoApp)
        );
    }

    #[test]
    fn a_widget_is_what_the_box_would_keep_or_a_fault() {
        let file = |name: &str, content: &str| WidgetFile {
            name: name.into(),
            content: content.into(),
        };
        let answer = |files: Vec<WidgetFile>| Answer {
            files,
            name: None,
            summary: None,
        };
        let w = widget(&answer(vec![
            file("style.css", "a{}"),
            file("README.md", "dropped"),
            file("index.html", "<p></p>"),
        ]))
        .unwrap();
        assert_eq!(
            w.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            ["index.html", "style.css"]
        );
        assert_eq!(
            widget(&answer(vec![file("app.js", "1")])),
            Err(Fault::NoWidget)
        );
        assert_eq!(
            widget(&answer(vec![file("index.html", "  ")])),
            Err(Fault::NoWidget)
        );
        let many: Vec<WidgetFile> = std::iter::once(file("index.html", "x"))
            .chain((0..MAX_FILES).map(|i| file(&format!("f{i}.js"), "1")))
            .collect();
        assert_eq!(widget(&answer(many)), Err(Fault::Unusable));
    }

    #[test]
    fn a_summary_is_one_short_line() {
        assert_eq!(clean_summary(" a\n\tb "), Some("a b".into()));
        assert_eq!(clean_summary("  "), None);
        let long = clean_summary(&"word ".repeat(400)).unwrap();
        assert_eq!(long.chars().count(), MAX_SUMMARY_CHARS);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn a_model_without_a_tag_is_its_latest() {
        assert!(same_model("qwen2.5-coder:1.5b", "qwen2.5-coder:1.5b"));
        assert!(same_model("qwen:latest", "qwen"));
        assert!(!same_model("llama3.2:1b", "llama3.2:3b"));
    }

    #[test]
    fn a_box_without_the_builder_says_so() {
        let b = Builder::new(None);
        assert_eq!(b.view(), json!({ "available": false, "reason": "off" }));
        let r = parse_request(&req("a clock")).unwrap();
        assert_eq!(b.start(r).unwrap_err().status, 409);
        assert_eq!(b.stop("b1-1").unwrap_err().status, 404);
    }

    #[test]
    fn the_checks_name_the_mistakes_a_small_model_makes_most() {
        let fine = vec![WidgetFile::entry(
            "<p id=\"t\"></p><script>var r = document.createElement('i'); r.id = 'x';\ndocument.getElementById(\"t\").textContent = losos.lang; document.getElementById('x'); losos.metric(\"storage.bytes\"); losos.resize();</script>",
        )];
        assert_eq!(problems(&fine), Vec::<String>::new());

        let wrong = vec![
            WidgetFile::entry("<span id=\"up\"></span><script src=\"app.js\"></script>"),
            WidgetFile {
                name: "app.js".into(),
                content: "document.getElementById(\"apps\").textContent = 1;\ndocument.getElementById(\"apps\");\nlosos.theme.on('x'); losos.data(); document.resize();".into(),
            },
        ];
        let found = problems(&wrong);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found[0].contains("getElementById(\"apps\")"));
        assert!(found[1].contains("losos.data"));
        assert!(found[2].contains("losos.resize()"));
        // `myLosos.x` and `"losos.example"` in a string are not the bridge.
        let other = vec![WidgetFile::entry(
            "<script>var myLosos = {}; myLosos.x = 1;</script>",
        )];
        assert!(problems(&other).is_empty());
    }

    #[test]
    fn an_answer_going_round_in_circles_is_seen() {
        let lines = "<span></span>\n".repeat(LOOP_LINES);
        assert!(looped(&format!("```html\n{lines}")));
        assert!(!looped(&"<span></span>\n".repeat(LOOP_LINES - 1)));
        assert!(!looped(ANSWER_WITH_BRACES));
        assert!(!looped(""));
    }

    const ANSWER_WITH_BRACES: &str = "a\n}\n}\n}\nb\n}\n}\n";

    #[test]
    fn a_second_turn_shows_the_model_its_answer_and_what_is_wrong() {
        let config = Config {
            url: "http://127.0.0.1:11434".into(),
            model: DEFAULT_MODEL.into(),
            context: DEFAULT_CONTEXT,
            try_wait: Duration::from_secs(1),
        };
        let r = parse_request(&req("a clock")).unwrap();
        let repair = Repair {
            answer: "x".repeat(MAX_REPAIR_CHARS + 50),
            note: repair_note(Kind::Widget, &["one".into()], "a clock"),
        };
        let body = chat_body(&config, &r, Some(&repair));
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 10);
        assert_eq!(messages[8]["role"], "assistant");
        assert_eq!(
            messages[8]["content"].as_str().unwrap().len(),
            MAX_REPAIR_CHARS
        );
        assert_eq!(
            messages[9]["content"],
            "Your widget has these mistakes:\n- one\nSend the whole widget again with them fixed, in the same shape: one ```html code block, then the Summary: line. It is still the widget for: a clock"
        );
        assert!(repair_note(Kind::App, &["two".into()], " a list\n").ends_with("one ```jsx code block, then the Name: and Summary: lines. It is still the app for: a list"));
        assert!(loop_note(Kind::App).contains(".map"));
    }
}
