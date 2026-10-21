//! Every call to `git` the server makes, in one place.
//!
//! Each command runs with the same environment: no system or user
//! configuration of the box leaks in (`GIT_CONFIG_NOSYSTEM`, `HOME` is the
//! state directory), git never asks a question on a terminal, and messages
//! are in English so the few that are read back parse. Arguments that came
//! from a request are names that passed [`super::names`] or object ids, and
//! go after the options, so none of them can be read as an option.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct Git {
    pub bin: PathBuf,
    /// `HOME` for git: the state directory.
    pub home: PathBuf,
}

/// One ref and the object it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    pub name: String,
    pub sha: String,
}

/// The author and committer of a commit the server makes itself.
#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub email: String,
}

impl Git {
    pub fn new(bin: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Git {
        Git {
            bin: bin.into(),
            home: home.into(),
        }
    }

    /// `git` with the server's environment, not yet pointed at a repository.
    pub fn command(&self) -> Command {
        let mut c = Command::new(&self.bin);
        self.environment(&mut c);
        c
    }

    /// The same environment for a [`tokio::process::Command`].
    pub fn async_command(&self) -> tokio::process::Command {
        let mut c = tokio::process::Command::new(&self.bin);
        c.env_clear();
        for (k, v) in self.env_pairs() {
            c.env(k, v);
        }
        c
    }

    fn env_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("HOME", self.home.display().to_string()),
            ("PATH", std::env::var("PATH").unwrap_or_default()),
            ("GIT_CONFIG_NOSYSTEM", "1".to_string()),
            ("GIT_TERMINAL_PROMPT", "0".to_string()),
            ("LC_ALL", "C".to_string()),
        ]
    }

    fn environment(&self, c: &mut Command) {
        c.env_clear();
        for (k, v) in self.env_pairs() {
            c.env(k, v);
        }
    }

    /// `git --git-dir=<repo> <args…>`, its stdout, or an error with stderr.
    pub fn run(&self, repo: &Path, args: &[&str]) -> anyhow::Result<String> {
        let out = self
            .command()
            .arg(format!("--git-dir={}", repo.display()))
            .args(args)
            .stdin(Stdio::null())
            .output()?;
        if !out.status.success() {
            anyhow::bail!(
                "git {} failed: {}",
                args.first().unwrap_or(&""),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Like [`Git::run`], with `input` on stdin.
    pub fn run_with_input(
        &self,
        repo: &Path,
        args: &[&str],
        env: &[(&str, &str)],
        input: &[u8],
    ) -> anyhow::Result<String> {
        use std::io::Write;
        let mut child = self
            .command()
            .arg(format!("--git-dir={}", repo.display()))
            .args(args)
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("no stdin for git"))?
            .write_all(input)?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            anyhow::bail!(
                "git {} failed: {}",
                args.first().unwrap_or(&""),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// A new, empty bare repository whose `HEAD` names `branch`.
    pub fn init_bare(&self, path: &Path, branch: &str) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let out = self
            .command()
            .args(["init", "--bare", "--quiet"])
            .arg(format!("--initial-branch={branch}"))
            .arg(path)
            .stdin(Stdio::null())
            .output()?;
        if !out.status.success() {
            anyhow::bail!(
                "git init failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        self.configure(path)
    }

    /// What every repository here is set up with: partial clones may ask
    /// for what they are missing, and pushes may rewrite a branch (the
    /// owner's call; protected branches are checked before git runs).
    pub fn configure(&self, path: &Path) -> anyhow::Result<()> {
        for (k, v) in [
            ("uploadpack.allowFilter", "true"),
            ("uploadpack.allowAnySHA1InWant", "true"),
            ("receive.denyNonFastForwards", "false"),
            ("receive.advertisePushOptions", "true"),
            ("core.logAllRefUpdates", "false"),
        ] {
            self.run(path, &["config", k, v])?;
        }
        Ok(())
    }

    /// Point `HEAD` at `branch`.
    pub fn set_head(&self, path: &Path, branch: &str) -> anyhow::Result<()> {
        self.run(
            path,
            &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")],
        )?;
        Ok(())
    }

    /// Every branch and tag, with what each points at.
    pub fn refs(&self, path: &Path) -> anyhow::Result<Vec<Ref>> {
        let out = self.run(
            path,
            &[
                "for-each-ref",
                "--format=%(objectname) %(refname)",
                "refs/heads",
                "refs/tags",
            ],
        )?;
        Ok(out
            .lines()
            .filter_map(|l| l.split_once(' '))
            .map(|(sha, name)| Ref {
                name: name.to_string(),
                sha: sha.to_string(),
            })
            .collect())
    }

    /// A first commit holding one `README.md` on `branch`, for a repository
    /// made with "add a README".
    pub fn initial_commit(
        &self,
        path: &Path,
        branch: &str,
        readme: &str,
        who: &Signature,
    ) -> anyhow::Result<String> {
        let blob = self
            .run_with_input(
                path,
                &["hash-object", "-w", "--stdin"],
                &[],
                readme.as_bytes(),
            )?
            .trim()
            .to_string();
        let tree = self
            .run_with_input(
                path,
                &["mktree"],
                &[],
                format!("100644 blob {blob}\tREADME.md\n").as_bytes(),
            )?
            .trim()
            .to_string();
        let env = [
            ("GIT_AUTHOR_NAME", who.name.as_str()),
            ("GIT_AUTHOR_EMAIL", who.email.as_str()),
            ("GIT_COMMITTER_NAME", who.name.as_str()),
            ("GIT_COMMITTER_EMAIL", who.email.as_str()),
        ];
        let commit = self
            .run_with_input(path, &["commit-tree", &tree], &env, b"Initial commit\n")?
            .trim()
            .to_string();
        self.run(
            path,
            &["update-ref", &format!("refs/heads/{branch}"), &commit, ""],
        )?;
        Ok(commit)
    }

    /// A copy of `from` at `to`, every branch and tag, for a fork.
    pub fn clone_bare(&self, from: &Path, to: &Path) -> anyhow::Result<()> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let out = self
            .command()
            .args(["clone", "--bare", "--quiet", "--no-local"])
            .arg(from)
            .arg(to)
            .stdin(Stdio::null())
            .output()?;
        if !out.status.success() {
            anyhow::bail!(
                "git clone failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        self.configure(to)
    }
}

/// How the refs moved between two looks, as `(name, old, new)` with the
/// all-zero id for "none", the shape git's own hooks use.
pub fn ref_changes(before: &[Ref], after: &[Ref]) -> Vec<(String, String, String)> {
    const ZERO: &str = "0000000000000000000000000000000000000000";
    let mut out = Vec::new();
    for a in after {
        match before.iter().find(|b| b.name == a.name) {
            Some(b) if b.sha == a.sha => {}
            Some(b) => out.push((a.name.clone(), b.sha.clone(), a.sha.clone())),
            None => out.push((a.name.clone(), ZERO.to_string(), a.sha.clone())),
        }
    }
    for b in before {
        if !after.iter().any(|a| a.name == b.name) {
            out.push((b.name.clone(), b.sha.clone(), ZERO.to_string()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &str, sha: &str) -> Ref {
        Ref {
            name: name.into(),
            sha: sha.into(),
        }
    }

    #[test]
    fn ref_changes_name_every_move() {
        let before = [r("refs/heads/main", "a"), r("refs/heads/old", "b")];
        let after = [r("refs/heads/main", "c"), r("refs/heads/new", "d")];
        let zero = "0".repeat(40);
        assert_eq!(
            ref_changes(&before, &after),
            vec![
                ("refs/heads/main".into(), "a".into(), "c".into()),
                ("refs/heads/new".into(), zero.clone(), "d".into()),
                ("refs/heads/old".into(), "b".into(), zero),
            ]
        );
        assert!(ref_changes(&before, &before).is_empty());
    }

    #[test]
    fn a_new_repository_takes_a_readme_commit() {
        let dir = std::env::temp_dir().join(format!("losos-git-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let git = Git::new("git", &dir);
        let repo = dir.join("o/r.git");
        git.init_bare(&repo, "main").unwrap();
        assert!(git.refs(&repo).unwrap().is_empty());
        let who = Signature {
            name: "Jana".into(),
            email: "jana@localhost".into(),
        };
        let sha = git.initial_commit(&repo, "main", "# r\n", &who).unwrap();
        let refs = git.refs(&repo).unwrap();
        assert_eq!(refs, vec![r("refs/heads/main", &sha)]);
        assert_eq!(
            git.run(&repo, &["show", "main:README.md"]).unwrap(),
            "# r\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
