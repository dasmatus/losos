//! Write a lososd plugin.
//!
//! A plugin is a WebAssembly component lososd loads from its read-only
//! plugin directory. It answers the routes its manifest lists and reaches the
//! box only through the host's imports, which this crate wraps as plain Rust:
//! [`fs`], [`exec`], [`units`], [`http`], [`lock`], [`cache`], [`jobs`],
//! [`dispatch`], [`config`], [`clock`], [`random`] and [`log`]. The contract
//! itself is `backend/plugins/wit/plugin.wit`.
//!
//! ```ignore
//! struct Hello;
//!
//! impl losos_plugin::Plugin for Hello {
//!     fn handle(req: losos_plugin::Request) -> losos_plugin::Response {
//!         losos_plugin::Response::json(200, &serde_json::json!({ "hello": req.path }))
//!     }
//! }
//!
//! losos_plugin::export!(Hello);
//! ```
//!
//! One call is one fresh instance: statics start empty every time, so
//! anything that must outlive a call goes in a file, the [`cache`] or a job.

#![allow(unreachable_pub)]

#[doc(hidden)]
pub mod bindings {
    wit_bindgen::generate!({
        world: "plugin",
        path: "../wit",
        pub_export_macro: true,
        export_macro_name: "export_bindings",
        default_bindings_module: "losos_plugin::bindings",
        generate_all,
    });
}

use bindings::losos::plugin as wit;

pub mod cache;
pub mod clock;
pub mod config;
pub mod dispatch;
pub mod exec;
pub mod fs;
pub mod http;
pub mod jobs;
pub mod lock;
pub mod log;
pub mod random;
pub mod units;

mod request;
pub use request::{Caller, Person, Principal, Request, Response};

/// Why a host call did not do what was asked: `wit/plugin.wit`'s `error`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Outside what the manifest grants.
    #[error("denied: {0}")]
    Denied(String),
    /// The file, unit, lock or key is not there.
    #[error("not found")]
    NotFound,
    /// Tried and failed; the words are for the journal.
    #[error("{0}")]
    Failed(String),
    /// A deadline passed first.
    #[error("timed out")]
    TimedOut,
}

impl Error {
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound)
    }
}

impl From<wit::types::Error> for Error {
    fn from(e: wit::types::Error) -> Self {
        match e {
            wit::types::Error::Denied(why) => Error::Denied(why),
            wit::types::Error::NotFound => Error::NotFound,
            wit::types::Error::Failed(why) => Error::Failed(why),
            wit::types::Error::TimedOut => Error::TimedOut,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// What a plugin does. Only [`Plugin::handle`] is required.
pub trait Plugin {
    /// One request to a route the manifest lists.
    fn handle(req: Request) -> Response;

    /// A manifest timer fired, or `start` once when lososd starts.
    fn tick(_timer: &str) {}

    /// A job this plugin spawned with [`jobs::spawn`].
    fn run(_kind: &str, _input: &[u8]) {}
}

/// Send panics to the journal before the instance traps: wasm32 has no
/// stderr, and a silent 500 is the worst kind.
#[doc(hidden)]
pub fn __install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            log::error(&format!("panic: {info}"));
        }));
    });
}

/// Export a type that implements [`Plugin`] as this component's plugin.
#[macro_export]
macro_rules! export {
    ($plugin:ty) => {
        #[doc(hidden)]
        struct __LososExport;

        impl $crate::bindings::exports::losos::plugin::handler::Guest for __LososExport {
            fn handle(
                request: $crate::bindings::losos::plugin::types::Request,
            ) -> $crate::bindings::losos::plugin::types::Response {
                $crate::__install_panic_hook();
                <$plugin as $crate::Plugin>::handle($crate::Request::from_wit(request)).into_wit()
            }
        }

        impl $crate::bindings::exports::losos::plugin::background::Guest for __LososExport {
            fn tick(timer: String) {
                $crate::__install_panic_hook();
                <$plugin as $crate::Plugin>::tick(&timer)
            }

            fn run(kind: String, input: Vec<u8>) {
                $crate::__install_panic_hook();
                <$plugin as $crate::Plugin>::run(&kind, &input)
            }
        }

        // Only a wasm32 build exports anything: the export names hold `:`,
        // which a host linker refuses, and a host build of a plugin exists
        // only to run its unit tests.
        #[cfg(target_arch = "wasm32")]
        $crate::bindings::export_bindings!(__LososExport);
        #[cfg(not(target_arch = "wasm32"))]
        const _: fn($crate::Request) -> $crate::Response = <$plugin as $crate::Plugin>::handle;
    };
}
