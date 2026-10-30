//! `losos-git` — LosOS Git, the box's own Git server.
//!
//! `serve`, run by `losos-git.service`, `import-forgejo`, run once by
//! `losos-git-forgejo-import.service` on a box that ran Forgejo
//! (`modules/git.nix`), `import-boards`, run once by
//! `losos-git-boards-import.service` to take LosOS Cloud's boards into the
//! issue tracker, and `review-diff`, which reviews a diff the way the
//! reviewer reviews a pull request and prints the review.
//! Everything it does is described in [`losos_ctl::forge`].

use clap::{Parser, Subcommand};
use losos_ctl::forge::reviewer::{model, patch, review};
use losos_ctl::forge::{db::Store, import, server, tracker, Config};
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
        /// The Unix socket for services on the box (Mesh CI): a commit's
        /// files, and checks on commits. Its directory is the gate.
        #[arg(long)]
        internal_socket: Option<PathBuf>,
        /// Tell this socket about pushes and pull requests (Mesh CI's).
        #[arg(long)]
        events_socket: Vec<PathBuf>,
        /// Take other boxes' self-signed certificates when asking their
        /// chat servers who someone is, keeping each one's key.
        #[arg(long)]
        matrix_self_signed: bool,
        /// Reach a chat server at a fixed address, `name=url` (tests).
        #[arg(long, hide = true)]
        matrix_server_url: Vec<String>,
        /// This box's own chat server name, which nobody signs in through.
        #[arg(long)]
        matrix_server_name: Option<String>,
    },
    /// Carry Forgejo's records into LosOS Git from the export in `--from`
    /// (`<from>/tables/*.json`), and write `<from>/import.json`.
    ImportForgejo {
        #[arg(long, default_value = "/var/lib/losos-git")]
        state_dir: PathBuf,
        #[arg(long)]
        from: PathBuf,
    },
    /// Take LosOS Cloud's boards into the issue tracker from a copy of the
    /// cloud's database, and write `<report>`.
    ImportBoards {
        #[arg(long, default_value = "/var/lib/losos-git")]
        state_dir: PathBuf,
        /// A copy of `/var/lib/losos-cloud/cloud.db`.
        #[arg(long)]
        from: PathBuf,
        /// Where to write what came over, as JSON.
        #[arg(long)]
        report: PathBuf,
    },
    /// Review the `git diff` on stdin with the box's code model, as LosOS
    /// Reviewer would review a pull request, and print the review. The
    /// model is the one in `LOSOS_REVIEWER_URL` and `LOSOS_REVIEWER_MODEL`.
    ReviewDiff {
        /// The pull request's title.
        #[arg(long, default_value = "")]
        title: String,
        /// Also print every question and answer, as JSON lines, here.
        #[arg(long)]
        record: Option<PathBuf>,
    },
}

/// The model, writing down what it was asked and what it said.
struct Recording {
    config: model::Config,
    log: Option<std::sync::Mutex<std::fs::File>>,
}

impl review::Ask for Recording {
    fn ask(&self, system: &str, user: &str, max_tokens: u32) -> anyhow::Result<String> {
        let started = std::time::Instant::now();
        let reply = model::ask(&self.config, system, user, max_tokens, false)?;
        if let Some(log) = &self.log {
            use std::io::Write;
            let line = serde_json::json!({
                "system": system, "user": user, "answer": reply.text,
                "promptTokens": reply.prompt_tokens, "answerTokens": reply.answer_tokens,
                "seconds": started.elapsed().as_secs_f64(),
            });
            writeln!(log.lock().unwrap_or_else(|p| p.into_inner()), "{line}")?;
        }
        Ok(reply.text)
    }
}

fn main() -> anyhow::Result<()> {
    losos_ctl::trace::init("info");
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
            // What Forgejo had as issues is the tracker's now.
            tracker::adopt::adopt_repo_issues(&store)?;
            for line in &report.not_imported {
                println!("not imported: {line}");
            }
            println!("imported: {}", serde_json::to_string(&report.imported)?);
            Ok(())
        }
        Cmd::ImportBoards {
            state_dir,
            from,
            report,
        } => {
            let store = Store::open(&state_dir.join("git.db"))?;
            tracker::adopt::adopt_repo_issues(&store)?;
            let done = tracker::boards::import(&store, &from)?;
            let text = serde_json::to_string_pretty(&serde_json::json!({
                "complete": true,
                "when": losos_ctl::forge::now(),
                "report": done,
            }))?;
            std::fs::write(&report, text + "\n")?;
            println!(
                "boards: {}, cards: {}, linked: {}, comments: {}, projects: {}",
                done.boards,
                done.cards,
                done.linked,
                done.comments,
                done.projects.join(" ")
            );
            Ok(())
        }
        Cmd::ReviewDiff { title, record } => {
            let config = model::Config::from_env().ok_or_else(|| {
                anyhow::anyhow!("set LOSOS_REVIEWER_URL and LOSOS_REVIEWER_MODEL")
            })?;
            let mut diff = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut diff)?;
            let ask = Recording {
                log: record
                    .map(|p| std::fs::File::create(p).map(std::sync::Mutex::new))
                    .transpose()?,
                config: config.clone(),
            };
            let out = review::run(&ask, &title, &patch::files(&diff), &mut |done, total| {
                eprintln!("reviewed {done} of {total} files");
                true
            })?;
            println!("{}\n", out.body(&config.model, false));
            for c in &out.comments {
                let from = if c.from_check { "check" } else { "model" };
                println!("--- {}:{} ({from})\n{}\n", c.path, c.line, c.body);
            }
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
            internal_socket,
            events_socket,
            matrix_self_signed,
            matrix_server_url,
            matrix_server_name,
        } => {
            for path in events_socket {
                losos_ctl::forge::events::deliver_to(path);
            }
            server::run(
                Config {
                    state_dir,
                    git,
                    require_sign_in,
                    default_private: !public_by_default,
                    push_create,
                    base_path,
                    matrix: losos_ctl::forge::matrix::MatrixConfig {
                        self_signed: matrix_self_signed,
                        urls: matrix_server_url
                            .iter()
                            .filter_map(|s| s.split_once('='))
                            .map(|(n, u)| (n.to_string(), u.to_string()))
                            .collect(),
                        own: matrix_server_name,
                    },
                },
                server::Listen {
                    socket,
                    tcp: listen,
                    internal: internal_socket,
                },
            )
        }
    }
}
