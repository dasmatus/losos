//! Lifecycle / phase labels for the registrar.
//!
//! Each variant names a distinct phase of the `serve`/`announce` lifecycle.
//! They serve two purposes:
//!   * [`Action::target`] returns a `tracing` target string so log events can
//!     be filtered per phase via `RUST_LOG` (e.g. `RUST_LOG=losos::reconcile=debug`).
//!   * [`Display`] renders a short `[tag]` prefix for the binary's fatal-error
//!     paths in `main` (kept for the human-facing boot messages).
//!
//! Add a variant here when a new phase needs its own log target; update both
//! `target()` and `Display` so the two views never drift.

use std::fmt::Display;

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Announce,
    Join,
    Serve,
    Seed,
    BuildRuntime,
    Register,
    Heartbeat,
    Deregister,
    Reconcile,
    Authenticate,
    Bind,
    LoadRegistry,
}

impl Action {
    /// The `tracing` target for events emitted during this phase. Stable
    /// string literals so `RUST_LOG=losos::<target>=<level>` filters work
    /// across releases.
    #[must_use]
    pub const fn target(self) -> &'static str {
        match self {
            Action::Announce => "losos::announce",
            Action::Join => "losos::join",
            Action::Serve => "losos::serve",
            Action::Seed => "losos::seed",
            Action::BuildRuntime => "losos::runtime",
            Action::Register => "losos::register",
            Action::Heartbeat => "losos::heartbeat",
            Action::Deregister => "losos::deregister",
            Action::Reconcile => "losos::reconcile",
            Action::Authenticate => "losos::auth",
            Action::Bind => "losos::bind",
            Action::LoadRegistry => "losos::registry",
        }
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Announce => f.write_str("[announce]"),
            Action::Join => f.write_str("[join]"),
            Action::Serve => f.write_str("[serve]"),
            Action::Seed => f.write_str("[seed]"),
            Action::BuildRuntime => f.write_str("[build runtime]"),
            Action::Register => f.write_str("[register]"),
            Action::Heartbeat => f.write_str("[heartbeat]"),
            Action::Deregister => f.write_str("[deregister]"),
            Action::Reconcile => f.write_str("[reconcile]"),
            Action::Authenticate => f.write_str("[authenticate]"),
            Action::Bind => f.write_str("[bind]"),
            Action::LoadRegistry => f.write_str("[load registry]"),
        }
    }
}
