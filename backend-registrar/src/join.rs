//! `losos-registrar join` — the appliance-side mesh enrolment client.
//!
//! Runs once per boot from `losos-mesh-join.service`, ordered before
//! `rke2-agent.service` and `Requires=`d by it. It POSTs `/cluster/join` with
//! the *same* `/var/secrets/losos-proxy-token` the announce client uses — mesh
//! enrolment mints no second credential — and writes the rke2 node token the
//! edge returns to `losos.cluster.tokenFile`.
//!
//! **The retry is bounded, unlike `announce`'s.** The announce loop runs
//! forever because the appliance stays useful while the edge is unreachable:
//! Nextcloud keeps serving on the LAN, only the public route is missing. Here
//! the unit is a `Requires=` dependency of the rke2 agent, so retrying forever
//! means `rke2-agent.service` sits in `activating` indefinitely — on a box with
//! no SSH and no shell logins, where nobody will ever see it. Giving up after
//! [`GIVE_UP_AFTER`] turns that into a failed unit with a journal line naming
//! the reason, and the agent simply never starts. The named failure this
//! protects against is an appliance whose `/var/secrets/losos-proxy-token` was
//! never provisioned: the box was never enrolled with the master proxy, so it
//! has nothing to authenticate a join with.
//!
//! A 4xx is not retried. `401` means the token is wrong, `403` means the
//! operator has not set `losos.edge.tenants.<id>.cluster`, and `400` means the
//! compute window is malformed — none of which five more minutes of requests
//! will change, and all of which are clearer in the journal as one line than as
//! sixty.
//!
//! Idempotent by construction: it always calls the edge (that call is what
//! triggers the stale-node cleanup, which is the whole reason a *reinstalled*
//! box can rejoin) and always rewrites the token file. Re-running it is the
//! reinstall path.

use std::time::{Duration, Instant};

use miette::{miette, Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::action::Action;
use crate::announce::jitter;
use crate::opts::JoinOpts;

/// First retry delay, and the value backoff resets to.
const BACKOFF_MIN: Duration = Duration::from_secs(2);
/// Ceiling on the retry delay. Lower than `announce`'s 60s: the whole budget
/// is five minutes, so a minute-long sleep would spend a fifth of it idle.
const BACKOFF_MAX: Duration = Duration::from_secs(30);
/// Give up on a single request after this long.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// TCP connect budget, separate from the whole-request budget: a connect that
/// hangs is the common failure when the edge VPS is unreachable.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Total wall-clock budget across all attempts. Long enough to ride out an
/// edge that is still booting (`losos-mesh-rbac.service` polls its own
/// apiserver and can take a minute), short enough that a permanently broken
/// enrolment shows up as a failed unit inside one boot.
const GIVE_UP_AFTER: Duration = Duration::from_secs(300);

/// The node token file is a secret: it authenticates this box to the mesh
/// control plane for the life of the cluster.
const TOKEN_FILE_MODE: u32 = 0o600;

#[derive(Serialize)]
struct ClusterJoinReq<'a> {
    appliance_id: &'a str,
    token: &'a str,
    node_name: &'a str,
    share_compute: bool,
    window_start: &'a str,
    window_end: &'a str,
    /// The appliance's own time.timeZone. The edge writes the taint (it is the
    /// only node the NodeRestriction plugin lets do so) and therefore compares
    /// on its own clock, so the zone has to travel with the hours or a window
    /// entered in Berlin is enforced in UTC.
    window_tz: &'a str,
}

#[derive(Deserialize)]
struct ClusterJoinResp {
    server_addr: String,
    token: String,
    node_name: String,
}

