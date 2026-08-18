//! losos-registrar binary entry point.
//!
//! The tokio runtime is built explicitly (rather than `#[tokio::main]`) so the
//! worker count, thread name, and stack size are pinned for a long-lived
//! daemon. `anyhow` lives only here and in the thin orchestration paths — the
//! library surfaces concrete `thiserror` enums ([`losos_registrar::ApiError`],
//! [`losos_registrar::RegistryError`]).

use std::process::ExitCode;

use losos_registrar::opts::{parse, Mode};
use tracing::error;

fn main() -> ExitCode {
    // Two workers are plenty: the registrar is IO-bound and low-traffic
    // (register/heartbeat on the order of once per appliance per TTL).
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("losos-registrar")
        .enable_all()
        .build();
    let runtime = match runtime {
        Ok(rt) => rt,
        Err(e) => {
            error!("{}: {e:#}", losos_registrar::action::Action::BuildRuntime);
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async_main())
}

async fn async_main() -> ExitCode {
    // Skip the program name; the rest is the subcommand + its flags.
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(args) {
        Ok(Mode::Serve(opts)) => match losos_registrar::server::run(opts).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                error!("fatal: {e:#}");
                ExitCode::FAILURE
            }
        },
        Ok(Mode::Announce(opts)) => match losos_registrar::announce::run(opts).await {
            // announce runs forever; reaching here is an unexpected return
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                error!("{} fatal: {e:#}", losos_registrar::action::Action::Announce);
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            error!("{e:#}");
            ExitCode::FAILURE
        }
    }
}