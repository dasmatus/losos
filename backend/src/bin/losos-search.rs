//! `losos-search` — text inside the owner's files, for the admin palette.
//!
//! Three subcommands, each one systemd unit in `modules/search.nix`:
//!
//!   * `index` brings the cloud index up to date and exits (a timer runs it);
//!   * `cloud` answers one search of that index on stdin/stdout;
//!   * `git` answers one search of the Git repositories the same way.
//!
//! The two searches run per connection under socket activation, so stdin and
//! stdout are the socket lososd connected to. The rules are in
//! [`losos_ctl::textsearch`].

use clap::{Parser, Subcommand};
use losos_ctl::textsearch::{self, extract, git, index::Index, Answer, CloudHit};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(version, about = "Search the text inside the owner's files")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Bring the cloud index up to date with the files under --root.
    Index {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        db: PathBuf,
        /// poppler's pdftotext; without it PDFs are skipped.
        #[arg(long)]
        pdftotext: Option<PathBuf>,
    },
    /// Answer one search of the cloud index, a JSON line in and out.
    Cloud {
        #[arg(long)]
        db: PathBuf,
    },
    /// Answer one search of the repositories under --repos.
    Git {
        #[arg(long)]
        repos: PathBuf,
        #[arg(long, default_value = "git")]
        git: PathBuf,
    },
}

/// How long a Git search may take before it answers with what it found.
const GIT_BUDGET: Duration = Duration::from_secs(4);

fn main() -> anyhow::Result<()> {
    losos_ctl::trace::init("info");
    match Cli::parse().command {
        Cmd::Index {
            root,
            db,
            pdftotext,
        } => {
            if !root.is_dir() {
                // LosOS Cloud makes the folder on the owner's first sign-in.
                tracing::info!(root = %root.display(), "no files yet");
                return Ok(());
            }
            let started = Instant::now();
            let mut index = Index::open(&db)?;
            let pdf = pdftotext.as_deref();
            let stats = index.sync(&root, &|path, size| extract::extract(path, size, pdf))?;
            tracing::info!(
                seen = stats.seen,
                read = stats.read,
                with_text = stats.with_text,
                removed = stats.removed,
                seconds = started.elapsed().as_secs_f32(),
                "index up to date"
            );
            Ok(())
        }
        Cmd::Cloud { db } => {
            textsearch::serve_one(std::io::stdin(), std::io::stdout(), |terms, limit| {
                match Index::open_existing(&db)? {
                    Some(index) => index.search(terms, limit),
                    // The first pass has not made the file yet.
                    None => Ok(Answer::<CloudHit> {
                        hits: Vec::new(),
                        partial: true,
                    }),
                }
            })
        }
        Cmd::Git { repos, git: bin } => {
            textsearch::serve_one(std::io::stdin(), std::io::stdout(), |terms, limit| {
                let deadline = Instant::now() + GIT_BUDGET;
                Ok(Answer {
                    hits: git::search(&repos, &bin, terms, limit, deadline),
                    partial: false,
                })
            })
        }
    }
}
