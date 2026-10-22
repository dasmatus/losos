//! Worktrees: which checkout a branch is being worked on in.
//!
//! A repository on the box has no worktrees of its own; they are on the
//! computers that push to it, one per `git worktree add`. A push names the
//! one it came from in its push options, which git sends when the
//! worktree's config asks it to:
//!
//! ```sh
//! git config extensions.worktreeConfig true
//! git config --worktree push.pushOption "worktree=fix-backup"
//! git config --worktree --add push.pushOption "computer=laptop"
//! ```
//!
//! Every push from that checkout then says `worktree=fix-backup` and
//! `computer=laptop`, and the box records, per person, computer and
//! worktree, the branch it pushed and the commit. The pages list a
//! repository's worktrees and mark a pull request whose branch one of them
//! is on. A person may also record or forget one by hand (`PUT`, `DELETE`).

use super::api::{find_repo, need_person, need_scope, seen_by_nobody, Answer};
use super::db::{Permission, Repo, User, Worktree};
use super::server::{self, Fail, State};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Value};

/// The longest worktree or computer name kept.
pub const MAX_LABEL: usize = 100;

pub fn routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .route("/repos/{owner}/{repo}/worktrees", web::get().to(list))
        .route("/repos/{owner}/{repo}/worktrees", web::put().to(mark))
        .route(
            "/repos/{owner}/{repo}/worktrees/{id}",
            web::delete().to(forget),
        )
}

/// A worktree or computer name as a push or a request gave it.
pub fn label(value: &str) -> Option<String> {
    let v = value.trim();
    let ok = !v.is_empty() && v.chars().count() <= MAX_LABEL && !v.chars().any(char::is_control);
    ok.then(|| v.to_string())
}

/// One ref a push updates: `(old, new, name)`.
pub type RefUpdate = (String, String, String);

/// What the start of a push's request body says: the refs it updates
/// `(old, new, name)` and its push options. `None` while the bytes run out
/// before the options end, or when they are not a push.
pub fn parse_push(bytes: &[u8]) -> Option<(Vec<RefUpdate>, Vec<String>)> {
    let mut at = 0;
    let mut next = || -> Option<Option<&[u8]>> {
        let len = std::str::from_utf8(bytes.get(at..at + 4)?).ok()?;
        let len = usize::from_str_radix(len, 16).ok()?;
        if len == 0 {
            at += 4;
            return Some(None);
        }
        if len < 4 {
            return None;
        }
        let payload = bytes.get(at + 4..at + len)?;
        at += len;
        Some(Some(payload))
    };
    let mut commands = Vec::new();
    let mut options_follow = false;
    while let Some(line) = next()? {
        let (line, caps) = match line.iter().position(|b| *b == 0) {
            Some(nul) => (&line[..nul], Some(&line[nul + 1..])),
            None => (line, None),
        };
        if let Some(caps) = caps {
            options_follow = String::from_utf8_lossy(caps)
                .split_whitespace()
                .any(|c| c == "push-options");
        }
        let line = String::from_utf8_lossy(line);
        let line = line.trim_end_matches('\n');
        if line.starts_with("shallow ") {
            continue;
        }
        let mut f = line.splitn(3, ' ');
        let (Some(old), Some(new), Some(name)) = (f.next(), f.next(), f.next()) else {
            return None;
        };
        commands.push((old.to_string(), new.to_string(), name.to_string()));
    }
    let mut options = Vec::new();
    if options_follow {
        while let Some(line) = next()? {
            options.push(
                String::from_utf8_lossy(line)
                    .trim_end_matches('\n')
                    .to_string(),
            );
        }
    }
    Some((commands, options))
}

/// The worktree and computer a push's options name.
pub fn named_in(options: &[String]) -> Option<(String, String)> {
    let get = |key: &str| {
        options
            .iter()
            .rev()
            .find_map(|o| o.strip_prefix(key)?.strip_prefix('=').and_then(label))
    };
    Some((get("worktree")?, get("computer").unwrap_or_default()))
}

/// After a push that named its worktree: what it pushed is recorded.
pub fn record_push(state: &State, repo: &Repo, pusher: &User, preamble: &[u8]) {
    let Some((commands, options)) = parse_push(preamble) else {
        return;
    };
    let Some((name, computer)) = named_in(&options) else {
        return;
    };
    const ZERO: &str = "0000000000000000000000000000000000000000";
    // The last branch the push updated is the one the worktree is on.
    let Some((new, branch)) = commands
        .iter()
        .rev()
        .filter(|(_, new, _)| new != ZERO)
        .find_map(|(_, new, r)| {
            r.strip_prefix("refs/heads/")
                .map(|b| (new.clone(), b.to_string()))
        })
    else {
        return;
    };
    if let Err(e) = state
        .store
        .record_worktree(repo.id, pusher.id, &computer, &name, &branch, &new)
    {
        tracing::warn!(error = ?e, "could not record the worktree");
    } else {
        tracing::info!(
            repo = format!("{}/{}", repo.owner, repo.name),
            worktree = name.as_str(),
            branch = branch.as_str(),
            "worktree"
        );
    }
}

