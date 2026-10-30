//! The pages plugin: `/api/pages*`, the pages of the admin UI and the apps
//! as MDX an administrator may change (admin-ui/app/src/pages/README.md).
//! What it keeps and checks is [`model`]; this file is the routes, LosOS's
//! own files under the admin UI's directory (`pages/blocks.json` and the
//! defaults, which the build writes) and the repository, which `git` keeps.
//!
//! Reading the changed pages is for everyone signed in, since everyone sees
//! them; reading a page's source and every write is for administrators,
//! which `backend/sections.json` says and the routes check again.

pub mod model;

use losos_plugin::{config, exec, fs, lock, Plugin, Principal, Request, Response};
use model::{Author, Commit, Invalid, NoSuchPage, Store, Write};

/// The repository and the admin UI's directory, from the manifest's config.
pub struct GitStore {
    repo: String,
    ui: String,
}

impl GitStore {
    fn from_config() -> Self {
        GitStore {
            repo: config::get_or("repo", "/var/lib/losos/pages"),
            ui: config::get_or("ui", ""),
        }
    }

    fn git(&self, args: &[&str]) -> anyhow::Result<exec::Output> {
        let out = exec::Command::new("git")
            .arg("-C")
            .arg(self.repo.clone())
            .args([
                "-c",
                "user.name=LosOS",
                "-c",
                "user.email=pages@losos.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args.iter().copied())
            .env("HOME", &self.repo)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()?;
        Ok(out)
    }

    fn git_ok(&self, args: &[&str]) -> anyhow::Result<exec::Output> {
        let out = self.git(args)?;
        if !out.success() {
            anyhow::bail!(
                "git {}: {}",
                args.first().unwrap_or(&""),
                out.stderr_last_line()
            );
        }
        Ok(out)
    }

    /// Whether anything was ever saved. The host refuses a path below a
    /// root that is not a directory yet, so the root is asked first.
    fn started(&self) -> anyhow::Result<bool> {
        Ok(fs::exists(&self.repo)? && fs::exists(&fs::join(&self.repo, ".git"))?)
    }

    /// Makes the repository the first time anything is saved.
    fn ensure_repo(&self) -> anyhow::Result<()> {
        if self.started()? {
            return Ok(());
        }
        if !fs::exists(&self.repo)? {
            fs::create_dir(&self.repo, Some(0o700))?;
        }
        self.git_ok(&["init", "-q"])?;
        fs::write(
            &fs::join(&self.repo, "README.md"),
            model::README.as_bytes(),
            fs::WriteOptions::secret(),
        )?;
        self.git_ok(&["add", "README.md"])?;
        self.git_ok(&["commit", "-q", "-m", "Start the pages' repository"])?;
        Ok(())
    }

    fn path(&self, file: &str) -> String {
        format!("{}/{file}", self.repo)
    }
}

impl Store for GitStore {
    fn registry(&mut self) -> anyhow::Result<String> {
        if self.ui.is_empty() {
            anyhow::bail!("no admin UI directory to read blocks.json from");
        }
        Ok(fs::read_string(&format!("{}/pages/blocks.json", self.ui))?)
    }

    fn default_source(&mut self, id: &str) -> anyhow::Result<Option<String>> {
        if self.ui.is_empty() {
            return Ok(None);
        }
        Ok(fs::read_string_optional(&format!(
            "{}/pages/{id}.mdx",
            self.ui
        ))?)
    }

    fn read(&mut self, file: &str) -> anyhow::Result<Option<String>> {
        if !fs::exists(&self.repo)? {
            return Ok(None);
        }
        Ok(fs::read_string_optional(&self.path(file))?)
    }

    fn commit(&mut self, writes: &[Write], author: &Author, message: &str) -> anyhow::Result<()> {
        self.ensure_repo()?;
        for (file, content) in writes {
            match content {
                Some(text) => fs::write(
                    &self.path(file),
                    text.as_bytes(),
                    fs::WriteOptions::secret(),
                )?,
                None => fs::remove_if_present(&self.path(file))?,
            }
        }
        self.git_ok(&["add", "-A", "--", "."])?;
        // Nothing staged (a save of the same text): no commit.
        if self.git(&["diff", "--cached", "--quiet"])?.success() {
            return Ok(());
        }
        let who = format!(
            "{} <{}@losos.invalid>",
            one_line(&author.name),
            one_line(&author.login)
        );
        self.git_ok(&["commit", "-q", "--author", &who, "-m", message])?;
        Ok(())
    }

    fn history(&mut self, file: &str, max: usize) -> anyhow::Result<Vec<Commit>> {
        if !self.started()? {
            return Ok(Vec::new());
        }
        let n = format!("-n{max}");
        let out = self.git(&["log", &n, "--format=%H%x1f%cI%x1f%an%x1f%s", "--", file])?;
        if !out.success() {
            return Ok(Vec::new());
        }
        Ok(out
            .stdout_string()
            .lines()
            .filter_map(|line| {
                let mut f = line.split('\u{1f}');
                Some(Commit {
                    commit: f.next()?.to_string(),
                    at: f.next()?.to_string(),
                    by: f.next()?.to_string(),
                    message: f.next()?.to_string(),
                })
            })
            .collect())
    }

    fn now(&mut self) -> u64 {
        losos_plugin::clock::now()
    }
}

/// A name for a commit's author line: no line breaks, no angle brackets.
fn one_line(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '<' && *c != '>')
        .collect::<String>()
        .trim()
        .to_string()
}

fn author(req: &Request) -> Author {
    match &req.caller.principal {
        Principal::Person(p) => Author {
            login: p.name.clone(),
            name: if p.display_name.trim().is_empty() {
                p.name.clone()
            } else {
                p.display_name.clone()
            },
        },
        _ => Author {
            login: req.caller.name().to_string(),
            name: "LosOS spare key".to_string(),
        },
    }
}

/// A command's answer: its JSON, a 400 or 404 with the sentence the editor
/// shows, or a 500 whose detail goes to the journal.
fn answer(result: anyhow::Result<serde_json::Value>) -> Response {
    match result {
        Ok(v) => Response::ok(&v),
        Err(e) => {
            if let Some(why) = e.downcast_ref::<Invalid>() {
                Response::error(400, &why.to_string())
            } else if let Some(none) = e.downcast_ref::<NoSuchPage>() {
                Response::error(404, &none.to_string())
            } else {
                Response::failed(&e)
            }
        }
    }
}

/// Runs a write with the pages' lock held, for administrators only.
fn write(
    req: &Request,
    f: impl FnOnce(&mut GitStore, &Author) -> anyhow::Result<serde_json::Value>,
) -> Response {
    if !req.caller.is_admin() {
        return Response::error(403, "only an administrator may change the pages");
    }
    let who = author(req);
    match lock::acquire("pages") {
        Ok(_guard) => answer(f(&mut GitStore::from_config(), &who)),
        Err(e) => Response::failed(&e),
    }
}

/// The `source` of a `{"source": "..."}` body.
fn source(req: &Request) -> Option<String> {
    req.json_value()
        .get("source")
        .and_then(|s| s.as_str())
        .map(str::to_string)
}

pub struct PagesPlugin;

impl Plugin for PagesPlugin {
    fn handle(req: Request) -> Response {
        let page = req.param("page").unwrap_or_default().to_string();
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/pages") => answer(model::cmd_list(&mut GitStore::from_config())),
            ("GET", "/api/pages/style.css") => match model::cmd_style(&mut GitStore::from_config())
            {
                // Versioned by `?v=`, so a change is a new URL.
                Ok(Some(css)) => Response::bytes(200, "text/css; charset=utf-8", css.into_bytes())
                    .header("cache-control", "private, max-age=31536000, immutable")
                    .header("x-content-type-options", "nosniff"),
                Ok(None) => Response::error(404, "no stylesheet"),
                Err(e) => Response::failed(&e),
            },
            ("PUT", "/api/pages/style.css") => match source(&req) {
                Some(css) => write(&req, |s, who| model::cmd_put_style(s, &css, who)),
                None => Response::error(400, r#"body must be JSON: {"source": "..."}"#),
            },
            ("DELETE", "/api/pages/style.css") => {
                write(&req, |s, who| model::cmd_reset_style(s, who))
            }
            ("GET", "/api/pages/{page}") => {
                if !req.caller.is_admin() {
                    return Response::error(403, "only an administrator may read a page's source");
                }
                answer(model::cmd_get(&mut GitStore::from_config(), &page))
            }
            ("PUT", "/api/pages/{page}") => match source(&req) {
                Some(text) => write(&req, |s, who| model::cmd_put(s, &page, &text, who)),
                None => Response::error(400, r#"body must be JSON: {"source": "..."}"#),
            },
            ("DELETE", "/api/pages/{page}") => {
                write(&req, |s, who| model::cmd_reset(s, &page, who))
            }
            _ => Response::error(404, "not found"),
        }
    }
}

losos_plugin::export!(PagesPlugin);
