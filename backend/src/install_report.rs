//! Problem reports from the installer medium: an install that stopped, and
//! memory or processor use that spiked while it ran.
//!
//! What a report says and how it is sent are `losos-report`'s, the crate
//! the box's problems plugin uses too (`backend/plugins/report`). This is
//! the medium's side: the step the install is on, the tools' output kept
//! in `/run`, a watcher that samples `/proc` once a second, and GitHub
//! reached with curl. Nothing leaves the machine until the person installing
//! has read the report on screen 7 and pressed a key to send it.

use losos_report::github::{self, Call, Progress, Transport};
use losos_report::install::{self, Failure, Process, Resource, Spike};
use losos_report::{Known, Report, System, SystemFiles, Target};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What a failed command's message ends with on the panel.
pub const TTY2_HINT: &str = "see tty2 for the live command log";

/// The tools' output, kept for a report (`LOSOS_INSTALL_LOG`).
pub const DEFAULT_LOG: &str = "/run/losos-install/output.log";
/// The report written for the person to read (`LOSOS_INSTALL_REPORT`).
pub const DEFAULT_REPORT: &str = "/run/losos-install/report.md";
/// How many lines of output a report carries.
pub const LOG_LINES: usize = 300;
/// The public handbook, for a failure it explains.
pub const HANDBOOK: &str = "https://losos.dasmat.us/";

/// Where the install is: the step's title, which it is from 1, and how
/// many there are. The watcher and a failure read it.
static STEP: Mutex<(String, usize, usize)> = Mutex::new((String::new(), 0, 0));

/// The install moved on to step `n` of `total`.
pub fn note_step(title: &str, n: usize, total: usize) {
    if let Ok(mut s) = STEP.lock() {
        *s = (title.to_owned(), n, total);
    }
}

fn current_step() -> (String, usize, usize) {
    STEP.lock().map(|s| s.clone()).unwrap_or_default()
}

fn env_path(name: &str, default: &str) -> PathBuf {
    std::env::var_os(name).map_or_else(|| PathBuf::from(default), PathBuf::from)
}

/// The file the tools' output goes to, its folder made owner-only.
#[must_use]
pub fn log_path() -> PathBuf {
    let path = env_path("LOSOS_INSTALL_LOG", DEFAULT_LOG);
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_ok() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
    }
    path
}

/// One line of the installer's own into the kept output.
pub fn keep_line(line: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(f, "{line}");
    }
}

/// A terminal's output as lines a person reads: colours and cursor moves
/// out, and of a line a progress bar redrew, the last drawing.
#[must_use]
pub fn clean_output(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in raw.split('\n') {
        let mut text = String::with_capacity(line.len());
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\u{1b}' => {
                    // CSI: ESC [ … final byte in @..~; anything else, one more.
                    if chars.peek() == Some(&'[') {
                        chars.next();
                        for f in chars.by_ref() {
                            if ('@'..='~').contains(&f) {
                                break;
                            }
                        }
                    } else {
                        chars.next();
                    }
                }
                '\r' => {
                    if chars.peek().is_some() {
                        text.clear();
                    }
                }
                c if c.is_control() && c != '\t' => {}
                c => text.push(c),
            }
        }
        let text = text.trim_end();
        if text.starts_with("Script started on ") || text.starts_with("Script done on ") {
            continue;
        }
        out.push(text.to_owned());
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    out
}

/// The last [`LOG_LINES`] lines of the kept output.
#[must_use]
pub fn read_log() -> Vec<String> {
    let raw = std::fs::read(log_path()).unwrap_or_default();
    let lines = clean_output(&String::from_utf8_lossy(&raw));
    let tail = lines.len().saturating_sub(LOG_LINES);
    lines[tail..].to_vec()
}

// ── Spikes ──────────────────────────────────────────────────────────────

