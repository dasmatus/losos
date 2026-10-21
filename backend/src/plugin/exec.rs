//! The `exec` import: programs the manifest lists, run to completion with a
//! clean environment, never through a shell.

use super::bindings::losos::plugin::exec::{Command, Output};
use super::bindings::losos::plugin::types::Error;
use super::manifest::{ExecRule, FileRoot};
use std::io::{Read, Write};
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Most bytes kept of each of stdout and stderr.
pub const MAX_OUTPUT: usize = 16 * 1024 * 1024;
/// When the plugin names no timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Environment a plugin may not set: what would change which code runs.
fn forbidden_env(key: &str) -> bool {
    key.is_empty()
        || key.contains('=')
        || key.contains('\0')
        || key == "PATH"
        || key.starts_with("LD_")
        || key == "BASH_ENV"
        || key == "ENV"
}

/// The primary group of `uid`, from `/etc/passwd`.
fn primary_gid(uid: u32) -> Option<u32> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.len() >= 4 && fields[2].parse::<u32>().ok() == Some(uid))
            .then(|| fields[3].parse::<u32>().ok())
            .flatten()
    })
}

/// The rule that grants `program`, if any.
pub fn rule_for<'a>(rules: &'a [ExecRule], program: &str) -> Option<&'a ExecRule> {
    rules.iter().find(|r| r.program == program)
}

/// Read a pipe to its end, keeping at most [`MAX_OUTPUT`] bytes.
fn drain(mut r: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let room = MAX_OUTPUT.saturating_sub(kept.len());
                    kept.extend_from_slice(&buf[..n.min(room)]);
                }
            }
        }
        kept
    })
}

/// Run `cmd` if the manifest allows it, until it ends or `deadline`.
pub fn run(
    rules: &[ExecRule],
    roots: &[FileRoot],
    cmd: &Command,
    deadline: Instant,
) -> Result<Output, Error> {
    let rule = rule_for(rules, &cmd.program).ok_or_else(|| {
        Error::Denied(format!(
            "{} is not on the manifest's exec list",
            cmd.program
        ))
    })?;
    if cmd.args.iter().any(|a| a.contains('\0')) {
        return Err(Error::Denied("an argument holds a NUL".to_string()));
    }
    let mut c = std::process::Command::new(&cmd.program);
    c.args(&cmd.args)
        .env_clear()
        .stdin(if cmd.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Ok(path) = std::env::var("PATH") {
        c.env("PATH", path);
    }
    for (k, v) in &cmd.env {
        if forbidden_env(k) || v.contains('\0') {
            return Err(Error::Denied(format!("{k} cannot be set for a program")));
        }
        c.env(k, v);
    }
    if let Some(dir) = &cmd.cwd {
        let allowed = super::files::resolve(roots, dir, false)?;
        c.current_dir(allowed.path);
    } else {
        c.current_dir("/");
    }
    if let Some(uid) = cmd.uid {
        use std::os::unix::process::CommandExt;
        if !rule.uids.contains(&uid) {
            return Err(Error::Denied(format!(
                "{} may not run as uid {uid}",
                cmd.program
            )));
        }
        let gid = primary_gid(uid)
            .ok_or_else(|| Error::Failed(format!("uid {uid} has no passwd entry")))?;
        // std drops the supplementary groups when root sets a uid.
        c.gid(gid).uid(uid);
    }

    let timeout = cmd
        .timeout_ms
        .map_or(DEFAULT_TIMEOUT, |ms| Duration::from_millis(u64::from(ms)));
    let until = (Instant::now() + timeout).min(deadline);

    let mut child = c
        .spawn()
        .map_err(|e| Error::Failed(format!("starting {}: {e}", cmd.program)))?;
    if let Some(input) = cmd.stdin.clone() {
        if let Some(mut stdin) = child.stdin.take() {
            std::thread::spawn(move || {
                let _ = stdin.write_all(&input);
            });
        }
    }
    let out = drain(child.stdout.take().expect("piped"));
    let err = drain(child.stderr.take().expect("piped"));
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= until => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::TimedOut);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(Error::Failed(format!("waiting for {}: {e}", cmd.program))),
        }
    };
    Ok(Output {
        status: status.code().unwrap_or(-1),
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(program: &str, args: &[&str]) -> Command {
        Command {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            env: vec![],
            stdin: None,
            cwd: None,
            timeout_ms: None,
            uid: None,
        }
    }

    fn rules() -> Vec<ExecRule> {
        vec![
            ExecRule {
                program: "echo".to_string(),
                uids: vec![],
            },
            ExecRule {
                program: "cat".to_string(),
                uids: vec![],
            },
            ExecRule {
                program: "sleep".to_string(),
                uids: vec![],
            },
            ExecRule {
                program: "printenv".to_string(),
                uids: vec![],
            },
        ]
    }

    fn far() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }

    #[test]
    fn a_listed_program_runs() {
        let out = run(&rules(), &[], &cmd("echo", &["hi", "$HOME"]), far()).unwrap();
        assert_eq!(out.status, 0);
        assert_eq!(out.stdout, b"hi $HOME\n", "no shell expands anything");
        let mut c = cmd("cat", &[]);
        c.stdin = Some(b"piped".to_vec());
        assert_eq!(run(&rules(), &[], &c, far()).unwrap().stdout, b"piped");
    }

    #[test]
    fn the_environment_is_clean() {
        std::env::set_var("LOSOS_EXEC_TEST_SECRET", "leak");
        let out = run(
            &rules(),
            &[],
            &cmd("printenv", &["LOSOS_EXEC_TEST_SECRET"]),
            far(),
        )
        .unwrap();
        assert_ne!(out.status, 0);
        assert!(out.stdout.is_empty());
        let mut c = cmd("printenv", &["X"]);
        c.env = vec![("X".to_string(), "1".to_string())];
        assert_eq!(run(&rules(), &[], &c, far()).unwrap().stdout, b"1\n");
        for bad in ["PATH", "LD_PRELOAD", "A=B", ""] {
            let mut c = cmd("printenv", &[]);
            c.env = vec![(bad.to_string(), "x".to_string())];
            assert!(
                matches!(run(&rules(), &[], &c, far()), Err(Error::Denied(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn an_unlisted_program_or_user_is_denied() {
        for p in ["rm", "/bin/echo", "sh"] {
            assert!(matches!(
                run(&rules(), &[], &cmd(p, &[]), far()),
                Err(Error::Denied(_))
            ));
        }
        let mut c = cmd("echo", &[]);
        c.uid = Some(1000);
        assert!(matches!(
            run(&rules(), &[], &c, far()),
            Err(Error::Denied(_))
        ));
        let mut c = cmd("echo", &[]);
        c.cwd = Some("/etc".to_string());
        assert!(matches!(
            run(&rules(), &[], &c, far()),
            Err(Error::Denied(_))
        ));
    }

    #[test]
    fn a_slow_program_is_killed_at_its_timeout() {
        let mut c = cmd("sleep", &["5"]);
        c.timeout_ms = Some(200);
        let started = Instant::now();
        assert!(matches!(
            run(&rules(), &[], &c, far()),
            Err(Error::TimedOut)
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
        // The call's own deadline bounds it too.
        let started = Instant::now();
        let soon = Instant::now() + Duration::from_millis(200);
        assert!(matches!(
            run(&rules(), &[], &cmd("sleep", &["5"]), soon),
            Err(Error::TimedOut)
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
