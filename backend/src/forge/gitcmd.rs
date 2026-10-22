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

/// A commit as the pages list it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Commit {
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    /// ISO 8601, the author's date.
    pub date: String,
    pub subject: String,
    pub body: String,
}

/// One file of a diff.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileDiff {
    pub path: String,
    /// Where a renamed or copied file came from.
    pub old_path: Option<String>,
    /// `added`, `modified`, `deleted`, `renamed`, `copied` or `changed`
    /// (its type, as a file turned link).
    pub status: &'static str,
    pub additions: i64,
    pub deletions: i64,
    pub binary: bool,
    /// The unified diff from the first `@@`, or `None` for a binary file and
    /// for one too large to show.
    pub patch: Option<String>,
    pub too_large: bool,
}

/// One entry of a tree.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    /// `tree`, `blob`, `link` or `commit` (a submodule).
    pub kind: &'static str,
    pub oid: String,
    pub size: Option<i64>,
}

/// What a merge of two commits would give, made without a worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeTree {
    pub tree: String,
    /// Files both sides changed in ways git could not put together.
    pub conflicts: Vec<String>,
}

/// The most of one file's patch a page is given.
pub const PATCH_FILE_LIMIT: usize = 256 * 1024;
/// The most of a whole diff's patches.
pub const PATCH_TOTAL_LIMIT: usize = 2 * 1024 * 1024;

fn is_object_id(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.chars().all(|c| c.is_ascii_hexdigit())
}