/// When use counts as a spike: at or over `percent` for `secs` seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Threshold {
    pub percent: u64,
    pub secs: u64,
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Memory at 90 % for 3 s, the processor at 95 % for a minute, unless
/// `LOSOS_SPIKE_{MEMORY,CPU}_{PERCENT,SECS}` say otherwise. The medium
/// runs from memory, so a few seconds near the top is already close to a
/// kill; a busy processor is normal while Nix unpacks, so only a long
/// stretch counts.
#[must_use]
pub fn thresholds() -> (Threshold, Threshold) {
    (
        Threshold {
            percent: env_u64("LOSOS_SPIKE_MEMORY_PERCENT", 90),
            secs: env_u64("LOSOS_SPIKE_MEMORY_SECS", 3),
        },
        Threshold {
            percent: env_u64("LOSOS_SPIKE_CPU_PERCENT", 95),
            secs: env_u64("LOSOS_SPIKE_CPU_SECS", 60),
        },
    )
}

/// How many samples a spike keeps: ten minutes, once a second.
const MAX_SAMPLES: usize = 600;

/// One resource's use, a sample a second, cut into spikes.
#[derive(Debug, Clone)]
pub struct Tracker {
    resource: Resource,
    threshold: Threshold,
    open: Option<Spike>,
    done: Vec<Spike>,
}

impl Tracker {
    #[must_use]
    pub fn new(resource: Resource, threshold: Threshold) -> Tracker {
        Tracker {
            resource,
            threshold,
            open: None,
            done: Vec::new(),
        }
    }

    /// Whether the next sample at `percent` would be the peak so far, so
    /// the caller reads the processes for it.
    #[must_use]
    pub fn wants_top(&self, percent: u64) -> bool {
        percent >= self.threshold.percent && self.open.as_ref().is_none_or(|s| percent > s.peak)
    }

    /// One second's use, the step it was in, and the processes at that
    /// second when [`Tracker::wants_top`] asked for them.
    pub fn sample(&mut self, percent: u64, step: &str, top: Option<Vec<Process>>) {
        if percent < self.threshold.percent {
            self.close();
            return;
        }
        let (resource, threshold) = (self.resource, self.threshold.percent);
        let spike = self.open.get_or_insert_with(|| Spike {
            resource,
            peak: 0,
            threshold,
            seconds: 0,
            step: step.to_owned(),
            top: Vec::new(),
            samples: Vec::new(),
        });
        spike.seconds += 1;
        if spike.samples.len() < MAX_SAMPLES {
            spike.samples.push(percent);
        }
        if spike.step.is_empty() && !step.is_empty() {
            spike.step = step.to_owned();
        }
        if percent > spike.peak {
            spike.peak = percent;
            if !step.is_empty() {
                spike.step = step.to_owned();
            }
            if let Some(top) = top {
                spike.top = top;
            }
        }
    }

    fn close(&mut self) {
        let Some(spike) = self.open.take() else {
            return;
        };
        if spike.seconds < self.threshold.secs {
            return;
        }
        // The same resource in the same step is one spike: the highest
        // peak with what was running then, and the longest stretch.
        match self
            .done
            .iter_mut()
            .find(|s| s.fingerprint() == spike.fingerprint())
        {
            Some(kept) => {
                let seconds = kept.seconds.max(spike.seconds);
                if spike.peak > kept.peak {
                    *kept = spike;
                }
                kept.seconds = seconds;
            }
            None => self.done.push(spike),
        }
    }

    /// The spikes, the one still running included.
    #[must_use]
    pub fn finish(mut self) -> Vec<Spike> {
        self.close();
        self.done
    }
}

/// `/proc/meminfo`'s share in use, in percent.
#[must_use]
pub fn memory_percent(meminfo: &str) -> Option<u64> {
    let total = losos_report::meminfo_field(meminfo, "MemTotal")?;
    let available = losos_report::meminfo_field(meminfo, "MemAvailable")?;
    (total > 0).then(|| (total.saturating_sub(available)) * 100 / total)
}

/// `/proc/stat`'s first line: the time spent busy and in all.
#[must_use]
pub fn cpu_times(stat: &str) -> Option<(u64, u64)> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let n: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    let total: u64 = n.iter().take(8).sum();
    // idle and iowait are the time the processor waited.
    let idle = n.get(3).copied().unwrap_or(0) + n.get(4).copied().unwrap_or(0);
    Some((total.saturating_sub(idle), total))
}

