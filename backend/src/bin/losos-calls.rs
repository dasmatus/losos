//! `losos-calls` — the SFU behind calls in LosOS chat.
//!
//! One process: the forwarding loop on its own thread, the signalling
//! listener on actix. `modules/calls.nix` runs it as `shared` with the
//! listener on a socket only nginx can open. The rules are in
//! [`losos_ctl::calls`].

use anyhow::Context;
use clap::Parser;
use losos_ctl::calls::{guests, person, relay, server, sfu, tap};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    version,
    about = "Forward end-to-end encrypted call media between chat members"
)]
struct Cli {
    /// Where the signalling listener binds: `unix:/path` or `host:port`.
    #[arg(long, default_value = "unix:/run/losos-calls/http.sock")]
    listen: String,
    /// The UDP port media arrives on, on every address offered.
    #[arg(long, default_value_t = 7882)]
    udp_port: u16,
    /// An address to take media on and offer as an ICE host candidate,
    /// `IP`, or `IP=OFFERED` when something in front forwards the same UDP
    /// port to IP and browsers must be told OFFERED. Repeat for more;
    /// without any, every address of every interface that is up, except
    /// loopback and the clusters' own interfaces.
    #[arg(long = "address", value_parser = parse_address)]
    addresses: Vec<(IpAddr, IpAddr)>,
    /// This box's Matrix server name: only its people start calls.
    #[arg(long)]
    server_name: String,
    /// Take media through the edge's relay at `HOST:PORT`, over QUIC, and
    /// open no UDP port here (`modules/calls.nix` with `losos.proxy` on).
    /// `--address` and `--udp-port` are then unused.
    #[arg(long, requires_all = ["relay_cert", "relay_id", "relay_token_file"])]
    relay: Option<String>,
    /// SHA-256 of the relay's certificate, in hex, from `GET /calls/relay`.
    #[arg(long, value_parser = relay::parse_sha256)]
    relay_cert: Option<[u8; 32]>,
    /// The appliance id this box registers with the edge under.
    #[arg(long)]
    relay_id: Option<String>,
    /// The file holding the box's edge token.
    #[arg(long)]
    relay_token_file: Option<PathBuf>,
    /// The ffmpeg podcast mode streams through; without it the call page
    /// cannot go live.
    #[arg(long)]
    ffmpeg: Option<PathBuf>,
    /// The registrar of the edge this box tunnels through: guest links
    /// come from it (`--relay-id` and `--relay-token-file` say who asks).
    #[arg(long, requires_all = ["relay_id", "relay_token_file"])]
    registrar: Option<String>,
    /// The curl the edge is asked with.
    #[arg(long, default_value = "curl")]
    curl: PathBuf,
    /// Write every forwarded frame under this directory (tests only).
    #[arg(long)]
    tap: Option<PathBuf>,
}

/// Interfaces whose addresses no browser can reach: the meshes' overlays,
/// container bridges and veths.
const SKIP_INTERFACES: &[&str] = &[
    "cali", "vxlan", "tunl", "flannel", "cni", "kube", "docker", "podman", "veth", "virbr", "lo",
];

fn parse_address(s: &str) -> Result<(IpAddr, IpAddr), String> {
    let (bind, offered) = s.split_once('=').unwrap_or((s, s));
    let ip = |v: &str| v.parse::<IpAddr>().map_err(|e| format!("{v}: {e}"));
    Ok((ip(bind)?, ip(offered)?))
}

fn host_addresses() -> anyhow::Result<Vec<IpAddr>> {
    let mut v: Vec<IpAddr> = if_addrs::get_if_addrs()?
        .into_iter()
        .filter(|i| !i.is_loopback() && !i.is_link_local() && i.is_oper_up())
        .filter(|i| !SKIP_INTERFACES.iter().any(|p| i.name.starts_with(p)))
        .map(|i| i.ip())
        .collect();
    v.sort();
    v.dedup();
    Ok(v)
}

fn main() -> anyhow::Result<()> {
    losos_ctl::trace::init("info,str0m=warn");
    let cli = Cli::parse();
    str0m::crypto::from_feature_flags().install_process_default();

    let addresses = if cli.relay.is_some() {
        Vec::new()
    } else if cli.addresses.is_empty() {
        host_addresses()?.into_iter().map(|ip| (ip, ip)).collect()
    } else {
        cli.addresses.clone()
    };
    anyhow::ensure!(
        cli.relay.is_some() || !addresses.is_empty(),
        "no address to offer media on"
    );
    let tap = cli.tap.clone().map(tap::Tap::new).transpose()?;

    let (commands, rx) = tokio::sync::mpsc::channel(256);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut sockets = Vec::new();
    for (ip, offered) in &addresses {
        let addr = SocketAddr::new(*ip, cli.udp_port);
        let socket = rt.block_on(tokio::net::UdpSocket::bind(addr))?;
        let offered = SocketAddr::new(*offered, cli.udp_port);
        tracing::info!("media on udp {addr}, offered as {offered}");
        sockets.push(sfu::Media { socket, offered });
    }
    let relay = match (
        &cli.relay,
        cli.relay_cert,
        &cli.relay_id,
        &cli.relay_token_file,
    ) {
        (Some(addr), Some(cert_sha256), Some(id), Some(token_file)) => {
            let token = std::fs::read_to_string(token_file)
                .with_context(|| format!("reading {}", token_file.display()))?;
            let (link, incoming) = rt.block_on(relay::connect(relay::Edge {
                addr,
                cert_sha256,
                id,
                token: token.trim(),
            }))?;
            for a in link.addresses() {
                tracing::info!("media through the edge at {addr}, offered as {a}");
            }
            // The edge took this box's port with the connection; without it
            // nothing can reach a call. Exit, and systemd starts over with
            // whatever edge the box has then.
            let watch = link.clone();
            rt.spawn(async move {
                let why = watch.closed().await;
                tracing::error!("the edge relay closed: {why}");
                std::process::exit(1);
            });
            Some((link, incoming))
        }
        _ => None,
    };
    let config = sfu::Config {
        server_name: cli.server_name.clone(),
        sockets,
        relay,
        tap,
    };
    std::thread::Builder::new()
        .name("sfu".into())
        .spawn(move || rt.block_on(sfu::run(config, rx)))?;

    let state = server::State {
        commands,
        resolve: Arc::new(person::Login {
            server_name: cli.server_name.clone(),
        }),
        ffmpeg: cli.ffmpeg.clone(),
        edge: match (&cli.registrar, &cli.relay_id, &cli.relay_token_file) {
            (Some(registrar), Some(id), Some(token_file)) => Some(Arc::new(guests::Edge {
                registrar: registrar.clone(),
                id: id.clone(),
                token_file: token_file.clone(),
                curl: cli.curl.clone(),
            })),
            _ => None,
        },
    };
    actix_web::rt::System::new().block_on(async move {
        let app = move || {
            actix_web::App::new()
                .app_data(actix_web::web::Data::new(state.clone()))
                .configure(server::routes)
        };
        let http = actix_web::HttpServer::new(app).workers(2);
        let http = match cli.listen.strip_prefix("unix:") {
            Some(path) => {
                let _ = std::fs::remove_file(path);
                let http = http.bind_uds(path)?;
                // nginx is in the socket's group (modules/calls.nix); nobody
                // else may open it.
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))?;
                http
            }
            None => http.bind(&cli.listen)?,
        };
        tracing::info!("signalling on {}", cli.listen);
        http.run().await
    })?;
    Ok(())
}
