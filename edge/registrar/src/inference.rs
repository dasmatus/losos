//! AI models too large for one box, run across a group of boxes.
//!
//! A box that lends memory runs llama.cpp's RPC server as its `shared` user
//! (`losos.inference.lend`) and says how much it lends on every heartbeat.
//! Its tunnel carries a second service, `<id>:llm`, that only the model
//! *head* on this edge reaches. For a model in the operator's catalogue
//! (`--inference-models`), the edge picks boxes until their memory covers
//! the model, starts one head for the group (`losos-llm-head@<group>`, a
//! `llama-server` that spreads the layers over the boxes in proportion to
//! what each lends) and talks to it over a Unix socket.
//!
//! The head runs here, so the edge counts the tokens of every request
//! itself: a box can make an answer worse, but it cannot make it cost more.
//! Billing is the market's ([`crate::meter`]); this module only keeps the
//! groups and answers requests.
//!
//! What it cannot do: the boxes in a group see the prompts and answers that
//! pass through them, and the layers talk over home connections, so a large
//! model answers a few tokens a second.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::market::MarketError;
use crate::meter::{EndReason, MAX_TAPS};

/// How long a box in a group may be gone before the group breaks. Long
/// enough for the boxes' nightly reboot.
pub const GRACE_SECS: u64 = 15 * 60;
/// How long a group nobody uses is kept before its boxes are let go.
pub const IDLE_SECS: u64 = 60 * 60;
/// How long a head may take to load its model over the boxes' connections
/// before the group is given up.
pub const LOAD_SECS: u64 = 30 * 60;
/// How long a head that was just started may take to open its socket.
pub const START_SECS: u64 = 60;
/// Starts in a row a head may fail before its group is given up.
pub const MAX_RESTARTS: u32 = 5;
/// How long a box whose group failed to load is left out of new groups.
pub const COOLDOWN_SECS: u64 = 60 * 60;
/// How long one answer may take.
pub const ANSWER_SECS: u64 = 15 * 60;
/// The largest answer a head may send back.
const MAX_REPLY: usize = 4 << 20;

/// One model in the operator's catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    /// The GGUF file the head loads, on this edge.
    pub file: String,
    /// Memory the boxes of a group must lend together, MiB.
    #[serde(rename = "needMiB")]
    pub need_mib: u64,
    /// Context length, tokens.
    pub context: u32,
    /// USDC base units per million prompt tokens.
    pub price_in: u64,
    /// USDC base units per million answer tokens.
    pub price_out: u64,
    pub licence: String,
}

/// Read and check the catalogue.
///
/// # Errors
/// A message naming what is wrong.
pub fn parse_models(bytes: &[u8]) -> Result<Vec<Model>, String> {
    let models: Vec<Model> =
        serde_json::from_slice(bytes).map_err(|e| format!("the model catalogue: {e}"))?;
    let mut seen = BTreeSet::new();
    for m in &models {
        let ok_id = !m.id.is_empty()
            && m.id.len() <= 64
            && m.id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !ok_id {
            return Err(format!("model id {:?} is not [a-z0-9-]", m.id));
        }
        if !seen.insert(m.id.clone()) {
            return Err(format!("model id {} is listed twice", m.id));
        }
        if m.need_mib == 0 || m.context == 0 {
            return Err(format!("model {} needs memory and a context length", m.id));
        }
        if m.price_in == 0 || m.price_out == 0 {
            return Err(format!("model {} needs both prices", m.id));
        }
    }
    Ok(models)
}

/// Where the edge keeps what it needs to run heads.
#[derive(Debug, Clone)]
pub struct InferenceOpts {
    pub models_file: String,
    /// The edge ports of the boxes' `:llm` services, one per tunnel port.
    pub port_range: (u16, u16),
    /// Where the head units' environment files are written.
    pub env_dir: String,
    /// Where the heads' sockets are.
    pub socket_dir: String,
}

/// A box in a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub tenant: String,
    pub mib: u64,
    /// The edge port of its `:llm` service.
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupState {
    /// The head is loading the model onto the boxes.
    Loading,
    /// The head answers.
    Ready,
    /// The head stopped, usually because a box went away for a moment. It is
    /// started again once every box is back.
    Down,
}

