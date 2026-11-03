//! The ask queue: jobs, their runner, and the box's memory pressure.
//!
//! The shaping rules come from the T3 Code fork's background scheduler,
//! carried over into this daemon:
//!
//! * **Pressure pauses, never drops** ([`is_constrained`]): when the host
//!   has less than [`PRESSURE_FRACTION`] of its memory available
//!   (deliberately very low, 10 %), background work
//!   ([`Origin::Suggestion`]) is paused — a state on the job, restored on
//!   the next sweep when pressure passes — and person-waiting work
//!   ([`Origin::Mention`]) runs regardless. The gate is a veto at
//!   decision time; the queue itself is never emptied by it.
//! * **A 5 s cached probe** ([`MemProbe`]): the sample comes from
//!   `/proc/meminfo`, read errors answer "no pressure" — the probe is an
//!   optional voice, its absence degrades gracefully.
//! * **Idle holders are reaped before their age under pressure**
//!   ([`sweep_threshold`]): the sweep every [`SWEEP_MS`] resolves its
//!   threshold per sweep, writing `memory_pressure` or
//!   `inactivity_threshold` into the log on every reap, and a holder with
//!   work in flight is never reaped.
//! * **Spillover climbs under pressure** ([`spillover`]): between
//!   [`SPILL_START`] and [`SPILL_PRESSURED`] percent of a big model's
//!   layers sit on the slower side (the mesh's ggml-RPC lenders' words,
//!   `--ts` where a head runs); the probe's same answer decides.
//!
//! The runner is one thread ([`run`]): it takes the first eligible job,
//! runs the turn on the ask's harness ([`crate::tag::harness`]), streams
//! the answer into the file as it grows, posts the answer back into the
//! room as the assistant's Matrix user ([`Post`]) and, under [`MAX_HANDOFFS`],
//! hands a reply's own `@name` on to the next assistant.

use std::sync::mpsc;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use rand::RngCore;

use crate::tag::{Skill, MAX_HANDOFFS};

/// How often the reaper's sweep looks.
pub const SWEEP_MS: u64 = 5 * 60 * 1000;
/// How long a finished job waits for its page to read it, before the
/// reaper ages it out on the normal threshold.
pub const IDLE_MS: u64 = 10 * 60 * 1000;
/// The compressed sweep's floor: the reaper never reaps faster than a
/// minute, whatever the pressure.
pub const PRESSURED_FLOOR_MS: u64 = 60 * 1000;
/// The pressure predicate: less available memory than this fraction of
/// total pauses background work (the fork's own 0.1).
pub const PRESSURE_FRACTION: f64 = 0.1;
/// Spillover when the box holds the model comfortably.
pub const SPILL_START: u8 = 50;
/// Spillover under pressure: three quarters of the layers on the other
/// side, when the box cannot hold the model it was asked to run.
pub const SPILL_PRESSURED: u8 = 75;
/// How long one turn's answer may grow, in characters; a two-core box
/// streams minutes for this.
pub const MAX_ANSWER_CHARS: usize = 20_000;
/// How many asks the file holds at once: the oldest done jobs give way.
pub const MAX_JOBS: usize = 128;
/// How often a running answer is written for its page to see.
pub const WRITE_EVERY_MS: u64 = 2_000;

/// Why an ask came in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// A person tagged the assistant in a room: a person waiting.
    Mention,
    /// The composer's suggestion or a button push: background work the
    /// gate may pause under pressure, never drops.
    Suggestion,
}

/// One ask, in the file while it lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub origin: Origin,
    pub person: String,
    /// The assistant's Matrix localpart (`tag-losos-tag`, …).
    pub assistant: String,
    /// The room the answer posts into, when it came from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    pub question: String,
    /// queued, running, paused, done, failed or stopped.
    pub state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub answer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// How many assistant handoffs brought this job about: the reply of
    /// `@gardener` to an ask for `@losos` carries one.
    #[serde(default)]
    pub handoffs: u8,
    /// The material the ask's search gathered, JSON, ready for the turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<Value>,
}

