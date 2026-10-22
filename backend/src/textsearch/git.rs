//! Text in LosOS Git: `git grep` over the default branch of every repository,
//! at query time.
//!
//! LosOS Git keeps bare repositories at `<root>/<owner>/<name>.git`. Each one
//! gets one `git grep` on `HEAD`, which in a bare repository is the default
//! branch, and the search stops at the hit limit or the deadline, whichever
//! comes first. Wikis (`<name>.wiki.git`, left by Forgejo) are left out.

use super::{mark, run_capped, GitHit};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Matching lines kept per file, so one file full of the word does not fill
/// the whole list.
const PER_FILE: u32 = 2;
/// Output read from one `git grep`.
const MAX_OUTPUT: usize = 256 << 10;
/// Characters of a matching line shown.
const LINE_WIDTH: usize = 120;

/// The names LosOS Git allows (`crate::forge::names`), and they go into a URL.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

/// Every repository under `root`, as (owner, name, path), sorted.
pub fn repositories(root: &Path) -> Vec<(String, String, PathBuf)> {
    let mut found = Vec::new();
    let Ok(owners) = std::fs::read_dir(root) else {
        return found;
    };
    for owner in owners.flatten() {
        let Some(owner_name) = owner.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !plain_name(&owner_name) || !owner.path().is_dir() {
            continue;
        }
        let Ok(repos) = std::fs::read_dir(owner.path()) else {
            continue;
        };
        for repo in repos.flatten() {
            let Some(dir) = repo.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Some(name) = dir.strip_suffix(".git") else {
                continue;
            };
            if name.ends_with(".wiki") || !plain_name(name) || !repo.path().is_dir() {
                continue;
            }
            found.push((owner_name.clone(), name.to_string(), repo.path()));
        }
    }
    found.sort();
    found
}

fn git(git: &Path, repo: &Path) -> Command {
    let mut cmd = Command::new(git);
    cmd.arg("-C")
        .arg(repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

/// Lines on the default branches that contain every one of `terms`,
/// ignoring case, at most `limit`, found before `deadline`.
pub fn search(
    root: &Path,
    git_bin: &Path,
    terms: &[String],
    limit: usize,
    deadline: Instant,
) -> Vec<GitHit> {
    let mut hits = Vec::new();
    if terms.is_empty() {
        return hits;
    }
    for (owner, repo, path) in repositories(root) {
        if hits.len() >= limit || Instant::now() >= deadline {
            break;
        }
        let branch_deadline = deadline.min(Instant::now() + Duration::from_secs(1));
        let Ok(Some((status, out))) = run_capped(
            git(git_bin, &path).args(["symbolic-ref", "--short", "HEAD"]),
            256,
            branch_deadline,
        ) else {
            continue;
        };
        let branch = String::from_utf8_lossy(&out).trim().to_string();
        if !status.success() || branch.is_empty() {
            continue;
        }
        let mut cmd = git(git_bin, &path);
        cmd.args(["grep", "-I", "-n", "-i", "-F", "--no-color", "-z"])
            .arg(format!("--max-count={PER_FILE}"));
        for (i, term) in terms.iter().enumerate() {
            if i > 0 {
                cmd.arg("--and");
            }
            cmd.arg("-e").arg(term);
        }
        cmd.args(["HEAD", "--"]);
        // Exit 1 is "nothing matched"; an empty repository has no HEAD and
        // fails, which is the same answer here.
        let Ok(Some((status, out))) = run_capped(&mut cmd, MAX_OUTPUT, deadline) else {
            continue;
        };
        if !status.success() && out.is_empty() {
            continue;
        }
        for (file, line, text) in parse(&out) {
            if hits.len() >= limit {
                break;
            }
            hits.push(GitHit {
                owner: owner.clone(),
                repo: repo.clone(),
                branch: branch.clone(),
                path: file,
                line,
                snippet: mark(&text, terms, LINE_WIDTH),
            });
        }
    }
    hits
}

/// `git grep -z` on a tree prints `HEAD:<path>\0<line>\0<text>\n` per match.
fn parse(out: &[u8]) -> Vec<(String, u64, String)> {
    let mut rows = Vec::new();
    let mut rest = out;
    while let Some(a) = rest.iter().position(|&b| b == 0) {
        let Some(b) = rest[a + 1..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| a + 1 + p)
        else {
            break;
        };
        let end = rest[b + 1..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(rest.len(), |p| b + 1 + p);
        let name = String::from_utf8_lossy(&rest[..a]);
        let line = std::str::from_utf8(&rest[a + 1..b])
            .ok()
            .and_then(|l| l.parse().ok());
        if let (Some((_, path)), Some(line)) = (name.split_once(':'), line) {
            rows.push((
                path.to_string(),
                line,
                String::from_utf8_lossy(&rest[b + 1..end]).into_owned(),
            ));
        }
        rest = rest.get(end + 1..).unwrap_or(&[]);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    fn terms(q: &str) -> Vec<String> {
        crate::textsearch::terms(q)
    }

    #[test]
    fn parses_grep_output() {
        let out = b"HEAD:a b.txt\x001\x00hello Tax\nHEAD:d/x:y.md\x0012\x00tax rate\n";
        assert_eq!(
            parse(out),
            [
                ("a b.txt".to_string(), 1, "hello Tax".to_string()),
                ("d/x:y.md".to_string(), 12, "tax rate".to_string()),
            ]
        );
        assert!(parse(b"garbage").is_empty());
    }

    #[test]
    fn searches_the_default_branch_of_each_repository() {
        let dir = assert_fs::TempDir::new().unwrap();
        let work = dir.path().join("work");
        std::fs::create_dir_all(work.join("src")).unwrap();
        sh(dir.path(), &["init", "-q", "work"]);
        std::fs::write(
            work.join("src/lib.rs"),
            "fn tax_rate() {}\n// TAX notes\nother\n",
        )
        .unwrap();
        std::fs::write(work.join("README.md"), "Nothing here\n").unwrap();
        sh(&work, &["add", "."]);
        sh(&work, &["commit", "-qm", "first"]);

        let root = dir.path().join("repositories");
        std::fs::create_dir_all(root.join("matus")).unwrap();
        sh(
            dir.path(),
            &[
                "clone",
                "-q",
                "--bare",
                "work",
                "repositories/matus/books.git",
            ],
        );
        sh(
            dir.path(),
            &[
                "clone",
                "-q",
                "--bare",
                "work",
                "repositories/matus/books.wiki.git",
            ],
        );
        sh(
            dir.path(),
            &["init", "-q", "--bare", "repositories/matus/empty.git"],
        );

        let found = search(
            &root,
            Path::new("git"),
            &terms("tax"),
            10,
            Instant::now() + Duration::from_secs(10),
        );
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found
            .iter()
            .all(|h| h.owner == "matus" && h.repo == "books" && h.branch == "main"));
        assert_eq!(found[0].path, "src/lib.rs");
        assert_eq!(found[0].line, 1);
        assert_eq!(found[0].snippet, ["fn ", "tax", "_rate() {}"]);

        let both = search(
            &root,
            Path::new("git"),
            &terms("tax notes"),
            10,
            Instant::now() + Duration::from_secs(10),
        );
        assert_eq!(both.len(), 1);
        assert_eq!(both[0].line, 2);

        let one = search(
            &root,
            Path::new("git"),
            &terms("tax"),
            1,
            Instant::now() + Duration::from_secs(10),
        );
        assert_eq!(one.len(), 1);
    }
}