/// Boxes running one model together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub model: String,
    /// The family of USDC networks every member has a payout address on.
    pub family: String,
    pub members: Vec<Member>,
    pub state: GroupState,
    pub created_at: u64,
    /// When the head was last started.
    pub started_at: u64,
    /// Members not lending now, and since when.
    #[serde(default)]
    pub missing: BTreeMap<String, u64>,
    #[serde(default)]
    pub idle_since: Option<u64>,
    /// Times the head was started again since it last answered.
    #[serde(default)]
    pub restarts: u32,
}

/// What a head's `/health` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Health {
    Up,
    Loading,
    /// Nothing listens: the head is not running.
    Dead,
}

/// Groups survive a restart of the registrar: the heads are their own units
/// and keep running.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    groups: BTreeMap<String, Group>,
    /// Boxes left out of new groups until then, unix seconds.
    #[serde(default)]
    cooldown: BTreeMap<String, u64>,
}

/// What a head is started with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadSpec {
    pub model_file: String,
    pub ports: Vec<u16>,
    /// Each member's share of the layers, in the order of `ports`.
    pub split: Vec<u64>,
    pub context: u32,
}

impl HeadSpec {
    /// The head unit's environment file.
    #[must_use]
    pub fn env(&self) -> String {
        let join = |v: Vec<String>| v.join(",");
        format!(
            "LLM_MODEL={}\nLLM_RPC={}\nLLM_SPLIT={}\nLLM_CTX={}\n",
            self.model_file,
            join(
                self.ports
                    .iter()
                    .map(|p| format!("127.0.0.1:{p}"))
                    .collect()
            ),
            join(self.split.iter().map(u64::to_string).collect()),
            self.context,
        )
    }
}

pub type HeadFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

/// Starting, stopping and asking heads. [`SystemdHeads`] on an edge; tests
/// put a fake in its place.
pub trait Heads: Send + Sync {
    fn start<'a>(&'a self, group: &'a str, spec: &'a HeadSpec) -> HeadFuture<'a, ()>;
    fn stop<'a>(&'a self, group: &'a str) -> HeadFuture<'a, ()>;
    /// One HTTP request to the group's head: status and body.
    fn request<'a>(
        &'a self,
        group: &'a str,
        method: &'a str,
        path: &'a str,
        body: Option<Vec<u8>>,
        timeout: Duration,
    ) -> HeadFuture<'a, (u16, Vec<u8>)>;
}

/// Heads as `losos-llm-head@<group>` units, reached on
/// `<socket_dir>/<group>.sock`.
pub struct SystemdHeads {
    env_dir: PathBuf,
    socket_dir: PathBuf,
}

impl SystemdHeads {
    #[must_use]
    pub fn new(opts: &InferenceOpts) -> Self {
        Self {
            env_dir: PathBuf::from(&opts.env_dir),
            socket_dir: PathBuf::from(&opts.socket_dir),
        }
    }

    async fn systemctl(verb: &str, group: &str) -> Result<(), String> {
        let unit = format!("losos-llm-head@{group}.service");
        let out = tokio::process::Command::new("systemctl")
            .args([verb, "--no-block", &unit])
            .output()
            .await
            .map_err(|e| format!("systemctl: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "systemctl {verb} {unit}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
}

impl Heads for SystemdHeads {
    fn start<'a>(&'a self, group: &'a str, spec: &'a HeadSpec) -> HeadFuture<'a, ()> {
        Box::pin(async move {
            let path = self.env_dir.join(format!("{group}.env"));
            let tmp = self.env_dir.join(format!(".{group}.env.tmp"));
            tokio::fs::write(&tmp, spec.env())
                .await
                .map_err(|e| format!("{}: {e}", tmp.display()))?;
            tokio::fs::rename(&tmp, &path)
                .await
                .map_err(|e| format!("{}: {e}", path.display()))?;
            // A restart, so a head that is still up picks up the new members.
            Self::systemctl("restart", group).await
        })
    }

    fn stop<'a>(&'a self, group: &'a str) -> HeadFuture<'a, ()> {
        Box::pin(async move {
            Self::systemctl("stop", group).await?;
            let _ = tokio::fs::remove_file(self.env_dir.join(format!("{group}.env"))).await;
            Ok(())
        })
    }

    fn request<'a>(
        &'a self,
        group: &'a str,
        method: &'a str,
        path: &'a str,
        body: Option<Vec<u8>>,
        timeout: Duration,
    ) -> HeadFuture<'a, (u16, Vec<u8>)> {
        Box::pin(async move {
            let socket = self.socket_dir.join(format!("{group}.sock"));
            tokio::time::timeout(timeout, unix_http(&socket, method, path, body))
                .await
                .map_err(|_| "the model took too long to answer".to_string())?
        })
    }
}

