//! `losos-git` — LosOS Git, the box's own Git server.
//!
//! One subcommand, `serve`, run by `losos-git.service` (`modules/git.nix`).
//! Everything it does is described in [`losos_ctl::forge`].

use clap::{Parser, Subcommand};
use losos_ctl::forge::{server, Config};
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
        /// A push to a repository that does not exist does not make it.
        #[arg(long)]
        no_push_create: bool,
        /// Where the pages are below the box's address.
        #[arg(long, default_value = "/git/")]
        base_path: String,
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
        Cmd::Serve {
            state_dir,
            socket,
            listen,
            git,
            require_sign_in,
            public_by_default,
            no_push_create,
            base_path,
        } => server::run(
            Config {
                state_dir,
                git,
                require_sign_in,
                default_private: !public_by_default,
                push_create: !no_push_create,
                base_path,
            },
            server::Listen {
                socket,
                tcp: listen,
            },
        ),
    }
}
