//! `losos-office` — native LosOS Office over a plain folder.
//!
//! Serves the folder UI's `/office/api/*` routes (list/open/extensions)
//! for `losos.office.folder`. Legacy Collabora/WOPI integration has been
//! removed; legacy formats are handled as native import targets.

use actix_web::{App, HttpServer};
use clap::Parser;
use losos_ctl::office::actix::{folder_routes, session_routes, Sessions};
use losos_ctl::office::store::DirStore;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(version, about = "Native LosOS Office over a folder")]
struct Cli {
    /// The folder whose documents are served.
    #[arg(long)]
    root: PathBuf,
    /// The Unix socket nginx relays the Office page's calls to.
    #[arg(long)]
    socket: PathBuf,
    /// Accepted for compatibility; unused since Collabora was removed.
    #[arg(long, default_value = "127.0.0.1:8093")]
    wopi: String,
    /// Accepted for compatibility; unused.
    #[arg(long, default_value = "http://127.0.0.1:9980")]
    coolwsd: String,
    /// Accepted for compatibility; unused.
    #[arg(long)]
    key_file: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    losos_ctl::trace::init("info");
    let cli = Cli::parse();
    let _ = (&cli.wopi, &cli.coolwsd, &cli.key_file);
    let folder = Arc::new(DirStore::new(&cli.root));
    let sessions = Arc::new(Sessions::new(folder.clone()));

    let _ = std::fs::remove_file(&cli.socket);
    actix_web::rt::System::new().block_on(async move {
        let socket = cli.socket.clone();
        let server = HttpServer::new(move || {
            App::new()
                .configure(session_routes("/office/api", sessions.clone()))
                .configure(folder_routes("/office/api", folder.clone()))
        })
        .workers(1)
        .bind_uds(&cli.socket)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o660))?;
        tracing::info!(root = %cli.root.display(), "native office host up");
        server.run().await?;
        Ok(())
    })
}
