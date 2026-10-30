//! What the installer reports: an install that stopped, and memory or
//! processor use that spiked while it ran.
//!
//! The installer medium runs from memory: the whole live system, the
//! downloads and the build sit in RAM on a box that may have 4 GiB. A spike
//! that comes close to the edge is worth hearing about even when the
//! install finished, because the next box, a little smaller, will not.

use crate::{details, fingerprint, known, redact, Known, Report, System, BODY_LIMIT};

/// What the person adds is quoted under this, before the report.
pub const NOTE_HEADING: &str = "From the person installing:";

/// What an install that failed left to report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Failure {
    /// The step's title, as the screen showed it.
    pub step: String,
    /// Which step it was, from 1, and how many there are.
    pub position: (usize, usize),
    pub message: String,
    /// The tools' output, oldest first.
    pub log: Vec<String>,
    /// The disks it was installing onto.
    pub disks: String,
}

/// Memory or processor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    Memory,
    Cpu,
}

impl Resource {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Resource::Memory => "memory",
            Resource::Cpu => "processor",
        }
    }
}

/// One process at the spike's peak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub name: String,
    /// Resident memory, in MiB.
    pub rss_mib: u64,
    /// Share of one core over the last second, in percent.
    pub cpu_percent: u64,
}

/// A stretch of time when use stayed at or over the threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spike {
    pub resource: Resource,
    /// The highest use seen, in percent of the machine's.
    pub peak: u64,
    /// The threshold it crossed, in percent.
    pub threshold: u64,
    /// How long it stayed over, in seconds.
    pub seconds: u64,
    /// The install's step at the peak.
    pub step: String,
    /// The biggest processes at the peak, the biggest first.
    pub top: Vec<Process>,
    /// Use once a second while it lasted, in percent, at most a few hundred.
    pub samples: Vec<u64>,
}

impl Spike {
    /// One line for the screen: "Memory reached 96% while Installing the system."
    #[must_use]
    pub fn line(&self) -> String {
        let what = match self.resource {
            Resource::Memory => "Memory use",
            Resource::Cpu => "Processor use",
        };
        format!(
            "{what} reached {}% for {} s {}.",
            self.peak,
            self.seconds,
            self.during()
        )
    }

    /// "while installing the system", or "before the first step" for a
    /// spike that started before the install named one.
    #[must_use]
    pub fn during(&self) -> String {
        if self.step.is_empty() {
            "before the first step".to_owned()
        } else {
            format!("while {}", lower_first(&self.step))
        }
    }

    /// Same resource, same step: the same spike on every machine.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        fingerprint(&format!("spike {}", self.resource.word()), &self.step)
    }
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The handbook page for a failed install, when it explains this one.
#[must_use]
pub fn documented(failure: &Failure) -> Option<Known> {
    let tail = failure.log.len().saturating_sub(20);
    let mut text = failure.message.clone();
    text.push('\n');
    text.push_str(&failure.log[tail..].join("\n"));
    known::documented(&text)
}

