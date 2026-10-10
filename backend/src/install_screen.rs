//! What the installer medium's boot screen shows while `losos-ctl install`
//! runs under it.
//!
//! The medium boots under the same Plymouth theme as an installed box
//! (modules/splash.nix, modules/splash/losos.script): the salmon, and a panel
//! of rows under it. The theme takes the rows as one status message,
//! `losos:` followed by the rows joined with `|`, each led by a style
//! character. This module builds those rows, so what each screen says is
//! tested here and the IO side (installer_io.rs) only sends them.
//!
//! The screens: the firmware menu with its countdown, the install's steps
//! with the current one marked, the wait for a network, and how it ended.

use crate::installer::InstallAction;

/// One row of the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// The panel's headline, in the accent colour.
    Head(String),
    /// Something to read off the screen: bold.
    Strong(String),
    /// A warning headline, in yellow.
    Warn(String),
    /// Secondary text.
    Muted(String),
    /// Plain text.
    Plain(String),
    /// The step running now, which the theme pulses.
    Current(String),
    /// An empty row.
    Gap,
}

impl Row {
    fn encode(&self) -> String {
        let (style, text) = match self {
            Row::Head(t) => ('#', t),
            Row::Strong(t) => ('@', t),
            Row::Warn(t) => ('!', t),
            Row::Muted(t) => ('~', t),
            Row::Plain(t) => (' ', t),
            Row::Current(t) => ('>', t),
            Row::Gap => return String::new(),
        };
        // `|` separates rows, so none may appear inside one.
        format!("{style}{}", text.replace('|', "/"))
    }
}

/// The longest part of a status the installer sends in one Plymouth
/// message. The protocol carries at most 254 bytes after its own framing;
/// this leaves room for the `losos+:` prefix.
pub const PART_BYTES: usize = 200;

/// The rows as the Plymouth messages that carry them: `losos+:` for each
/// part but the last, `losos:` for the last, which the theme draws.
pub fn messages(rows: &[Row]) -> Vec<String> {
    let text = rows.iter().map(Row::encode).collect::<Vec<_>>().join("|");
    let mut parts = Vec::new();
    let mut rest = text.as_str();
    while rest.len() > PART_BYTES {
        let mut cut = PART_BYTES;
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        parts.push(format!("losos+:{}", &rest[..cut]));
        rest = &rest[cut..];
    }
    parts.push(format!("losos:{rest}"));
    parts
}

/// Break `text` into lines of at most `width` characters at spaces, so a
/// long error fits the panel. A word longer than a line gets a line of its
/// own.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Characters per row of wrapped text. The panel grows to fit its widest
/// row, up to the screen's width; this keeps it well inside that at the
/// theme's type size.
const WRAP: usize = 64;

/// The firmware menu, with the seconds left before it picks autodetect.
pub fn firmware_menu(secure_boot: &str, seconds_left: u64) -> Vec<Row> {
    vec![
        Row::Head("Choose the firmware mode for the installed system".into()),
        Row::Muted(secure_boot.into()),
        Row::Gap,
        Row::Strong("1   BIOS".into()),
        Row::Strong("2   UEFI".into()),
        Row::Strong("3   Autodetect".into()),
        Row::Gap,
        Row::Muted(format!(
            "Press 1, 2 or 3. Autodetect starts in {seconds_left} s and uses the"
        )),
        Row::Muted("firmware that started this stick, which is the right choice".into()),
        Row::Muted("on almost every machine.".into()),
    ]
}

/// One step of the install as the screen names it: what it is while it
/// waits, and what it says while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub todo: &'static str,
    pub doing: &'static str,
}

const DOWNLOAD: Step = Step {
    todo: "Download LosOS",
    doing: "Downloading LosOS",
};
const DISKS: Step = Step {
    todo: "Erase and encrypt the disks",
    doing: "Erasing and encrypting the disks",
};
const SEAL: Step = Step {
    todo: "Seal the disk key to the TPM chip",
    doing: "Sealing the disk key to the TPM chip",
};
const SYSTEM: Step = Step {
    todo: "Install the system",
    doing: "Installing the system, the longest step",
};
const FINISH: Step = Step {
    todo: "Finish",
    doing: "Finishing",
};

/// The step an action opens, for the actions that open one.
fn opens(action: &InstallAction) -> Option<Step> {
    match action {
        InstallAction::CloneFlake(..) => Some(DOWNLOAD),
        InstallAction::RunDisko(_) | InstallAction::RunDiskoScript(_) => Some(DISKS),
        InstallAction::EnrollTpm(..) => Some(SEAL),
        InstallAction::RunNixosInstall(_) => Some(SYSTEM),
        InstallAction::LayFlake(..) => Some(FINISH),
        _ => None,
    }
}