impl Git {
    /// `git --git-dir=<repo> <args…>`: the exit code, stdout and stderr,
    /// for the commands whose exit code is an answer.
    fn run_status(&self, repo: &Path, args: &[&str]) -> anyhow::Result<(i32, String, String)> {
        let out = self
            .command()
            .arg(format!("--git-dir={}", repo.display()))
            .args(args)
            .stdin(Stdio::null())
            .output()?;
        Ok((
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }

    /// The commit a branch (`refs/heads/<branch>`) or an object id names.
    pub fn commit_of(&self, repo: &Path, rev: &str) -> anyhow::Result<Option<String>> {
        let full = if is_object_id(rev) {
            rev.to_string()
        } else {
            format!("refs/heads/{rev}")
        };
        let (code, out, _) = self.run_status(
            repo,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{full}^{{commit}}"),
            ],
        )?;
        Ok((code == 0).then(|| out.trim().to_string()))
    }

    pub fn merge_base(&self, repo: &Path, a: &str, b: &str) -> anyhow::Result<Option<String>> {
        let (code, out, err) = self.run_status(repo, &["merge-base", a, b])?;
        match code {
            0 => Ok(Some(out.trim().to_string())),
            1 => Ok(None),
            _ => anyhow::bail!("git merge-base failed: {}", err.trim()),
        }
    }

    /// Whether `a` is `b` or one of its ancestors.
    pub fn is_ancestor(&self, repo: &Path, a: &str, b: &str) -> anyhow::Result<bool> {
        let (code, _, err) = self.run_status(repo, &["merge-base", "--is-ancestor", a, b])?;
        match code {
            0 => Ok(true),
            1 => Ok(false),
            _ => anyhow::bail!("git merge-base failed: {}", err.trim()),
        }
    }

    /// Commits newest first: those of `to` that `from` lacks, or with no
    /// `from` all of `to`'s, at most `limit`.
    pub fn log(
        &self,
        repo: &Path,
        from: Option<&str>,
        to: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<Commit>> {
        let range = match from {
            Some(f) => format!("{f}..{to}"),
            None => to.to_string(),
        };
        let out = self.run(
            repo,
            &[
                "log",
                &format!("--max-count={limit}"),
                "--format=%H%x1f%an%x1f%ae%x1f%aI%x1f%s%x1f%b%x1e",
                &range,
                "--",
            ],
        )?;
        Ok(out
            .split('\x1e')
            .filter_map(|rec| {
                let rec = rec.trim_start_matches('\n');
                let f: Vec<&str> = rec.splitn(6, '\x1f').collect();
                (f.len() == 6).then(|| Commit {
                    sha: f[0].to_string(),
                    author_name: f[1].to_string(),
                    author_email: f[2].to_string(),
                    date: f[3].to_string(),
                    subject: f[4].to_string(),
                    body: f[5].trim_end().to_string(),
                })
            })
            .collect())
    }

    /// How many commits each side has that the other lacks: (ahead, behind)
    /// of `head` against `base`.
    pub fn ahead_behind(&self, repo: &Path, base: &str, head: &str) -> anyhow::Result<(i64, i64)> {
        let out = self.run(
            repo,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{base}...{head}"),
            ],
        )?;
        let mut n = out
            .split_whitespace()
            .map(|v| v.parse::<i64>().unwrap_or(0));
        let behind = n.next().unwrap_or(0);
        let ahead = n.next().unwrap_or(0);
        Ok((ahead, behind))
    }

    /// Every file that differs between two commits, with its patch.
    pub fn diff(&self, repo: &Path, from: &str, to: &str) -> anyhow::Result<Vec<FileDiff>> {
        let opts = [
            "--no-color",
            "--no-ext-diff",
            "--find-renames",
            "--full-index",
        ];
        let mut args = vec!["-c", "core.quotePath=false", "diff", "--name-status", "-z"];
        args.extend(opts);
        args.extend([from, to, "--"]);
        let names = self.run(repo, &args)?;
        let mut files = parse_name_status(&names);

        let mut args = vec!["-c", "core.quotePath=false", "diff", "--numstat", "-z"];
        args.extend(opts);
        args.extend([from, to, "--"]);
        let numstat = self.run(repo, &args)?;
        for (file, (add, del)) in files.iter_mut().zip(parse_numstat(&numstat)) {
            match (add, del) {
                (Some(a), Some(d)) => {
                    file.additions = a;
                    file.deletions = d;
                }
                _ => file.binary = true,
            }
        }

        let mut args = vec!["-c", "core.quotePath=false", "diff", "--unified=3"];
        args.extend(opts);
        args.extend([from, to, "--"]);
        let patch = self.run(repo, &args)?;
        let patches = split_patch(&patch);
        let mut total = 0usize;
        if patches.len() == files.len() {
            for (file, p) in files.iter_mut().zip(patches) {
                if file.binary {
                    continue;
                }
                let Some(p) = p else { continue };
                if p.len() > PATCH_FILE_LIMIT || total + p.len() > PATCH_TOTAL_LIMIT {
                    file.too_large = true;
                    continue;
                }
                total += p.len();
                file.patch = Some(p);
            }
        } else {
            // The two lists did not line up; better no patch than the wrong one.
            for file in files.iter_mut() {
                file.too_large = !file.binary;
            }
        }
        Ok(files)
    }

    /// The tree a merge of `theirs` into `ours` would have, and the files
    /// it could not merge (`git merge-tree --write-tree`, no worktree).
    pub fn merge_tree(&self, repo: &Path, ours: &str, theirs: &str) -> anyhow::Result<MergeTree> {
        let (code, out, err) = self.run_status(
            repo,
            &[
                "-c",
                "core.quotePath=false",
                "merge-tree",
                "--write-tree",
                "--name-only",
                "--no-messages",
                ours,
                theirs,
            ],
        )?;
        if code != 0 && code != 1 {
            anyhow::bail!("git merge-tree failed: {}", err.trim());
        }
        let mut lines = out.lines();
        let tree = lines.next().unwrap_or("").trim().to_string();
        if !is_object_id(&tree) {
            anyhow::bail!("git merge-tree gave no tree");
        }
        let mut conflicts: Vec<String> = if code == 1 {
            lines
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };
        conflicts.dedup();
        Ok(MergeTree { tree, conflicts })
    }

    /// The tree of a commit.
    pub fn tree_of(&self, repo: &Path, commit: &str) -> anyhow::Result<String> {
        Ok(self
            .run(
                repo,
                &["rev-parse", "--verify", &format!("{commit}^{{tree}}")],
            )?
            .trim()
            .to_string())
    }

    /// A commit of `tree` on `parents`, written by `who` now.
    pub fn commit_tree(
        &self,
        repo: &Path,
        tree: &str,
        parents: &[&str],
        message: &str,
        who: &Signature,
    ) -> anyhow::Result<String> {
        let mut args = vec!["commit-tree", tree];
        for p in parents {
            args.push("-p");
            args.push(p);
        }
        let env = [
            ("GIT_AUTHOR_NAME", who.name.as_str()),
            ("GIT_AUTHOR_EMAIL", who.email.as_str()),
            ("GIT_COMMITTER_NAME", who.name.as_str()),
            ("GIT_COMMITTER_EMAIL", who.email.as_str()),
        ];
        let mut text = message.trim_end().to_string();
        text.push('\n');
        Ok(self
            .run_with_input(repo, &args, &env, text.as_bytes())?
            .trim()
            .to_string())
    }

    /// Move `refs/heads/<branch>` from `old` to `new`, refused when the
    /// branch moved in between.
    pub fn move_branch(
        &self,
        repo: &Path,
        branch: &str,
        new: &str,
        old: &str,
    ) -> anyhow::Result<()> {
        self.run(
            repo,
            &["update-ref", &format!("refs/heads/{branch}"), new, old],
        )?;
        Ok(())
    }

    pub fn delete_branch(&self, repo: &Path, branch: &str, old: &str) -> anyhow::Result<()> {
        self.run(
            repo,
            &["update-ref", "-d", &format!("refs/heads/{branch}"), old],
        )?;
        Ok(())
    }

    /// The entries of the directory `path` ("" for the top) in `commit`,
    /// folders first; or the one entry when `path` is a file.
    pub fn ls_tree(&self, repo: &Path, commit: &str, path: &str) -> anyhow::Result<Vec<TreeEntry>> {
        let spec = if path.is_empty() {
            format!("{commit}^{{tree}}")
        } else {
            format!("{commit}:{path}")
        };
        // Is it a folder? Asking for the type first keeps a file's path
        // from being listed as a folder of one.
        let (code, kind, _) = self.run_status(repo, &["cat-file", "-t", &spec])?;
        if code != 0 {
            return Ok(Vec::new());
        }
        let (target, prefix) = if kind.trim() == "tree" {
            (spec, path.to_string())
        } else if path.is_empty() {
            return Ok(Vec::new());
        } else {
            let (dir, _) = path.rsplit_once('/').unwrap_or(("", path));
            let all = self.ls_tree(repo, commit, dir)?;
            return Ok(all.into_iter().filter(|e| e.path == path).collect());
        };
        let out = self.run(repo, &["ls-tree", "-z", "--long", &target])?;
        let mut entries: Vec<TreeEntry> = out
            .split('\0')
            .filter_map(|rec| {
                let (meta, name) = rec.split_once('\t')?;
                let mut m = meta.split_whitespace();
                let mode = m.next()?;
                let kind = m.next()?;
                let oid = m.next()?;
                let size = m.next().and_then(|v| v.parse().ok());
                let kind = match (kind, mode) {
                    ("tree", _) => "tree",
                    ("commit", _) => "commit",
                    ("blob", "120000") => "link",
                    _ => "blob",
                };
                Some(TreeEntry {
                    name: name.to_string(),
                    path: if prefix.is_empty() {
                        name.to_string()
                    } else {
                        format!("{prefix}/{name}")
                    },
                    kind,
                    oid: oid.to_string(),
                    size,
                })
            })
            .collect();
        entries.sort_by(|a, b| {
            (a.kind != "tree")
                .cmp(&(b.kind != "tree"))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(entries)
    }

    /// The first `max` bytes of a blob, and its whole size.
    pub fn blob(&self, repo: &Path, oid: &str, max: usize) -> anyhow::Result<(Vec<u8>, usize)> {
        if !is_object_id(oid) {
            anyhow::bail!("not an object id");
        }
        let out = self
            .command()
            .arg(format!("--git-dir={}", repo.display()))
            .args(["cat-file", "blob", oid])
            .stdin(Stdio::null())
            .output()?;
        if !out.status.success() {
            anyhow::bail!("git cat-file failed");
        }
        let size = out.stdout.len();
        let mut bytes = out.stdout;
        bytes.truncate(max);
        Ok((bytes, size))
    }
}

fn status_word(code: &str) -> &'static str {
    match code.chars().next() {
        Some('A') => "added",
        Some('D') => "deleted",
        Some('R') => "renamed",
        Some('C') => "copied",
        Some('T') => "changed",
        _ => "modified",
    }
}

/// `git diff --name-status -z`: a status, then one path, or two for a
/// rename or a copy.
fn parse_name_status(out: &str) -> Vec<FileDiff> {
    let mut fields = out.split('\0').filter(|f| !f.is_empty());
    let mut files = Vec::new();
    while let Some(code) = fields.next() {
        let status = status_word(code);
        let (old_path, path) = if matches!(status, "renamed" | "copied") {
            let old = fields.next().unwrap_or_default().to_string();
            (Some(old), fields.next().unwrap_or_default().to_string())
        } else {
            (None, fields.next().unwrap_or_default().to_string())
        };
        files.push(FileDiff {
            path,
            old_path,
            status,
            additions: 0,
            deletions: 0,
            binary: false,
            patch: None,
            too_large: false,
        });
    }
    files
}

/// `git diff --numstat -z`: added and deleted lines per file, `-` for a
/// binary one. A rename is `<add>\t<del>\t` then the two paths as fields
/// of their own.
fn parse_numstat(out: &str) -> Vec<(Option<i64>, Option<i64>)> {
    let mut fields = out.split('\0');
    let mut counts = Vec::new();
    while let Some(f) = fields.next() {
        if f.is_empty() {
            continue;
        }
        let mut parts = f.splitn(3, '\t');
        let add = parts.next().and_then(|v| v.parse().ok());
        let del = parts.next().and_then(|v| v.parse().ok());
        let rest = parts.next().unwrap_or("");
        if rest.is_empty() {
            // A rename: the two paths follow.
            fields.next();
            fields.next();
        }
        counts.push((add, del));
    }
    counts
}

/// A whole `git diff` cut into one piece per file, each from its first
/// `@@` (`None` for a file with no hunks: a rename without changes, a
/// mode change, a binary file).
fn split_patch(patch: &str) -> Vec<Option<String>> {
    let mut out = Vec::new();
    for (i, part) in patch.split("\ndiff --git ").enumerate() {
        let part = if i == 0 {
            match part.strip_prefix("diff --git ") {
                Some(p) => p,
                None if part.trim().is_empty() => continue,
                None => part,
            }
        } else {
            part
        };
        let hunks = part.find("\n@@ ").map(|at| {
            let mut p = part[at + 1..].to_string();
            if !p.ends_with('\n') {
                p.push('\n');
            }
            p
        });
        out.push(hunks);
    }
    out
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