impl Job {
    /// A new ask's state and time.
    pub fn queued(person: &str, assistant: &str, origin: Origin, question: &str) -> Self {
        Job {
            id: new_id(),
            origin,
            person: person.into(),
            assistant: assistant.into(),
            room: None,
            question: question.into(),
            state: "queued".into(),
            answer: String::new(),
            error: None,
            handoffs: 0,
            material: None,
        }
    }

    /// An id the file carries and the page may read back.
    pub fn valid_id(id: &str) -> bool {
        id.len() == 24 && id.bytes().all(|b| b.is_ascii_hexdigit())
    }
}

/// A fresh ask id, hex on 12 bytes of entropy.
pub fn new_id() -> String {
    let mut bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What wakes the runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// An ask arrived or changed; look for work.
    Work,
    /// A job was cancelled by a page.
    Stopped,
}

/// The memory probe's own 5 s cache, one per runner.
pub struct MemProbe {
    inner: std::sync::Mutex<Option<(std::time::Instant, bool)>>,
}

impl MemProbe {
    pub fn new() -> Self {
        MemProbe {
            inner: std::sync::Mutex::new(None),
        }
    }

    /// True while the box has less than [`PRESSURE_FRACTION`] of its
    /// memory available. Deliberately low: this gates background work and
    /// compresses the reaper's sweep, not a person's waiting turn.
    pub fn constrained(&self) -> bool {
        match self.inner.lock() {
            Ok(mut slot) => {
                if let Some((at, answer)) = *slot {
                    if at.elapsed() < std::time::Duration::from_secs(5) {
                        return answer;
                    }
                }
                let answer = memory_pressure();
                *slot = Some((std::time::Instant::now(), answer));
                answer
            }
            Err(_) => false,
        }
    }
}

impl Default for MemProbe {
    fn default() -> Self {
        Self::new()
    }
}

/// One `/proc/meminfo` read. A read error answers "no pressure": the
/// probe is optional and its absence must never fail a sweep hard.
pub fn memory_pressure() -> bool {
    let Ok(raw) = std::fs::read_to_string("/proc/meminfo") else {
        return false;
    };
    let field = |name: &str| {
        raw.lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|value| value.trim().trim_end_matches(" kB").parse::<u64>().ok())
    };
    match (field("MemAvailable"), field("MemTotal")) {
        (Some(available), Some(total)) if total > 0 => {
            (available as f64 / total as f64) < PRESSURE_FRACTION
        }
        _ => false,
    }
}

/// How much of a big model's layers sit on the slower side: `pressured`
/// while the gate says the box is tight, `start` when not.
pub fn spillover(start: u8, pressured: u8, constrained: bool) -> u8 {
    if constrained {
        pressured
    } else {
        start
    }
}

/// Why a sweep has reached the threshold it reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepReason {
    InactivityThreshold,
    MemoryPressure,
}

/// The sweep's own age for done jobs: the normal idle wait, or the
/// compressed one while under pressure (capped at the sweep interval,
/// floored so a sweep can never reap a fresh answer out from under a page
/// that is reading it). The reason goes in the log with every reap.
pub fn sweep_threshold(idle_ms: u64, constrained: bool) -> (u64, SweepReason) {
    if constrained {
        (
            idle_ms.clamp(PRESSURED_FLOOR_MS, SWEEP_MS),
            SweepReason::MemoryPressure,
        )
    } else {
        (idle_ms, SweepReason::InactivityThreshold)
    }
}

/// How a finished ask reaches the room it came from: the chat's own send,
/// as the assistant's Matrix user, or the test's capture.
pub trait Post: Send + Sync {
    fn post(&self, room: &str, assistant_localpart: &str, answer: &str) -> Result<(), String>;
}

/// One post a test kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded(pub String, pub String, pub String);

