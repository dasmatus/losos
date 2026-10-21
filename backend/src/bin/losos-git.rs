//! `losos-git` — LosOS Git, the box's own Git server.
//!
//! `serve`, run by `losos-git.service`, and `import-forgejo`, run once by
//! `losos-git-forgejo-import.service` on a box that ran Forgejo
//! (`modules/git.nix`).
//! Everything it does is described in [`losos_ctl::forge`].

use clap::{Parser, Subcommand};
use losos_ctl::forge::{db::Store, import, server, Config};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "LosOS Git: the box's own Git server")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve git's smart HTTP protocol and the JSON API.
    Serve {
        /// The database, the repositories and the admin token.
        #[arg(long, default_value = "/var/lib/losos-git")]
        state_dir: PathBuf,
        /// The Unix socket nginx proxies to; the person header is believed
        /// on it and nowhere else.
        #[arg(long)]
        socket: Option<PathBuf>,
        /// A TCP address for callers with a token, such as 127.0.0.1:3000.
        #[arg(long)]
        listen: Option<String>,
        /// The git binary.
        #[arg(long, default_value = "git")]
        git: PathBuf,
        /// Nobody sees anything, public repositories included, without
        /// signing in.
        #[arg(long)]
        require_sign_in: bool,
        /// New repositories are public unless their maker says otherwise.
        #[arg(long)]
        public_by_default: bool,
        /// A push to a repository that does not exist yet makes it, in the
        /// pusher's own namespace.
        #[arg(long)]
        push_create: bool,
        /// Where the pages are below the box's address.
        #[arg(long, default_value = "/git/")]
        base_path: String,
    },
    /// Carry Forgejo's records into LosOS Git from the export in `--from`
    /// (`<from>/tables/*.json`), and write `<from>/import.json`.
    ImportForgejo {
        #[arg(long, default_value = "/var/lib/losos-git")]
        state_dir: PathBuf,
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
    match Cli::parse().command {
        Cmd::ImportForgejo { state_dir, from } => {
            let store = Store::open(&state_dir.join("git.db"))?;
            let report = import::import(&store, &state_dir, &from)?;
            let text = serde_json::to_string_pretty(&serde_json::json!({
                "complete": true,
                "when": losos_ctl::forge::now(),
                "forgejo": report.forgejo,
                "imported": report.imported,
                "notImported": report.not_imported,
            }))?;
            std::fs::write(from.join("import.json"), text + "\n")?;
            for line in &report.not_imported {
                println!("not imported: {line}");
            }
            println!("imported: {}", serde_json::to_string(&report.imported)?);
            Ok(())
        }
        Cmd::Serve {
            state_dir,
            socket,
            listen,
            git,
            require_sign_in,
            public_by_default,
            push_create,
            base_path,
        } => server::run(
            Config {
                state_dir,
                git,
                require_sign_in,
                default_private: !public_by_default,
                push_create,
                base_path,
            },
            server::Listen {
                socket,
                tcp: listen,
            },
        ),
    }
}
