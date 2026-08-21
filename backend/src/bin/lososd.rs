//! `lososd` — the appliance control daemon.
//!
//! Runs as root under systemd and does three things, in this order:
//!
//!   1. claims `org.losos1` on the system bus and exports the control
//!      interface for `losos-ctl`,
//!   2. re-attaches a watcher to any rebuild still recorded as in flight, and
//!   3. serves the Bearer-authed loopback HTTP API the admin UI talks to.
//!
//! Step 2 must not be dropped: `nixos-rebuild switch` restarts this daemon
//! during activation, so the watcher that started a rebuild dies partway
//! through it.

use losos_ctl::io_backend::Paths;
use losos_ctl::{dbus, http, supervisor};

fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("lososd: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let paths = Paths::from_env();

    // The HTTP server needs an actix `System` (a current-thread runtime with a
    // LocalSet); zbus needs a general tokio runtime. Running both in one
    // runtime is the classic way to deadlock this pair, so each gets its own
    // thread. Nothing crosses the boundary but plain data, because the command
    // core is synchronous.
    let http_paths = paths.clone();
    let http_thread = std::thread::Builder::new()
        .name("lososd-http".into())
        .spawn(move || http::serve(http_paths))?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("lososd")
        .enable_all()
        .build()?;

    rt.block_on(async {
        // LOSOS_NO_DBUS exists for smoke-testing the HTTP surface on a machine
        // with no system bus; the daemon is otherwise useless without it.
        let _conn = match std::env::var("LOSOS_NO_DBUS") {
            Ok(_) => {
                tracing::warn!("LOSOS_NO_DBUS set — D-Bus listener disabled");
                None
            }
            Err(_) => Some(dbus::serve(paths.clone()).await?),
        };

        supervisor::start_supervisor(&paths);

        // The bus connection and the HTTP thread carry the load from here.
        std::future::pending::<()>().await;
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    })?;

    match http_thread.join() {
        Ok(r) => r?,
        Err(_) => anyhow::bail!("http thread panicked"),
    }
    Ok(())
}