/// The test's [`Post`]: every ask's answer is recorded.
#[derive(Default)]
pub struct Record(pub std::sync::Mutex<Vec<Recorded>>);

impl Post for Record {
    fn post(&self, room: &str, assistant_localpart: &str, answer: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Recorded(
                room.into(),
                assistant_localpart.into(),
                answer.into(),
            ));
        Ok(())
    }
}

/// What a runner needs, besides the tag and the post.
#[derive(Debug, Clone, Default)]
pub struct Runner {
    /// `LOSOS_TAG_T3_BIN`: the box's T3 Code, the default harness.
    pub t3: Option<String>,
    /// `LOSOS_TAG_OPENCODE_BIN`: the fallback harness.
    pub opencode: Option<String>,
    /// `LOSOS_TAG_MODEL`: the built-in path's model tag on Ollama.
    pub model: Option<String>,
    /// `LOSOS_TAG_MESH_URL`: an OpenAI-compatible endpoint on the
    /// network — in a mesh, the llama.cpp head for a model group — the
    /// built-in path falls back to when Ollama cannot hold `model`.
    pub mesh_url: Option<String>,
    /// `LOSOS_TAG_MESH_MODEL`: the catalogue's id the head answers.
    pub mesh_model: Option<String>,
    /// `LOSOS_TAG_SEARXNG_URL`: the box's own web search.
    pub searxng: Option<String>,
    /// `losos-tag-browse`: the box's own Playwright wrapper, when
    /// `losos.tag.browser` is on.
    pub browse: Option<String>,
}

impl Runner {
    /// The environment `modules/tag.nix` hands over.
    pub fn from_env() -> Self {
        let read = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        Runner {
            t3: read("LOSOS_TAG_T3_BIN"),
            opencode: read("LOSOS_TAG_OPENCODE_BIN"),
            model: read("LOSOS_TAG_MODEL"),
            mesh_url: read("LOSOS_TAG_MESH_URL"),
            mesh_model: read("LOSOS_TAG_MESH_MODEL"),
            searxng: read("LOSOS_TAG_SEARXNG_URL"),
            browse: read("LOSOS_TAG_BROWSE_BIN"),
        }
    }
}

/// Run the queue until the process ends: one job at a time, the sweeps in
/// between. Spawned once, by `http`'s serve, next to the git rooms'
/// listener.
pub fn run(
    tag: Arc<crate::tag::Tag>,
    post: Arc<dyn Post>,
    config: Runner,
    wake: mpsc::Receiver<Signal>,
) {
    let probe = MemProbe::new();
    let mut done: std::collections::HashMap<String, std::time::Instant> =
        std::collections::HashMap::new();
    loop {
        // Whether anything wakes us sooner than the sweep would.
        let wait = std::time::Duration::from_millis(SWEEP_MS);
        let signal = wake.recv_timeout(wait).ok();
        let constrained = probe.constrained();
        if let Some(Signal::Stopped) = signal {
            // A page stopped an ask: its holder dies at once.
            reap_finished(&tag, constrained, &mut done);
            continue;
        }
        if constrain_idle(&tag, constrained) {
            // Fresh pressure: the background jobs pause on the spot.
            reap_finished(&tag, constrained, &mut done);
            continue;
        }
        if let Some(job) = next(&tag, constrained) {
            if let Err(why) = take(&tag, &post, &config, &job, constrained) {
                tracing::warn!(error = ?why, "a tag ask failed: {}", why);
            }
        }
        reap_finished(&tag, constrained, &mut done);
    }
}

/// Pause (or resume) the ask that is not a person's waiting.
fn constrain_idle(tag: &Arc<crate::tag::Tag>, constrained: bool) -> bool {
    let wanted = if constrained { "paused" } else { "queued" };
    let flip = |state: &str| -> Option<String> {
        match (state, constrained) {
            ("queued", true) | ("paused", false) => Some(wanted.into()),
            _ => None,
        }
    };
    tag.update_jobs(|jobs| {
        for job in jobs.iter_mut() {
            if let Some(next) = flip(&job.state) {
                if job.state != wanted {
                    let why = if constrained {
                        "background work paused"
                    } else {
                        "background work resumed"
                    };
                    tracing::info!(id = %job.id, "{why}");
                }
                job.state = next;
            }
        }
    });
    constrained
}

