//! The two things `losos-ci` talks to: the edge's job queue (`/ci/*` on
//! the registrar, `edge/registrar/src/ci.rs`) and LosOS Git's internal
//! socket (`crate::forge::internal`). Each is a trait, so the dispatcher
//! runs against fakes in tests.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// A job as the edge hands it to a runner.
#[derive(Debug, Clone, Deserialize)]
pub struct Claimed {
    pub id: String,
    pub origin: String,
    pub job: EdgeSpec,
    /// The commit's tree, a gzipped tar.
    #[serde(with = "b64")]
    pub archive: Vec<u8>,
}

/// What the edge queues: the registrar's `JobSpec`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub name: String,
    pub payload: Value,
    pub timeout_secs: u32,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub commit: String,
}

/// A job as the edge reports it back to the box that asked.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeJob {
    pub id: String,
    pub state: String,
    #[serde(default)]
    pub runner: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i64>,
    #[serde(default)]
    pub outputs: Option<Map<String, Value>>,
    #[serde(default)]
    pub log: String,
    #[serde(default)]
    pub log_len: i64,
}

/// What a runner says about a job it took, to the edge or to this box.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    #[serde(default)]
    pub log: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Map<String, Value>>,
}

/// The edge's queue.
pub trait Mesh: Send + Sync {
    /// Whether there is an edge to talk to at all.
    fn reachable(&self) -> bool;
    fn submit(&self, spec: &EdgeSpec, archive: &[u8]) -> anyhow::Result<String>;
    /// The jobs this box submitted, their logs from the offsets given.
    fn jobs(&self, wanted: &[(String, i64)]) -> anyhow::Result<Vec<EdgeJob>>;
    fn cancel(&self, id: &str) -> anyhow::Result<()>;
    /// A job for this box's runner; `foreign`: other boxes' jobs too.
    fn claim(&self, foreign: bool, wait_secs: u32) -> anyhow::Result<Option<Claimed>>;
    /// Returns whether the job's box asked for it to stop.
    fn update(&self, id: &str, report: &Report) -> anyhow::Result<bool>;
}

/// A repository as LosOS Git describes it to Mesh CI.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub private: bool,
    pub default_branch: String,
    /// none, read, write or admin, for the person asked about.
    pub permission: String,
    /// The full ref that was resolved.
    pub r#ref: String,
    pub sha: Option<String>,
}

impl RepoInfo {
    pub fn can_read(&self) -> bool {
        self.permission != "none"
    }

    pub fn can_write(&self) -> bool {
        self.permission == "write" || self.permission == "admin"
    }
}

/// LosOS Git.
pub trait Forge: Send + Sync {
    /// `user` may be empty (nobody); `git_ref` empty is the default branch.
    fn info(&self, owner: &str, repo: &str, user: &str, git_ref: &str) -> anyhow::Result<RepoInfo>;
    fn archive(&self, owner: &str, repo: &str, sha: &str) -> anyhow::Result<Vec<u8>>;
    fn status(&self, owner: &str, repo: &str, sha: &str, status: &Value) -> anyhow::Result<()>;
}

mod b64 {
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer};

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD
            .decode(s.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

// ── the edge, by curl ─────────────────────────────────────────────────────

/// Where the edge is and who this box is to it.
#[derive(Debug, Clone)]
pub struct EdgeConfig {
    /// lososd's choice of edge (`/run/losos/edge-path.env`), read on every
    /// call because it changes when the box finds a LAN edge.
    pub path_file: PathBuf,
    /// lososd's marker for "no edge in reach".
    pub none_file: PathBuf,
    /// The configured registrar, when the path file is not there yet.
    pub registrar_url: String,
    pub appliance_id: String,
    /// The box's proxy token, as a systemd credential. `None` on a box
    /// with no edge configured: everything runs at home.
    pub token_file: Option<PathBuf>,
}

impl EdgeConfig {
    fn token(&self) -> Option<String> {
        let path = self.token_file.as_ref()?;
        let token = std::fs::read_to_string(path).ok()?;
        let token = token.trim();
        (!token.is_empty()).then(|| token.to_string())
    }

    /// From `LOSOS_CI_*`, set by modules/ci.nix. The token is a systemd
    /// credential, so only this service reads it.
    pub fn from_env() -> Option<EdgeConfig> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let token_file = std::env::var_os("CREDENTIALS_DIRECTORY")
            .map(|dir| PathBuf::from(dir).join("edge-token"))
            .filter(|p| p.exists())
            .or_else(|| var("LOSOS_CI_TOKEN_FILE").map(PathBuf::from));
        Some(EdgeConfig {
            path_file: var("LOSOS_EDGE_PATH_FILE")
                .unwrap_or_else(|| "/run/losos/edge-path.env".into())
                .into(),
            none_file: var("LOSOS_EDGE_NONE_FILE")
                .unwrap_or_else(|| "/run/losos/edge-none".into())
                .into(),
            registrar_url: var("LOSOS_CI_REGISTRAR_URL").unwrap_or_default(),
            appliance_id: var("LOSOS_CI_APPLIANCE_ID")?,
            token_file,
        })
    }

    /// The registrar to call now, or `None` with no edge in reach.
    pub fn url(&self) -> Option<String> {
        if self.none_file.exists() || self.token().is_none() {
            return None;
        }
        let from_path = std::fs::read_to_string(&self.path_file)
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|l| l.strip_prefix("LOSOS_EDGE_PATH_URL=").map(str::to_string))
            });
        let url = from_path.unwrap_or_else(|| self.registrar_url.clone());
        let url = url.trim().trim_end_matches('/').to_string();
        (url.starts_with("https://") || url.starts_with("http://")).then_some(url)
    }
}

