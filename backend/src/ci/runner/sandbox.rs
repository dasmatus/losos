//! Where a job's steps run. [`super::pm::Pm`] is the real one: the job's
//! containers, started and stopped through pm with Mesh CI's pm plugin, as
//! the `shared` user. [`Host`] runs the steps as plain processes in the
//! job's directory, for the tests and for a box without Podman.

use anyhow::Context;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The job's directories on the host. Each is mounted into the sandbox.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub root: PathBuf,
    pub workspace: PathBuf,
    pub temp: PathBuf,
    pub actions: PathBuf,
    pub home: PathBuf,
}

impl Dirs {
    pub fn new(root: &Path) -> std::io::Result<Dirs> {
        let d = Dirs {
            root: root.to_path_buf(),
            workspace: root.join("workspace"),
            temp: root.join("temp"),
            actions: root.join("actions"),
            home: root.join("home"),
        };
        for p in [&d.workspace, &d.temp, &d.actions, &d.home] {
            std::fs::create_dir_all(p)?;
        }
        Ok(d)
    }
}

/// The same directories as the steps see them.
#[derive(Debug, Clone)]
pub struct Inside {
    pub workspace: String,
    pub temp: String,
    pub actions: String,
    pub home: String,
}

/// How a process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    TimedOut,
    Stopped,
}

/// A program to run in the sandbox.
pub struct Exec<'a> {
    pub argv: &'a [String],
    pub env: &'a BTreeMap<String, String>,
    pub cwd: &'a str,
    pub timeout: Duration,
}

pub trait Sandbox {
    fn inside(&self) -> &Inside;
    /// `$PATH` in the sandbox before the steps add to it.
    fn base_path(&self) -> String;
    /// The node binary JavaScript actions run with, if there is one.
    fn node(&self) -> Option<String>;
    fn exec(
        &mut self,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit>;
    /// A container action: `image` run once with the job's directories.
    fn run_image(
        &mut self,
        image: &str,
        entrypoint: Option<&str>,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit>;
    /// Build an action's Dockerfile; the image's name.
    fn build_image(
        &mut self,
        context: &Path,
        dockerfile: &str,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<String>;
    /// Take everything down.
    fn finish(&mut self);
}

/// Run `cmd`, handing each line of its output (both streams) to `out`
/// until it ends, `timeout` passes or `stop` says so.
pub fn stream(
    mut cmd: Command,
    timeout: Duration,
    stop: &dyn Fn() -> bool,
    out: &mut dyn FnMut(&str),
) -> anyhow::Result<Exit> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting the step")?;
    let (tx, rx) = mpsc::channel::<String>();
    let mut readers = Vec::new();
    let pipes: [Option<Box<dyn Read + Send>>; 2] = [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    ];
    for pipe in pipes.into_iter().flatten() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            let mut r = BufReader::new(pipe);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match r.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        let line = line.trim_end_matches(['\n', '\r']).to_string();
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        }));
    }
    drop(tx);
    let deadline = Instant::now() + timeout;
    let mut ended: Option<Exit> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => out(&line),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if ended.is_none() {
            if stop() {
                ended = Some(Exit::Stopped);
                let _ = child.kill();
            } else if Instant::now() >= deadline {
                ended = Some(Exit::TimedOut);
                let _ = child.kill();
            }
        }
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child.wait()?;
    Ok(ended.unwrap_or(Exit::Code(status.code().unwrap_or(-1))))
}

// ── the host ──────────────────────────────────────────────────────────────

pub struct Host {
    inside: Inside,
}

impl Host {
    pub fn new(dirs: &Dirs) -> Host {
        let s = |p: &Path| p.to_string_lossy().into_owned();
        Host {
            inside: Inside {
                workspace: s(&dirs.workspace),
                temp: s(&dirs.temp),
                actions: s(&dirs.actions),
                home: s(&dirs.home),
            },
        }
    }
}

impl Sandbox for Host {
    fn inside(&self) -> &Inside {
        &self.inside
    }

    fn base_path(&self) -> String {
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
    }

    fn node(&self) -> Option<String> {
        std::env::var("LOSOS_CI_NODE").ok()
    }

    fn exec(
        &mut self,
        e: &Exec<'_>,
        stop: &dyn Fn() -> bool,
        out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit> {
        let (program, args) = e.argv.split_first().context("nothing to run")?;
        let mut cmd = Command::new(program);
        cmd.args(args).env_clear().envs(e.env).current_dir(e.cwd);
        stream(cmd, e.timeout, stop, out)
    }

    fn run_image(
        &mut self,
        _image: &str,
        _entrypoint: Option<&str>,
        _e: &Exec<'_>,
        _stop: &dyn Fn() -> bool,
        _out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<Exit> {
        anyhow::bail!("container actions need Podman, and this runner has none")
    }

    fn build_image(
        &mut self,
        _context: &Path,
        _dockerfile: &str,
        _out: &mut dyn FnMut(&str),
    ) -> anyhow::Result<String> {
        anyhow::bail!("container actions need Podman, and this runner has none")
    }

    fn finish(&mut self) {}
}

// ── what the containers get ───────────────────────────────────────────

/// What a job may use of the box that runs it.
#[derive(Debug, Clone)]
pub struct Limits {
    /// `--memory`, e.g. `2g`.
    pub memory: String,
    /// `--cpus`, e.g. `1.5`.
    pub cpus: String,
}

/// Split a container's `options:` string the way a shell would, quotes
/// and all, without running anything.
pub fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                any = true;
            }
            (None, c) if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_are_split_like_a_shell() {
        assert_eq!(
            words(r#"--health-cmd "pg_isready -U x" --health-interval=10s"#),
            vec!["--health-cmd", "pg_isready -U x", "--health-interval=10s"]
        );
        assert_eq!(words("a '' b"), vec!["a", "", "b"]);
    }

    #[test]
    fn the_host_streams_and_times_out() {
        let mut lines = Vec::new();
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo one; echo two >&2; exit 3"]);
        let exit = stream(cmd, Duration::from_secs(10), &|| false, &mut |l| {
            lines.push(l.to_string())
        })
        .unwrap();
        assert_eq!(exit, Exit::Code(3));
        lines.sort();
        assert_eq!(lines, vec!["one", "two"]);
        let mut cmd = Command::new("sleep");
        cmd.arg("10");
        let exit = stream(cmd, Duration::from_millis(300), &|| false, &mut |_| {}).unwrap();
        assert_eq!(exit, Exit::TimedOut);
    }
}
