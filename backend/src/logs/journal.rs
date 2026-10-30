//! Reading the journal through `journalctl --output=json`.
//!
//! journalctl rather than the journal files themselves: it is the one
//! reader systemd keeps compatible with its own file format, and it already
//! knows how to match a unit (`--unit` also takes the lines PID 1 writes
//! about it). Its output is read a line at a time and the process is
//! stopped once enough lines are in, so a search through a month of the
//! journal holds no more than one page in memory.
//!
//! The text filter runs here, after [`Redactor`], never as `--grep`: a
//! search must not find a secret by letters the page then hides.

use super::catalogue::Units;
use super::redact::Redactor;
use losos_page::logs::Line;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

/// The fields asked for; journalctl adds the cursor and the time itself.
const FIELDS: &str = "MESSAGE,PRIORITY,_SYSTEMD_UNIT,UNIT,SYSLOG_IDENTIFIER,_COMM,_PID,_TRANSPORT";

/// The most lines one search reads before it gives up on finding more.
pub const SCAN_MAX: usize = 200_000;

/// The most lines a page holds, and a download.
pub const PAGE_MAX: usize = 1000;
pub const DOWNLOAD_MAX: usize = 20_000;

/// journald's names for its priorities, as `journalctl -o short` has none
/// but a bug report wants one.
pub const LEVELS: [&str; 8] = [
    "emerg", "alert", "crit", "err", "warning", "notice", "info", "debug",
];

/// When.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Boot,
    Previous,
    Hour,
    Day,
    Week,
}

impl Span {
    #[must_use]
    pub fn parse(s: &str) -> Option<Span> {
        Some(match s {
            "boot" => Span::Boot,
            "previous" => Span::Previous,
            "hour" => Span::Hour,
            "day" => Span::Day,
            "week" => Span::Week,
            _ => return None,
        })
    }

    fn arg(self) -> &'static str {
        match self {
            Span::Boot => "--boot=0",
            Span::Previous => "--boot=-1",
            Span::Hour => "--since=-1h",
            Span::Day => "--since=-1d",
            Span::Week => "--since=-7d",
        }
    }
}

/// One search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub units: Units,
    /// At most this priority: 3 is errors and worse.
    pub priority: u8,
    pub span: Option<Span>,
    /// Lines whose redacted text holds this, ignoring case.
    pub text: String,
    /// Lines older than this cursor, newest first.
    pub before: Option<String>,
    /// Lines newer than this cursor, oldest first (following).
    pub after: Option<String>,
    /// Lines up to this time, in microseconds since 1970 (an excerpt).
    pub until_usec: Option<u64>,
    pub limit: usize,
}

/// One line of the journal, redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub cursor: String,
    pub usec: u64,
    pub unit: String,
    pub pid: Option<u32>,
    pub priority: u8,
    pub message: String,
}

/// What a search found, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub entries: Vec<Entry>,
    /// The cursor to ask for older lines with, when there are more.
    pub older: Option<String>,
    /// The cursor to follow on from.
    pub newer: Option<String>,
}

/// Whether `c` is a journal cursor (`s=…;i=…;b=…`), so it can go on
/// journalctl's command line.
#[must_use]
pub fn cursor(c: &str) -> bool {
    (1..=512).contains(&c.len())
        && c.split(';').all(|part| {
            let mut kv = part.splitn(2, '=');
            matches!(
                (kv.next(), kv.next()),
                (Some(k), Some(v)) if k.len() == 1
                    && k.bytes().all(|b| b.is_ascii_lowercase())
                    && !v.is_empty()
                    && v.bytes().all(|b| b.is_ascii_hexdigit())
            )
        })
}