pub struct Edge {
    pub config: EdgeConfig,
}

/// The largest answer the edge gives: a claim with a 24 MiB archive in
/// base64 and the job.
const MAX_EDGE_ANSWER: usize = 34 * 1024 * 1024;

impl Edge {
    /// One POST of `body` plus the credentials. The body goes to curl on
    /// stdin, so the token is never in an argument vector.
    fn post(&self, path: &str, mut body: Value, timeout: u32) -> anyhow::Result<(u16, Vec<u8>)> {
        let url = self.config.url().context("no edge in reach")?;
        let token = self.config.token().context("this box has no edge token")?;
        body["appliance_id"] = json!(self.config.appliance_id);
        body["token"] = json!(token);
        let body = serde_json::to_vec(&body)?;
        let mut child = Command::new("curl")
            .args(["--silent", "--show-error", "--max-redirs", "0"])
            .args(["--proto", "=https,http"])
            .args(["--max-time", &timeout.to_string()])
            .args(["--max-filesize", &MAX_EDGE_ANSWER.to_string()])
            .args(["--header", "Content-Type: application/json"])
            .args(["--data-binary", "@-"])
            .args(["--write-out", "\n%{http_code}"])
            .arg(format!("{url}{path}"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("running curl")?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&body)
                .context("sending the request to curl")?;
        }
        let out = child.wait_with_output()?;
        if !out.status.success() {
            anyhow::bail!(
                "the edge did not answer: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let mut raw = out.stdout;
        let cut = raw
            .iter()
            .rposition(|b| *b == b'\n')
            .context("curl gave no status")?;
        let code = std::str::from_utf8(&raw[cut + 1..])?.trim().parse()?;
        raw.truncate(cut);
        Ok((code, raw))
    }

    fn ok(&self, path: &str, body: Value, timeout: u32) -> anyhow::Result<Vec<u8>> {
        let (code, raw) = self.post(path, body, timeout)?;
        if !(200..300).contains(&code) {
            anyhow::bail!(
                "the edge answered {code} to {path}: {}",
                String::from_utf8_lossy(&raw)
                    .chars()
                    .take(300)
                    .collect::<String>()
            );
        }
        Ok(raw)
    }
}

impl Mesh for Edge {
    fn reachable(&self) -> bool {
        self.config.url().is_some()
    }

    fn submit(&self, spec: &EdgeSpec, archive: &[u8]) -> anyhow::Result<String> {
        use base64::Engine as _;
        let body = json!({
            "job": spec,
            "archive": base64::engine::general_purpose::STANDARD.encode(archive),
        });
        let raw = self.ok("/ci/submit", body, 120)?;
        let v: Value = serde_json::from_slice(&raw)?;
        v["id"]
            .as_str()
            .map(str::to_string)
            .context("the edge gave no job id")
    }

    fn jobs(&self, wanted: &[(String, i64)]) -> anyhow::Result<Vec<EdgeJob>> {
        let jobs: Vec<Value> = wanted
            .iter()
            .map(|(id, from)| json!({ "id": id, "logFrom": from }))
            .collect();
        let raw = self.ok("/ci/jobs", json!({ "jobs": jobs }), 30)?;
        #[derive(Deserialize)]
        struct Answer {
            jobs: Vec<EdgeJob>,
        }
        Ok(serde_json::from_slice::<Answer>(&raw)?.jobs)
    }

    fn cancel(&self, id: &str) -> anyhow::Result<()> {
        let (code, raw) = self.post("/ci/cancel", json!({ "id": id }), 30)?;
        // Gone already is cancelled enough.
        if (200..300).contains(&code) || code == 404 {
            Ok(())
        } else {
            anyhow::bail!(
                "the edge answered {code}: {}",
                String::from_utf8_lossy(&raw)
            )
        }
    }

    fn claim(&self, foreign: bool, wait_secs: u32) -> anyhow::Result<Option<Claimed>> {
        let wait = wait_secs.min(25);
        let (code, raw) = self.post(
            "/ci/claim",
            json!({ "foreign": foreign, "wait": wait }),
            wait + 60,
        )?;
        match code {
            204 => Ok(None),
            200 => Ok(Some(serde_json::from_slice(&raw)?)),
            _ => anyhow::bail!(
                "the edge answered {code} to a claim: {}",
                String::from_utf8_lossy(&raw)
                    .chars()
                    .take(300)
                    .collect::<String>()
            ),
        }
    }

    fn update(&self, id: &str, report: &Report) -> anyhow::Result<bool> {
        let mut body = serde_json::to_value(report)?;
        body["id"] = json!(id);
        let raw = self.ok("/ci/update", body, 30)?;
        let v: Value = serde_json::from_slice(&raw)?;
        Ok(v["cancel"].as_bool().unwrap_or(false))
    }
}

// ── LosOS Git, over its internal socket ────────────────────────────────────

/// One HTTP/1.0 exchange over a Unix socket: the answer's status and body.
/// HTTP/1.0, so the server closes the connection at the end of the body
/// and there is no chunking to undo.
pub fn unix_http(
    socket: &Path,
    method: &str,
    path: &str,
    content_type: Option<&str>,
    body: &[u8],
    max: u64,
    timeout: Duration,
) -> anyhow::Result<(u16, Vec<u8>)> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket)
        .with_context(|| format!("connecting to {}", socket.display()))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let mut head = format!(
        "{method} {path} HTTP/1.0\r\nHost: localhost\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(ct) = content_type {
        head.push_str(&format!("Content-Type: {ct}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    let mut raw = Vec::new();
    (&mut stream).take(max + 64 * 1024).read_to_end(&mut raw)?;
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .context("no end of headers")?;
    let status_line = std::str::from_utf8(&raw[..end])?
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let code = status_line
        .split_whitespace()
        .nth(1)
        .context("no status")?
        .parse()?;
    let body = raw.split_off(end + 4);
    if body.len() as u64 > max {
        anyhow::bail!("the answer is over {max} bytes");
    }
    Ok((code, body))
}

pub struct Git {
    pub socket: PathBuf,
}

fn segment(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

impl Forge for Git {
    fn info(&self, owner: &str, repo: &str, user: &str, git_ref: &str) -> anyhow::Result<RepoInfo> {
        let path = format!(
            "/internal/repos/{}/{}/info?user={}&ref={}",
            segment(owner),
            segment(repo),
            segment(user),
            segment(git_ref)
        );
        let (code, body) = unix_http(
            &self.socket,
            "GET",
            &path,
            None,
            &[],
            64 * 1024,
            Duration::from_secs(30),
        )?;
        if code == 404 {
            anyhow::bail!("there is no repository {owner}/{repo}");
        }
        if code != 200 {
            anyhow::bail!("LosOS Git answered {code} about {owner}/{repo}");
        }
        Ok(serde_json::from_slice(&body)?)
    }

    fn archive(&self, owner: &str, repo: &str, sha: &str) -> anyhow::Result<Vec<u8>> {
        let path = format!(
            "/internal/repos/{}/{}/archive/{}",
            segment(owner),
            segment(repo),
            segment(sha)
        );
        let (code, body) = unix_http(
            &self.socket,
            "GET",
            &path,
            None,
            &[],
            super::MAX_ARCHIVE_BYTES as u64,
            Duration::from_secs(120),
        )?;
        if code != 200 {
            anyhow::bail!("LosOS Git answered {code} for the commit's files");
        }
        Ok(body)
    }

    fn status(&self, owner: &str, repo: &str, sha: &str, status: &Value) -> anyhow::Result<()> {
        let path = format!(
            "/internal/repos/{}/{}/statuses/{}",
            segment(owner),
            segment(repo),
            segment(sha)
        );
        let (code, body) = unix_http(
            &self.socket,
            "POST",
            &path,
            Some("application/json"),
            &serde_json::to_vec(status)?,
            64 * 1024,
            Duration::from_secs(30),
        )?;
        if !(200..300).contains(&code) {
            anyhow::bail!(
                "LosOS Git answered {code} to a status: {}",
                String::from_utf8_lossy(&body)
            );
        }
        Ok(())
    }
}