/// One process in `/proc/<pid>/stat`: its name, its processor time in
/// ticks and its resident memory in pages.
#[must_use]
pub fn process_stat(stat: &str) -> Option<(String, u64, u64)> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat.get(open + 1..close)?.to_owned();
    // After the name: state is field 3, utime 14, stime 15, rss 24.
    let rest: Vec<&str> = stat.get(close + 2..)?.split_whitespace().collect();
    let field = |n: usize| rest.get(n - 3).and_then(|v| v.parse::<u64>().ok());
    Some((name, field(14)? + field(15)?, field(24)?))
}

/// Every process now: pid, name, ticks, resident pages.
fn processes() -> Vec<(u32, String, u64, u64)> {
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    dir.flatten()
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(e.path().join("stat")).ok()?;
            let (name, ticks, pages) = process_stat(&stat)?;
            Some((pid, name, ticks, pages))
        })
        .collect()
}

/// The biggest processes by memory or processor, the biggest first.
fn top(now: &[(u32, String, u64, u64)], before: &HashMap<u32, u64>, by: Resource) -> Vec<Process> {
    // USER_HZ is 100 and pages are 4 KiB on every x86_64 Linux.
    let mut list: Vec<Process> = now
        .iter()
        .map(|(pid, name, ticks, pages)| Process {
            name: name.clone(),
            rss_mib: pages * 4 / 1024,
            cpu_percent: ticks.saturating_sub(before.get(pid).copied().unwrap_or(*ticks)),
        })
        .collect();
    match by {
        Resource::Memory => list.sort_by_key(|p| std::cmp::Reverse(p.rss_mib)),
        Resource::Cpu => list.sort_by_key(|p| std::cmp::Reverse(p.cpu_percent)),
    }
    list.truncate(8);
    list
}

/// The watcher: a thread that samples memory and processor once a second
/// while the install runs.
pub struct Watch {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Vec<Spike>>>,
}

impl Watch {
    #[must_use]
    pub fn start() -> Watch {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("losos-spikes".into())
            .spawn(move || {
                let (mem_t, cpu_t) = thresholds();
                let mut memory = Tracker::new(Resource::Memory, mem_t);
                let mut cpu = Tracker::new(Resource::Cpu, cpu_t);
                let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
                let mut last_cpu = cpu_times(&read("/proc/stat"));
                let mut last_ticks: HashMap<u32, u64> = HashMap::new();
                while !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_secs(1));
                    let step = current_step().0;
                    let mem = memory_percent(&read("/proc/meminfo")).unwrap_or(0);
                    let now_cpu = cpu_times(&read("/proc/stat"));
                    let busy = match (last_cpu, now_cpu) {
                        (Some((b0, t0)), Some((b1, t1))) if t1 > t0 => (b1 - b0) * 100 / (t1 - t0),
                        _ => 0,
                    };
                    last_cpu = now_cpu;
                    let high = mem >= mem_t.percent || busy >= cpu_t.percent;
                    let procs = if high { processes() } else { Vec::new() };
                    let mem_top = memory
                        .wants_top(mem)
                        .then(|| top(&procs, &last_ticks, Resource::Memory));
                    let cpu_top = cpu
                        .wants_top(busy)
                        .then(|| top(&procs, &last_ticks, Resource::Cpu));
                    memory.sample(mem, &step, mem_top);
                    cpu.sample(busy, &step, cpu_top);
                    // Each process's ticks, for the next second's shares.
                    last_ticks = procs.iter().map(|(pid, _, t, _)| (*pid, *t)).collect();
                }
                let mut spikes = memory.finish();
                spikes.extend(cpu.finish());
                spikes
            })
            .ok();
        Watch { stop, thread }
    }

    /// Stops the watcher and answers what it saw.
    #[must_use]
    pub fn finish(mut self) -> Vec<Spike> {
        self.stop.store(true, Ordering::Relaxed);
        self.thread
            .take()
            .and_then(|t| t.join().ok())
            .unwrap_or_default()
    }
}

// ── The report ──────────────────────────────────────────────────────────

