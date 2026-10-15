//! Logging with tracing, to stderr, without timestamps: a terminal command's
//! log is read right after it ran, and the time adds nothing but width.
//!
//! Quiet by default (warnings only). `-v` adds what the client does, `-vv`
//! each request and the keychain calls, `-vvv` everything; `LOSOS_LOG`
//! takes a full filter (`LOSOS_LOG=losos=debug,ureq=debug`). While a full
//! screen is up, lines are held and written once it closes, so they never
//! draw over it. Secrets, tokens and keys are never logged.

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

static HOLD: AtomicBool = AtomicBool::new(false);
static HELD: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Hold log lines while a full screen is up; write them when it closes.
pub fn hold(on: bool) {
    HOLD.store(on, Ordering::SeqCst);
    if !on {
        let held = std::mem::take(&mut *HELD.lock().unwrap_or_else(|e| e.into_inner()));
        if !held.is_empty() {
            let _ = io::stderr().write_all(&held);
        }
    }
}

pub struct Stderr;

impl Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if HOLD.load(Ordering::SeqCst) {
            HELD.lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend_from_slice(buf);
            Ok(buf.len())
        } else {
            io::stderr().write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }
}

struct MakeStderr;

impl<'a> MakeWriter<'a> for MakeStderr {
    type Writer = Stderr;

    fn make_writer(&'a self) -> Stderr {
        Stderr
    }
}

/// The filter for `-v` given `verbose` times.
pub fn filter_for(verbose: u8) -> String {
    let level = match verbose {
        0 => return "warn".into(),
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    format!("warn,losos={level},losos_access={level}")
}

/// Start logging. `env` is `LOSOS_LOG`, which wins over `-v`.
pub fn init(verbose: u8, env: Option<String>) {
    let filter = env
        .and_then(|e| EnvFilter::try_new(e).ok())
        .unwrap_or_else(|| EnvFilter::new(filter_for(verbose)));
    let _ = tracing_subscriber::fmt()
        .without_time()
        .with_target(verbose >= 2)
        .with_ansi(io::stderr().is_terminal())
        .with_writer(MakeStderr)
        .with_env_filter(filter)
        .try_init();
}
