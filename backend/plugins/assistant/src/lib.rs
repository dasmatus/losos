//! The assistant plugin: `/api/assistant`, questions to the box's own
//! language model about what the person shows it.
//!
//! Every LosOS app offers "Ask AI" where it fits: a mail thread, a file in
//! LosOS Cloud, a note. The page reads the material with the person's own
//! session, so the model sees nothing the person could not open, and sends
//! it here with the question. This plugin asks Ollama on loopback, the
//! model for everything that is not code (`modules/builder.nix`,
//! `losos.assistant.model`, Gemma 3 1B; the builder keeps its coder):
//! the question and the material never leave the box, and nothing is kept
//! but the answer, in lososd's memory, for an hour.
//!
//! A small model on a mini PC's processor takes minutes over a long thread,
//! so an ask is a job: `POST /api/assistant/asks` answers at once with an
//! id, the job streams the model's answer into the cache, and the page
//! polls `GET /api/assistant/asks/{id}` and draws the answer as it grows.
//! One job at a time (the manifest's `jobs: 1`): the worst box LosOS runs on
//! has two cores and 4 GiB, and a second model run beside the first would
//! only make both slower. A second ask meanwhile gets 429 and the page tries
//! again. `DELETE` stops one.

pub mod model;

use losos_plugin::{cache, clock, config, http, jobs, random, Caller, Plugin, Request, Response};
use model::{Ask, Kept, State};
use serde_json::{json, Value};

/// How long an answer is kept for the page to read.
const KEEP_SECS: u32 = 3600;
/// How often a running answer is written for the page to see.
const WRITE_EVERY_MS: u64 = 300;

fn base_url() -> Option<String> {
    let url = config::get_or("url", "");
    let url = url.trim().trim_end_matches('/');
    // Loopback only: the material is the person's.
    (url.starts_with("http://127.") || url.starts_with("http://localhost")).then(|| url.to_string())
}

fn model_name() -> String {
    config::get_or("model", "gemma3:1b")
}

fn context() -> u32 {
    config::get_or("context", "8192")
        .trim()
        .parse()
        .unwrap_or(8192)
}

/// Who an answer belongs to: the person, or the spare key.
fn owner(caller: &Caller) -> Option<String> {
    match caller.person() {
        Some(p) => Some(p.name.clone()),
        None if caller.is_admin() => Some(caller.name().to_string()),
        None => None,
    }
}

fn key(id: &str) -> String {
    format!("ask/{id}")
}

fn stop_key(id: &str) -> String {
    format!("stop/{id}")
}

fn save(kept: &Kept) {
    cache::set_json(&key(&kept.state.id), kept, KEEP_SECS);
}

/// The ask, if `caller` made it.
fn mine(req: &Request) -> Result<Kept, Response> {
    let id = req.param("id").unwrap_or("");
    let who = owner(&req.caller).ok_or_else(|| Response::error(403, "sign in first"))?;
    if !model::valid_id(id) {
        return Err(Response::error(404, "no such question"));
    }
    match cache::get_json::<Kept>(&key(id)) {
        Some(kept) if kept.owner == who => Ok(kept),
        _ => Err(Response::error(404, "no such question")),
    }
}

/// Whether the model is there, for the page to show "Ask AI" at all.
fn status() -> Value {
    let Some(url) = base_url() else {
        return json!({ "available": false });
    };
    let wanted = model_name();
    let ready = http::Request::get(&format!("{url}/api/tags"))
        .timeout_ms(3000)
        .fetch(1024 * 1024)
        .ok()
        .filter(http::Response::is_success)
        .and_then(|r| r.json::<Value>())
        .is_some_and(|tags| model::has_model(&tags, &wanted));
    json!({ "available": true, "ready": ready, "model": wanted })
}