/// The systemd credential the GitHub App's client ID comes in.
pub const CLIENT_ID_CREDENTIAL: &str = "report-client-id";

/// The GitHub App reports sign in with: the installer unit's credential
/// `report-client-id` (modules/installer.nix: `ImportCredential=`, or
/// `SetCredential=` from a medium built with one), never a variable. None
/// on a medium that has none.
#[must_use]
pub fn client_id() -> Option<String> {
    let dir = std::env::var_os("CREDENTIALS_DIRECTORY")?;
    client_id_in(Path::new(&dir))
}

fn client_id_in(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(CLIENT_ID_CREDENTIAL)).ok()?;
    let id = text.trim();
    (!id.is_empty()).then(|| id.to_string())
}

/// Where reports go, when this medium can send them.
#[must_use]
pub fn target() -> Option<Target> {
    let env = |n: &str| std::env::var(n).ok();
    Target::from_settings(
        env("LOSOS_REPORT_REPOSITORY").as_deref(),
        client_id().as_deref(),
        env("LOSOS_REPORT_LOGIN").as_deref(),
        env("LOSOS_REPORT_API").as_deref(),
    )
}

/// The machine, as the report's table names it.
#[must_use]
pub fn system() -> System {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let mut files = SystemFiles {
        os_release: read("/etc/os-release"),
        cpuinfo: read("/proc/cpuinfo"),
        meminfo: read("/proc/meminfo"),
        vendor: read("/sys/class/dmi/id/sys_vendor"),
        product: read("/sys/class/dmi/id/product_name"),
        kernel: read("/proc/sys/kernel/osrelease"),
        efi: Path::new("/sys/firmware/efi").exists(),
    };
    // The medium stamps its release into the environment as the box does.
    if let Ok(release) = std::env::var("LOSOS_RELEASE") {
        if !release.is_empty() {
            files.os_release = format!("IMAGE_VERSION={release}\n{}", files.os_release);
        }
    }
    System::from_files(&files, std::env::consts::ARCH)
}

