//! What the box reports: a service that failed, a program that crashed and
//! one the kernel killed because memory ran out.
//!
//! The `problems` plugin finds them in the journal (systemd's own messages
//! about a unit's processes and results, and the kernel's) and keeps each
//! by its fingerprint, so a service that fails every night is one problem
//! seen many times. The report carries what says why: the unit's state, its
//! journal around the time, and what the machine is.
//!
//! A crash leaves no core file on a box: modules/hardening.nix sends cores
//! nowhere, because one of lososd's would hold the admin token. The signal,
//! the unit and the lines before it are what the report has instead.

use crate::{
    details, fingerprint, known, lines_word, redact, short, Known, Report, System, BODY_LIMIT,
};

/// How many journal lines a report carries.
pub const JOURNAL_LINES: usize = 200;

/// What went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A unit ended in a failed result.
    Failed,
    /// A process ended on a signal that means a bug: SEGV, ABRT, BUS, ILL,
    /// FPE, SYS or TRAP.
    Crashed,
    /// The kernel killed a process because memory ran out.
    OutOfMemory,
}

impl Kind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Failed => "failed",
            Kind::Crashed => "crashed",
            Kind::OutOfMemory => "out-of-memory",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "failed" => Some(Kind::Failed),
            "crashed" => Some(Kind::Crashed),
            "out-of-memory" => Some(Kind::OutOfMemory),
            _ => None,
        }
    }
}

/// One problem, as the journal named it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub kind: Kind,
    /// `lososd.service`, when it happened in a unit.
    pub unit: Option<String>,
    /// The program's name as the kernel had it, `lososd`.
    pub process: Option<String>,
    /// The signal's name without `SIG`: `SEGV`.
    pub signal: Option<String>,
    /// systemd's result for a failed unit: `exit-code`, `timeout`, `signal`.
    pub result: Option<String>,
}

/// Signals that mean the program itself went wrong, by number on x86_64
/// and arm64, and by name.
pub const CRASH_SIGNALS: &[(u32, &str)] = &[
    (4, "ILL"),
    (5, "TRAP"),
    (6, "ABRT"),
    (7, "BUS"),
    (8, "FPE"),
    (11, "SEGV"),
    (31, "SYS"),
];

/// The crash signal's name for `status`, which systemd writes as a number
/// (`11`) or a name (`SEGV`, `SIGSEGV`).
#[must_use]
pub fn crash_signal(status: &str) -> Option<&'static str> {
    let status = status.trim();
    let name = status.strip_prefix("SIG").unwrap_or(status);
    CRASH_SIGNALS
        .iter()
        .find(|(n, s)| status.parse::<u32>() == Ok(*n) || name.eq_ignore_ascii_case(s))
        .map(|(_, s)| *s)
}

/// `unit` with its instance taken out, `losos-app@3.service` as
/// `losos-app@.service`, so every instance is the same problem. Numbers in
/// the rest are [`crate::normalise`]'s to take out.
#[must_use]
pub fn template(unit: &str) -> String {
    match (unit.split_once('@'), unit.rsplit_once('.')) {
        (Some((name, _)), Some((_, kind))) => format!("{name}@.{kind}"),
        _ => unit.to_owned(),
    }
}

impl Problem {
    /// What it happened in: the unit, else the program.
    #[must_use]
    pub fn subject(&self) -> &str {
        self.unit
            .as_deref()
            .or(self.process.as_deref())
            .unwrap_or("A program")
    }

