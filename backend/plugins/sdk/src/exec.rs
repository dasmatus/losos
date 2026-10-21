//! Programs the manifest's `exec` list names, run to completion.

use crate::wit::exec as w;

/// A command to run. Built like `std::process::Command`.
#[derive(Debug, Clone, Default)]
pub struct Command {
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    stdin: Option<Vec<u8>>,
    cwd: Option<String>,
    timeout_ms: Option<u32>,
    uid: Option<u32>,
}

/// How it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The exit code; -1 when a signal ended it.
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Output {
    #[must_use]
    pub fn success(&self) -> bool {
        self.status == 0
    }

    #[must_use]
    pub fn stdout_string(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// The last non-empty line of stderr, which is where a tool says why.
    #[must_use]
    pub fn stderr_last_line(&self) -> String {
        String::from_utf8_lossy(&self.stderr)
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

impl Command {
    #[must_use]
    pub fn new(program: &str) -> Self {
        Command {
            program: program.to_string(),
            ..Command::default()
        }
    }

    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }

    #[must_use]
    pub fn stdin(mut self, bytes: Vec<u8>) -> Self {
        self.stdin = Some(bytes);
        self
    }

    #[must_use]
    pub fn cwd(mut self, dir: &str) -> Self {
        self.cwd = Some(dir.to_string());
        self
    }

    #[must_use]
    pub fn timeout_ms(mut self, ms: u32) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    #[must_use]
    pub fn uid(mut self, uid: u32) -> Self {
        self.uid = Some(uid);
        self
    }

    /// Run it and wait.
    pub fn output(&self) -> crate::Result<Output> {
        let out = w::run(&w::Command {
            program: self.program.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            stdin: self.stdin.clone(),
            cwd: self.cwd.clone(),
            timeout_ms: self.timeout_ms,
            uid: self.uid,
        })?;
        Ok(Output {
            status: out.status,
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }
}