/// One HTTP/1.1 request over a Unix socket, `Connection: close`.
async fn unix_http(
    socket: &Path,
    method: &str,
    path: &str,
    body: Option<Vec<u8>>,
) -> Result<(u16, Vec<u8>), String> {
    let mut stream = tokio::net::UnixStream::connect(socket)
        .await
        .map_err(|e| format!("the model's head is not running: {e}"))?;
    let body = body.unwrap_or_default();
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: head\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    stream.write_all(&body).await.map_err(|e| e.to_string())?;
    let mut reply = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let n = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if reply.len() + n > MAX_REPLY {
            return Err("the model's answer is too large".into());
        }
        reply.extend_from_slice(&chunk[..n]);
    }
    parse_http(&reply)
}

/// Status and body of a whole HTTP/1.1 response. Chunked bodies are refused:
/// a head asked with `Connection: close` and no streaming sends a length.
fn parse_http(reply: &[u8]) -> Result<(u16, Vec<u8>), String> {
    let split = reply
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("the model's head sent no headers")?;
    let head = std::str::from_utf8(&reply[..split]).map_err(|_| "bad headers")?;
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("the model's head sent no status")?;
    let mut length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if name == "transfer-encoding" {
            return Err("the model's head sent a chunked answer".into());
        }
        if name == "content-length" {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let body = &reply[split + 4..];
    let body = match length {
        Some(n) if n <= body.len() => &body[..n],
        Some(_) => return Err("the model's answer was cut off".into()),
        None => body,
    };
    Ok((status, body.to_vec()))
}

/// Pick boxes for `model` from `lenders` (id, tunnel port, MiB): the
/// largest first, until their memory covers it, at most [`MAX_TAPS`]. A box
/// already in a group, cooling down, or that `eligible` turns away is
/// skipped. `None` when the boxes that are left cannot hold it.
#[must_use]
pub fn pick(
    need_mib: u64,
    lenders: &[(String, u16, u64)],
    taken: &BTreeSet<String>,
    eligible: impl Fn(&str) -> bool,
    port_of: impl Fn(u16) -> Option<u16>,
) -> Option<Vec<Member>> {
    let mut free: Vec<&(String, u16, u64)> = lenders
        .iter()
        .filter(|(id, _, mib)| *mib > 0 && !taken.contains(id) && eligible(id))
        .collect();
    free.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let mut out = Vec::new();
    let mut sum = 0u64;
    for (id, tunnel, mib) in free {
        if sum >= need_mib || out.len() >= MAX_TAPS {
            break;
        }
        let Some(port) = port_of(*tunnel) else {
            continue;
        };
        sum = sum.saturating_add(*mib);
        out.push(Member {
            tenant: id.clone(),
            mib: *mib,
            port,
        });
    }
    (sum >= need_mib).then_some(out)
}

/// What one model looks like to a buyer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelView {
    pub id: String,
    pub name: String,
    pub need_mib: u64,
    pub context: u32,
    /// Base units per million tokens, as decimal text.
    pub price_in: String,
    pub price_out: String,
    pub licence: String,
    /// Memory the boxes lending now offer together, MiB.
    pub lent_mib: u64,
    /// The running group's state for this family, if one runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boxes: Option<usize>,
}

/// A group that stopped for good, and why its meters end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    pub group: String,
    pub reason: EndReason,
}

/// What one served request used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub content: String,
    pub finish_reason: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

pub struct Inference {
    opts: InferenceOpts,
    models: Vec<Model>,
    tunnel_lo: u16,
    heads: Arc<dyn Heads>,
    state_file: PathBuf,
    saved: Mutex<Saved>,
}

fn now_secs() -> u64 {
    crate::market::now_secs()
}

