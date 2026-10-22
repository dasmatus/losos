//! The box's code model: the Ollama that Build with AI runs
//! (`modules/builder.nix`), asked one question at a time over loopback.
//!
//! Every request carries the builder's context size. Ollama loads the model
//! again whenever `num_ctx` changes, so a review asking with another size
//! would throw away the builder's loaded model, and the builder the
//! reviewer's, each time the two took turns. Ollama answers one request at
//! a time (`OLLAMA_NUM_PARALLEL=1`) and queues four; the reviewer never has
//! more than one waiting, so a build waits at most for one of its
//! questions.

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

/// Where the model is: `LOSOS_REVIEWER_URL`, `LOSOS_REVIEWER_MODEL`,
/// `LOSOS_REVIEWER_CONTEXT`, which `modules/git.nix` sets from
/// `losos.builder.*` when Build with AI is on. No URL, no reviewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub url: String,
    pub model: String,
    pub context: u32,
    /// `curl`.
    pub curl: String,
}

impl Config {
    #[must_use]
    pub fn from_env() -> Option<Config> {
        let url = std::env::var("LOSOS_REVIEWER_URL").ok()?;
        let url = url.trim().trim_end_matches('/').to_string();
        if !(url.starts_with("http://127.") || url.starts_with("http://localhost")) {
            // Loopback only: the code under review never leaves the box.
            tracing::warn!(%url, "LOSOS_REVIEWER_URL is not on loopback; the reviewer is off");
            return None;
        }
        let model = std::env::var("LOSOS_REVIEWER_MODEL")
            .ok()
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())?;
        let context = std::env::var("LOSOS_REVIEWER_CONTEXT")
            .ok()
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or(12288);
        let curl = std::env::var("LOSOS_REVIEWER_CURL").unwrap_or_else(|_| "curl".to_string());
        Some(Config {
            url,
            model,
            context,
            curl,
        })
    }
}

/// One answer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reply {
    pub text: String,
    /// The likeliest first tokens and their probabilities, when asked for.
    pub first: Vec<(String, f64)>,
    pub prompt_tokens: u64,
    pub answer_tokens: u64,
}

impl Reply {
    /// How likely the model thought its answer starts with `word`, summed
    /// over the spellings it might use (" YES", "Yes", ...).
    pub fn chance_of(&self, word: &str) -> f64 {
        let want = word.to_ascii_lowercase();
        self.first
            .iter()
            .filter(|(t, _)| t.trim().to_ascii_lowercase() == want)
            .map(|(_, p)| p)
            .sum()
    }
}

/// The body of `POST /api/chat`.
pub fn chat_body(
    config: &Config,
    system: &str,
    user: &str,
    max_tokens: u32,
    first_tokens: bool,
) -> Value {
    let mut body = json!({
        "model": config.model,
        "stream": false,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "options": {
            "num_ctx": config.context,
            "num_predict": max_tokens,
            "temperature": 0,
        },
    });
    if first_tokens {
        body["logprobs"] = json!(true);
        body["top_logprobs"] = json!(5);
    }
    body
}

/// Read Ollama's answer.
pub fn read_reply(doc: &Value) -> anyhow::Result<Reply> {
    if let Some(e) = doc["error"].as_str() {
        anyhow::bail!("ollama: {e}");
    }
    let text = doc["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("ollama answered without a message"))?
        .to_string();
    let first = doc["logprobs"]
        .get(0)
        .and_then(|t| t["top_logprobs"].as_array())
        .map(|all| {
            all.iter()
                .filter_map(|t| {
                    Some((
                        t["token"].as_str()?.to_string(),
                        t["logprob"].as_f64()?.exp(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Reply {
        text,
        first,
        prompt_tokens: doc["prompt_eval_count"].as_u64().unwrap_or(0),
        answer_tokens: doc["eval_count"].as_u64().unwrap_or(0),
    })
}

/// How long one question may take. A long piece on two cores is read at
/// about 45 tokens a second and answered at about 7.
const TIMEOUT_SECS: u32 = 600;

/// Ask once and wait for the whole answer.
pub fn ask(
    config: &Config,
    system: &str,
    user: &str,
    max_tokens: u32,
    first_tokens: bool,
) -> anyhow::Result<Reply> {
    let body = chat_body(config, system, user, max_tokens, first_tokens).to_string();
    let mut child = Command::new(&config.curl)
        .args(["--silent", "--show-error", "--fail-with-body"])
        .args(["--max-time", &TIMEOUT_SECS.to_string()])
        .args(["--header", "Content-Type: application/json"])
        .args(["--data-binary", "@-"])
        .arg(format!("{}/api/chat", config.url))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("curl has no stdin"))?
        .write_all(body.as_bytes())?;
    let out = child.wait_with_output()?;
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if !out.status.success() {
        let said = doc["error"].as_str().map_or_else(
            || String::from_utf8_lossy(&out.stderr).trim().to_string(),
            str::to_string,
        );
        anyhow::bail!("ollama: {said}");
    }
    read_reply(&doc)
}

/// Whether Ollama has the model yet: `GET /api/tags`. The builder's loader
/// pulls it once, the first time the box is online.
pub fn has_model(config: &Config) -> anyhow::Result<bool> {
    let out = Command::new(&config.curl)
        .args(["--silent", "--show-error", "--fail", "--max-time", "10"])
        .arg(format!("{}/api/tags", config.url))
        .stdin(Stdio::null())
        .output()?;
    if !out.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let doc: Value = serde_json::from_slice(&out.stdout)?;
    let full = |m: &str| {
        if m.contains(':') {
            m.to_string()
        } else {
            format!("{m}:latest")
        }
    };
    Ok(doc["models"].as_array().is_some_and(|all| {
        all.iter()
            .filter_map(|m| m["name"].as_str())
            .any(|m| full(m) == full(&config.model))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            url: "http://127.0.0.1:11434".into(),
            model: "qwen2.5-coder:1.5b".into(),
            context: 12288,
            curl: "curl".into(),
        }
    }

    #[test]
    fn a_request_keeps_the_builders_context_and_asks_for_first_tokens_only_when_told() {
        let body = chat_body(&config(), "sys", "user", 3, true);
        assert_eq!(body["options"]["num_ctx"], 12288);
        assert_eq!(body["options"]["num_predict"], 3);
        assert_eq!(body["options"]["temperature"], 0);
        assert_eq!(body["stream"], false);
        assert_eq!(body["logprobs"], true);
        assert_eq!(body["messages"][1]["content"], "user");
        let plain = chat_body(&config(), "sys", "user", 3, false);
        assert!(plain.get("logprobs").is_none());
        assert!(plain["options"].get("num_thread").is_none());
    }

    #[test]
    fn a_reply_says_how_sure_the_model_was_of_its_first_word() {
        let doc = json!({
            "message": { "content": "YES" },
            "logprobs": [{ "token": "YES", "logprob": -0.1, "top_logprobs": [
                { "token": "YES", "logprob": (0.6f64).ln() },
                { "token": " Yes", "logprob": (0.1f64).ln() },
                { "token": "NO", "logprob": (0.3f64).ln() },
            ]}],
            "prompt_eval_count": 40, "eval_count": 1,
        });
        let r = read_reply(&doc).unwrap();
        assert!((r.chance_of("yes") - 0.7).abs() < 1e-9);
        assert!((r.chance_of("NO") - 0.3).abs() < 1e-9);
        assert_eq!(r.prompt_tokens, 40);
        assert!(read_reply(&json!({ "error": "model not found" })).is_err());
    }
}
