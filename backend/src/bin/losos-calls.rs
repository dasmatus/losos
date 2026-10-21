//! `losos-calls` — the SFU behind calls in LosOS chat.
//!
//! One process: the forwarding loop on its own thread, the signalling
//! listener on actix. `modules/calls.nix` runs it as `shared` with the
//! listener on a socket only nginx can open. The rules are in
//! [`losos_ctl::calls`].

use clap::Parser;
use losos_ctl::calls::{openid, server, sfu, tap};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(version, about = "Forward end-to-end encrypted call media between chat members")]
struct Cli {
    /// Where the signalling listener binds: `unix:/path` or `host:port`.
    #[arg(long, default_value = "unix:/run/losos-calls/http.sock")]
    listen: String,
    /// The UDP port media arrives on, on every address offered.
    #[arg(long, default_value_t = 7882)]
    udp_port: u16,
    /// An address to offer as an ICE host candidate. Repeat for more;
    /// without any, every address of every interface that is up, except
    /// loopback and the clusters' own interfaces.
    #[arg(long = "address")]
    addresses: Vec<IpAddr>,
    /// This box's Matrix server name: only its people start calls.
    #[arg(long)]
    server_name: String,
    /// The homeserver's federation listener, asked who an OpenID token is for.
    #[arg(long, default_value = "127.0.0.1:8008")]
    homeserver: SocketAddr,
    /// Write every forwarded frame under this directory (tests only).
    #[arg(long)]
    tap: Option<PathBuf>,
}

/// Interfaces whose addresses no browser can reach: the meshes' overlays,
/// container bridges and veths.
const SKIP_INTERFACES: &[&str] = &[
    "cali", "vxlan", "tunl", "flannel", "cni", "kube", "docker", "podman", "veth", "virbr", "lo",
];

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
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,str0m=warn")),
        )
        .init();
    let cli = Cli::parse();
    str0m::crypto::from_feature_flags().install_process_default();

    let addresses = if cli.addresses.is_empty() {
        host_addresses()?
    } else {
        cli.addresses.clone()
    };
    anyhow::ensure!(!addresses.is_empty(), "no address to offer media on");
    let tap = cli.tap.clone().map(tap::Tap::new).transpose()?;

    let (commands, rx) = tokio::sync::mpsc::channel(256);
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let mut sockets = Vec::new();
    for ip in &addresses {
        let addr = SocketAddr::new(*ip, cli.udp_port);
        let s = rt.block_on(tokio::net::UdpSocket::bind(addr))?;
        tracing::info!("media on udp {addr}");
        sockets.push(s);
    }
    let config = sfu::Config {
        server_name: cli.server_name.clone(),
        sockets,
        tap,
    };
    std::thread::Builder::new()
        .name("sfu".into())
        .spawn(move || rt.block_on(sfu::run(config, rx)))?;

    let state = server::State {
        commands,
        resolve: Arc::new(openid::Loopback {
            addr: cli.homeserver,
            server_name: cli.server_name.clone(),
        }),
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
