//! The tinycode plugin: `/api/tinycode`, agentic work on the box through
//! the box's own coding agent CLI (github.com/dasmatus/tinycode,
//! AGPL-3.0-or-later, the appliance ships it beside this plugin).
//!
//! An administrator sends a task; the agent — six tools in a workspace the
//! box owns — works on it in a job and the page reads the answer off the
//! cache. An ask with `auto: true` lets the agent write, edit and run
//! commands, as lososd's user: this is the box's own agent doing what an
//! administrator on the box would, not a page's helper, so its routes sit
//! in the admin section and every caller is checked twice (the section in
//! `sections.json` and again here).
//!
//! The CLI runs as a child process through the exec import, one call for
//! one task: tinycode caps its own tool outputs and turns, and the job's
//! `jobSecs` caps the whole run. The agent's work and its sessions live
//! under the state directory (the manifest's `config.dir`), persisted with
//! the rest of /var/lib/losos; its provider config comes from the two
//! OpenCode config paths the manifest may name, else from the CLI's own
//! defaults under the HOME the plugin gives it.

pub mod model;

use losos_plugin::log;
use losos_plugin::{cache, config, exec, fs, jobs, random, Plugin, Request, Response};
use model::{Kept, State};
use serde_json::{json, Value};

/// How long an ask is kept for the page to read.
const KEEP_SECS: u32 = model::KEEP_SECS;

fn key(id: &str) -> String {
    format!("ask/{id}")
}

fn stop_key(id: &str) -> String {
    format!("stop/{id}")
}

fn save(kept: &Kept) {
    cache::set_json(&key(&kept.state.id), kept, KEEP_SECS);
}

/// Who an ask belongs to: an administrator, the spare key, or lososd.
fn owner(caller: &losos_plugin::Caller) -> Option<String> {
    if caller.is_admin() {
        Some(caller.name().to_string())
    } else {
        None
    }
}

/// The ask, if `caller` made it.
fn mine(req: &Request) -> Result<Kept, Response> {
    let id = req.param("id").unwrap_or("");
    let who = owner(&req.caller).ok_or_else(|| Response::error(403, "an administrator asks"))?;
    if !model::valid_id(id) {
        return Err(Response::error(404, "no such task"));
    }
    match cache::get_json::<Kept>(&key(id)) {
        Some(kept) if kept.owner == who => Ok(kept),
        _ => Err(Response::error(404, "no such task")),
    }
}

/// Whether the CLI can run: the workspace there, and whether the provider
/// config is on the disk — either file the manifest names, or either file
/// of the CLI's own fallback paths under the HOME the plugin gives it. A
/// box whose provider is env keys alone (tinycode falls back to those
/// before anything on disk) keeps no file for this glance to find, and
/// stays `false` until one of the files is there.
fn status() -> Value {
    let dir = config::get_or("dir", "");
    let workspace = model::fs_join(&dir, "work");
    let home = model::fs_join(&dir, "home");
    let workspace_ok = !dir.is_empty() && fs::exists(&workspace).unwrap_or(false);
    let provider = [
        config::get("opencodeConfig"),
        config::get("opencodeAuth"),
        Some(model::fs_join(&home, ".config/opencode/opencode.json")),
        Some(model::fs_join(&home, ".local/share/opencode/auth.json")),
    ]
    .iter()
    .flatten()
    .any(|p| fs::exists(p).unwrap_or(false));
    json!({
        "available": workspace_ok,
        "workspace": workspace_ok.then_some(workspace),
        "providerConfigured": provider,
    })
}

fn start(req: &Request) -> Response {
    let Some(who) = owner(&req.caller) else {
        return Response::error(403, "an administrator asks");
    };
    let ask = match model::parse_ask(&req.json_value()) {
        Ok(a) => a,
        Err(model::Invalid(why)) => return Response::error(400, &why),
    };
    let id = random::hex(12);
    let kept = Kept {
        owner: who,
        state: State {
            id: id.clone(),
            state: "queued".into(),
            answer: String::new(),
            turns: None,
            session: None,
            error: None,
            exit: None,
        },
    };
    save(&kept);
    let input = serde_json::to_vec(&json!({ "id": id, "ask": ask })).unwrap_or_default();
    if let Err(e) = jobs::spawn("run", &input) {
        cache::remove(&key(&id));
        log::info(&format!("an ask waits: {e}"));
        return Response::json(
            429,
            &json!({ "error": "the box's agent is busy on another task", "busy": true }),
        );
    }
    Response::json(202, &serde_json::to_value(&kept.state).unwrap_or_default())
}

/// The state directory the run needs, plus its two children. The host's
/// create-dir is fine when it is there, so a timer that never fired
/// changes nothing.
fn make_dirs() {
    let dir = config::get_or("dir", "");
    for path in [
        dir.as_str(),
        &model::fs_join(&dir, "work"),
        &model::fs_join(&dir, "home"),
    ] {
        if !path.is_empty() {
            if let Err(e) = fs::create_dir(path, None) {
                log::warn(&format!("a state directory would not come up: {e}"));
            }
        }
    }
}

