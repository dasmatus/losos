//! Hand-rolled argument parsing. Kept dependency-free (no clap) because the
//! surface is small and fixed; every flag has a NixOS-module default so the
//! binary is only ever invoked with the values the module generates.

use std::time::Duration;

use miette::{miette, IntoDiagnostic, Result};

fn arg<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(std::string::String::as_str))
}

fn req<'a>(args: &'a [String], flag: &str) -> Result<&'a str> {
    arg(args, flag).ok_or_else(|| miette!("missing required flag {flag}"))
}

/// Parse a `<n><unit>` duration (`ms`/`s`/`m`/`h`, bare number = seconds).
///
/// Zero is rejected: every duration this crate parses is a loop period or a
/// TTL, and `0` turns the reconciler into a busy-loop, the heartbeat into a
/// flood, or the TTL into "prune every tenant on the next tick". The
/// multiplication is checked — `parse_dur("5124095576030432h")` used to
/// overflow `u64` seconds (a debug-build panic, a silently tiny interval in
/// release) instead of being rejected as the nonsense it is.
fn parse_dur(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (n, secs_per) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, None)
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, Some(1))
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, Some(60))
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, Some(3600))
    } else {
        (s, Some(1))
    };
    let n: u64 = n
        .trim()
        .parse()
        .map_err(|_| miette!("bad duration {s:?}; expected <n>[ms|s|m|h]"))?;
    if n == 0 {
        return Err(miette!("duration {s:?} must be greater than zero"));
    }
    match secs_per {
        None => Ok(Duration::from_millis(n)),
        Some(mul) => n
            .checked_mul(mul)
            .map(Duration::from_secs)
            .ok_or_else(|| miette!("duration {s:?} overflows")),
    }
}

/// `serve` options. The registrar is the sole writer of `traefik_dir`'s
/// `losos.yml` and `rathole_config`; rathole hot-reloads the latter via its
/// `notify` file-watcher (no signal needed), so there is no rathole-service
/// handle here.
///
/// The `mesh_*` / `kube_*` fields are the `/cluster/join` half and are all
/// optional. `modules/edge.nix` only appends their flags under
/// `lib.optionals cfg.cluster.enable`, so an edge with
/// `losos.edge.cluster.enable = false` parses exactly the arguments it always
/// did and the join route answers 503 — the master-proxy half is untouched by
/// the mesh existing.
#[derive(Debug, Clone)]
pub struct ServeOpts {
    pub listen: String,
    pub registry_path: String,
    pub traefik_dir: String,
    pub rathole_config: String,
    pub rathole_bind_addr: String,
    pub rathole_bind_port: u16,
    pub port_range: (u16, u16),
    pub bootstrap_token_file: String,
    pub tenants_file: String,
    pub reconcile_interval: Duration,
    pub heartbeat_ttl: Duration,
    /// The rke2 *agent* token an accepted join is handed back, read at request
    /// time rather than at boot so a rotated file needs no restart. `None`
    /// disables the join route.
    pub mesh_agent_token_file: Option<String>,
    /// `https://<advertiseAddr>:<supervisorPort>` — the rke2 **supervisor**
    /// port (9345), not the apiserver port (6443). An agent registers on the
    /// supervisor; pointing it at 6443 fails at join time with a TLS error
    /// that names neither port.
    pub mesh_server_addr: Option<String>,
    /// The mesh apiserver the stale-node cleanup talks to. Loopback by
    /// default: the registrar runs on the same host as `rke2-server`.
    pub kube_api: String,
    /// Bearer token for `kube_api`, extracted from the `losos-registrar`
    /// ServiceAccount by `losos-mesh-rbac.service`.
    pub kube_token_file: Option<String>,
    /// The mesh cluster CA. The cleanup client pins its root store to this
    /// file and disables the built-in webpki roots, so a public CA cannot
    /// impersonate the cluster's apiserver.
    pub kube_ca_file: Option<String>,
    /// Where the reconciler publishes the per-node compute windows for the
    /// edge's `losos-mesh-taint.service` to read.
    pub compute_windows_file: String,
}

/// `seed` options. Writes the declarative rathole `[server]` base (for zero
/// tenants) so rathole can start before `serve`'s reconciler has run once.
/// Reuses [`crate::desired_config`] — the same function `serve`'s reconciler
/// calls — so the seed and the steady-state output can never drift.
#[derive(Debug, Clone)]
pub struct SeedOpts {
    pub rathole_config: String,
    pub rathole_bind_addr: String,
    pub rathole_bind_port: u16,
    pub bootstrap_token_file: String,
}

