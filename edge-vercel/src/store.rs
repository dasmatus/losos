//! The registry's home between requests: one key in a Redis spoken to over
//! HTTPS.
//!
//! Upstash's REST API is a plain HTTP shape — `POST <url>` with the command as
//! a JSON array and a bearer token, `{"result": …}` or `{"error": …}` back —
//! so the whole client is a few lines of `reqwest`, which the registrar already
//! links. Redis rather than a blob store because the registry is rewritten on
//! every heartbeat and read on every request: an eventually consistent object
//! store could hand a request a snapshot from before the previous one's write
//! and lose that write when this one saved. A `GET`/`SET` on one key is
//! consistent, and the host serialises requests within an instance (see
//! [`crate::host`]); two instances can still race, and the loser's write wins,
//! which for a registry of a few boxes heartbeating every half minute costs at
//! most one heartbeat.
//!
//! The snapshot carries no secret (see [`losos_registrar::Registry::export`]),
//! so the store holds tenant ids, hostnames, ports and ages, and nothing else.

use losos_registrar::Snapshot;
use serde::Deserialize;
use std::time::Duration;

use crate::settings::StoreSettings;

/// Budget for one round trip. The store is in the same cloud as the function;
/// anything slower than this is an outage, and the request budget it has to
/// fit inside is the registrar's five seconds.
const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store request: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("store answered {0}")]
    Status(reqwest::StatusCode),
    #[error("store error: {0}")]
    Redis(String),
    #[error("stored snapshot is not valid JSON: {0}")]
    Corrupt(#[from] serde_json::Error),
    #[error("store answered a {0} where a string or null was expected")]
    Shape(&'static str),
}

/// An Upstash-compatible Redis, holding the snapshot under one key.
#[derive(Debug, Clone)]
pub struct Store {
    client: reqwest::Client,
    url: String,
    token: String,
    key: String,
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
}

impl Store {
    pub fn new(settings: &StoreSettings) -> Result<Self, StoreError> {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("losos-edge-vercel/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            url: settings.url.clone(),
            token: settings.token.clone(),
            key: settings.key.clone(),
        })
    }

    /// The saved snapshot, or `None` when nothing has been saved yet.
    pub async fn load(&self) -> Result<Option<Snapshot>, StoreError> {
        let reply = self.command(&["GET", &self.key]).await?;
        match reply {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(json)) => Ok(Some(serde_json::from_str(&json)?)),
            Some(serde_json::Value::Array(_)) => Err(StoreError::Shape("array")),
            Some(serde_json::Value::Object(_)) => Err(StoreError::Shape("object")),
            Some(serde_json::Value::Number(_)) => Err(StoreError::Shape("number")),
            Some(serde_json::Value::Bool(_)) => Err(StoreError::Shape("boolean")),
        }
    }

    /// Replace the saved snapshot.
    pub async fn save(&self, snapshot: &Snapshot) -> Result<(), StoreError> {
        let json = serde_json::to_string(snapshot)?;
        self.command(&["SET", &self.key, &json]).await?;
        Ok(())
    }

    /// One Redis command, as Upstash takes it: `POST <url>` with the command as
    /// a JSON array. The `result` is handed back as-is for the caller to shape.
    async fn command(&self, argv: &[&str]) -> Result<Option<serde_json::Value>, StoreError> {
        let response = self
            .client
            .post(&self.url)
            .bearer_auth(&self.token)
            .json(&argv)
            .send()
            .await?;
        let status = response.status();
        // Upstash answers 4xx with `{"error": "..."}`; read it before judging
        // the status so the log carries the reason, not just the number.
        let reply: Reply = match response.json().await {
            Ok(reply) => reply,
            Err(_) if !status.is_success() => return Err(StoreError::Status(status)),
            Err(e) => return Err(StoreError::Transport(e)),
        };
        if let Some(error) = reply.error {
            return Err(StoreError::Redis(error));
        }
        if !status.is_success() {
            return Err(StoreError::Status(status));
        }
        Ok(reply.result)
    }
}
