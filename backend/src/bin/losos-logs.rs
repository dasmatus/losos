//! `losos-logs`: the box's journal for Settings › Logs, on a Unix socket
//! nginx reaches, apart from lososd ([`losos_ctl::logs`]).

use actix_web::{web, App, HttpServer};
use clap::Parser;
use losos_ctl::logs::catalogue::Catalogue;
use losos_ctl::logs::lososd::Lososd;
use losos_ctl::logs::server::{routes, Reader};
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Read the box's journal for the admin pages")]
struct Cli {
    /// The Unix socket nginx relays to.
    #[arg(long)]
    socket: PathBuf,
    /// The groups of units the page offers (modules/log-reader.nix).
    #[arg(long)]
    units: PathBuf,
    /// The spare key, as the credential lososd reads.
    #[arg(long)]
    token_file: Option<PathBuf>,
    /// lososd's loopback API, asked who is signed in.
    #[arg(long)]
    lososd: Option<SocketAddr>,
    /// The built admin page, for its catalogue and stylesheet.
    #[arg(long)]
    ui: Option<PathBuf>,
    /// nginx asks LosOS Login and names the person.
    #[arg(long)]
    identity: bool,
    /// The sidebar has Office.
    #[arg(long)]
    office: bool,
    #[arg(long, default_value = "journalctl")]
    journalctl: PathBuf,
}

fn main() -> anyhow::Result<()> {
    losos_ctl::trace::init("info");
    let cli = Cli::parse();
    let reader = web::Data::new(Reader {
        journalctl: cli.journalctl,
        catalogue: Catalogue::load(&cli.units),
        token_file: cli.token_file,
        lososd: cli.lososd.map(Lososd::new),
        ui_dir: cli.ui,
        identity: cli.identity,
        office: cli.office,
    });
    let _ = std::fs::remove_file(&cli.socket);
    actix_web::rt::System::new().block_on(async move {
        let server = HttpServer::new(move || App::new().app_data(reader.clone()).configure(routes))
            .workers(2)
            .bind_uds(&cli.socket)?;
        // Owner and group (nginx's) only.
        std::fs::set_permissions(&cli.socket, std::fs::Permissions::from_mode(0o660))?;
        tracing::info!(socket = %cli.socket.display(), "log reader up");
        server.run().await?;
        Ok(())
    })
}
