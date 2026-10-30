//! How the backend's services write their own log lines.
//!
//! Under systemd a service's stderr is a journal stream, and every line of
//! it lands at the stream's one priority, info. Settings › Logs filters by
//! priority (`journalctl --priority`), so an error lososd wrote would never
//! show under Errors. journald reads a `<N>` syslog prefix at the start of
//! a line as that line's priority and strips it, so here each event starts
//! with its level's: `<3>` for an error, `<4>` for a warning.

use std::fmt;

use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::EnvFilter;

/// The syslog priority journald files a line of `level` under.
#[must_use]
pub fn priority(level: Level) -> u8 {
    if level == Level::ERROR {
        3
    } else if level == Level::WARN {
        4
    } else if level == Level::INFO {
        6
    } else {
        7
    }
}

/// A format with the event's syslog prefix in front.
pub struct Prioritised<F>(pub F);

impl<S, N, F> FormatEvent<S, N> for Prioritised<F>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    F: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        write!(writer, "<{}>", priority(*event.metadata().level()))?;
        self.0.format_event(ctx, writer, event)
    }
}

/// Logs to stderr at `RUST_LOG`, else `default`, without a time (the
/// journal stamps its own) and, on a journal stream, without colours and
/// with each line's priority.
pub fn init(default: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    let builder = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .without_time()
        .with_env_filter(filter);
    if std::env::var_os("JOURNAL_STREAM").is_some() {
        builder
            .with_ansi(false)
            .event_format(Prioritised(
                tracing_subscriber::fmt::format()
                    .without_time()
                    .with_ansi(false),
            ))
            .init();
    } else {
        builder.init();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Kept(Arc<Mutex<Vec<u8>>>);

    impl Write for Kept {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn each_line_starts_with_its_priority() {
        let kept = Kept::default();
        let into = kept.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || into.clone())
            .with_ansi(false)
            .with_max_level(Level::TRACE)
            .event_format(Prioritised(
                tracing_subscriber::fmt::format()
                    .without_time()
                    .with_ansi(false),
            ))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(what = "disk", "storage failed");
            tracing::warn!("no size reading");
            tracing::info!("started");
            tracing::debug!("details");
        });
        let text = String::from_utf8(kept.0.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4, "{text}");
        assert!(lines[0].starts_with("<3>ERROR "), "{}", lines[0]);
        assert!(
            lines[0].ends_with("storage failed what=\"disk\""),
            "{}",
            lines[0]
        );
        assert!(lines[1].starts_with("<4> WARN "), "{}", lines[1]);
        assert!(lines[2].starts_with("<6> INFO "), "{}", lines[2]);
        assert!(lines[3].starts_with("<7>DEBUG "), "{}", lines[3]);
    }
}