/// journalctl's arguments for `q`.
#[must_use]
pub fn args(q: &Query) -> Vec<String> {
    let mut a = vec![
        "--no-pager".to_string(),
        "--quiet".to_string(),
        "--all".to_string(),
        "--output=json".to_string(),
        format!("--output-fields={FIELDS}"),
        format!("--priority={}", q.priority.min(7)),
    ];
    if let Some(span) = q.span {
        a.push(span.arg().to_string());
    }
    if let Some(until) = q.until_usec {
        a.push(format!("--until=@{}", until.div_ceil(1_000_000)));
    }
    match (&q.before, &q.after) {
        (_, Some(after)) => a.push(format!("--after-cursor={after}")),
        (Some(before), None) => {
            a.push("--reverse".to_string());
            a.push(format!("--after-cursor={before}"));
        }
        (None, None) => a.push("--reverse".to_string()),
    }
    match &q.units {
        Units::Named(units) => a.extend(units.iter().map(|u| format!("--unit={u}"))),
        Units::Kernel => a.push("_TRANSPORT=kernel".to_string()),
        Units::All => {}
    }
    a
}

/// One line of journalctl's output, redacted; `None` for one that is not
/// an entry.
#[must_use]
pub fn parse(line: &str, redactor: &Redactor) -> Option<Entry> {
    let v: Value = serde_json::from_str(line).ok()?;
    let text = |key: &str| v[key].as_str().filter(|s| !s.is_empty());
    let cursor = text("__CURSOR")?.to_string();
    let usec = text("__REALTIME_TIMESTAMP")?.parse().ok()?;
    let message = match &v["MESSAGE"] {
        Value::String(s) => s.clone(),
        // A message that is not UTF-8 comes as its bytes.
        Value::Array(bytes) => String::from_utf8_lossy(
            &bytes
                .iter()
                .filter_map(|b| b.as_u64().and_then(|b| u8::try_from(b).ok()))
                .collect::<Vec<u8>>(),
        )
        .into_owned(),
        _ => String::new(),
    };
    let kernel = text("_TRANSPORT") == Some("kernel");
    // What PID 1 says about a unit belongs to that unit.
    let unit = match (text("_SYSTEMD_UNIT"), text("UNIT")) {
        _ if kernel => "kernel",
        (Some("init.scope") | None, Some(unit)) => unit,
        (Some(unit), _) => unit,
        (None, None) => text("SYSLOG_IDENTIFIER")
            .or_else(|| text("_COMM"))
            .unwrap_or("?"),
    }
    .to_string();
    Some(Entry {
        cursor,
        usec,
        unit,
        pid: if kernel {
            None
        } else {
            text("_PID").and_then(|p| p.parse().ok())
        },
        priority: text("PRIORITY").and_then(|p| p.parse().ok()).unwrap_or(6),
        message: redactor.line(message.trim_end()),
    })
}

/// Runs the search.
///
/// # Errors
///
/// journalctl could not start, or stopped with an error before it found
/// anything: its first line of complaint.
pub fn read(journalctl: &Path, q: &Query, redactor: &Redactor) -> Result<Page, String> {
    if matches!(&q.units, Units::Named(u) if u.is_empty()) {
        return Ok(Page::default());
    }
    let mut child = Command::new(journalctl)
        .args(args(q))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("journalctl: {e}"))?;
    let stdout = child.stdout.take().ok_or("journalctl: no output")?;
    let needle = q.text.to_lowercase();
    let mut found = Vec::new();
    let mut scanned = 0usize;
    let mut cut = false;
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        scanned += 1;
        if let Some(e) = parse(&line, redactor) {
            if needle.is_empty() || e.message.to_lowercase().contains(&needle) {
                found.push(e);
                if found.len() > q.limit {
                    cut = true;
                    break;
                }
            }
        }
        if scanned >= SCAN_MAX {
            cut = true;
            break;
        }
    }
    if cut {
        let _ = child.kill();
    }
    let status = child.wait().map_err(|e| format!("journalctl: {e}"))?;
    let mut complaint = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut complaint);
    }
    if !cut && !status.success() && found.is_empty() {
        // An earlier boot this journal does not hold is nothing to show,
        // not a failure.
        if complaint.contains("boot") && q.span == Some(Span::Previous) {
            return Ok(Page::default());
        }
        let first = complaint
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("journalctl failed");
        return Err(redactor.line(first));
    }
    let more = found.len() > q.limit;
    found.truncate(q.limit);
    Ok(if let Some(after) = &q.after {
        Page {
            newer: Some(
                found
                    .last()
                    .map_or_else(|| after.clone(), |e| e.cursor.clone()),
            ),
            entries: found,
            older: None,
        }
    } else {
        found.reverse();
        Page {
            older: (more || scanned >= SCAN_MAX)
                .then(|| found.first().map(|e| e.cursor.clone()))
                .flatten(),
            newer: found.last().map(|e| e.cursor.clone()),
            entries: found,
        }
    })
}