impl Inference {
    /// Read the catalogue and the groups kept from before.
    ///
    /// # Errors
    /// The catalogue does not parse, or the groups file is unreadable.
    pub async fn open(
        opts: InferenceOpts,
        tunnel_range: (u16, u16),
        state_file: PathBuf,
        heads: Arc<dyn Heads>,
    ) -> Result<Self, String> {
        let bytes = tokio::fs::read(&opts.models_file)
            .await
            .map_err(|e| format!("{}: {e}", opts.models_file))?;
        let models = parse_models(&bytes)?;
        if u32::from(opts.port_range.1).saturating_sub(u32::from(opts.port_range.0))
            < u32::from(tunnel_range.1).saturating_sub(u32::from(tunnel_range.0))
        {
            return Err("--llm-port-range is smaller than the tunnel port range".into());
        }
        let saved = match tokio::fs::read(&state_file).await {
            Ok(b) => {
                serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", state_file.display()))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Saved::default(),
            Err(e) => return Err(format!("{}: {e}", state_file.display())),
        };
        Ok(Self {
            opts,
            models,
            tunnel_lo: tunnel_range.0,
            heads,
            state_file,
            saved: Mutex::new(saved),
        })
    }

    #[must_use]
    pub fn models(&self) -> &[Model] {
        &self.models
    }

    #[must_use]
    pub fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    /// The edge port of the `:llm` service of the box on tunnel port
    /// `tunnel_port`.
    #[must_use]
    pub fn llm_port(&self, tunnel_port: u16) -> Option<u16> {
        let offset = tunnel_port.checked_sub(self.tunnel_lo)?;
        let port = self.opts.port_range.0.checked_add(offset)?;
        (port <= self.opts.port_range.1).then_some(port)
    }

    async fn save(&self, saved: &Saved) {
        let tmp = self.state_file.with_extension("json.tmp");
        let bytes = match serde_json::to_vec_pretty(saved) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("could not write the model groups: {e}");
                return;
            }
        };
        if let Err(e) = async {
            tokio::fs::write(&tmp, bytes).await?;
            tokio::fs::rename(&tmp, &self.state_file).await
        }
        .await
        {
            tracing::error!(
                "could not write the model groups to {}: {e}",
                self.state_file.display()
            );
        }
    }

    fn spec(model: &Model, members: &[Member]) -> HeadSpec {
        HeadSpec {
            model_file: model.file.clone(),
            ports: members.iter().map(|m| m.port).collect(),
            split: members.iter().map(|m| m.mib).collect(),
            context: model.context,
        }
    }

    /// The group that runs `model` for buyers paying on `family`: the one
    /// running, or a new one picked from `lenders` and started now.
    /// `eligible` says whether a box can be paid on that family.
    ///
    /// # Errors
    /// [`MarketError::Unavailable`] when the boxes lending now cannot hold
    /// the model.
    pub async fn group_for(
        &self,
        model_id: &str,
        family: &str,
        lenders: &[(String, u16, u64)],
        eligible: impl Fn(&str) -> bool,
    ) -> Result<Group, MarketError> {
        let model = self
            .model(model_id)
            .ok_or(MarketError::Invalid("this edge does not run that model"))?;
        let now = now_secs();
        let mut saved = self.saved.lock().await;
        if let Some(g) = saved
            .groups
            .values()
            .find(|g| g.model == model_id && g.family == family)
        {
            return Ok(g.clone());
        }
        saved.cooldown.retain(|_, until| *until > now);
        let taken: BTreeSet<String> = saved
            .groups
            .values()
            .flat_map(|g| g.members.iter().map(|m| m.tenant.clone()))
            .chain(saved.cooldown.keys().cloned())
            .collect();
        let members = pick(model.need_mib, lenders, &taken, eligible, |p| {
            self.llm_port(p)
        })
        .ok_or(MarketError::Unavailable(
            "not enough boxes lend memory for this model right now",
        ))?;
        let id = crate::identity::random_hex(8)
            .ok_or_else(|| MarketError::Store("no randomness".into()))?;
        let group = Group {
            id: id.clone(),
            model: model_id.to_string(),
            family: family.to_string(),
            members,
            state: GroupState::Loading,
            created_at: now,
            started_at: now,
            missing: BTreeMap::new(),
            idle_since: None,
            restarts: 0,
        };
        if let Err(e) = self
            .heads
            .start(&id, &Self::spec(model, &group.members))
            .await
        {
            tracing::error!("could not start the head of group {id}: {e}");
            return Err(MarketError::Unavailable("the model could not be started"));
        }
        tracing::info!(
            "group {id} runs {model_id} on {}",
            group
                .members
                .iter()
                .map(|m| format!("{} ({} MiB)", m.tenant, m.mib))
                .collect::<Vec<_>>()
                .join(", ")
        );
        saved.groups.insert(id, group.clone());
        self.save(&saved).await;
        Ok(group)
    }

    pub async fn group(&self, id: &str) -> Option<Group> {
        self.saved.lock().await.groups.get(id).cloned()
    }

    /// Every model as buyers on `family` see it.
    pub async fn views(&self, family: &str, lenders: &[(String, u16, u64)]) -> Vec<ModelView> {
        let saved = self.saved.lock().await;
        let lent: u64 = lenders.iter().map(|l| l.2).sum();
        self.models
            .iter()
            .map(|m| {
                let g = saved
                    .groups
                    .values()
                    .find(|g| g.model == m.id && g.family == family);
                ModelView {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    need_mib: m.need_mib,
                    context: m.context,
                    price_in: m.price_in.to_string(),
                    price_out: m.price_out.to_string(),
                    licence: m.licence.clone(),
                    lent_mib: lent,
                    group: g.map(|g| g.state),
                    boxes: g.map(|g| g.members.len()),
                }
            })
            .collect()
    }

    /// The group a box serves in, if any.
    pub async fn group_of(&self, tenant: &str) -> Option<Group> {
        self.saved
            .lock()
            .await
            .groups
            .values()
            .find(|g| g.members.iter().any(|m| m.tenant == tenant))
            .cloned()
    }

    /// Look after the groups once: note boxes gone and back, start heads
    /// again, mark loaded heads ready, and let go of groups that broke or that
    /// nobody uses. `in_use` says whether a group still has buyers. Returns
    /// the groups that broke, whose buyers' meters end.
    pub async fn pass(
        &self,
        lenders: &[(String, u16, u64)],
        in_use: impl Fn(&str) -> bool,
    ) -> Vec<Broken> {
        let now = now_secs();
        let lent: BTreeMap<&str, u64> = lenders
            .iter()
            .map(|(id, _, mib)| (id.as_str(), *mib))
            .collect();
        let groups: Vec<Group> = self.saved.lock().await.groups.values().cloned().collect();
        let mut broken = Vec::new();
        let mut gone = Vec::new();
        let mut updated = Vec::new();
        for mut g in groups {
            let before = g.clone();
            for m in &g.members {
                if lent.get(m.tenant.as_str()).is_some_and(|mib| *mib >= m.mib) {
                    g.missing.remove(&m.tenant);
                } else {
                    g.missing.entry(m.tenant.clone()).or_insert(now);
                }
            }
            if g.missing
                .values()
                .any(|since| now.saturating_sub(*since) >= GRACE_SECS)
            {
                broken.push(Broken {
                    group: g.id.clone(),
                    reason: EndReason::GroupLeft,
                });
                gone.push((g.id.clone(), Vec::new()));
                continue;
            }
            if in_use(&g.id) {
                g.idle_since = None;
            } else if now.saturating_sub(*g.idle_since.get_or_insert(now)) >= IDLE_SECS {
                gone.push((g.id.clone(), Vec::new()));
                continue;
            }
            let health = self.health(&g.id).await;
            let since_start = now.saturating_sub(g.started_at);
            match (g.state, health) {
                (GroupState::Loading | GroupState::Down, Health::Up) => {
                    g.state = GroupState::Ready;
                    g.restarts = 0;
                }
                (GroupState::Loading, Health::Dead) if since_start >= START_SECS => {
                    g.state = GroupState::Down;
                }
                (GroupState::Loading, _) if since_start >= LOAD_SECS && g.missing.is_empty() => {
                    g.state = GroupState::Down;
                    g.restarts = MAX_RESTARTS;
                }
                (GroupState::Ready, Health::Dead | Health::Loading) => g.state = GroupState::Down,
                _ => {}
            }
            if g.state == GroupState::Down && g.restarts >= MAX_RESTARTS {
                // It never came up, or keeps falling over: some box in it is
                // not serving what it claims. Its boxes sit out a while.
                broken.push(Broken {
                    group: g.id.clone(),
                    reason: EndReason::GroupFailed,
                });
                let members = g.members.iter().map(|m| m.tenant.clone()).collect();
                gone.push((g.id.clone(), members));
                continue;
            }
            if g.state == GroupState::Down && g.missing.is_empty() {
                // Every box is back: start the head again.
                if let Some(model) = self.model(&g.model) {
                    match self
                        .heads
                        .start(&g.id, &Self::spec(model, &g.members))
                        .await
                    {
                        Ok(()) => {
                            g.state = GroupState::Loading;
                            g.started_at = now;
                            g.restarts += 1;
                        }
                        Err(e) => tracing::warn!("restarting group {}: {e}", g.id),
                    }
                }
            }
            if g != before {
                updated.push(g);
            }
        }
        if gone.is_empty() && updated.is_empty() {
            return broken;
        }
        for (id, _) in &gone {
            if let Err(e) = self.heads.stop(id).await {
                tracing::warn!("stopping group {id}: {e}");
            }
        }
        let mut saved = self.saved.lock().await;
        for (id, cool) in gone {
            saved.groups.remove(&id);
            for tenant in cool {
                saved.cooldown.insert(tenant, now + COOLDOWN_SECS);
            }
        }
        for g in updated {
            if let Some(slot) = saved.groups.get_mut(&g.id) {
                // A group a quote just reused keeps its newest state.
                *slot = g;
            }
        }
        self.save(&saved).await;
        broken
    }

    async fn health(&self, group: &str) -> Health {
        match self
            .heads
            .request(group, "GET", "/health", None, Duration::from_secs(5))
            .await
        {
            Ok((200, _)) => Health::Up,
            // llama-server answers 503 while it loads.
            Ok(_) => Health::Loading,
            Err(_) => Health::Dead,
        }
    }

    /// How many tokens `messages` make once the model's chat template is
    /// applied: what the request will be billed for its prompt.
    ///
    /// # Errors
    /// The head did not answer.
    pub async fn prompt_tokens(&self, group: &str, messages: &Value) -> Result<u64, MarketError> {
        let prompt = self
            .call(
                group,
                "/apply-template",
                &json!({ "messages": messages }),
                30,
            )
            .await?;
        let prompt = prompt["prompt"]
            .as_str()
            .ok_or(MarketError::Unavailable("the model did not answer"))?;
        let tokens = self
            .call(
                group,
                "/tokenize",
                &json!({ "content": prompt, "add_special": true }),
                30,
            )
            .await?;
        tokens["tokens"]
            .as_array()
            .map(|t| t.len() as u64)
            .ok_or(MarketError::Unavailable("the model did not answer"))
    }

    /// Ask the group's head. `body` holds only the fields the edge lets
    /// through ([`chat_body`]).
    ///
    /// # Errors
    /// The head did not answer, or answered with an error.
    pub async fn chat(&self, group: &str, body: &Value) -> Result<Answer, MarketError> {
        let v = self
            .call(group, "/v1/chat/completions", body, ANSWER_SECS)
            .await?;
        let choice = &v["choices"][0];
        let content = choice["message"]["content"]
            .as_str()
            .ok_or(MarketError::Unavailable("the model did not answer"))?
            .to_string();
        let usage = &v["usage"];
        let (Some(tokens_in), Some(tokens_out)) = (
            usage["prompt_tokens"].as_u64(),
            usage["completion_tokens"].as_u64(),
        ) else {
            return Err(MarketError::Unavailable(
                "the model did not say what it used",
            ));
        };
        Ok(Answer {
            content,
            finish_reason: choice["finish_reason"].as_str().unwrap_or("").to_string(),
            tokens_in,
            tokens_out,
        })
    }

    async fn call(
        &self,
        group: &str,
        path: &str,
        body: &Value,
        secs: u64,
    ) -> Result<Value, MarketError> {
        let bytes = serde_json::to_vec(body)?;
        let (status, reply) = self
            .heads
            .request(group, "POST", path, Some(bytes), Duration::from_secs(secs))
            .await
            .map_err(|e| {
                tracing::warn!("group {group} {path}: {e}");
                MarketError::Unavailable("the model is not answering right now; try again soon")
            })?;
        if status == 400 {
            return Err(MarketError::Invalid("the model refused the request"));
        }
        if status != 200 {
            tracing::warn!("group {group} {path}: the head answered {status}");
            return Err(MarketError::Unavailable(
                "the model is not answering right now; try again soon",
            ));
        }
        serde_json::from_slice(&reply)
            .map_err(|_| MarketError::Unavailable("the model did not answer"))
    }
}

