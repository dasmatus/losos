//! Hand-rolled argument parsing. Kept dependency-free (no clap) because the
//! surface is small and fixed; every flag has a NixOS-module default so the
//! binary is only ever invoked with the values the module generates.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

fn arg<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(std::string::String::as_str))
}

fn req<'a>(args: &'a [String], flag: &str) -> Result<&'a str> {
    arg(args, flag).ok_or_else(|| anyhow!("missing required flag {flag}"))
}

fn parse_dur(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (n, unit) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, "ms")
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, "s")
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, "m")
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, "h")
    } else {
        (s, "s")
    };
    let n: u64 = n.parse().map_err(|_| anyhow!("bad duration {s:?}"))?;
    let d = match unit {
        "ms" => Duration::from_millis(n),
        "s" => Duration::from_secs(n),
        "m" => Duration::from_secs(n * 60),
        "h" => Duration::from_secs(n * 3600),
        _ => unreachable!(),
    };
    Ok(d)
}

/// `serve` options. The registrar is the sole writer of `traefik_yaml_path`
/// and `rathole_config_path`; it sends SIGHUP to `rathole_service` when
/// rathole config changes (the systemd unit hot-reloads rathole).
#[derive(Debug, Clone)]
pub struct ServeOpts {
    pub listen: String,
    pub registry_path: String,
    pub traefik_dir: String,
    pub rathole_config: String,
    pub rathole_service: String,
    pub rathole_bind_addr: String,
    pub rathole_bind_port: u16,
    pub port_range: (u16, u16),
    pub bootstrap_token_file: String,
    pub tenants_file: String,
    pub reconcile_interval: Duration,
    pub heartbeat_ttl: Duration,
    /// tmpfs directory the `POST /config` upload endpoint spills received
    /// files into. Must be tmpfs (a `RuntimeDirectory`, NOT under `/persist`)
    /// so an upload that survives a missed unlink still vanishes on reboot.
    pub upload_dir: PathBuf,
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
}

pub fn parse(args: Vec<String>) -> Result<Mode> {
    if args.is_empty() {
        bail!("usage: losos-registrar serve|announce ...");
    }
    let mode = &args[0];
    let rest = &args[1..];
    // Convert slice to Vec<String> for the helper closures.
    let rest: Vec<String> = rest.to_vec();
    match mode.as_str() {
        "serve" => {
            let range = arg(&rest, "--port-range")
                .ok_or_else(|| anyhow!("missing --port-range"))?;
            let (lo, hi) = parse_range(range)?;
            Ok(Mode::Serve(ServeOpts {
                listen: arg(&rest, "--listen").unwrap_or("127.0.0.1:8443").to_string(),
                registry_path: req(&rest, "--registry")?.to_string(),
                traefik_dir: req(&rest, "--traefik-dir")?.to_string(),
                rathole_config: req(&rest, "--rathole-config")?.to_string(),
                rathole_service: arg(&rest, "--rathole-service")
                    .unwrap_or("rathole.service")
                    .to_string(),
                rathole_bind_addr: arg(&rest, "--rathole-bind-addr")
                    .unwrap_or("0.0.0.0")
                    .to_string(),
                rathole_bind_port: arg(&rest, "--rathole-bind-port")
                    .unwrap_or("2333")
                    .parse()
                    .map_err(|_| anyhow!("bad --rathole-bind-port"))?,
                port_range: (lo, hi),
                bootstrap_token_file: req(&rest, "--bootstrap-token-file")?.to_string(),
                tenants_file: req(&rest, "--tenants-file")?.to_string(),
                reconcile_interval: parse_dur(
                    arg(&rest, "--reconcile-interval").unwrap_or("15s"),
                )?,
                heartbeat_ttl: parse_dur(arg(&rest, "--heartbeat-ttl").unwrap_or("120s"))?,
                upload_dir: PathBuf::from(
                    arg(&rest, "--upload-dir").unwrap_or("/run/losos-registrar"),
                ),
            }))
        }
        "announce" => Ok(Mode::Announce(AnnounceOpts {
            registrar_url: req(&rest, "--registrar-url")?.to_string(),
            appliance_id: req(&rest, "--appliance-id")?.to_string(),
            hostname: req(&rest, "--hostname")?.to_string(),
            token_file: req(&rest, "--token-file")?.to_string(),
            heartbeat_interval: parse_dur(
                arg(&rest, "--heartbeat-interval").unwrap_or("30s"),
            )?,
        })),
        other => bail!("unknown subcommand {other:?}; expected serve|announce"),
    }
}

fn parse_range(s: &str) -> Result<(u16, u16)> {
    let (lo, hi) = s
        .split_once('-')
        .ok_or_else(|| anyhow!("bad port range {s:?}; expected lo-hi"))?;
    Ok((lo.parse()?, hi.parse()?))
}