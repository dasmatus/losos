//! `losos-office` — Office documents in a folder, through Collabora Online.
//!
//! The WOPI host of [`losos_ctl::office`] over a plain folder, for a box that
//! edits documents without LosOS Cloud's file store (and for the VM test).
//! LosOS Cloud mounts the same routes over its own store instead.
//!
//! Two listeners, because they trust different things:
//!
//!   * `--wopi` (TCP, loopback): `/wopi/files/…` for coolwsd, which cannot
//!     dial a Unix socket. The access token is the whole authentication.
//!   * `--socket` (Unix, only nginx may open it): `/office/api/open`,
//!     `/office/api/extensions` and `/office/api/files` for the Office page,
//!     where nginx names the signed-in person in `X-Losos-Cloud-User`.

use actix_web::{App, HttpServer};
use clap::Parser;
use losos_ctl::office::actix::{folder_routes, session_routes, wopi_routes, Sessions};
use losos_ctl::office::discovery::Coolwsd;
use losos_ctl::office::store::DirStore;
use losos_ctl::office::{TokenKey, Wopi};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    version,
    about = "Edit the Office documents in a folder with Collabora Online"
)]
struct Cli {
    /// The folder whose documents are edited.
    #[arg(long)]
    root: PathBuf,
    /// Where coolwsd reaches the WOPI routes.
    #[arg(long, default_value = "127.0.0.1:8093")]
    wopi: String,
    /// The Unix socket nginx relays the editor page's calls to.
    #[arg(long)]
    socket: PathBuf,
    /// coolwsd's own address, for its discovery document.
    #[arg(long, default_value = "http://127.0.0.1:9980")]
    coolwsd: String,
    /// The access tokens' signing key, made on first start.
    #[arg(long)]
    key_file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    losos_ctl::trace::init("info");
    let cli = Cli::parse();
    let key = TokenKey::load_or_create(&cli.key_file)?;
    let store = Arc::new(DirStore::new(&cli.root));
    let folder = store.clone();
    let wopi = Arc::new(Wopi::new(store, key, &format!("http://{}", cli.wopi)));
    let sessions = Arc::new(Sessions {
        wopi: wopi.clone(),
        coolwsd: Coolwsd::new(&cli.coolwsd),
    });

    let _ = std::fs::remove_file(&cli.socket);
    actix_web::rt::System::new().block_on(async move {
        let for_coolwsd = HttpServer::new(move || App::new().configure(wopi_routes(wopi.clone())))
            .workers(2)
            .bind(&cli.wopi)?
            .run();
        let socket = cli.socket.clone();
        let for_nginx = HttpServer::new(move || {
            App::new()
                .configure(session_routes("/office/api", sessions.clone()))
                .configure(folder_routes("/office/api", folder.clone()))
        })
        .workers(1)
        .bind_uds(&cli.socket)?;
        // Owner and group (nginx's) only; the folder's mode keeps others out.
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o660))?;
        tracing::info!(root = %cli.root.display(), wopi = %cli.wopi, "office host up");
        tokio::try_join!(for_coolwsd, for_nginx.run())?;
        Ok(())
    })
}
