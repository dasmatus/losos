//! `losos-registrar announce` — the appliance-side registration client.
//!
//! Reads its per-appliance token (0600, persisted via /var, provisioned out of
//! band), POSTs `/register` once on start, then `/heartbeat` on a cadence.
//! Stateless: a 404 (unknown appliance on the edge, e.g. after the edge's
//! registry was wiped) triggers a fresh `/register`. All failures retry with
//! capped backoff — the edge pruning only removes after TTL, so brief network
//! blips don't kill the route.

use std::time::Duration;

use miette::{Context, IntoDiagnostic, Result};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::action::Action;
use crate::opts::AnnounceOpts;

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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .into_diagnostic()
        .context("build HTTP client")?;

    let token = tokio::fs::read_to_string(&opts.token_file)
        .await
        .into_diagnostic()
        .with_context(|| format!("read token file {}", opts.token_file))?;
    let token = token.trim().to_string();

    let register_url = format!("{}/register", opts.registrar_url.trim_end_matches('/'));
    let heartbeat_url = format!("{}/heartbeat", opts.registrar_url.trim_end_matches('/'));

    let mut backoff = Duration::from_secs(2);
    let mut registered = false;

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
                    backoff = Duration::from_secs(2);
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
                    backoff = Duration::from_secs(2);
                }
                Ok(r) if r.status().as_u16() == 404 => {
                    // Edge forgot us (e.g. registry wiped) — re-enroll next loop.
                    warn!(
                        target: Action::Heartbeat.target(),
                        "edge reports unknown; re-registering",
                    );
                    registered = false;
                }
                Ok(r) => {
                    warn!(
                        target: Action::Heartbeat.target(),
                        "heartbeat rejected: {}",
                        r.status(),
                    );
                }
                Err(e) => {
                    error!(target: Action::Heartbeat.target(), "heartbeat error: {e}");
                }
            }
        }

        let sleep = if registered { opts.heartbeat_interval } else { backoff };
        tokio::time::sleep(sleep).await;
        if !registered {
            // capped exponential backoff for the register retry
            backoff = (backoff * 2).min(Duration::from_mins(1));
        }
    }
}