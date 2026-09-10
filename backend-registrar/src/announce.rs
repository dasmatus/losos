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
}

#[derive(Serialize)]
struct HeartbeatReq<'a> {
    appliance_id: &'a str,
    token: &'a str,
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
        if !registered {
            let req = RegisterReq {
                appliance_id: &opts.appliance_id,
                token: &token,
                hostname: &opts.hostname,
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
            let req = HeartbeatReq {
                appliance_id: &opts.appliance_id,
                token: &token,
            };
            match client.post(&heartbeat_url).json(&req).send().await {
                Ok(r) if r.status().is_success() => {
                    healthy = true;
                    backoff = BACKOFF_MIN;
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