pub fn worktree_json(state: &State, repo: &Repo, w: &Worktree) -> Value {
    let dir = state.config.repo_path(&repo.owner, &repo.name);
    let tip = state.git.commit_of(&dir, &w.branch).ok().flatten();
    json!({
        "id": w.id,
        "owner": { "login": w.owner },
        "computer": w.computer,
        "name": w.name,
        "branch": w.branch,
        "head": w.head,
        // The branch is gone, or moved on from another checkout.
        "branch_exists": tip.is_some(),
        "behind_branch": tip.as_deref().is_some_and(|t| t != w.head),
        "updated_at": w.updated,
    })
}

async fn list(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", false)?;
    let (repo, _) = find_repo(&state, &c, &path.0, &path.1)?;
    let list: Vec<Value> = state
        .store
        .worktrees(repo.id)?
        .iter()
        .map(|w| worktree_json(&state, &repo, w))
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

#[derive(Deserialize)]
struct Mark {
    name: String,
    #[serde(default)]
    computer: String,
    branch: String,
}

async fn mark(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<Mark>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", true)?;
    let user = need_person(&c)?.clone();
    let (repo, _) = find_repo(&state, &c, &path.0, &path.1)?;
    if c.repo_access(&state.store, &repo)? < Permission::Write {
        return Err(Fail::forbidden(
            "Only people who may push can mark a worktree.",
        ));
    }
    let name = label(&body.name).ok_or_else(|| Fail::bad("Name the worktree."))?;
    let computer = if body.computer.trim().is_empty() {
        String::new()
    } else {
        label(&body.computer).ok_or_else(|| Fail::bad("That computer name is too long."))?
    };
    super::names::branch(&body.branch).map_err(Fail::bad)?;
    let dir = state.config.repo_path(&repo.owner, &repo.name);
    let head = state
        .git
        .commit_of(&dir, &body.branch)?
        .ok_or_else(|| Fail::bad(format!("There is no branch {}.", body.branch)))?;
    state
        .store
        .record_worktree(repo.id, user.id, &computer, &name, &body.branch, &head)?;
    let w = state
        .store
        .worktrees(repo.id)?
        .into_iter()
        .find(|w| w.owner == user.login && w.computer == computer && w.name == name)
        .ok_or_else(|| Fail::not_found("worktree"))?;
    Ok(HttpResponse::Ok().json(worktree_json(&state, &repo, &w)))
}

async fn forget(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", true)?;
    let user = need_person(&c)?.clone();
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    let w = state
        .store
        .worktrees(repo.id)?
        .into_iter()
        .find(|w| w.id == path.2)
        .ok_or_else(|| Fail::not_found("worktree"))?;
    if w.owner != user.login && p < Permission::Admin {
        return Err(Fail::forbidden("Only its owner can forget a worktree."));
    }
    state.store.delete_worktree(repo.id, w.id)?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::transport::{pkt_line, FLUSH};

    fn push(caps: &str, options: &[&str]) -> Vec<u8> {
        let zero = "0".repeat(40);
        let a = "a".repeat(40);
        let mut b = pkt_line(&format!("{zero} {a} refs/heads/fix\0{caps}\n"));
        b.extend(pkt_line(&format!("{a} {zero} refs/heads/old\n")));
        b.extend(FLUSH);
        for o in options {
            b.extend(pkt_line(&format!("{o}\n")));
        }
        if !options.is_empty() {
            b.extend(FLUSH);
        }
        b.extend(b"PACK....");
        b
    }

    #[test]
    fn a_push_names_its_worktree_in_its_options() {
        let body = push(
            "report-status push-options agent=git/2.50",
            &["worktree=fix-backup", "computer=laptop", "ci.skip"],
        );
        let (commands, options) = parse_push(&body).unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].2, "refs/heads/fix");
        assert_eq!(
            named_in(&options),
            Some(("fix-backup".to_string(), "laptop".to_string()))
        );
    }

    #[test]
    fn a_push_without_options_names_none() {
        let body = push("report-status", &[]);
        let (commands, options) = parse_push(&body).unwrap();
        assert_eq!(commands.len(), 2);
        assert!(options.is_empty());
        assert_eq!(named_in(&options), None);
        assert_eq!(named_in(&["worktree=".into()]), None);
        assert_eq!(
            named_in(&["worktree=x".into()]),
            Some(("x".to_string(), String::new()))
        );
    }

    #[test]
    fn a_cut_off_preamble_is_not_read() {
        let body = push("push-options", &["worktree=w"]);
        assert!(parse_push(&body[..20]).is_none());
        assert!(parse_push(b"garbage").is_none());
    }
}