/// An entry as the pane draws it, in the box's own time zone.
#[must_use]
pub fn line(e: &Entry) -> Line {
    let at = i64::try_from(e.usec)
        .ok()
        .and_then(chrono::DateTime::from_timestamp_micros)
        .unwrap_or_default()
        .with_timezone(&chrono::Local);
    Line {
        date: at.format("%Y-%m-%d").to_string(),
        time: at.format("%H:%M:%S").to_string(),
        stamp: at.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        unit: e.unit.clone(),
        pid: e.pid,
        priority: e.priority,
        message: e.message.clone(),
    }
}

/// An entry as one line of plain text, for a download or a problem report:
/// `2026-10-29 12:00:01 lososd.service[123] err: message`.
#[must_use]
pub fn text(e: &Entry) -> String {
    let l = line(e);
    let who = match e.pid {
        Some(pid) => format!("{}[{pid}]", e.unit),
        None => e.unit.clone(),
    };
    format!(
        "{} {} {who} {}: {}\n",
        l.date,
        l.time,
        LEVELS[usize::from(e.priority.min(7))],
        e.message.replace('\n', "\n    ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> Query {
        Query {
            units: Units::Named(vec!["lososd.service".into()]),
            priority: 6,
            span: Some(Span::Boot),
            text: String::new(),
            before: None,
            after: None,
            until_usec: None,
            limit: 200,
        }
    }

    #[test]
    fn newest_first_unless_following() {
        let a = args(&query());
        assert!(a.contains(&"--reverse".to_string()));
        assert!(a.contains(&"--priority=6".to_string()));
        assert!(a.contains(&"--boot=0".to_string()));
        assert!(a.contains(&"--unit=lososd.service".to_string()));
        assert!(!a.iter().any(|s| s.starts_with("--grep")));

        let mut q = query();
        q.before = Some("s=1;i=2".into());
        let a = args(&q);
        assert!(a.contains(&"--reverse".to_string()));
        assert!(a.contains(&"--after-cursor=s=1;i=2".to_string()));

        let mut q = query();
        q.after = Some("s=1;i=3".into());
        let a = args(&q);
        assert!(!a.contains(&"--reverse".to_string()));
        assert!(a.contains(&"--after-cursor=s=1;i=3".to_string()));

        let mut q = query();
        q.units = Units::Kernel;
        q.span = Some(Span::Week);
        q.until_usec = Some(1_000_000_001);
        let a = args(&q);
        assert!(a.contains(&"_TRANSPORT=kernel".to_string()));
        assert!(a.contains(&"--since=-7d".to_string()));
        assert!(a.contains(&"--until=@1001".to_string()));
    }

    #[test]
    fn cursors_are_checked() {
        assert!(cursor("s=0f;i=1a2;b=ff;m=3;t=4;x=5"));
        for bad in ["", "s=", "s=zz", "--unit=x", "s=1 i=2", "ss=1"] {
            assert!(!cursor(bad), "{bad}");
        }
    }

    #[test]
    fn entries_say_who_wrote_them() {
        let r = Redactor::default();
        let e = parse(
            r#"{"__CURSOR":"s=1;i=2","__REALTIME_TIMESTAMP":"1761739201000000","MESSAGE":"up","PRIORITY":"6","_SYSTEMD_UNIT":"lososd.service","_PID":"42"}"#,
            &r,
        )
        .unwrap();
        assert_eq!(
            (e.unit.as_str(), e.pid, e.priority, e.message.as_str()),
            ("lososd.service", Some(42), 6, "up")
        );

        let e = parse(
            r#"{"__CURSOR":"c","__REALTIME_TIMESTAMP":"1","MESSAGE":"Started x.","PRIORITY":"6","_SYSTEMD_UNIT":"init.scope","UNIT":"lososd.service","_PID":"1"}"#,
            &r,
        )
        .unwrap();
        assert_eq!(e.unit, "lososd.service");

        let e = parse(
            r#"{"__CURSOR":"c","__REALTIME_TIMESTAMP":"1","MESSAGE":[104,105,255],"PRIORITY":"3","_TRANSPORT":"kernel"}"#,
            &r,
        )
        .unwrap();
        assert_eq!((e.unit.as_str(), e.pid, e.priority), ("kernel", None, 3));
        assert_eq!(e.message, "hi\u{fffd}");

        let e = parse(
            r#"{"__CURSOR":"c","__REALTIME_TIMESTAMP":"1","MESSAGE":"password=x","SYSLOG_IDENTIFIER":"sudo"}"#,
            &r,
        )
        .unwrap();
        assert_eq!(
            (e.unit.as_str(), e.message.as_str()),
            ("sudo", "password=[hidden]")
        );

        assert!(parse("not json", &r).is_none());
        assert!(parse(r#"{"MESSAGE":"no cursor"}"#, &r).is_none());
    }

    #[test]
    fn a_line_of_text() {
        let e = Entry {
            cursor: "c".into(),
            usec: 0,
            unit: "lososd.service".into(),
            pid: Some(7),
            priority: 3,
            message: "one\ntwo".into(),
        };
        let t = text(&e);
        assert!(t.ends_with(" lososd.service[7] err: one\n    two\n"), "{t}");
    }

    /// A stand-in journalctl: prints what `$LOG_LINES` holds, newest first.
    fn fake(dir: &Path, lines: &[String]) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let data = dir.join("lines");
        std::fs::write(&data, lines.join("\n") + "\n").unwrap();
        let bin = dir.join("journalctl");
        std::fs::write(&bin, format!("#!/bin/sh\ncat {}\n", data.display())).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    }

    fn entry(i: u32, message: &str) -> String {
        format!(
            r#"{{"__CURSOR":"s=1;i={i:x}","__REALTIME_TIMESTAMP":"{}","MESSAGE":"{message}","PRIORITY":"6","_SYSTEMD_UNIT":"lososd.service"}}"#,
            1_761_739_200_000_000u64 + u64::from(i)
        )
    }

    #[test]
    fn a_page_is_oldest_first_with_the_way_back() {
        let dir = tempfile::tempdir().unwrap();
        let lines: Vec<String> = (0..5)
            .rev()
            .map(|i| entry(i, &format!("line {i}")))
            .collect();
        let bin = fake(dir.path(), &lines);
        let mut q = query();
        q.limit = 3;
        let page = read(&bin, &q, &Redactor::default()).unwrap();
        let got: Vec<&str> = page.entries.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(got, ["line 2", "line 3", "line 4"]);
        assert_eq!(page.older.as_deref(), Some("s=1;i=2"));
        assert_eq!(page.newer.as_deref(), Some("s=1;i=4"));

        q.limit = 10;
        let page = read(&bin, &q, &Redactor::default()).unwrap();
        assert_eq!(page.entries.len(), 5);
        assert_eq!(page.older, None);
    }

    #[test]
    fn the_text_filter_never_sees_a_secret() {
        let dir = tempfile::tempdir().unwrap();
        let lines = vec![entry(2, "password=hunter2"), entry(1, "hunter2 alone")];
        let bin = fake(dir.path(), &lines);
        let mut q = query();
        q.text = "HUNTER2".into();
        let page = read(&bin, &q, &Redactor::default()).unwrap();
        let got: Vec<&str> = page.entries.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(got, ["hunter2 alone"]);
    }

    #[test]
    fn following_keeps_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let bin = fake(dir.path(), &[]);
        let mut q = query();
        q.after = Some("s=1;i=9".into());
        let page = read(&bin, &q, &Redactor::default()).unwrap();
        assert!(page.entries.is_empty());
        assert_eq!(page.newer.as_deref(), Some("s=1;i=9"));
    }

    #[test]
    fn a_failure_says_why() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("journalctl");
        std::fs::write(
            &bin,
            "#!/bin/sh\necho 'No journal files were opened' >&2\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let err = read(&bin, &query(), &Redactor::default()).unwrap_err();
        assert_eq!(err, "No journal files were opened");
    }
}
