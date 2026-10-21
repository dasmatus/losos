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
    /// Move everything out of a Nextcloud: with `--dump`, first its
    /// calendars, contacts, boards and trash bin from a dump of its database
    /// (`modules/cloud.nix` makes it), then every person's files out of its
    /// data folder (`<from>/<login>/files/`). Files already in the cloud
    /// keep their place; a name taken gets the Nextcloud copy beside it.
    Import {
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        dump: Option<PathBuf>,
        /// Nextcloud's `config.php`, whose secret keeps its app passwords
        /// working.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Write what came across, per person, as JSON here.
        #[arg(long)]
        report: Option<PathBuf>,
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
        } => {
            let cloud = Arc::new(Cloud::open(
                &cli.root,
                Config {
                    product,
                    trash_days,
                },
            )?);
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
        Cmd::Import {
            from,
            dump,
            config,
            report,
        } => {
            let cloud = Cloud::open(&cli.root, Config::default())?;
            // The database first: should it fail, the files are still where
            // Nextcloud kept them and the next run starts over.
            let mut moved = match &dump {
                Some(d) => cloud::import::database(&cloud, d)?,
                None => cloud::import::Report::default(),
            };
            if let (Some(d), Some(c)) = (&dump, &config) {
                for (login, n) in cloud::import::passwords(&cloud, d, c)? {
                    moved.people.entry(login).or_default().app_passwords += n;
                }
            }
            if let Some(d) = &dump {
                for (login, n) in cloud::import::trash(&cloud, &from, d)? {
                    moved.people.entry(login).or_default().trash += n;
                }
            }
            for (login, n) in cloud::import::nextcloud(&cloud, &from)? {
                moved.people.entry(login).or_default().files += n;
            }
            cloud::import::write_notes(&cloud, &moved)?;
            for (login, m) in &moved.people {
                println!(
                    "{login}: {} files, {} in the trash, {} calendars ({} events, {} tasks), {} address books ({} contacts), {} boards ({} cards), {} app passwords, {} not moved",
                    m.files, m.trash, m.calendars, m.events, m.tasks, m.address_books, m.contacts, m.boards, m.cards, m.app_passwords, m.not_moved.len()
                );
            }
            if let Some(path) = report {
                std::fs::write(path, serde_json::to_vec_pretty(&moved)?)?;
            }
            Ok(())
        }
    }
}