    /// Eight hex digits that name this problem on every box.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let subject = match &self.unit {
            Some(unit) => template(unit),
            None => self.process.clone().unwrap_or_default(),
        };
        match self.kind {
            Kind::Failed => fingerprint(
                "failed",
                &format!("{subject} {}", self.result.as_deref().unwrap_or("failed")),
            ),
            Kind::Crashed => fingerprint(
                "crashed",
                &format!("{subject} {}", self.signal.as_deref().unwrap_or("")),
            ),
            Kind::OutOfMemory => fingerprint("out of memory", &subject),
        }
    }

    /// The issue's title, the fingerprint at its end.
    #[must_use]
    pub fn title(&self) -> String {
        let subject = short(self.subject(), 72);
        let what = match self.kind {
            Kind::Failed => format!("failed ({})", self.result.as_deref().unwrap_or("failed")),
            Kind::Crashed => format!("crashed (SIG{})", self.signal.as_deref().unwrap_or("?")),
            Kind::OutOfMemory => "was killed: out of memory".to_owned(),
        };
        format!("{subject} {what} [{}]", self.fingerprint())
    }

    /// One sentence that says what happened, for the report and the page.
    #[must_use]
    pub fn sentence(&self) -> String {
        let unit = self.unit.as_deref();
        let process = self.process.as_deref();
        match (self.kind, unit, process) {
            (Kind::Failed, Some(u), _) => format!(
                "The service **{u}** failed: `{}`.",
                self.result.as_deref().unwrap_or("failed")
            ),
            (Kind::Crashed, Some(u), _) => format!(
                "The main process of **{u}** crashed with SIG{}.",
                self.signal.as_deref().unwrap_or("?")
            ),
            (Kind::Crashed, None, Some(p)) => format!(
                "The program **{p}** crashed with SIG{}.",
                self.signal.as_deref().unwrap_or("?")
            ),
            (Kind::OutOfMemory, _, Some(p)) => {
                format!("The kernel killed **{p}** because the box ran out of memory.")
            }
            _ => format!("**{}** stopped.", self.subject()),
        }
    }
}

/// What was read about a problem when its report was written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Evidence {
    /// `systemctl show`'s lines that say how the unit is and how it ended.
    pub state: Vec<String>,
    /// The unit's journal around the problem, or the kernel's, oldest first.
    pub journal: Vec<String>,
    /// How often, and when first and last, in seconds since the epoch.
    pub count: u64,
    pub first: u64,
    pub last: u64,
}

/// The handbook page for this problem, when it is one the handbook
/// explains: only a failed unit can be, by its own log's last lines.
#[must_use]
pub fn documented(problem: &Problem, evidence: &Evidence) -> Option<Known> {
    if problem.kind != Kind::Failed {
        return None;
    }
    let tail = evidence.journal.len().saturating_sub(20);
    let mut text = evidence.journal[tail..].join("\n");
    text.push('\n');
    text.push_str(problem.result.as_deref().unwrap_or(""));
    known::documented(&text)
}

/// `secs` since the epoch as `2026-10-29 03:00 UTC`.
#[must_use]
pub fn utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rest / 3600,
        rest % 3600 / 60
    )
}