/// The plan's steps, and for each action the step it belongs to.
///
/// The small actions between two steps (writing the target file, the
/// keyfile, the bind mount) belong to the step that follows them, since
/// they prepare it; the ones after the last step belong to the last step.
pub fn steps(acts: &[InstallAction]) -> (Vec<Step>, Vec<usize>) {
    let list: Vec<Step> = acts.iter().filter_map(opens).collect();
    let mut owner = vec![0; acts.len()];
    let mut next = list.len();
    for (i, a) in acts.iter().enumerate().rev() {
        if opens(a).is_some() {
            next -= 1;
            owner[i] = next;
        } else {
            owner[i] = next.min(list.len().saturating_sub(1));
        }
    }
    (list, owner)
}

/// What the screen knows about the install while it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub steps: Vec<Step>,
    pub current: usize,
    pub drives: Vec<String>,
    pub tpm: bool,
    /// Seconds spent waiting for the network so far, while waiting.
    pub network_wait: Option<u64>,
}

/// The install's steps, the current one marked.
pub fn progress(p: &Progress) -> Vec<Row> {
    let mut rows = vec![
        Row::Head("Installing LosOS".into()),
        Row::Muted(format!("Onto {}", p.drives.join(", "))),
        Row::Gap,
    ];
    for (i, step) in p.steps.iter().enumerate() {
        rows.push(if i < p.current {
            Row::Plain(format!("✓  {}", step.todo))
        } else if i == p.current {
            Row::Current(step.doing.into())
        } else {
            Row::Muted(step.todo.into())
        });
    }
    rows.push(Row::Gap);
    if let Some(secs) = p.network_wait {
        rows.push(Row::Warn(format!("No network yet ({secs} s)")));
        rows.push(Row::Muted(
            "Plug in an Ethernet cable; the install goes on by itself.".into(),
        ));
        rows.push(Row::Muted(
            "For Wi-Fi, press Alt+F2 and run nmtui there.".into(),
        ));
    } else {
        rows.push(Row::Muted(
            "Leave the machine on. The install needs no answers from here.".into(),
        ));
    }
    if !p.tpm {
        rows.push(Row::Gap);
        rows.push(Row::Warn("This machine has no TPM chip.".into()));
        rows.push(Row::Muted(
            "The disk key goes on the unencrypted boot partition.".into(),
        ));
    }
    rows
}

/// How the install ended when it worked. `installed` is false for the
/// runs that stop after the disks (`--disko-script`, `--no-install`).
pub fn finished(installed: bool, tpm: bool) -> Vec<Row> {
    let mut rows = if installed {
        vec![
            Row::Head("LosOS is installed".into()),
            Row::Gap,
            Row::Strong("Remove the stick, then press any key to restart.".into()),
            Row::Muted("The box shows its address on this screen once it is up.".into()),
        ]
    } else {
        vec![
            Row::Head("The disks are ready".into()),
            Row::Gap,
            Row::Muted("This run stopped before installing the system.".into()),
        ]
    };
    if !tpm {
        rows.extend([
            Row::Gap,
            Row::Warn("This box has no TPM chip.".into()),
            Row::Muted("Its disk key is on the unencrypted boot partition,".into()),
            Row::Muted("so anyone who takes the disk can read your files.".into()),
        ]);
    }
    rows
}

