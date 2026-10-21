//! `losos-cloud` — LosOS Cloud, the owner's files and the people's.
//!
//! `serve` is the systemd unit (`modules/cloud.nix`); `scan` brings the
//! table in line with files changed on disk; `import` moves a Nextcloud
//! data folder's files in. The rules are in [`losos_ctl::cloud`].

use clap::{Parser, Subcommand};
use losos_ctl::cloud::{self, server, Cloud, Config};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    version,
    about = "LosOS Cloud: files, WebDAV and the Nextcloud app protocol"
)]
struct Cli {
    /// Where the files and the database are.
    #[arg(long, default_value = "/var/lib/losos-cloud")]
    root: PathBuf,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve on a Unix socket (the box) or a TCP address (development).
    Serve {
        #[arg(long, conflicts_with = "listen")]
        socket: Option<PathBuf>,
        #[arg(long)]
        listen: Option<String>,
        /// Believe X-Losos-Cloud-User on a TCP listener too. Never on a
        /// port anyone but a proxy can reach.
        #[arg(long)]
        trust_header: bool,
        #[arg(long, default_value_t = 4)]
        workers: usize,
        #[arg(long, default_value = "LosOS Cloud")]
        product: String,
        #[arg(long, default_value_t = 30)]
        trash_days: i64,
        /// Watch the files on disk and bring changes made behind the
        /// service's back into the table as they happen (the Windows share).
        #[arg(long)]
        watch: bool,
    },
    /// Bring the table in line with the files on disk, for one person or all.
    Scan {
        #[arg(long)]
        owner: Option<String>,
    },
    /// Make an app password for a person, printed once. For the box's own
    /// tools and for tests; people get theirs through Login Flow v2.
    AppPassword {
        #[arg(long)]
        login: String,
        #[arg(long, default_value = "losos-cloud")]
        name: String,
    },
    /// Move every person's files out of a Nextcloud data folder
    /// (`<from>/<login>/files/`) into the cloud. Files already in the cloud
    /// keep their place; a name taken gets the Nextcloud copy beside it.
    Import {
        #[arg(long)]
        from: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let cli = Cli::parse();
    match cli.command {
        Cmd::Serve {
            socket,
            listen,
            trust_header,
            workers,
            product,
            trash_days,
            watch,
        } => {
            let cloud = Arc::new(Cloud::open(
                &cli.root,
                Config {
                    product,
                    trash_days,
                },
            )?);
            if watch {
                cloud::watch::spawn(cloud.clone())?;
            }
            let listen = match (socket, listen) {
                (Some(s), _) => server::Listen::Unix(s),
                (None, Some(l)) => server::Listen::Tcp(l),
                (None, None) => server::Listen::Unix("/run/losos-cloud/http.sock".into()),
            };
            actix_web::rt::System::new().block_on(server::serve(
                cloud,
                server::Options {
                    listen,
                    trust_header,
                    workers,
                },
            ))?;
            Ok(())
        }
        Cmd::Scan { owner } => {
            let cloud = Cloud::open(&cli.root, Config::default())?;
            let owners = match owner {
                Some(o) => vec![o],
                None => cloud.store.owners()?,
            };
            for o in owners {
                cloud.store.ensure_home(&o)?;
                let n = cloud.store.scan(&o)?;
                println!("{o}: {n} changes");
            }
            Ok(())
        }
        Cmd::AppPassword { login, name } => {
            let cloud = Cloud::open(&cli.root, Config::default())?;
            cloud.store.ensure_home(&login)?;
            let (_, pw) = cloud.auth.new_app_password(&cloud.store, &login, &name)?;
            println!("{pw}");
            Ok(())
        }
        Cmd::Import { from } => {
            let cloud = Cloud::open(&cli.root, Config::default())?;
            for (login, n) in cloud::import::nextcloud(&cloud, &from)? {
                println!("{login}: {n} entries");
            }
            Ok(())
        }
    }
}