pub async fn run(opts: JoinOpts) -> Result<()> {
    let base = opts.registrar_url.trim_end_matches('/');
    // Same reasoning as `announce`: refuse a redirect that would downgrade the
    // token to cleartext, but only when the configured URL is already https,
    // so the VM tests can dial a loopback registrar over plain HTTP.
    let https_only = base.starts_with("https://");
    if !https_only {
        warn!(
            target: Action::Join.target(),
            "registrar URL {base} is not https; the appliance token crosses the network in cleartext",
        );
    }
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
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
        // Fatal rather than a warning, unlike `announce`: there is no useful
        // partial outcome here, and this is the exact shape of "the box was
        // never enrolled with the master proxy".
        return Err(miette!(
            "token file {} is empty; this appliance has no master-proxy token to enrol with",
            opts.token_file,
        ));
    }

    let url = format!("{base}/cluster/join");
    let deadline = Instant::now() + GIVE_UP_AFTER;
    let mut backoff = BACKOFF_MIN;

    loop {
        let request = ClusterJoinReq {
            appliance_id: &opts.appliance_id,
            token: &token,
            node_name: &opts.node_name,
            share_compute: opts.share_compute,
            window_start: &opts.window_start,
            window_end: &opts.window_end,
            window_tz: &opts.window_tz,
        };
        match attempt(&client, &url, &request).await {
            Ok(response) => return finish(&opts, response).await,
            Err(Fault::Permanent(e)) => return Err(e),
            Err(Fault::Retryable(reason)) => {
                let sleep = jitter(backoff);
                if Instant::now() + sleep >= deadline {
                    return Err(miette!(
                        "gave up enrolling {} in the mesh after {}s: {reason}",
                        opts.appliance_id,
                        GIVE_UP_AFTER.as_secs(),
                    ));
                }
                warn!(target: Action::Join.target(), "join attempt failed, retrying: {reason}");
                tokio::time::sleep(sleep).await;
                backoff = (backoff * 2).min(BACKOFF_MAX);
            }
        }
    }
}

/// Why an attempt did not produce a token: worth trying again, or not.
enum Fault {
    Retryable(String),
    Permanent(miette::Report),
}

/// One POST. A 2xx yields the parsed body; a 5xx (the edge's apiserver is not
/// up yet, or its mesh credentials are not in place yet) and any transport
/// failure are retryable; everything else is not.
async fn attempt(
    client: &reqwest::Client,
    url: &str,
    request: &ClusterJoinReq<'_>,
) -> Result<ClusterJoinResp, Fault> {
    let response = match client.post(url).json(request).send().await {
        Ok(r) => r,
        Err(e) => return Err(Fault::Retryable(format!("{e}"))),
    };
    let status = response.status();
    if status.is_server_error() {
        // The edge's body is a fixed string per status, so it is safe to log
        // and is the only diagnostic an operator gets from the appliance side.
        let body = response.text().await.unwrap_or_default();
        return Err(Fault::Retryable(format!("edge returned {status}: {body}")));
    }
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(Fault::Permanent(miette!(
            "the edge refused this appliance's mesh enrolment with {status}: {body}"
        )));
    }
    response
        .json::<ClusterJoinResp>()
        .await
        .map_err(|e| Fault::Permanent(miette!("the edge's join response did not parse: {e}")))
}

/// Validate what came back and write the node token.
async fn finish(opts: &JoinOpts, response: ClusterJoinResp) -> Result<()> {
    if response.token.trim().is_empty() {
        return Err(miette!(
            "the edge returned an empty mesh token; refusing to write {}",
            opts.out_token_file,
        ));
    }
    if response.server_addr.trim().is_empty() {
        return Err(miette!("the edge returned an empty server address"));
    }
    if response.node_name != opts.node_name {
        return Err(miette!(
            "the edge enrolled this box as {:?}, not the requested {:?}",
            response.node_name,
            opts.node_name,
        ));
    }
    // A mismatch is the operator having changed `losos.edge.cluster.advertiseAddr`
    // without updating `losos.cluster.serverAddr` on this box. Not fatal — the
    // edge is the authority on its own address, and `services.rke2.serverAddr`
    // is what the agent will actually dial — but silence here is a box that
    // joins nothing and an operator with no way to see why.
    if let Some(expected) = &opts.expect_server_addr {
        if expected != &response.server_addr {
            warn!(
                target: Action::Join.target(),
                "losos.cluster.serverAddr is {expected}, but the edge advertises {}; the agent will dial the configured value and fail until they agree",
                response.server_addr,
            );
        }
    }

    // Same atomic temp+fsync+rename every other secret this crate writes gets:
    // `rke2-agent.service` starts the moment this unit exits, and a torn token
    // file is an agent that fails with a TLS error naming nothing.
    crate::fsutil::atomic_write(
        std::path::Path::new(&opts.out_token_file),
        response.token.trim().as_bytes(),
        TOKEN_FILE_MODE,
    )
    .await
    .into_diagnostic()
    .with_context(|| format!("write mesh token to {}", opts.out_token_file))?;

    info!(
        target: Action::Join.target(),
        "enrolled {} as mesh node {} against {}",
        opts.appliance_id,
        response.node_name,
        response.server_addr,
    );
    Ok(())
}
