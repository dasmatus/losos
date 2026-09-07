//! losos-registrar binary entry point.
//!
//! The tokio runtime is built explicitly (rather than `#[tokio::main]`) so the
//! worker count, thread name, and stack size are pinned for a long-lived
//! daemon. Errors aggregate to `miette::Report`: the library surfaces
//! concrete `thiserror` enums ([`losos_registrar::ApiError`],
//! [`losos_registrar::RegistryError`]) which implement `miette::Diagnostic`,
//! and `main` renders a fatal `Report` with the `fancy` handler so the cause
//! chain + context print as a readable diagnostic rather than a bare message.
//!
//! Structured runtime logging goes through `tracing` (initialised here with
//! an env-filter subscriber) so `RUST_LOG=losos::reconcile=debug` etc. filter
//! per phase — see [`losos_registrar::action::Action::target`].

use std::process::ExitCode;

use losos_registrar::opts::{parse, Mode};
use miette::miette;

fn main() -> ExitCode {
    // Env-filter honours `RUST_LOG`; default to `info` when it's unset so the
    // boot/reconcile lifecycle is visible without configuring anything.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

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
            // `e` is a bare io::Error, not Diagnostic — wrap it in a Report so
            // it prints with the same fancy handler as the paths below.
            eprintln!(
                "{:?}",
                miette!("{}: {e}", losos_registrar::action::Action::BuildRuntime),
            );
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
            // `{:?}` on a miette::Report renders the full diagnostic + cause
            // chain via the `fancy` handler.
            Err(e) => {
                eprintln!("{:?}", e);
                ExitCode::FAILURE
            }
        },
        Ok(Mode::Announce(opts)) => match losos_registrar::announce::run(opts).await {
            // announce runs forever; reaching here is an unexpected return
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{} fatal", losos_registrar::action::Action::Announce);
                eprintln!("{:?}", e);
                ExitCode::FAILURE
            }
        },
        Ok(Mode::Seed(opts)) => match losos_registrar::seed::run(opts).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{:?}", e);
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("{:?}", e);
            ExitCode::FAILURE
        }
    }
}