/// This boot's warnings and errors.
#[must_use]
pub fn journal() -> Vec<String> {
    Command::new("journalctl")
        .args([
            "-b",
            "-p",
            "warning",
            "--no-pager",
            "-o",
            "short-iso",
            "--no-hostname",
            "-n",
            "200",
        ])
        .stderr(Stdio::null())
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// What a failed install left: the step it stopped at, the error and the
/// output.
#[must_use]
pub fn failure(message: &str, disks: &str) -> Failure {
    let (step, n, total) = current_step();
    // The pointer to tty2 is for the person at the screen; the report
    // carries the output itself.
    let message = message
        .trim_end_matches('.')
        .trim_end_matches(TTY2_HINT)
        .trim_end_matches("; ");
    Failure {
        step: if step.is_empty() {
            "Preparing the installer".to_owned()
        } else {
            step
        },
        position: (n, total),
        message: message.to_owned(),
        log: read_log(),
        disks: disks.to_owned(),
    }
}

/// The handbook's page for `failure`, when it explains it.
#[must_use]
pub fn documented(failure: &Failure) -> Option<Known> {
    install::documented(failure)
}

/// The page's address on the public handbook.
#[must_use]
pub fn handbook_url(known: &Known) -> String {
    format!("{HANDBOOK}{}/", known.page)
}

/// The console the report is read on: 7, which no login takes (tty3 to
/// tty6 log root in, and a getty starting there would wipe it).
#[must_use]
pub fn report_tty() -> u32 {
    std::env::var("LOSOS_REPORT_TTY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7)
}

/// Writes `report` where the person reads it: the file, and a pager on
/// screen [`report_tty`] that starts again when it is closed. Scrolling a
/// console back is gone from the kernel, so a long report needs `less`.
pub fn show(report: &Report) {
    let path = env_path("LOSOS_INSTALL_REPORT", DEFAULT_REPORT);
    let _ = std::fs::write(&path, format!("# {}\n\n{}", report.title, report.body));
    let view = path.with_extension("view");
    let n = report_tty();
    let _ = std::fs::write(
        &view,
        format!(
            "This is the whole report the installer would send to GitHub, in public.\n\
             Nothing has been sent. Arrows and Page Up/Down scroll; hold Alt and press\n\
             F1 to go back, then R to send it.\n\n# {}\n\n{}",
            report.title, report.body
        ),
    );
    let Ok(tty) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(format!("/dev/tty{n}"))
    else {
        return;
    };
    let (Ok(out), Ok(err)) = (tty.try_clone(), tty.try_clone()) else {
        return;
    };
    // Its own session with screen n as the terminal, so less reads that
    // screen's keys. -f: it outlives the installer, for a reader who
    // comes back after a shell.
    let _ = Command::new("setsid")
        .args([
            "-c",
            "-f",
            "sh",
            "-c",
            "while :; do less -R -- \"$0\"; done",
        ])
        .arg(&view)
        .env("LESSSECURE", "1")
        .stdin(tty)
        .stdout(out)
        .stderr(err)
        .spawn();
}

/// Reaches GitHub with curl. A token never goes on a command line: curl
/// reads it from its standard input as a config line.
pub struct Curl {
    pub cancel: Arc<AtomicBool>,
    dir: PathBuf,
}

impl Curl {
    #[must_use]
    pub fn new(cancel: Arc<AtomicBool>) -> Curl {
        let dir = log_path()
            .parent()
            .map_or_else(|| PathBuf::from("/run/losos-install"), Path::to_path_buf);
        Curl { cancel, dir }
    }

    fn run(&self, args: &[String], config: &str) -> Result<(u16, Value), String> {
        let mut child = Command::new("curl")
            .args(["-sS", "--max-time", "60", "-K", "-", "-w", "\n%{http_code}"])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not start curl: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(config.as_bytes());
        }
        let out = child.wait_with_output().map_err(|e| format!("curl: {e}"))?;
        if !out.status.success() {
            let why = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "Could not reach GitHub: {}",
                why.trim().trim_start_matches("curl: ")
            ));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let (body, status) = text.rsplit_once('\n').unwrap_or(("", &text));
        let status = status.trim().parse().unwrap_or(0);
        Ok((status, serde_json::from_str(body).unwrap_or(Value::Null)))
    }
}