/// An error as the panel shows it: a capital first letter and a full stop.
/// The installer's errors are written for its log, lower case and bare.
fn sentence(error: &str) -> String {
    let error = error.trim();
    let mut chars = error.chars();
    let mut out: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return String::new(),
    };
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// How the install ended when it failed.
pub fn failed(error: &str) -> Vec<Row> {
    let mut rows = vec![Row::Warn("The install stopped".into()), Row::Gap];
    rows.extend(wrap(&sentence(error), WRAP).into_iter().map(Row::Plain));
    rows.extend([
        Row::Gap,
        Row::Muted("Press any key for the full log and a shell.".into()),
    ]);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rows_encode_with_their_style_and_no_stray_separator() {
        let msgs = messages(&[
            Row::Head("A".into()),
            Row::Gap,
            Row::Current("B|C".into()),
            Row::Plain("D".into()),
        ]);
        assert_eq!(msgs, vec!["losos:#A||>B/C| D".to_string()]);
    }

    #[test]
    fn a_long_status_goes_in_parts_cut_on_character_boundaries() {
        let rows: Vec<Row> = (0..40).map(|_| Row::Plain("✓  step".into())).collect();
        let msgs = messages(&rows);
        assert!(msgs.len() > 1);
        let (last, parts) = msgs.split_last().unwrap();
        assert!(last.starts_with("losos:"));
        let mut joined = String::new();
        for p in parts {
            let body = p.strip_prefix("losos+:").expect("a part");
            assert!(body.len() <= PART_BYTES);
            joined.push_str(body);
        }
        joined.push_str(last.strip_prefix("losos:").unwrap());
        assert_eq!(
            joined,
            rows.iter().map(Row::encode).collect::<Vec<_>>().join("|")
        );
    }

    #[test]
    fn wrap_breaks_at_spaces_and_keeps_long_words_whole() {
        assert_eq!(wrap("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrap("a verylongword b", 4), vec!["a", "verylongword", "b"]);
        assert!(wrap("", 10).is_empty());
    }

    fn full_plan() -> Vec<InstallAction> {
        let p = PathBuf::from;
        vec![
            InstallAction::Log("drives".into()),
            InstallAction::EnsureKeyfile(p("/k")),
            InstallAction::CloneFlake("u".into(), p("/w")),
            InstallAction::WriteTarget(p("/w/t"), String::new()),
            InstallAction::RunDisko(p("/w")),
            InstallAction::Log("sealing".into()),
            InstallAction::EnrollTpm(p("/dev/x"), p("/k")),
            InstallAction::BindMount(p("/a"), p("/b")),
            InstallAction::CopyKeyfile(p("/k"), p("/c")),
            InstallAction::RunNixosInstall(p("/w")),
            InstallAction::LayFlake(p("/w"), p("/d")),
            InstallAction::Log("done".into()),
        ]
    }

    #[test]
    fn every_action_belongs_to_the_step_it_prepares() {
        let (list, owner) = steps(&full_plan());
        assert_eq!(list, vec![DOWNLOAD, DISKS, SEAL, SYSTEM, FINISH]);
        assert_eq!(owner, vec![0, 0, 0, 1, 1, 2, 2, 3, 3, 3, 4, 4]);
    }

    #[test]
    fn the_test_mode_plan_has_the_disks_and_the_seal() {
        let p = PathBuf::from;
        let (list, owner) = steps(&[
            InstallAction::Log("drives".into()),
            InstallAction::EnsureKeyfile(p("/k")),
            InstallAction::WriteTarget(p("/t"), String::new()),
            InstallAction::RunDiskoScript(p("/s")),
            InstallAction::Log("done".into()),
        ]);
        assert_eq!(list, vec![DISKS]);
        assert_eq!(owner, vec![0; 5]);
    }

    #[test]
    fn progress_marks_done_current_and_waiting_steps() {
        let (list, _) = steps(&full_plan());
        let rows = progress(&Progress {
            steps: list,
            current: 1,
            drives: vec!["/dev/vdb".into(), "/dev/vdc".into()],
            tpm: true,
            network_wait: None,
        });
        assert_eq!(rows[1], Row::Muted("Onto /dev/vdb, /dev/vdc".into()));
        assert_eq!(rows[3], Row::Plain("✓  Download LosOS".into()));
        assert_eq!(
            rows[4],
            Row::Current("Erasing and encrypting the disks".into())
        );
        assert_eq!(
            rows[5],
            Row::Muted("Seal the disk key to the TPM chip".into())
        );
        assert!(!rows.iter().any(|r| matches!(r, Row::Warn(_))));
    }

    #[test]
    fn progress_says_when_it_waits_for_a_network_and_when_there_is_no_tpm() {
        let rows = progress(&Progress {
            steps: vec![DOWNLOAD],
            current: 0,
            drives: vec!["/dev/sda".into()],
            tpm: false,
            network_wait: Some(12),
        });
        assert!(rows.contains(&Row::Warn("No network yet (12 s)".into())));
        assert!(rows.contains(&Row::Warn("This machine has no TPM chip.".into())));
    }

    #[test]
    fn the_endings_say_what_to_do_next() {
        assert!(finished(true, true).contains(&Row::Strong(
            "Remove the stick, then press any key to restart.".into()
        )));
        assert_eq!(
            finished(false, true)[0],
            Row::Head("The disks are ready".into())
        );
        assert!(finished(true, false).contains(&Row::Warn("This box has no TPM chip.".into())));
        let f = failed("disko failed (exit status: 1); see the output above");
        assert_eq!(f[0], Row::Warn("The install stopped".into()));
        assert!(f.contains(&Row::Plain(
            "Disko failed (exit status: 1); see the output above.".into()
        )));
    }

    #[test]
    fn the_menu_counts_down() {
        let rows = firmware_menu("Secure Boot: enabled.", 27);
        assert!(rows
            .iter()
            .any(|r| matches!(r, Row::Muted(t) if t.contains("in 27 s"))));
    }
}
