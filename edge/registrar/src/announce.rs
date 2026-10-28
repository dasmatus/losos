//! `losos-registrar announce` — the appliance-side registration client.
//!
//! Reads its per-appliance token (0600, persisted via /var, provisioned out of
//! band), POSTs `/register` once on start, then `/heartbeat` on a cadence.
//! Stateless: a 404 (unknown appliance on the edge, e.g. after the edge's
//! registry was wiped) triggers a fresh `/register`. All failures retry with
//! capped, jittered backoff — the edge pruning only removes after TTL, so
//! brief network blips don't kill the route.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use miette::{Context, IntoDiagnostic, Result};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::action::Action;
use crate::opts::AnnounceOpts;

/// First retry delay, and the value backoff resets to after a success.
const BACKOFF_MIN: Duration = Duration::from_secs(2);
/// Ceiling on the retry delay. Long enough not to hammer a down edge, short
/// enough that an appliance is back inside the heartbeat TTL soon after the
/// edge returns.
const BACKOFF_MAX: Duration = Duration::from_secs(60);
/// Give up on a single request after this long, so a black-holed edge cannot
/// stall the loop past its own backoff.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// TCP connect budget, separate from the whole-request budget: a connect that
/// hangs is the common failure when the edge VPS is unreachable.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize)]
struct RegisterReq<'a> {
    appliance_id: &'a str,
    token: &'a str,
    hostname: &'a str,
    /// The relay pass from this box's official edge (`crate::routes`), for a
    /// local edge to forward. Left out when there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    relay_pass: Option<&'a str>,
}

#[derive(Serialize)]
struct HeartbeatReq<'a> {
    appliance_id: &'a str,
    token: &'a str,
    /// Whether this box is free right now. See crate::idle for why it is a
    /// load average and not something cleverer.
    ///
    /// Sent on every heartbeat rather than only on change, because the edge
    /// ages it out: a report older than the heartbeat TTL counts as busy, so a
    /// box that goes quiet stops being scheduled onto without anyone having to
    /// notice it went quiet.
    idle: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    relay_pass: Option<&'a str>,
    /// The owner lets this edge manage the box's LosOS Cloud, LosOS Git and
    /// apps (`crate::server::manage`). Only lets the edge's page list it:
    /// the box decides what the edge may do on every call it makes.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    managed: bool,
    /// The memory this box lends to AI models now, MiB (`crate::inference`).
    /// Left out when it lends none.
    #[serde(skip_serializing_if = "Option::is_none")]
    llm_mib: Option<u64>,
}

/// The MiB in the lend file, re-read every heartbeat: the file is there only
/// while the box's RPC server runs, so a stopped server lends nothing on the
/// next heartbeat.
pub async fn read_lend(file: Option<&str>) -> Option<u64> {
    let text = tokio::fs::read_to_string(file?).await.ok()?;
    text.trim().parse::<u64>().ok().filter(|mib| *mib > 0)
}

/// The pass in `file`, re-read every time: lososd refreshes it every few
/// minutes. A missing, unreadable or empty file is no pass, quietly; the
/// edge only uses it for custom domains behind a local edge.
async fn read_pass(file: Option<&str>) -> Option<String> {
    let text = tokio::fs::read_to_string(file?).await.ok()?;
    let text = text.trim();
    crate::routes::well_formed_pass(text).then(|| text.to_string())
}

