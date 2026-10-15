//! The widget builder: a language model on this box writes a widget from the
//! owner's description.
//!
//! A hand-written widget (`crate::look`) is HTML, style and script that the
//! admin page draws in a sandboxed frame. Writing one takes some knowledge of
//! that frame, so the builder lets the owner describe the widget in a sentence
//! and has a model write it. The result is text and nothing more: it lands in
//! the same editor and the same frame as a widget typed by hand, with no
//! privilege a typed one lacks, and nothing is saved until the owner saves it.
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
//! `builder-prompt.md`, the widget contract written for a small model: the
//! answer's shape, the frame's limits, every reading `losos.metric` serves,
//! two worked examples and the mistakes such models make most. Its size and
//! [`MAX_ANSWER_TOKENS`] set the context window (`LOSOS_BUILDER_CONTEXT`); a
//! widget to change has to fit in what is left, which is why a change has
//! its own, smaller limit ([`MAX_BASE_BYTES`]).
//!
//! # Checking the answer
//!
//! A 3B model gets the shape right far more often than the details. Before a
//! widget goes to the editor, [`problems`] looks for the mistakes it makes
//! most: a script that looks up an element the markup does not have, and a
//! `losos` member that does not exist. A model that repeats one line until
//! it runs out of room is cut off as soon as it does ([`looped`]). Either
//! way the model gets one more turn, with the problem named, and the better
//! of the two answers is kept.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{json, Value};

use crate::look::{is_file_name, validate_files, WidgetFile, ENTRY_FILE, MAX_FILES};

/// What the model is told before every request.
pub const PROMPT: &str = include_str!("builder-prompt.md");
/// The model Ollama serves when the configuration names none.
pub const DEFAULT_MODEL: &str = "llama3.2:3b";
/// The context window when the configuration names none, in tokens. The
/// prompt is about 3,000, an answer at most [`MAX_ANSWER_TOKENS`], and the
/// rest is the request and the widget to change.
pub const DEFAULT_CONTEXT: u32 = 8192;
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
/// What the frame's `losos` object has (`admin-ui/app/public/widget-frame/`).
const LOSOS_MEMBERS: [&str; 8] = [
    "metric", "lang", "resize", "theme", "onTheme", "files", "file", "asset",
];

/// Where the model is: `LOSOS_BUILDER_URL` (Ollama's address),
/// `LOSOS_BUILDER_MODEL` and `LOSOS_BUILDER_CONTEXT`, set by
/// `modules/builder.nix`. No URL, no builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub url: String,
    pub model: String,
    pub context: u32,
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
        Some(Config {
            url,
            model: model.trim().to_string(),
            context,
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
    /// It had files the box would not keep: badly named, too many, too large.
    Unusable,
    /// The model did not answer.
    Upstream,
    /// Ollama would not load the model: the box has too little free memory.
    Memory,
}

/// One build, as the page sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Build {
    pub id: String,
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
    /// What the model says it made.
    pub notes: Option<String>,
    /// The model is writing a second answer, fixing what the checks found
    /// wrong with the first.
    pub fixing: bool,
}

impl Build {
    /// The build without its files, for the list.
    fn summary(&self) -> Value {
        json!({
            "id": self.id,
            "status": self.status,
            "prompt": self.prompt,
            "revision": self.revision,
            "created_at": self.created_at,
            "finished_at": self.finished_at,
            "lines": self.lines,
            "fixing": self.fixing,
            "fault": self.fault,
            "has_source": self.files.is_some(),
        })
    }
}

/// A checked request to start a build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub prompt: String,
    pub base: Option<Vec<WidgetFile>>,
    pub lang: String,
}

