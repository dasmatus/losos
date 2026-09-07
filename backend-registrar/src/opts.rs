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
}

pub enum Mode {
    Serve(ServeOpts),
    Announce(AnnounceOpts),
    Seed(SeedOpts),
}

pub fn parse(args: Vec<String>) -> Result<Mode> {
    if args.is_empty() {
        return Err(miette!("usage: losos-registrar serve|announce|seed ..."));
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
            }))
        }
        "announce" => Ok(Mode::Announce(AnnounceOpts {
            registrar_url: req(&rest, "--registrar-url")?.to_string(),
            appliance_id: req(&rest, "--appliance-id")?.to_string(),
            hostname: req(&rest, "--hostname")?.to_string(),
            token_file: req(&rest, "--token-file")?.to_string(),
            heartbeat_interval: parse_dur(arg(&rest, "--heartbeat-interval").unwrap_or("30s"))?,
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
        other => Err(miette!(
            "unknown subcommand {other:?}; expected serve|announce|seed"
        )),
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