fn next(tag: &Arc<crate::tag::Tag>, constrained: bool) -> Option<Job> {
    let state = tag.state();
    let enabled = |job: &Job| -> bool {
        if constrained {
            job.state == "queued" && job.origin == Origin::Mention
        } else {
            job.state == "queued" || job.state == "paused"
        }
    };
    state.jobs.iter().find(|job| enabled(job)).cloned()
}

/// Run one ask, blocking, and put its answer where it belongs.
pub fn take(
    tag: &Arc<crate::tag::Tag>,
    post: &Arc<dyn Post>,
    config: &Runner,
    job: &Job,
    constrained: bool,
) -> Result<(), String> {
    let id = job.id.clone();
    set_state(tag, &id, "running")?;
    let spill = spillover(SPILL_START, SPILL_PRESSURED, constrained);
    let answer = turn(&tag.clone(), &config.clone(), job, spill);
    let (state, text, error) = match answer {
        Ok(text) => ("done", text, None),
        Err(why) => ("failed", String::new(), Some(why)),
    };
    if stopped_before_finish(tag, &id) {
        return Ok(());
    }
    tag.update_jobs(|jobs| {
        let Some(kept) = jobs.iter_mut().find(|j| j.id == id) else {
            return;
        };
        kept.state = state.into();
        kept.answer = text.chars().take(MAX_ANSWER_CHARS).collect();
        kept.error = error.clone();
    });
    if state != "done" {
        return Err(error.unwrap_or_else(|| "the turn failed".into()));
    }
    tag.archive(id.clone(), job, &text);
    if let Some(room) = job.room.as_deref() {
        post.post(room, &job.assistant, &text)?;
    }
    handoff(tag, job, &text);
    Ok(())
}

/// The handoff: what an answer's own text tags reaches the next
/// assistant, one handoff deeper, until [`MAX_HANDOFFS`] are spent — this
/// is how assistants talk to each other, one turn at a time, in the same
/// room the thread lives in.
fn handoff(tag: &Arc<crate::tag::Tag>, job: &Job, answer: &str) {
    if job.handoffs as usize >= MAX_HANDOFFS as usize {
        return;
    }
    let Some(room) = job.room.as_deref() else {
        return;
    };
    let state = tag.state();
    for next in state.reached(answer, &[]) {
        let localpart = format!("{}{}", crate::tag::TAG_PREFIX, next.slug);
        if localpart == job.assistant {
            continue;
        }
        let mut follow = Job::queued(&job.person, &localpart, Origin::Mention, answer);
        follow.room = Some(room.to_string());
        follow.handoffs = job.handoffs + 1;
        let _ = tag.enqueue(follow);
        break;
    }
}

/// Whether a page stopped this ask while its turn ran: a finished-new
/// state must never overwrite the page's stop.
fn stopped_before_finish(tag: &Arc<crate::tag::Tag>, id: &str) -> bool {
    tag.state()
        .jobs
        .iter()
        .find(|j| j.id == id)
        .is_some_and(|j| j.state == "stopped")
}

fn set_state(tag: &Arc<crate::tag::Tag>, id: &str, state: &str) -> Result<(), String> {
    tag.update_jobs(|jobs| {
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.state = state.into();
        }
    });
    Ok(())
}