pub async fn run(opts: AnnounceOpts) -> Result<()> {
    let base = opts.registrar_url.trim_end_matches('/');
    // `https_only` refuses to follow a redirect that would downgrade the
    // token to cleartext. It is conditional on the configured scheme because
    // the VM test dials the registrar's loopback API over plain HTTP with no
    // Traefik in front; forcing it unconditionally would break that path
    // rather than secure it. In production `losos.proxy.registrarUrl` is
    // `https://register.<domain>`, so this is on.
    let https_only = base.starts_with("https://");
    if !https_only {
        warn!(
            target: Action::Announce.target(),
            "registrar URL {base} is not https; the appliance token crosses the network in cleartext",
        );
    }
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        // Identifies the appliance side in the edge's access log; an
        // unlabelled client is indistinguishable from a scanner.
        .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
        .https_only(https_only)
        .build()
        .into_diagnostic()
        .context("build HTTP client")?;

    let token = tokio::fs::read_to_string(&opts.token_file)
        .await
        .into_diagnostic()
        .with_context(|| format!("read token file {}", opts.token_file))?;
    let token = token.trim().to_string();
    if token.is_empty() {
        warn!(
            target: Action::Announce.target(),
            "token file {} is empty; the edge will reject every request",
            opts.token_file,
        );
    }

    let register_url = format!("{base}/register");
    let heartbeat_url = format!("{base}/heartbeat");

    let mut backoff = BACKOFF_MIN;
    let mut registered = false;
    // Tracked separately from `registered`: a heartbeat that fails for a
    // *transport* reason leaves us registered on the edge but out of contact,
    // and retrying that at the full heartbeat cadence is what made N
    // appliances re-converge in lockstep after an edge restart.
    let mut healthy = false;

    loop {
        let pass = read_pass(opts.relay_pass_file.as_deref()).await;
        if !registered {
            let req = RegisterReq {
                appliance_id: &opts.appliance_id,
                token: &token,
                hostname: &opts.hostname,
                relay_pass: pass.as_deref(),
            };
            match client.post(&register_url).json(&req).send().await {
                Ok(r) if r.status().is_success() => {
                    registered = true;
                    healthy = true;
                    backoff = BACKOFF_MIN;
                    info!(
                        target: Action::Register.target(),
                        "registered {} -> {}",
                        opts.appliance_id,
                        opts.hostname,
                    );
                }
                Ok(r) => {
                    error!(
                        target: Action::Register.target(),
                        "register rejected: {}",
                        r.status(),
                    );
                }
                Err(e) => {
                    error!(target: Action::Register.target(), "register error: {e}");
                }
            }
        }

        if registered {
            // Measured per heartbeat rather than cached: the whole value of
            // this bit is that it is current. An unreadable /proc/loadavg
            // reports BUSY, because "I could not tell" and "I am free" must
            // never be the same answer to the mesh.
            let idle = crate::idle::current_load()
                .is_some_and(|load| crate::idle::is_idle(load, opts.idle_load_threshold));
            let req = HeartbeatReq {
                appliance_id: &opts.appliance_id,
                token: &token,
                idle,
                relay_pass: pass.as_deref(),
                managed: match opts.managed_file.as_deref() {
                    Some(file) => tokio::fs::try_exists(file).await.unwrap_or(false),
                    None => false,
                },
                llm_mib: read_lend(opts.llm_lend_file.as_deref()).await,
            };
            match client.post(&heartbeat_url).json(&req).send().await {
                Ok(r) if r.status().is_success() => {
                    healthy = true;
                    backoff = BACKOFF_MIN;
                    if let Some(file) = opts.hidden_file.as_deref() {
                        let body = if r.status() == reqwest::StatusCode::OK {
                            r.text().await.ok()
                        } else {
                            None
                        };
                        if let Err(e) = record_hidden(file, body.as_deref()).await {
                            warn!(
                                target: Action::Heartbeat.target(),
                                "could not keep this box's Tor and I2P addresses in {file}: {e}",
                            );
                        }
                    }
                }
                Ok(r) if r.status().as_u16() == 404 => {
                    // Edge forgot us (e.g. registry wiped) — re-enroll next loop.
                    warn!(
                        target: Action::Heartbeat.target(),
                        "edge reports unknown; re-registering",
                    );
                    registered = false;
                    healthy = false;
                }
                Ok(r) => {
                    healthy = false;
                    warn!(
                        target: Action::Heartbeat.target(),
                        "heartbeat rejected: {}",
                        r.status(),
                    );
                }
                Err(e) => {
                    healthy = false;
                    error!(target: Action::Heartbeat.target(), "heartbeat error: {e}");
                }
            }
        }

        // Backoff now covers consecutive *heartbeat* failures too, not just
        // registration. Without it every appliance kept hammering a struggling
        // edge at its full heartbeat cadence, and — because they all retried
        // on the same fixed period — they stayed in lockstep, re-converging as
        // one thundering herd the moment the edge came back.
        let sleep = if healthy {
            opts.heartbeat_interval
        } else {
            jitter(backoff)
        };
        tokio::time::sleep(sleep).await;
        if !healthy {
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
    }
}

/// The addresses in a heartbeat's answer, as the file keeps them: only
/// names that look right, `None` when there are none.
#[must_use]
pub fn hidden_from_answer(body: Option<&str>) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Answer {
        #[serde(default)]
        hidden: Option<crate::hidden::Addresses>,
    }
    let a = serde_json::from_str::<Answer>(body?)
        .ok()?
        .hidden?
        .checked();
    if a.is_empty() {
        return None;
    }
    serde_json::to_string(&a).ok().map(|mut text| {
        text.push('\n');
        text
    })
}

/// Keep the edge's addresses for this box in `file` for LosOS Login and the
/// Network page, or remove it when the edge makes none. Written only when
/// it changes, since a path unit watches it.
async fn record_hidden(file: &str, body: Option<&str>) -> std::io::Result<()> {
    let path = std::path::Path::new(file);
    match hidden_from_answer(body) {
        None => match tokio::fs::remove_file(path).await {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
        Some(text) => {
            if tokio::fs::read_to_string(path).await.ok().as_deref() == Some(text.as_str()) {
                return Ok(());
            }
            if let Some(dir) = path.parent() {
                tokio::fs::create_dir_all(dir).await?;
            }
            let tmp = path.with_extension("json.tmp");
            tokio::fs::write(&tmp, text).await?;
            tokio::fs::rename(&tmp, path).await
        }
    }
}

/// Spread a retry delay over `[75%, 125%]` of `base` so a fleet that failed
/// together does not retry together.
///
/// The source is the clock's sub-second component rather than a PRNG: it costs
/// no dependency, and it differs across appliances (which boot at different
/// times) and across successive retries (which happen at different instants),
/// which is the entire requirement. Nothing here is security-relevant.
///
/// Shared with [`crate::join`], whose backoff wants the same anti-lockstep
/// property for the same reason: a rack of appliances rebooted together would
/// otherwise hit `/cluster/join` in unison.
pub(crate) fn jitter(base: Duration) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::from(d.subsec_nanos()));
    // 0..=50 percent, added to a 75% floor.
    let spread = nanos % 51;
    let millis = base.as_millis().min(u128::from(u64::MAX)) as u64;
    Duration::from_millis(millis.saturating_mul(75 + spread) / 100)
}