/// `announce` options. The appliance reads its token from `token_file`
/// (0600, provisioned out of band, persisted via /var).
#[derive(Debug, Clone)]
pub struct AnnounceOpts {
    pub registrar_url: String,
    pub appliance_id: String,
    pub hostname: String,
    pub token_file: String,
    pub heartbeat_interval: Duration,
    /// One-minute load average per core, below which this box calls itself
    /// idle and lets the mesh schedule onto it inside its compute window.
    ///
    /// See crate::idle for why the default is 0.25 rather than something near
    /// zero: an idle losos box runs two kubelets, containerd, Postgres, Redis
    /// and a PHP-FPM pool, so a threshold near zero would mean "never idle"
    /// and the feature would quietly never fire.
    pub idle_load_threshold: f64,
}

/// `join` options. Runs once per boot on the appliance, from
/// `losos-mesh-join.service`, before `rke2-agent.service`.
///
/// It authenticates with the *same* `/var/secrets/losos-proxy-token` the
/// announce client uses — mesh enrolment mints no second credential — and
/// writes the rke2 node token the edge hands back to `out_token_file`, which
/// is `losos.cluster.tokenFile`.
#[derive(Debug, Clone)]
pub struct JoinOpts {
    pub registrar_url: String,
    pub appliance_id: String,
    pub node_name: String,
    pub token_file: String,
    pub out_token_file: String,
    pub share_compute: bool,
    pub window_start: String,
    pub window_end: String,
    /// The zone the two bounds are wall-clock times in, taken from the
    /// appliance's own `time.timeZone` by modules/cluster.nix. The edge writes
    /// the taint and so compares on its own clock; without this the hours are
    /// reinterpreted in the edge's zone.
    pub window_tz: String,
    /// `losos.cluster.serverAddr` as this box has it configured. Purely a
    /// consistency check: a mismatch against the edge's answer is logged, not
    /// enforced, because the edge is the authority on its own address.
    pub expect_server_addr: Option<String>,
}

pub enum Mode {
    Serve(ServeOpts),
    Announce(AnnounceOpts),
    Seed(SeedOpts),
    Join(JoinOpts),
}

pub fn parse(args: Vec<String>) -> Result<Mode> {
    if args.is_empty() {
        return Err(miette!(
            "usage: losos-registrar serve|announce|seed|join ..."
        ));
    }
    let mode = &args[0];
    let rest = &args[1..];
    // Convert slice to Vec<String> for the helper closures.
    let rest: Vec<String> = rest.to_vec();
    match mode.as_str() {
        "serve" => {
            let range =
                arg(&rest, "--port-range").ok_or_else(|| miette!("missing --port-range"))?;
            let (lo, hi) = parse_range(range)?;
            Ok(Mode::Serve(ServeOpts {
                listen: arg(&rest, "--listen")
                    .unwrap_or("127.0.0.1:8443")
                    .to_string(),
                registry_path: req(&rest, "--registry")?.to_string(),
                traefik_dir: req(&rest, "--traefik-dir")?.to_string(),
                rathole_config: req(&rest, "--rathole-config")?.to_string(),
                rathole_bind_addr: arg(&rest, "--rathole-bind-addr")
                    .unwrap_or("0.0.0.0")
                    .to_string(),
                rathole_bind_port: arg(&rest, "--rathole-bind-port")
                    .unwrap_or("2333")
                    .parse()
                    .map_err(|_| miette!("bad --rathole-bind-port"))?,
                port_range: (lo, hi),
                bootstrap_token_file: req(&rest, "--bootstrap-token-file")?.to_string(),
                tenants_file: req(&rest, "--tenants-file")?.to_string(),
                reconcile_interval: parse_dur(arg(&rest, "--reconcile-interval").unwrap_or("15s"))?,
                heartbeat_ttl: parse_dur(arg(&rest, "--heartbeat-ttl").unwrap_or("120s"))?,
                mesh_agent_token_file: arg(&rest, "--mesh-agent-token-file").map(str::to_string),
                mesh_server_addr: arg(&rest, "--mesh-server-addr").map(str::to_string),
                kube_api: arg(&rest, "--kube-api")
                    .unwrap_or("https://127.0.0.1:6443")
                    .trim_end_matches('/')
                    .to_string(),
                kube_token_file: arg(&rest, "--kube-token-file").map(str::to_string),
                kube_ca_file: arg(&rest, "--kube-ca-file").map(str::to_string),
                compute_windows_file: arg(&rest, "--compute-windows-file")
                    .unwrap_or("/var/lib/losos-registrar/compute-windows.json")
                    .to_string(),
            }))
        }
        "announce" => Ok(Mode::Announce(AnnounceOpts {
            registrar_url: req(&rest, "--registrar-url")?.to_string(),
            appliance_id: req(&rest, "--appliance-id")?.to_string(),
            hostname: req(&rest, "--hostname")?.to_string(),
            token_file: req(&rest, "--token-file")?.to_string(),
            heartbeat_interval: parse_dur(arg(&rest, "--heartbeat-interval").unwrap_or("30s"))?,
            idle_load_threshold: parse_threshold(
                arg(&rest, "--idle-load-threshold").unwrap_or("0.25"),
            )?,
        })),
        "seed" => Ok(Mode::Seed(SeedOpts {
            rathole_config: req(&rest, "--rathole-config")?.to_string(),
            rathole_bind_addr: arg(&rest, "--rathole-bind-addr")
                .unwrap_or("0.0.0.0")
                .to_string(),
            rathole_bind_port: arg(&rest, "--rathole-bind-port")
                .unwrap_or("2333")
                .parse()
                .map_err(|_| miette!("bad --rathole-bind-port"))?,
            bootstrap_token_file: req(&rest, "--bootstrap-token-file")?.to_string(),
        })),
        "join" => Ok(Mode::Join(JoinOpts {
            registrar_url: req(&rest, "--registrar-url")?.to_string(),
            appliance_id: req(&rest, "--appliance-id")?.to_string(),
            node_name: req(&rest, "--node-name")?.to_string(),
            token_file: req(&rest, "--token-file")?.to_string(),
            out_token_file: req(&rest, "--out-token-file")?.to_string(),
            share_compute: parse_bool(arg(&rest, "--share-compute").unwrap_or("false"))?,
            window_start: parse_hhmm(arg(&rest, "--window-start").unwrap_or("23:00"))?.to_string(),
            window_end: parse_hhmm(arg(&rest, "--window-end").unwrap_or("07:00"))?.to_string(),
            window_tz: parse_tz(arg(&rest, "--window-tz").unwrap_or("UTC"))?.to_string(),
            expect_server_addr: arg(&rest, "--expect-server-addr").map(str::to_string),
        })),
        other => Err(miette!(
            "unknown subcommand {other:?}; expected serve|announce|seed|join"
        )),
    }
}