pub struct TinycodePlugin;

impl Plugin for TinycodePlugin {
    fn handle(req: Request) -> Response {
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/tinycode") => Response::ok(&status()),
            ("POST", "/api/tinycode/asks") => start(&req),
            ("GET", "/api/tinycode/asks/{id}") => match mine(&req) {
                Ok(kept) => Response::ok(&serde_json::to_value(&kept.state).unwrap_or_default()),
                Err(r) => r,
            },
            // A queued ask is taken off the job queue; a running one keeps
            // to its turns — its exec is already on — and is marked
            // stopped when the CLI leaves, as the assistant's ask is.
            ("DELETE", "/api/tinycode/asks/{id}") => match mine(&req) {
                Ok(mut kept) => {
                    cache::set(&stop_key(&kept.state.id), b"1", KEEP_SECS);
                    if kept.state.state == "queued" || kept.state.state == "running" {
                        kept.state.state = "stopped".into();
                        save(&kept);
                    }
                    Response::ok(&serde_json::to_value(&kept.state).unwrap_or_default())
                }
                Err(r) => r,
            },
            _ => Response::error(404, "not found"),
        }
    }

    fn tick(timer: &str) {
        if timer == "start" {
            make_dirs();
        }
    }

    fn run(kind: &str, input: &[u8]) {
        if kind != "run" {
            return;
        }
        let Ok(doc) = serde_json::from_slice::<Value>(input) else {
            return;
        };
        let id = doc["id"].as_str().unwrap_or("").to_string();
        let Ok(ask) = serde_json::from_value::<model::Ask>(doc["ask"].clone()) else {
            return;
        };
        // Cancelled between the answer and the job: the CLI never starts.
        if cache::get(&stop_key(&id)).is_some() {
            log::info(&format!(
                "task {id}: the agent never ran, the ask was taken back"
            ));
            return;
        }
        let Some(mut kept) = cache::get_json::<Kept>(&key(&id)) else {
            return;
        };
        match the_run(&mut kept, &ask) {
            Ok(()) => {}
            Err(why) => {
                log::warn(&format!("task {id} failed: {why}"));
                kept.state.state = "failed".into();
                kept.state.error = Some(why);
            }
        }
        // Marked stopped on the way out: a running ask cannot be
        // interrupted, the CLI's exec already on, but it does not stay
        // done-looking either.
        if cache::get(&stop_key(&id)).is_some() && !kept.state.completed() {
            kept.state.state = "stopped".into();
        }
        save(&kept);
    }
}

/// The whole task as one exec call: the CLI answers its own NDJSON —
/// capped outputs, run timeouts, its cap on turns — and the plugin reads
/// the final state off stdout.
fn the_run(kept: &mut Kept, ask: &model::Ask) -> Result<(), String> {
    let dir = config::get_or("dir", "");
    if dir.is_empty() {
        return Err("the manifest named no state directory".into());
    }
    kept.state.state = "running".into();
    save(kept);
    let spec = model::command(
        ask,
        &dir,
        config::get("opencodeConfig").as_deref(),
        config::get("opencodeAuth").as_deref(),
    );
    // The whole job but ten seconds: the agent may think for minutes
    // between turns.
    let mut cmd = exec::Command::new(spec.program)
        .args(spec.args.iter().map(String::as_str))
        .cwd(&spec.cwd);
    for (key, value) in &spec.env {
        cmd = cmd.env(key, value);
    }
    let out = cmd
        .stdin(spec.stdin.clone())
        .timeout_ms(3_590_000)
        .output()
        .map_err(|e| format!("tinycode did not run: {e}"))?;
    kept.state.exit = Some(out.status);
    let stdout = out.stdout_string();
    let outcome = model::parse_events(&stdout);
    if !out.success() && !outcome.completed() {
        log::warn(&model::journal_line(
            &outcome,
            out.status,
            &out.stderr_last_line(),
        ));
    }
    // The session: a run's `start`/`done` names one, and a run that used
    // its turns up names the file to carry on with.
    kept.state.session = outcome
        .session
        .as_deref()
        .or(outcome.max_turns.as_deref())
        .map(as_file_name);
    kept.state.answer = outcome.answer.as_deref().unwrap_or_default().to_string();
    kept.state.turns = outcome.turns;

    kept.state.state = outcome.to_state();
    if let Some(why) = outcome.error {
        return Err(why);
    }
    Ok(())
}

/// What the cache stores of a session — the file name under the workspace
/// the CLI printed as a full path.
fn as_file_name(printed: &str) -> String {
    printed
        .rsplit_once("/.tinycode/sessions/")
        .map_or_else(|| printed.to_string(), |(_, name)| name.to_string())
}

losos_plugin::export!(TinycodePlugin);