/// A curl config line's value, quoted.
fn quoted(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

impl Transport for Curl {
    fn call(&mut self, call: &Call) -> Result<(u16, Value), String> {
        let accept = "header = \"Accept: application/json\"\n";
        match call {
            Call::Form { url, fields } => {
                let config = format!("{accept}data = {}\n", quoted(&github::form(fields)));
                self.run(std::slice::from_ref(url), &config)
            }
            Call::Get { url, query, token } => {
                let config = format!(
                    "{accept}header = {}\nheader = \"X-GitHub-Api-Version: 2022-11-28\"\n",
                    quoted(&format!("Authorization: Bearer {token}"))
                );
                self.run(&[format!("{url}?{}", github::form(query))], &config)
            }
            Call::Post { url, body, token } => {
                // The body can be long; it goes through a file of its own.
                let file = self.dir.join("send.json");
                std::fs::write(&file, body.to_string()).map_err(|e| e.to_string())?;
                let config = format!(
                    "{accept}header = {}\nheader = \"X-GitHub-Api-Version: 2022-11-28\"\n\
                     header = \"Content-Type: application/json\"\n",
                    quoted(&format!("Authorization: Bearer {token}"))
                );
                let result = self.run(
                    &[
                        "--data-binary".into(),
                        format!("@{}", file.display()),
                        url.clone(),
                    ],
                    &config,
                );
                let _ = std::fs::remove_file(&file);
                result
            }
        }
    }

    fn wait(&mut self, secs: u64) -> bool {
        for _ in 0..secs {
            if self.cancel.load(Ordering::Relaxed) {
                return false;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        !self.cancel.load(Ordering::Relaxed)
    }
}

/// Sends `report` on a thread of its own, telling `progress` as it goes;
/// set `cancel` to stop it while it waits for the code.
pub fn send_in_background(
    target: Target,
    report: Report,
    cancel: Arc<AtomicBool>,
    progress: impl FnMut(Progress) + Send + 'static,
) -> std::thread::JoinHandle<Result<(), String>> {
    std::thread::spawn(move || {
        let mut progress = progress;
        let mut curl = Curl::new(cancel);
        github::send(&target, &report, &report.body, &mut curl, &mut progress)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_client_id_is_the_credential_or_nothing() {
        let dir = std::env::temp_dir().join(format!("losos-report-cred-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(client_id_in(&dir), None);
        std::fs::write(dir.join(CLIENT_ID_CREDENTIAL), "\n").unwrap();
        assert_eq!(client_id_in(&dir), None);
        std::fs::write(dir.join(CLIENT_ID_CREDENTIAL), "Iv23test\n").unwrap();
        assert_eq!(client_id_in(&dir).as_deref(), Some("Iv23test"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failure_leaves_the_screen_pointer_out() {
        let f = failure(
            &format!("/etc/losos/disko-script failed (exit status: 7); {TTY2_HINT}"),
            "/dev/vdb",
        );
        assert_eq!(f.message, "/etc/losos/disko-script failed (exit status: 7)");
    }

    #[test]
    fn output_loses_colours_and_redrawn_bars() {
        let raw = "Script started on 2026-10-29 [COMMAND=\"x\"]\n\
                   \x1b[1mbold\x1b[0m line\n\
                   copying 10%\rcopying 50%\rcopying 100%\n\
                   tail\r\n\
                   \nScript done on 2026-10-29\n";
        assert_eq!(clean_output(raw), vec!["bold line", "copying 100%", "tail"]);
    }

    #[test]
    fn a_short_high_is_no_spike_and_a_long_one_keeps_its_peak() {
        let t = Threshold {
            percent: 90,
            secs: 3,
        };
        let mut tr = Tracker::new(Resource::Memory, t);
        for p in [95, 96, 50] {
            tr.sample(p, "Downloading", None);
        }
        assert!(tr.clone().finish().is_empty());
        let nix = Process {
            name: "nix".into(),
            rss_mib: 3000,
            cpu_percent: 100,
        };
        for p in [91, 97, 93, 92, 10] {
            let top = tr.wants_top(p).then(|| vec![nix.clone()]);
            tr.sample(p, "Installing the system", top);
        }
        let spikes = tr.finish();
        assert_eq!(spikes.len(), 1);
        assert_eq!(spikes[0].peak, 97);
        assert_eq!(spikes[0].seconds, 4);
        assert_eq!(spikes[0].samples, vec![91, 97, 93, 92]);
        assert_eq!(spikes[0].top, vec![nix]);
        assert_eq!(spikes[0].step, "Installing the system");
    }

    #[test]
    fn the_same_step_twice_is_one_spike_and_one_still_running_counts() {
        let t = Threshold {
            percent: 50,
            secs: 1,
        };
        let mut tr = Tracker::new(Resource::Cpu, t);
        for p in [60, 10, 80, 10, 70, 70] {
            tr.sample(p, "Unpacking", None);
        }
        let spikes = tr.finish();
        assert_eq!(spikes.len(), 1);
        assert_eq!(spikes[0].peak, 80);
        assert_eq!(spikes[0].seconds, 2);
    }

    #[test]
    fn proc_files_read_as_numbers() {
        let meminfo = "MemTotal:        4000000 kB\nMemFree: 1 kB\nMemAvailable:     400000 kB\n";
        assert_eq!(memory_percent(meminfo), Some(90));
        assert_eq!(
            cpu_times("cpu  100 0 100 700 100 0 0 0 0 0\ncpu0 1 2 3\n"),
            Some((200, 1000))
        );
        let stat = "1234 (nix (worker)) R 1 1 1 0 -1 4194560 100 0 0 0 250 50 0 0 20 0 4 0 100 \
                    9000000 51200 18446744073709551615";
        assert_eq!(
            process_stat(stat),
            Some(("nix (worker)".to_owned(), 300, 51200))
        );
    }

    #[test]
    fn curl_config_values_are_quoted() {
        assert_eq!(quoted("a \"b\" \\ c\nd"), r#""a \"b\" \\ c\nd""#);
    }
}