/// The report for `problem` on `system`, from `evidence`, with `private`
/// (the box's name, the people on it) taken out.
#[must_use]
pub fn build(
    problem: &Problem,
    system: &System,
    evidence: &Evidence,
    private: &[String],
) -> Report {
    let fingerprint = problem.fingerprint();
    // The box's name and its people come out of what the box said (the
    // sentence, the unit, the program and their logs), not out of this
    // text around it: a box named `box` keeps "a LosOS box". Addresses
    // and the like come out of everything, at the end.
    let private_out = |t: &str| redact(t, private);
    let lines = |ls: &[String]| ls.iter().map(|l| private_out(l)).collect::<Vec<_>>();
    let mut body = format!("{}\n\n", private_out(&problem.sentence()));
    if evidence.count > 1 {
        body.push_str(&format!(
            "It happened {} times, first at {} and last at {}.\n\n",
            evidence.count,
            utc(evidence.first),
            utc(evidence.last)
        ));
    } else if evidence.last > 0 {
        body.push_str(&format!("It happened at {}.\n\n", utc(evidence.last)));
    }
    let unit = problem.unit.as_deref().map(private_out);
    body.push_str(&system.table(&[
        ("Unit", unit.as_deref().unwrap_or("")),
        (
            "Program",
            &private_out(problem.process.as_deref().unwrap_or("")),
        ),
    ]));
    let mut remaining = BODY_LIMIT.saturating_sub(body.len() + 600);
    if !evidence.state.is_empty() {
        let block = details(
            &format!("State of {}", unit.as_deref().unwrap_or("the unit")),
            &lines(&evidence.state),
            remaining / 4,
        );
        remaining = remaining.saturating_sub(block.len());
        body.push_str(&block);
    }
    let from = unit.clone().unwrap_or_else(|| "the kernel".to_owned());
    body.push_str(&details(
        &format!(
            "Journal of {from} around it ({})",
            lines_word(evidence.journal.len())
        ),
        &lines(&evidence.journal),
        remaining,
    ));
    body.push_str(&format!(
        "\n<sub>Sent from a LosOS box. Fingerprint {fingerprint}: reports of the same problem \
         are added here as comments.</sub>\n"
    ));
    Report {
        title: redact(&problem.title(), private),
        body: redact(&body, &[]),
        fingerprint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crash() -> Problem {
        Problem {
            kind: Kind::Crashed,
            unit: Some("losos-cloud.service".into()),
            process: None,
            signal: Some("SEGV".into()),
            result: None,
        }
    }

    #[test]
    fn instances_and_numbers_are_one_problem() {
        let a = Problem {
            kind: Kind::Failed,
            unit: Some("losos-rebuild-20261029030000-812-1.service".into()),
            process: None,
            signal: None,
            result: Some("exit-code".into()),
        };
        let b = Problem {
            unit: Some("losos-rebuild-20261030030000-77-1.service".into()),
            ..a.clone()
        };
        assert_eq!(a.fingerprint(), b.fingerprint());
        let c = Problem {
            unit: Some("losos-podman-app@immich.service".into()),
            ..a.clone()
        };
        let d = Problem {
            unit: Some("losos-podman-app@jellyfin.service".into()),
            ..a.clone()
        };
        assert_eq!(c.fingerprint(), d.fingerprint());
        assert_ne!(a.fingerprint(), c.fingerprint());
        assert_eq!(crash_signal("11"), Some("SEGV"));
        assert_eq!(crash_signal("SIGABRT"), Some("ABRT"));
        assert_eq!(crash_signal("TERM"), None);
        assert_eq!(crash_signal("15"), None);
    }

    #[test]
    fn a_report_says_what_and_when_and_fits_on_github() {
        let system = System {
            release: "v0.6.1, x86_64".into(),
            ..System::default()
        };
        let evidence = Evidence {
            state: vec!["ActiveState=failed".into(), "Result=core-dump".into()],
            journal: (0..400)
                .map(|i| format!("line {i} kitchen.local {}", "x".repeat(300)))
                .collect(),
            count: 3,
            first: 1_761_706_800,
            last: 1_761_710_400,
        };
        let report = build(&crash(), &system, &evidence, &["kitchen".into()]);
        assert!(report
            .title
            .starts_with("losos-cloud.service crashed (SIGSEGV) ["));
        assert!(report.title.ends_with(&format!("[{}]", report.fingerprint)));
        assert!(report
            .body
            .contains("The main process of **losos-cloud.service** crashed with SIGSEGV."));
        assert!(report.body.contains(
            "It happened 3 times, first at 2025-10-29 03:00 UTC and last at 2025-10-29 04:00 UTC."
        ));
        assert!(report.body.contains("| Unit | losos-cloud.service |"));
        assert!(report.body.contains("Result=core-dump"));
        assert!(report.body.contains("earlier lines left out"));
        assert!(report.body.contains("[name].local"));
        assert!(!report.body.contains("kitchen"));
        assert!(report.body.len() < 65_536);
    }

    #[test]
    fn a_box_named_box_keeps_the_words_around_its_log() {
        let evidence = Evidence {
            state: vec!["ActiveState=failed".into()],
            journal: vec!["eva-sync on box: matus asked 192.168.1.20".into()],
            count: 1,
            first: 1_761_706_800,
            last: 1_761_706_800,
        };
        let names = ["box".to_owned(), "matus".to_owned()];
        let report = build(&crash(), &System::default(), &evidence, &names);
        assert!(report
            .body
            .contains("eva-sync on [name]: [name] asked [IP]"));
        assert!(report.body.contains("Sent from a LosOS box."));
        assert!(!report.body.contains("matus") && !report.body.contains("on box"));
    }

    #[test]
    fn only_a_failed_unit_can_be_documented() {
        let evidence = Evidence {
            journal: vec!["error: writing to file: No space left on device".into()],
            ..Evidence::default()
        };
        let failed = Problem {
            kind: Kind::Failed,
            result: Some("exit-code".into()),
            ..crash()
        };
        assert_eq!(
            documented(&failed, &evidence).map(|k| k.id),
            Some("out-of-room".to_owned())
        );
        assert_eq!(documented(&crash(), &evidence), None);
    }

    #[test]
    fn times_read_as_utc() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(1_793_246_400), "2026-10-29 04:00 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00 UTC");
    }
}