/// Read `{"prompt", "base"?, "lang"?}` and check it: a description of
/// 1 to [`MAX_PROMPT_CHARS`] characters with no control characters but
/// newlines and tabs, a widget to change that the box would keep and that
/// fits [`MAX_BASE_BYTES`], and one of [`LANGS`].
///
/// # Errors
/// A sentence for a 400.
pub fn parse_request(doc: &Value) -> Result<Request, Refused> {
    let Some(prompt) = doc.get("prompt").and_then(Value::as_str) else {
        return refuse(400, "describe the widget you want");
    };
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return refuse(400, "describe the widget you want");
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
    let base = match doc.get("base") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.trim().is_empty() => None,
        Some(Value::String(s)) => Some(vec![WidgetFile::entry(s)]),
        Some(files) => match crate::look::parse_files(files) {
            Some(files) => Some(files),
            None => return refuse(400, "the widget to change is not a list of files"),
        },
    };
    if let Some(files) = &base {
        if let Err(why) = validate_files(files) {
            return refuse(400, why.0);
        }
        let bytes: usize = files.iter().map(|f| f.content.len()).sum();
        if bytes > MAX_BASE_BYTES {
            return refuse(
                400,
                format!(
                    "this widget is too large for the builder to change; it changes widgets of up to {} KiB",
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

/// The request as the model reads it: the owner's words, quoted, and the
/// widget to change when there is one, file by file in the shape the model
/// answers in.
#[must_use]
pub fn user_message(req: &Request) -> String {
    let lang = language(&req.lang);
    let mut text = format!("<request>\n{}\n</request>\n", req.prompt);
    if let Some(base) = &req.base {
        text.push_str(
            "\nChange this widget, the one the owner has now, and keep what the request does not ask to change. Answer with every file of the changed widget.\n\n",
        );
        for file in base {
            // The names are checked (`is_file_name`), so they need no quoting.
            text.push_str(&format!(
                "<file name=\"{}\">\n{}\n</file>\n",
                file.name, file.content
            ));
        }
    }
    text.push_str(&format!("\nWrite the summary in {lang}.\n"));
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
    let mut messages = vec![
        json!({ "role": "system", "content": PROMPT }),
        json!({ "role": "user", "content": user_message(req) }),
    ];
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
    pub summary: Option<String>,
}

/// Read an answer: `<file name="…">…</file>` blocks and a `<summary>`. A
/// small model drifts from the shape now and then, so this is forgiving
/// where the meaning is plain: a file the answer ran out before closing is
/// kept to where it stops, and an answer with no file blocks but one fenced
/// HTML block is read as that `index.html`.
#[must_use]
pub fn parse_answer(text: &str) -> Answer {
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
            let content = strip_fence(content);
            match files.iter_mut().find(|f| f.name == name) {
                // A name given twice: the later one is the model's correction.
                Some(same) => same.content = content.to_string(),
                None => files.push(WidgetFile {
                    name: name.to_string(),
                    content: content.to_string(),
                }),
            }
        }
        rest = next;
    }
    if files.is_empty() {
        if let Some(html) = fenced(text) {
            files.push(WidgetFile::entry(html));
        }
    }
    let summary = text
        .split_once("<summary>")
        .map(|(_, s)| s.split("</summary>").next().unwrap_or(s))
        .and_then(clean_summary);
    Answer { files, summary }
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

fn fenced(text: &str) -> Option<&str> {
    let (_, after) = text.split_once("```html")?;
    let after = after.strip_prefix('\n').unwrap_or(after);
    Some(after.split("```").next().unwrap_or(after).trim_end())
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
const LOOP_NOTE: &str = "Your answer repeated the same line over and over and was cut off. Write the whole widget again, short: make repeated elements in the script with a for loop, never one line per element. Same shape: every file in a <file> block, then <summary>.";

fn repair_note(problems: &[String]) -> String {
    let mut note = String::from("Your widget has these mistakes:\n");
    for p in problems {
        note.push_str("- ");
        note.push_str(p);
        note.push('\n');
    }
    note.push_str("Send the whole widget again with them fixed, in the same shape: every file in a <file> block, then <summary>.");
    note
}

// ── the running builder ─────────────────────────────────────────────────────

#[derive(Default)]
struct Inner {
    builds: VecDeque<Build>,
    /// The `curl` of the build running now, so the owner can stop it.
    running: Option<(String, Arc<Mutex<Option<Child>>>)>,
    counter: u64,
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
            return refuse(409, "this box does not run the widget builder");
        };
        let mut inner = lock(&self.inner);
        if inner.running.is_some() {
            return refuse(
                409,
                "a widget is already being built; wait for it or stop it",
            );
        }
        inner.counter += 1;
        let id = format!("b{}-{}", now(), inner.counter);
        let build = Build {
            id: id.clone(),
            status: Status::Running,
            prompt: req.prompt.clone(),
            revision: req.base.is_some(),
            created_at: now(),
            finished_at: None,
            lines: 0,
            fault: None,
            at_limit: false,
            files: None,
            notes: None,
            fixing: false,
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

/// What an answer makes: its widget and what the checks found wrong with it,
/// or why it makes none.
/// A widget's files and the mistakes [`problems`] found in them.
type Made = Result<(Vec<WidgetFile>, Vec<String>), Fault>;

fn judge(answer: &Streamed) -> (Answer, Made) {
    let parsed = parse_answer(&answer.text);
    let made = widget(&parsed).map(|files| {
        let found = problems(&files);
        (files, found)
    });
    (parsed, made)
}

/// One build, start to finish, on its own thread: an answer, and a second
/// turn when the first repeated itself or the checks found mistakes in it.
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
    let mut progress = |text: &str| {
        let lines = u32::try_from(text.lines().count()).unwrap_or(u32::MAX);
        update(&mut |b| b.lines = lines);
    };

    let outcome = stream(config, req, None, slot, &mut progress).map(|first| {
        let (_, made) = judge(&first);
        let note = match &made {
            _ if first.looped => LOOP_NOTE.to_string(),
            Ok((_, found)) if !found.is_empty() => repair_note(found),
            _ => return first,
        };
        if stopped() {
            return first;
        }
        tracing::info!(note = %note, "a build gets a second turn");
        update(&mut |b| {
            b.fixing = true;
            b.lines = 0;
        });
        let repair = Repair {
            answer: first.text.clone(),
            note,
        };
        match stream(config, req, Some(&repair), slot, &mut progress) {
            Ok(second) => {
                let better = match (&made, judge(&second).1) {
                    // The first made a widget and the second none, or one
                    // with more mistakes: keep the first.
                    (Ok(_), Err(_)) => false,
                    (Ok((_, a)), Ok((_, b))) => b.len() <= a.len(),
                    (Err(_), _) => true,
                };
                if better {
                    second
                } else {
                    first
                }
            }
            Err(e) => {
                tracing::warn!(error = ?e, "a build's second turn failed");
                first
            }
        }
    });

    let mut inner = lock(shared);
    inner.running = None;
    let Some(build) = inner.builds.iter_mut().find(|b| b.id == id) else {
        return;
    };
    if build.status == Status::Stopped {
        return;
    }
    build.finished_at = Some(now());
    build.fixing = false;
    match outcome {
        Ok(answer) => {
            let (parsed, made) = judge(&answer);
            build.at_limit = answer.at_limit || answer.looped;
            build.notes = parsed.summary.clone();
            match made {
                Ok((files, found)) => {
                    if !found.is_empty() {
                        tracing::info!(problems = ?found, "a build kept a widget with mistakes");
                    }
                    build.status = Status::Done;
                    build.files = Some(files);
                }
                Err(fault) => {
                    tracing::info!(?fault, "a build made no widget");
                    build.status = Status::Failed;
                    build.fault = Some(fault);
                }
            }
        }
        Err(e) => {
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
    fn the_model_reads_the_request_quoted_and_the_widget_file_by_file() {
        let r = Request {
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
            text.starts_with("<request>\nshow the disk\n</request>\n"),
            "{text}"
        );
        assert!(text.contains("<file name=\"index.html\">\n<div></div>\n</file>\n"));
        assert!(text.contains("<file name=\"app.js\">\ngo()\n</file>\n"));
        assert!(text.ends_with("Write the summary in German.\n"));

        let config = Config {
            url: "http://127.0.0.1:11434".into(),
            model: DEFAULT_MODEL.into(),
            context: 8192,
        };
        let body = chat_body(&config, &r, None);
        assert_eq!(body["model"], "llama3.2:3b");
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["content"], PROMPT);
        assert_eq!(body["options"]["num_ctx"], 8192);
        assert_eq!(body["options"]["num_predict"], MAX_ANSWER_TOKENS);
    }

    #[test]
    fn an_answer_is_read_into_files_and_a_summary() {
        let a = parse_answer(
            "Here it is.\n<file name=\"index.html\">\n<p id=a></p>\n<script src=\"app.js\"></script>\n</file>\n<file name=\"app.js\">\nx();\n</file>\n<summary>A  thing.\n</summary>",
        );
        assert_eq!(a.files.len(), 2);
        assert_eq!(
            a.files[0].content,
            "<p id=a></p>\n<script src=\"app.js\"></script>"
        );
        assert_eq!(
            a.files[1],
            WidgetFile {
                name: "app.js".into(),
                content: "x();".into()
            }
        );
        assert_eq!(a.summary.as_deref(), Some("A thing."));
    }

    #[test]
    fn an_answer_that_drifts_from_the_shape_is_read_where_its_meaning_is_plain() {
        // Cut off by the token limit: the file is kept to where it stops.
        let a = parse_answer("<file name=\"index.html\">\n<p>half");
        assert_eq!(a.files, vec![WidgetFile::entry("<p>half")]);
        // A fence inside the block, single quotes, a name given twice.
        let a = parse_answer(
            "<file name='index.html'>\n```html\n<b>1</b>\n```\n</file>\n<file name=\"index.html\">\n<b>2</b>\n</file>",
        );
        assert_eq!(a.files, vec![WidgetFile::entry("<b>2</b>")]);
        let a = parse_answer("<file name='index.html'>\n```html\n<b>1</b>\n```\n</file>");
        assert_eq!(a.files, vec![WidgetFile::entry("<b>1</b>")]);
        // No blocks at all, one fenced HTML block.
        let a = parse_answer("Sure!\n```html\n<i>x</i>\n```\nEnjoy.");
        assert_eq!(a.files, vec![WidgetFile::entry("<i>x</i>")]);
        assert_eq!(parse_answer("I cannot help with that.").files, vec![]);
    }

    #[test]
    fn a_widget_is_what_the_box_would_keep_or_a_fault() {
        let file = |name: &str, content: &str| WidgetFile {
            name: name.into(),
            content: content.into(),
        };
        let answer = |files: Vec<WidgetFile>| Answer {
            files,
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
        assert!(same_model("llama3.2:3b", "llama3.2:3b"));
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
        assert!(looped(&format!("<file name=\"index.html\">\n{lines}")));
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
        };
        let r = parse_request(&req("a clock")).unwrap();
        let repair = Repair {
            answer: "x".repeat(MAX_REPAIR_CHARS + 50),
            note: repair_note(&["one".into()]),
        };
        let body = chat_body(&config, &r, Some(&repair));
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(
            messages[2]["content"].as_str().unwrap().len(),
            MAX_REPAIR_CHARS
        );
        assert_eq!(
            messages[3]["content"],
            "Your widget has these mistakes:\n- one\nSend the whole widget again with them fixed, in the same shape: every file in a <file> block, then <summary>."
        );
    }
}