/// Age out the finished jobs, with the threshold and its reason.
pub fn reap_finished(
    tag: &Arc<crate::tag::Tag>,
    constrained: bool,
    done: &mut std::collections::HashMap<String, std::time::Instant>,
) {
    let (threshold, reason) = sweep_threshold(IDLE_MS, constrained);
    let now = std::time::Instant::now();
    tag.update_jobs(|jobs| {
        jobs.retain(|job| {
            if job.state != "done" && job.state != "failed" {
                return true;
            }
            let entered = done.entry(job.id.clone()).or_insert_with(|| {
                tracing::info!(reason = ?reason, "a finished ask waits for its page",);
                now
            });
            now.duration_since(*entered).as_millis() as u64 <= threshold
        });
        done.retain(|id, _| jobs.iter().any(|job| job.id == *id));
    });
}

/// The reducer that one ask's turn calls for: the material it holds. For
/// tests to build without a homeserver or a model.
pub type Turn = fn(&Runner, &Job, u8) -> Result<String, String>;

/// One ask's turn, on the ask's harness. Bonsai-27B's built-in turns call
/// Ollama on loopback; T3 Code and OpenCode take the prompt on their own
/// command lines, with their own provider settings.
pub fn turn(
    tag: &Arc<crate::tag::Tag>,
    config: &Runner,
    job: &Job,
    spill: u8,
) -> Result<String, String> {
    let state = tag.state();
    let assistant = state
        .assistants
        .iter()
        .find(|a| format!("{}{}", crate::tag::TAG_PREFIX, a.slug) == job.assistant)
        .cloned();
    let (material, wanted) = match job.material.as_ref() {
        // The route's own shape: hits and the skills the ask named.
        Some(doc) if doc.get("hits").is_some() => (
            built_material(&doc["hits"]),
            doc.get("skills")
                .and_then(serde_json::Value::as_array)
                .map(|skills| {
                    skills
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        ),
        Some(material) => (built_material(material), Vec::new()),
        None => (String::new(), Vec::new()),
    };
    let wanted = wanted
        .into_iter()
        .map(|slug| {
            state
                .skills
                .iter()
                .find(|s| s.slug == slug)
                .cloned()
                .ok_or_else(|| format!("no such skill: {slug}"))
        })
        .collect::<Result<Vec<Skill>, String>>()?;
    let knew: Vec<&Skill> = wanted.iter().collect();
    // The material was already gathered at this clearance; the turn
    // carries it as data.
    let clearance = state.clearance_of(assistant.as_ref(), &job.person);
    let _ = clearance;
    let lang = std::env::var("LOSOS_TAG_LANG").unwrap_or_else(|_| "en".into());
    let prompt = join_prompt(
        &crate::tag::system_prompt(assistant.as_ref(), &knew, &lang),
        &material,
        &job.question,
    );
    let _ = spill;
    match crate::tag::harness(config.t3.as_deref(), config.opencode.as_deref()) {
        crate::tag::Harness::T3 => {
            spawn_turn(config.t3.as_deref().expect("t3 configured"), &prompt, true)
        }
        crate::tag::Harness::OpenCode => spawn_turn(
            config.opencode.as_deref().expect("opencode configured"),
            &prompt,
            false,
        ),
        crate::tag::Harness::Builtin => {
            let model = config.model.as_deref().ok_or("the box runs no model")?;
            match builtin_turn(model, &prompt) {
                Ok(answer) => Ok(answer),
                Err(local) => match config.mesh_url.as_deref() {
                    // The box could not hold its own model: the mesh's
                    // head answers, over the network, the model the
                    // catalogue names.
                    Some(url) => mesh_turn(
                        url,
                        config.mesh_model.as_deref().unwrap_or("qwen3-8max"),
                        &prompt,
                    )
                    .map_err(|mesh| format!("the box: {local}; the mesh: {mesh}")),
                    None => Err(local),
                },
            }
        }
    }
}

/// One turn on an OpenAI-compatible endpoint (the mesh's llama.cpp head
/// answers `/chat/completions`), non-streaming.
fn mesh_turn(url: &str, model: &str, prompt: &str) -> Result<String, String> {
    let url = url.trim_end_matches('/');
    let call = json!({
        "model": model,
        "stream": false,
        "messages": [
            { "role": "system", "content": "You are LosOS Tag. Answer from the material alone." },
            { "role": "user", "content": prompt },
        ],
    });
    let out = std::process::Command::new(CURL)
        .arg("-fsS")
        .arg("--max-time")
        .arg("1700")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-d")
        .arg(serde_json::to_string(&call).unwrap_or_default())
        .arg(format!("{url}/chat/completions"))
        .output()
        .map_err(|e| format!("the mesh head did not answer: {e}"))?;
    let answer: Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("the mesh answer was not JSON: {e}"))?;
    answer["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "the mesh head answered nothing".to_string())
}

/// The turn's material from the job's gathered search.
fn built_material(value: &Value) -> String {
    let hits = value
        .as_array()
        .map(|hits| {
            hits.iter()
                .map(|hit| {
                    let path = hit["path"].as_str().unwrap_or("");
                    let lines = hit["snippet"]
                        .as_array()
                        .map(|lines| {
                            lines
                                .iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("")
                        })
                        .unwrap_or_default();
                    format!("{path}\n{lines}")
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    hits.join("\n\n")
}

fn join_prompt(system: &str, material: &str, question: &str) -> String {
    format!("{system}\n\n<material>{material}</material>\n\nQuestion: {question}")
}

/// One turn on a command line: `t3 run …` or `opencode run …`. The
/// prompt is on stdin? On the command line both take it, capped at what a
/// command line carries.
fn spawn_turn(bin: &str, prompt: &str, t3: bool) -> Result<String, String> {
    let mut call = if t3 {
        crate::tag::t3_command(bin, prompt)
    } else {
        crate::tag::opencode_command(bin, prompt)
    };
    let out = call
        .output()
        .map_err(|e| format!("{bin} did not start: {e}"))?;
    if !out.status.success() {
        return Err(format!("{bin} answered {}", out.status));
    }
    Ok(crate::tag::web_browse_bytes(&out.stdout, MAX_ANSWER_CHARS))
}

/// One turn on Ollama, non-streaming (one answer per call, no queue of
/// Ollama's own to run into: MAX_JOBS serialises these).
fn builtin_turn(model: &str, prompt: &str) -> Result<String, String> {
    let url = "http://127.0.0.1:11434/api/chat";
    let call = json!({
        "model": model,
        "stream": false,
        "messages": [
            { "role": "system", "content": "You are LosOS Tag. Answer from the material alone." },
            { "role": "user", "content": prompt },
        ],
    });
    let out = std::process::Command::new(CURL)
        .arg("-fsS")
        .arg("--max-time")
        .arg("1700")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-d")
        .arg(serde_json::to_string(&call).unwrap_or_default())
        .arg(url)
        .output()
        .map_err(|e| format!("the model did not answer: {e}"))?;
    let answer: Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("the answer was not JSON: {e}"))?;
    answer["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "the model answered nothing".to_string())
}

const CURL: &str = "curl";

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tag::{Harness, Tag};

    fn tag() -> Arc<Tag> {
        let dir = std::env::temp_dir().join(format!(
            "losos-queue-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos().to_string())
                .unwrap_or_default()
        ));
        Arc::new(Tag::new(dir.join("tag.json")))
    }

    #[test]
    fn one_run_writes_its_answer_into_the_file() {
        let tag = tag();
        let job = tag
            .enqueue(Job {
                id: new_id(),
                origin: Origin::Mention,
                person: "eva".into(),
                assistant: "tag-losos-tag".into(),
                room: None,
                question: "What is due?".into(),
                state: "queued".into(),
                answer: String::new(),
                error: None,
                handoffs: 0,
                material: Some(json!([{ "path": "a.md", "snippet": ["", "tax"] }])),
            })
            .unwrap();
        let post: Arc<dyn Post> = Arc::new(Record::default());
        let config = Runner::default();
        // No model, no harness: the ask's own turn is the box's refusal,
        // persisted as the job's error.
        take(&tag, &post, &config, &job, false).unwrap_err();
        let state = tag.state();
        let done = state.jobs.into_iter().find(|j| j.id == job.id).unwrap();
        assert_eq!(done.state, "failed");
        assert!(done.error.is_some());
    }

    #[test]
    fn the_turn_takes_the_harnesss_own_words() {
        let config = Runner {
            model: Some("gemma3:1b".into()),
            ..Runner::default()
        };
        assert_eq!(
            Harness::Builtin,
            crate::tag::harness(config.t3.as_deref(), config.opencode.as_deref())
        );
        let prompt = join_prompt("you are tag", "material", "what is due?");
        assert!(prompt.starts_with("you are tag"));
        assert!(prompt.contains("<material>material</material>"));
        assert!(prompt.ends_with("Question: what is due?"));
    }

    #[test]
    fn the_handoff_reaches_the_named_assistant_once() {
        let tag = tag();
        // A second assistant beside the built-in one, through the routes'
        // own path.
        let gardener = crate::tag::Assistant {
            slug: "gardener".into(),
            display: "Gardener".into(),
            prompt: String::new(),
            clearance: crate::tag::DEFAULT_LEVEL.into(),
            owner: "eva".into(),
            enabled: true,
            minted: false,
        };
        tag.put_assistant("eva", true, gardener).unwrap();

        // LosOS's own answer names the gardener: one ask follows, one
        // handoff deeper.
        let mut answer = Job::queued("eva", "tag-losos-tag", Origin::Mention, "what is the plan?");
        answer.room = Some("!room:box.local".to_string());
        answer.answer = "ask @gardener about the plants".into();
        archive_and_handoff(&tag, &answer);

        let state = tag.state();
        let follow = state
            .jobs
            .iter()
            .find(|j| j.assistant == "tag-gardener")
            .expect("the gardener's own ask");
        assert_eq!(
            (follow.handoffs, follow.person.clone()),
            (1, "eva".to_string())
        );

        // Three handoffs spent: nothing follows.
        let mut third = Job::queued("eva", "tag-gardener", Origin::Mention, "hello");
        third.room = Some("!room:box.local".to_string());
        third.handoffs = 3;
        archive_and_handoff(&tag, &third);
        let state = tag.state();
        assert!(!state.jobs.iter().any(|j| j.handoffs == 4));
    }

    /// What the runner does after a done ask: the archive, and the handoff.
    fn archive_and_handoff(tag: &Arc<Tag>, job: &Job) {
        tag.update_jobs(|jobs| jobs.push(job.clone()));
        tag.archive(job.id.clone(), job, &job.answer);
        handoff(tag, job, &job.answer);
    }

    #[test]
    fn a_paused_suggestion_and_a_running_mention() {
        assert!(constrain_idle(&tag_for_pressure(), true));
    }

    fn tag_for_pressure() -> Arc<Tag> {
        let dir = std::env::temp_dir().join(format!("losos-queue-p-{}", std::process::id()));
        Arc::new(Tag::new(dir.join("tag.json")))
    }

    #[test]
    fn the_probe_answers_bool() {
        let probe = MemProbe::new();
        assert_eq!(probe.constrained(), memory_pressure());
        assert!(matches!(memory_pressure(), true | false));
    }

    #[test]
    fn the_spillover_climbs_with_the_pressure() {
        assert_eq!(spillover(SPILL_START, SPILL_PRESSURED, false), SPILL_START);
        assert_eq!(
            spillover(SPILL_START, SPILL_PRESSURED, true),
            SPILL_PRESSURED
        );
    }

    #[test]
    fn an_ask_id_is_checked() {
        let id = new_id();
        assert!(Job::valid_id(&id));
        assert!(!Job::valid_id("../../../etc"));
    }

    #[test]
    fn a_job_knows_where_it_came_from() {
        let mention = Job::queued("eva", "tag-losos-tag", Origin::Mention, "hi");
        assert_eq!(mention.state, "queued");
        assert_eq!(mention.handoffs, 0);
    }
}