fn spikes_section(spikes: &[Spike], budget: usize) -> String {
    let mut out = String::new();
    for spike in spikes {
        out.push_str(&format!("\n- {}", spike.line()));
    }
    if !spikes.is_empty() {
        out.push('\n');
    }
    for spike in spikes {
        let mut lines = vec![format!(
            "{} over {}% for {} s, peak {}%, during: {}",
            spike.resource.word(),
            spike.threshold,
            spike.seconds,
            spike.peak,
            spike.step
        )];
        lines.push(String::new());
        lines.push(format!(
            "{:<24} {:>8} {:>6}",
            "process at the peak", "MiB", "cpu %"
        ));
        for p in &spike.top {
            lines.push(format!(
                "{:<24} {:>8} {:>6}",
                p.name, p.rss_mib, p.cpu_percent
            ));
        }
        lines.push(String::new());
        lines.push("use each second, in percent:".to_owned());
        for chunk in spike.samples.chunks(20) {
            lines.push(
                chunk
                    .iter()
                    .map(|s| format!("{s:>3}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        out.push_str(&details(
            &format!("{} spike {}", spike.resource.word(), spike.during()),
            &lines,
            budget / spikes.len().max(1),
        ));
    }
    out
}

/// The report for a failed install on `system`: the step, the error, the
/// tools' output, the boot's warnings in `journal`, and any `spikes`.
#[must_use]
pub fn failure_report(
    failure: &Failure,
    system: &System,
    journal: &[String],
    spikes: &[Spike],
) -> Report {
    let fingerprint = fingerprint(&failure.step, &failure.message);
    let short = crate::short(&failure.message, 72);
    let title = format!("Installer: {short} [{fingerprint}]");
    let mut body = if failure.position.1 > 0 {
        format!(
            "The install stopped at **{}** (step {} of {}):\n\n",
            failure.step, failure.position.0, failure.position.1
        )
    } else {
        "The install stopped before its first step:\n\n".to_owned()
    };
    body.push_str(
        &failure
            .message
            .lines()
            .map(|l| format!("> {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    body.push_str("\n\n");
    body.push_str(&system.table(&[("Disks", &failure.disks)]));
    let remaining = BODY_LIMIT.saturating_sub(body.len() + 600);
    let spikes_block = spikes_section(spikes, remaining / 4);
    let remaining = remaining.saturating_sub(spikes_block.len());
    if !spikes.is_empty() {
        body.push_str("\nWhile it ran:\n");
        body.push_str(&spikes_block);
    }
    let log = details(
        &format!("Install output ({})", crate::lines_word(failure.log.len())),
        &failure.log,
        remaining * 2 / 3,
    );
    let remaining = remaining.saturating_sub(log.len());
    body.push_str(&log);
    body.push_str(&details(
        "Journal warnings and errors from this boot",
        journal,
        remaining,
    ));
    body.push_str(&format!(
        "\n<sub>Sent by the LosOS installer. Fingerprint {fingerprint}: reports of the same \
         failure are added here as comments.</sub>\n"
    ));
    Report {
        title: redact(&title, &[]),
        body: redact(&body, &[]),
        fingerprint,
    }
}

/// The report for spikes in an install that finished, or the one spike a
/// person chose; the first spike names it.
#[must_use]
pub fn spike_report(spikes: &[Spike], system: &System, finished: bool) -> Option<Report> {
    let first = spikes.first()?;
    let fingerprint = first.fingerprint();
    let title = format!(
        "Installer: {} use reached {}% {} [{fingerprint}]",
        first.resource.word(),
        first.peak,
        first.during()
    );
    let mut body = format!(
        "Memory or processor use went over its line while the installer ran. The install {}.\n",
        if finished {
            "finished"
        } else {
            "did not finish"
        }
    );
    body.push_str(&spikes_section(spikes, BODY_LIMIT / 2));
    body.push('\n');
    body.push_str(&system.table(&[]));
    body.push_str(&format!(
        "\n<sub>Sent by the LosOS installer. Fingerprint {fingerprint}: reports of the same \
         spike are added here as comments.</sub>\n"
    ));
    Some(Report {
        title: redact(&title, &[]),
        body: redact(&body, &[]),
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spike() -> Spike {
        Spike {
            resource: Resource::Memory,
            peak: 96,
            threshold: 90,
            seconds: 14,
            step: "Installing the system (substituting from cache)".into(),
            top: vec![Process {
                name: "nix".into(),
                rss_mib: 2210,
                cpu_percent: 180,
            }],
            samples: (90..97).collect(),
        }
    }

    #[test]
    fn a_failure_report_says_where_it_stopped_and_fits_on_github() {
        let failure = Failure {
            step: "Erasing and encrypting the disks".into(),
            position: (2, 5),
            message: "disko failed (exit status: 1)".into(),
            log: (0..300)
                .map(|i| format!("line {i} {}", "x".repeat(400)))
                .collect(),
            disks: "/dev/vda".into(),
        };
        let system = System {
            release: "v0.6.1, x86_64".into(),
            firmware: "UEFI".into(),
            ..System::default()
        };
        let journal: Vec<String> = (0..200).map(|i| format!("journal {i}")).collect();
        let report = failure_report(&failure, &system, &journal, &[spike()]);
        assert!(report
            .title
            .starts_with("Installer: disko failed (exit status: 1) ["));
        assert!(report
            .body
            .contains("**Erasing and encrypting the disks** (step 2 of 5)"));
        assert!(report.body.contains("| Disks | /dev/vda |"));
        assert!(report
            .body
            .contains("Memory use reached 96% for 14 s while installing the system"));
        assert!(report.body.contains("line 299 "));
        assert!(report.body.contains("earlier lines left out"));
        assert!(report.body.len() < 65_536);
        assert_eq!(documented(&failure), None);
    }

    #[test]
    fn a_spike_report_names_its_first_spike_and_numbers_do_not_split_it() {
        let report = spike_report(&[spike()], &System::default(), true).unwrap();
        assert!(report
            .title
            .starts_with("Installer: memory use reached 96% while installing the system"));
        assert!(report.body.contains("The install finished."));
        assert!(report.body.contains("nix"));
        let other = Spike {
            peak: 99,
            seconds: 3,
            ..spike()
        };
        assert_eq!(other.fingerprint(), spike().fingerprint());
        let cpu = Spike {
            resource: Resource::Cpu,
            ..spike()
        };
        assert_ne!(cpu.fingerprint(), spike().fingerprint());
        assert_eq!(spike_report(&[], &System::default(), true), None);
        let early = Spike {
            step: String::new(),
            ..spike()
        };
        assert_eq!(
            early.line(),
            "Memory use reached 96% for 14 s before the first step."
        );
        let report = spike_report(&[early], &System::default(), true).unwrap();
        assert!(report
            .title
            .starts_with("Installer: memory use reached 96% before the first step"));
    }

    #[test]
    fn no_network_is_documented() {
        let failure = Failure {
            message: "no network: github.com did not resolve within 600s".into(),
            ..Failure::default()
        };
        assert_eq!(
            documented(&failure).map(|k| k.page),
            Some("start/install".to_owned())
        );
    }
}