#[cfg(test)]
impl Inference {
    /// Move every group's clocks `secs` into the past, as if that much time
    /// had gone by.
    pub(crate) async fn backdate(&self, secs: u64) {
        let mut saved = self.saved.lock().await;
        for g in saved.groups.values_mut() {
            g.created_at = g.created_at.saturating_sub(secs);
            g.started_at = g.started_at.saturating_sub(secs);
            for since in g.missing.values_mut() {
                *since = since.saturating_sub(secs);
            }
            if let Some(i) = g.idle_since.as_mut() {
                *i = i.saturating_sub(secs);
            }
        }
    }
}

/// The longest prompt a buyer may send, bytes.
pub const MAX_PROMPT_BYTES: usize = 256 * 1024;

/// What a buyer asked of a model, checked. Only these fields reach the head.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatAsk {
    pub messages: Value,
    pub max_tokens: u64,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Vec<String>,
}

/// Check a buyer's request: messages with a known role and text content,
/// one to `context` answer tokens, sane sampling and at most four stop
/// strings.
///
/// # Errors
/// What is wrong with it.
pub fn chat_ask(req: &Value, context: u32) -> Result<ChatAsk, MarketError> {
    let messages = req
        .get("messages")
        .and_then(Value::as_array)
        .filter(|m| !m.is_empty() && m.len() <= 256)
        .ok_or(MarketError::Invalid("send one to 256 messages"))?;
    let mut clean = Vec::with_capacity(messages.len());
    let mut bytes = 0usize;
    for m in messages {
        let role = m.get("role").and_then(Value::as_str).unwrap_or("");
        if !matches!(role, "system" | "user" | "assistant") {
            return Err(MarketError::Invalid(
                "a message's role is system, user or assistant",
            ));
        }
        let content = m
            .get("content")
            .and_then(Value::as_str)
            .ok_or(MarketError::Invalid("a message's content is text"))?;
        bytes += content.len();
        clean.push(json!({ "role": role, "content": content }));
    }
    if bytes > MAX_PROMPT_BYTES {
        return Err(MarketError::Invalid("the messages are too long"));
    }
    let max_tokens = match req.get("maxTokens") {
        None | Some(Value::Null) => 256,
        Some(v) => v
            .as_u64()
            .ok_or(MarketError::Invalid("maxTokens is a whole number"))?,
    };
    if max_tokens == 0 || max_tokens > u64::from(context) {
        return Err(MarketError::Invalid(
            "maxTokens is from one to the model's context length",
        ));
    }
    let unit = |key: &str, hi: f64| -> Result<Option<f64>, MarketError> {
        match req.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_f64()
                .filter(|x| x.is_finite() && (0.0..=hi).contains(x))
                .map(Some)
                .ok_or(MarketError::Invalid("a sampling setting is out of range")),
        }
    };
    let temperature = unit("temperature", 2.0)?;
    let top_p = unit("topP", 1.0)?;
    let stop = match req.get("stop") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a
            .iter()
            .map(|s| s.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or(MarketError::Invalid("stop is text"))?,
        Some(_) => return Err(MarketError::Invalid("stop is text")),
    };
    if stop.len() > 4 || stop.iter().any(|s| s.is_empty() || s.len() > 64) {
        return Err(MarketError::Invalid(
            "at most four stop strings, each short",
        ));
    }
    Ok(ChatAsk {
        messages: Value::Array(clean),
        max_tokens,
        temperature,
        top_p,
        stop,
    })
}

