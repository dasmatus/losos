//! `losos-ci` — Mesh CI: GitHub Actions workflows run on the boxes of a
//! mesh.
//!
//! `serve`, run by `losos-ci.service` as `losos-ci`, and `runner`, run by
//! `losos-ci-runner.service` as `shared` (`modules/ci.nix`). Everything it
//! does is described in [`losos_ctl::ci`].

use clap::{Parser, Subcommand};
use losos_ctl::ci::{api, db::Store, dispatch::Ci, net, runner};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    version,
    about = "Mesh CI: GitHub Actions workflows on the boxes of a mesh"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Take LosOS Git's events, plan runs, follow jobs on the mesh.
    Serve {
        /// The database, the logs and the commits in flight.
        #[arg(long, default_value = "/var/lib/losos-ci")]
        state_dir: PathBuf,
        /// Where LosOS Git sends pushes and pull requests.
        #[arg(long, default_value = "/run/losos-ci-events/events.sock")]
        events_socket: PathBuf,
        /// Where this box's runner asks for work.
        #[arg(long, default_value = "/run/losos-ci-runner/runner.sock")]
        runner_socket: PathBuf,
        /// Where lososd's ci plugin relays the admin pages' requests.
        #[arg(long, default_value = "/run/losos-ci/api.sock")]
        page_socket: PathBuf,
        /// LosOS Git's internal socket.
        #[arg(long, default_value = "/run/losos-git-peers/internal.sock")]
        git_socket: PathBuf,
        /// `github.server_url` for the workflows: this box's LosOS Git.
        #[arg(long)]
        server_url: String,
        /// Jobs of private repositories may go to other boxes.
        #[arg(long)]
        private_on_mesh: bool,
        /// The compute window, HH:MM-HH:MM in the box's time zone.
        #[arg(long)]
        window: Option<String>,
    },
    /// Run jobs, one at a time, as the box's `shared` user.
    Runner {
        #[arg(long, default_value = "/run/losos-ci-runner/runner.sock")]
        socket: PathBuf,
        /// Job directories, one per job, gone after it.
        #[arg(long, default_value = "/var/lib/losos-ci-runner/jobs")]
        work_dir: PathBuf,
        /// This box's name in the logs, `runner.name`.
        #[arg(long)]
        name: String,
        /// `pm`, or `host` to run steps as plain processes.
        #[arg(long, default_value = "pm")]
        sandbox: String,
        #[arg(long, default_value = "localhost/losos-ci-base:latest")]
        default_image: String,
        /// pm, which starts and stops the job's containers.
        #[arg(long, default_value = "pm")]
        pm: PathBuf,
        /// pm's configuration directory with Mesh CI's plugin in it
        /// (pm/plugins/losos-ci.wasm and the key that signed it).
        #[arg(long)]
        pm_config: Option<PathBuf>,
        /// A runs-on label and the image it runs in, `label=image`.
        #[arg(long = "image")]
        images: Vec<String>,
        #[arg(long, default_value = "2g")]
        memory: String,
        #[arg(long, default_value = "1.5")]
        cpus: String,
        /// The node JavaScript actions run with.
        #[arg(long)]
        node: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .without_time()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    match Cli::parse().command {
        Cmd::Serve {
            state_dir,
            events_socket,
            runner_socket,
            page_socket,
            git_socket,
            server_url,
            private_on_mesh,
            window,
        } => {
            std::fs::create_dir_all(&state_dir)?;
            let store = Store::open(&state_dir.join("ci.db"))?;
            let edge = net::EdgeConfig::from_env().ok_or_else(|| {
                anyhow::anyhow!("LOSOS_CI_APPLIANCE_ID and the edge token are not set")
            })?;
            let box_id = edge.appliance_id.clone();
            let window = window.and_then(|w| {
                let (s, e) = w.split_once('-')?;
                Some((s.to_string(), e.to_string()))
            });
            let ci = Ci::new(
                store,
                Box::new(net::Edge { config: edge }),
                Box::new(net::Git { socket: git_socket }),
                state_dir,
                box_id,
                server_url,
                private_on_mesh,
                window,
            );
            api::serve(
                Arc::new(ci),
                api::Sockets {
                    events: events_socket,
                    runner: runner_socket,
                    page: page_socket,
                },
            )
        }
        Cmd::Runner {
            socket,
            work_dir,
            name,
            sandbox,
            default_image,
            pm,
            pm_config,
            images,
            memory,
            cpus,
            node,
        } => {
            let images: BTreeMap<String, String> = images
                .iter()
                .filter_map(|i| i.split_once('='))
                .map(|(l, i)| (l.trim().to_ascii_lowercase(), i.trim().to_string()))
                .collect();
            runner::run(runner::Config {
                socket,
                work: work_dir,
                name,
                sandbox,
                default_image,
                pm: pm_config.map(|config| runner::pm::Tool { bin: pm, config }),
                images,
                limits: runner::sandbox::Limits { memory, cpus },
                node,
            })
        }
    }
}