fn start(req: &Request) -> Response {
    let Some(who) = owner(&req.caller) else {
        return Response::error(403, "sign in first");
    };
    if base_url().is_none() {
        return Response::error(404, "this box runs no language model");
    }
    let ask = match model::parse_ask(&req.json_value()) {
        Ok(ask) => ask,
        Err(model::Invalid(why)) => return Response::error(400, &why),
    };
    let id = random::hex(12);
    let kept = Kept {
        owner: who,
        state: State {
            id: id.clone(),
            state: "queued".into(),
            answer: String::new(),
            cut: ask.cut,
            error: None,
        },
    };
    save(&kept);
    let input = serde_json::to_vec(&json!({ "id": id, "ask": ask })).unwrap_or_default();
    if let Err(e) = jobs::spawn("ask", &input) {
        cache::remove(&key(&id));
        losos_plugin::log::info(&format!("an ask waits: {e}"));
        return Response::json(
            429,
            &json!({ "error": "the box's AI is answering another question", "busy": true }),
        );
    }
    Response::json(202, &serde_json::to_value(&kept.state).unwrap_or_default())
}

pub struct AssistantPlugin;

impl Plugin for AssistantPlugin {
    fn handle(req: Request) -> Response {
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/assistant") => Response::ok(&status()),
            ("POST", "/api/assistant/asks") => start(&req),
            ("GET", "/api/assistant/asks/{id}") => match mine(&req) {
                Ok(kept) => Response::ok(&serde_json::to_value(&kept.state).unwrap_or_default()),
                Err(r) => r,
            },
            ("DELETE", "/api/assistant/asks/{id}") => match mine(&req) {
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

    fn run(kind: &str, input: &[u8]) {
        if kind != "ask" {
            return;
        }
        let Ok(doc) = serde_json::from_slice::<Value>(input) else {
            return;
        };
        let id = doc["id"].as_str().unwrap_or("").to_string();
        let Ok(ask) = serde_json::from_value::<Ask>(doc["ask"].clone()) else {
            return;
        };
        let Some(mut kept) = cache::get_json::<Kept>(&key(&id)) else {
            return;
        };
        match answer(&mut kept, &ask) {
            Ok(()) => {}
            Err(why) => {
                losos_plugin::log::warn(&format!("an ask failed: {why}"));
                kept.state.state = "failed".into();
                kept.state.error = Some(why);
            }
        }
        if cache::get(&stop_key(&id)).is_some() {
            kept.state.state = "stopped".into();
        }
        save(&kept);
    }
}

/// Stream the model's answer into the cache.
fn answer(kept: &mut Kept, ask: &Ask) -> Result<(), String> {
    let id = kept.state.id.clone();
    let url = base_url().ok_or("this box runs no language model")?;
    kept.state.state = "running".into();
    save(kept);
    let body = model::chat_body(&model_name(), context(), ask);
    let stream = http::Request::post(&format!("{url}/api/chat"))
        .json(&body)
        .timeout_ms(890_000)
        .send()
        .map_err(|e| format!("the model did not answer: {e}"))?;
    if !(200..300).contains(&stream.status) {
        return Err(format!("the model answered {}", stream.status));
    }
    let mut pending: Vec<u8> = Vec::new();
    let mut written = clock::monotonic_ms();
    loop {
        if cache::get(&stop_key(&id)).is_some() {
            return Ok(());
        }
        let Some(bytes) = stream
            .read(16 * 1024)
            .map_err(|e| format!("reading the answer: {e}"))?
        else {
            break;
        };
        pending.extend_from_slice(&bytes);
        while let Some(at) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=at).collect();
            let Some(chunk) = model::parse_chunk(&String::from_utf8_lossy(&line)) else {
                continue;
            };
            if let Some(e) = chunk.error {
                return Err(e);
            }
            kept.state.answer.push_str(&chunk.text);
            if chunk.done {
                kept.state.state = "done".into();
                kept.state.answer = kept.state.answer.trim().to_string();
                return Ok(());
            }
        }
        if clock::monotonic_ms().saturating_sub(written) >= WRITE_EVERY_MS {
            save(kept);
            written = clock::monotonic_ms();
        }
    }
    Err("the answer ended before the model said it was done".into())
}

losos_plugin::export!(AssistantPlugin);