/// The body the head gets for `ask`: never streamed, never more than
/// `max_tokens` answer tokens.
#[must_use]
pub fn chat_body(ask: &ChatAsk) -> Value {
    let mut body = json!({
        "messages": ask.messages,
        "max_tokens": ask.max_tokens,
        "n_predict": ask.max_tokens,
        "stream": false,
    });
    if let Some(t) = ask.temperature {
        body["temperature"] = json!(t);
    }
    if let Some(p) = ask.top_p {
        body["top_p"] = json!(p);
    }
    if !ask.stop.is_empty() {
        body["stop"] = json!(ask.stop);
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lenders(list: &[(&str, u16, u64)]) -> Vec<(String, u16, u64)> {
        list.iter()
            .map(|(a, b, c)| ((*a).to_string(), *b, *c))
            .collect()
    }

    #[test]
    fn groups_take_the_largest_boxes_until_the_model_fits() {
        let l = lenders(&[
            ("a", 1, 4096),
            ("b", 2, 16384),
            ("c", 3, 8192),
            ("d", 4, 8192),
        ]);
        let got = pick(24_000, &l, &BTreeSet::new(), |_| true, |p| Some(p + 100)).unwrap();
        let ids: Vec<&str> = got.iter().map(|m| m.tenant.as_str()).collect();
        assert_eq!(ids, ["b", "c"]);
        assert_eq!(got[0].port, 102);
        assert!(pick(40_000, &l, &BTreeSet::new(), |_| true, Some).is_none());
    }

    #[test]
    fn groups_skip_taken_ineligible_and_portless_boxes() {
        let l = lenders(&[("a", 1, 8192), ("b", 2, 8192), ("c", 3, 8192)]);
        let taken: BTreeSet<String> = ["a".to_string()].into();
        let got = pick(8192, &l, &taken, |id| id != "b", Some).unwrap();
        assert_eq!(got[0].tenant, "c");
        assert!(pick(8192, &l, &taken, |id| id != "c", |p| (p != 2).then_some(p)).is_none());
    }

    #[test]
    fn a_group_has_at_most_sixteen_boxes() {
        let many: Vec<(String, u16, u64)> =
            (0..40u16).map(|n| (format!("b{n:02}"), n, 1)).collect();
        assert!(pick(17, &many, &BTreeSet::new(), |_| true, Some).is_none());
        assert_eq!(
            pick(16, &many, &BTreeSet::new(), |_| true, Some)
                .unwrap()
                .len(),
            16
        );
    }

    #[test]
    fn the_head_is_told_every_box_and_its_share() {
        let spec = HeadSpec {
            model_file: "/nix/store/x-model.gguf".into(),
            ports: vec![23001, 23007],
            split: vec![8192, 24576],
            context: 4096,
        };
        assert_eq!(
            spec.env(),
            "LLM_MODEL=/nix/store/x-model.gguf\nLLM_RPC=127.0.0.1:23001,127.0.0.1:23007\nLLM_SPLIT=8192,24576\nLLM_CTX=4096\n"
        );
    }

    #[test]
    fn the_catalogue_is_checked() {
        let good = br#"[{"id":"llama-3-70b","name":"Llama","file":"/m.gguf","needMiB":45000,"context":8192,"priceIn":300000,"priceOut":900000,"licence":"Llama 3"}]"#;
        let got = parse_models(good).unwrap();
        assert_eq!(got[0].need_mib, 45000);
        assert!(parse_models(br#"[{"id":"Bad id","name":"","file":"","needMiB":1,"context":1,"priceIn":1,"priceOut":1,"licence":""}]"#).is_err());
        let twice = br#"[{"id":"a","name":"","file":"","needMiB":1,"context":1,"priceIn":1,"priceOut":1,"licence":""},{"id":"a","name":"","file":"","needMiB":1,"context":1,"priceIn":1,"priceOut":1,"licence":""}]"#;
        assert!(parse_models(twice).is_err());
        let free = br#"[{"id":"a","name":"","file":"","needMiB":1,"context":1,"priceIn":0,"priceOut":1,"licence":""}]"#;
        assert!(parse_models(free).is_err());
    }

    #[test]
    fn only_known_fields_reach_the_head() {
        let req = json!({
            "messages": [{"role": "user", "content": "hi", "name": "x"}],
            "maxTokens": 10,
            "temperature": 0.5,
            "grammar": "root ::= .*",
            "n_probs": 50,
            "stream": true,
            "cache_prompt": true,
        });
        let body = chat_body(&chat_ask(&req, 4096).unwrap());
        let keys: BTreeSet<&str> = body
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "max_tokens",
                "messages",
                "n_predict",
                "stream",
                "temperature"
            ]
            .into()
        );
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"], json!([{"role": "user", "content": "hi"}]));
    }

    #[test]
    fn a_request_is_refused_when_it_is_malformed() {
        let ok = |v: Value| chat_ask(&v, 4096).is_ok();
        assert!(!ok(json!({})));
        assert!(!ok(json!({"messages": []})));
        assert!(!ok(json!({"messages": [{"role": "tool", "content": "x"}]})));
        assert!(!ok(
            json!({"messages": [{"role": "user", "content": ["x"]}]})
        ));
        let m = json!([{"role": "user", "content": "x"}]);
        assert!(!ok(json!({"messages": m, "maxTokens": 0})));
        assert!(!ok(json!({"messages": m, "maxTokens": 5000})));
        assert!(!ok(json!({"messages": m, "maxTokens": -1})));
        assert!(!ok(json!({"messages": m, "temperature": 9})));
        assert!(!ok(
            json!({"messages": m, "stop": ["a", "b", "c", "d", "e"]})
        ));
        let long = "x".repeat(MAX_PROMPT_BYTES + 1);
        assert!(!ok(
            json!({"messages": [{"role": "user", "content": long}]})
        ));
        assert!(ok(json!({"messages": m, "stop": "\n"})));
    }

    #[test]
    fn http_answers_are_read_to_their_length() {
        let r = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}junk";
        assert_eq!(parse_http(r).unwrap(), (200, b"{}".to_vec()));
        assert!(parse_http(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n{}").is_err());
        assert!(
            parse_http(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}").is_err()
        );
        assert!(parse_http(b"garbage").is_err());
    }
}