/// Accept an IANA zone name, rejecting anything the edge's `date` would
/// silently resolve to UTC.
///
/// Defaulting to `UTC` when the flag is absent matches what the edge did before
/// the zone travelled at all, so an un-upgraded caller keeps its old behaviour
/// instead of acquiring a new one.
fn parse_tz(v: &str) -> miette::Result<&str> {
    if crate::window::valid_tz(v) {
        Ok(v)
    } else {
        Err(miette!(
            "invalid --window-tz {v:?}; expected an IANA zone name such as Europe/Berlin or UTC"
        ))
    }
}

/// A non-negative load threshold.
///
/// Rejects negatives and non-finite values rather than clamping: both mean the
/// module generated something it did not intend, and a box that silently
/// decides it is never idle (or always idle) is worse than one that refuses to
/// start and says why.
fn parse_threshold(v: &str) -> miette::Result<f64> {
    match v.parse::<f64>() {
        Ok(n) if n.is_finite() && n >= 0.0 => Ok(n),
        _ => Err(miette!(
            "invalid --idle-load-threshold {v:?}; expected a non-negative number such as 0.25"
        )),
    }
}

/// Parse the Nix spelling of a boolean. `modules/cluster.nix` interpolates
/// `losos.cluster.shareCompute` straight into the unit's `ExecStart`, so the
/// only two values that can arrive are `true` and `false` — and anything else
/// means the module was edited into producing something it did not intend, so
/// it is a boot-time failure that names the flag rather than a silent `false`
/// that quietly stops the box contributing compute.
fn parse_bool(s: &str) -> Result<bool> {
    match s.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(miette!(
            "bad --share-compute {other:?}; expected true|false"
        )),
    }
}

/// Validate an `HH:MM` compute-window bound, returning it unchanged.
///
/// The same rule lososd enforces on `losos.cluster.computeWindow.{start,end}`
/// and the same one the edge re-checks on the wire. Rejecting here means a
/// malformed window fails `losos-mesh-join.service` at boot with the flag
/// named, rather than travelling to the edge to come back as an opaque 400 on
/// a box with no shell to read it from.
fn parse_hhmm(s: &str) -> Result<&str> {
    let s = s.trim();
    if crate::window::valid_hhmm(s) {
        Ok(s)
    } else {
        Err(miette!("bad window {s:?}; expected HH:MM in 00:00-23:59"))
    }
}

/// Parse a `lo-hi` rathole port range, inclusive on both ends.
///
/// `lo` of 0 and an inverted range are rejected here rather than surfacing
/// later: port 0 is not bindable, and `hi < lo` makes `alloc_port`'s
/// `(lo..=hi)` sweep empty, so the first appliance to register would be told
/// the range is exhausted with no hint that the *range itself* is malformed.
fn parse_range(s: &str) -> Result<(u16, u16)> {
    let (lo, hi) = s
        .trim()
        .split_once('-')
        .ok_or_else(|| miette!("bad port range {s:?}; expected lo-hi"))?;
    let lo: u16 = lo.trim().parse().into_diagnostic()?;
    let hi: u16 = hi.trim().parse().into_diagnostic()?;
    if lo == 0 {
        return Err(miette!("bad port range {s:?}; port 0 is not bindable"));
    }
    if hi < lo {
        return Err(miette!(
            "bad port range {s:?}; high end is below the low end"
        ));
    }
    Ok((lo, hi))
}
