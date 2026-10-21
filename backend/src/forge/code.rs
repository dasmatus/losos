//! What the pages show of a repository's code: its branches, a folder or a
//! file at a commit, the README, and the history.
//!
//! Every read goes through git (`ls-tree`, `cat-file`, `log`); nothing is
//! checked out. A path from a request is checked before git sees it: no
//! `..`, no empty part, nothing that git could read as an option.

use super::api::{find_repo, need_scope, seen_by_nobody, Answer};
use super::db::Repo;
use super::server::{self, Fail, State};
use actix_web::{web, HttpRequest, HttpResponse};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// The most of one file a page is given.
const FILE_LIMIT: usize = 1024 * 1024;
/// The most commits one page of history lists.
const LOG_LIMIT: usize = 100;

pub fn routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .route("/repos/{owner}/{repo}/branches", web::get().to(branches))
        .route(
            "/repos/{owner}/{repo}/contents",
            web::get().to(contents_root),
        )
        .route(
            "/repos/{owner}/{repo}/contents/{path:.*}",
            web::get().to(contents),
        )
        .route("/repos/{owner}/{repo}/readme", web::get().to(readme))
        .route("/repos/{owner}/{repo}/commits", web::get().to(commits))
}

/// A path inside a repository as a request gave it, or why not.
pub fn clean_path(path: &str) -> Result<String, Fail> {
    let path = path.trim_matches('/');
    if path.is_empty() {
        return Ok(String::new());
    }
    let ok = path.len() <= 4096
        && !path.contains('\0')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if ok {
        Ok(path.to_string())
    } else {
        Err(Fail::not_found("file"))
    }
}

fn dir_of(state: &State, repo: &Repo) -> PathBuf {
    state.config.repo_path(&repo.owner, &repo.name)
}

#[derive(Deserialize)]
struct RefQuery {
    /// A branch or a commit id; the default branch when left out.
    #[serde(rename = "ref")]
    rev: Option<String>,
}

/// The commit a `ref` query names, or `None` for an empty repository.
fn commit_for(state: &State, repo: &Repo, rev: Option<&str>) -> Result<Option<String>, Fail> {
    let rev = rev.unwrap_or(&repo.default_branch);
    let is_id = (rev.len() == 40 || rev.len() == 64) && rev.chars().all(|c| c.is_ascii_hexdigit());
    if !is_id {
        super::names::branch(rev).map_err(|_| Fail::not_found("branch"))?;
    }
    Ok(state.git.commit_of(&dir_of(state, repo), rev)?)
}

fn readable(req: &HttpRequest, state: &State, owner: &str, name: &str) -> Result<Repo, Fail> {
    let c = server::caller(req, state)?;
    seen_by_nobody(state, &c)?;
    need_scope(&c, "repository", false)?;
    let (repo, _) = find_repo(state, &c, owner, name)?;
    Ok(repo)
}

async fn branches(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let repo = readable(&req, &state, &path.0, &path.1)?;
    let dir = dir_of(&state, &repo);
    let mut list = Vec::new();
    for r in state.git.refs(&dir)? {
        let Some(name) = r.name.strip_prefix("refs/heads/") else {
            continue;
        };
        let last = state.git.log(&dir, None, &r.sha, 1)?.into_iter().next();
        list.push(json!({
            "name": name,
            "default": name == repo.default_branch,
            "commit": {
                "sha": r.sha,
                "subject": last.as_ref().map(|c| c.subject.clone()),
                "date": last.as_ref().map(|c| c.date.clone()),
                "author": last.as_ref().map(|c| c.author_name.clone()),
            },
        }));
    }
    list.sort_by_key(|b| {
        (
            !b["default"].as_bool().unwrap_or(false),
            b["name"].as_str().unwrap_or("").to_lowercase(),
        )
    });
    Ok(HttpResponse::Ok().json(list))
}

fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|b| *b == 0) || std::str::from_utf8(bytes).is_err()
}

fn entry_at(state: &State, repo: &Repo, commit: &str, path: &str) -> Result<Value, Fail> {
    let dir = dir_of(state, repo);
    let entries = state.git.ls_tree(&dir, commit, path)?;
    let single_file = entries.len() == 1 && entries[0].path == path && entries[0].kind != "tree";
    if path.is_empty() || !single_file {
        if entries.is_empty() && !path.is_empty() {
            return Err(Fail::not_found("file"));
        }
        let list: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "name": e.name,
                    "path": e.path,
                    "type": match e.kind { "tree" => "dir", "blob" => "file", "link" => "symlink", _ => "submodule" },
                    "sha": e.oid,
                    "size": e.size,
                })
            })
            .collect();
        return Ok(json!({ "type": "dir", "path": path, "commit": commit, "entries": list }));
    }
    let e = &entries[0];
    let (bytes, size) = state.git.blob(&dir, &e.oid, FILE_LIMIT)?;
    let binary = looks_binary(&bytes);
    let truncated = size > bytes.len();
    Ok(json!({
        "type": if e.kind == "link" { "symlink" } else { "file" },
        "name": e.name,
        "path": e.path,
        "sha": e.oid,
        "commit": commit,
        "size": size,
        "binary": binary,
        "truncated": truncated,
        "text": if binary { Value::Null } else { json!(String::from_utf8_lossy(&bytes)) },
        "content": if binary && !truncated {
            json!(base64::engine::general_purpose::STANDARD.encode(&bytes))
        } else {
            Value::Null
        },
    }))
}

async fn contents_root(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    query: web::Query<RefQuery>,
    state: web::Data<State>,
) -> Answer {
    let repo = readable(&req, &state, &path.0, &path.1)?;
    let Some(commit) = commit_for(&state, &repo, query.rev.as_deref())? else {
        return Ok(HttpResponse::Ok()
            .json(json!({ "type": "dir", "path": "", "commit": null, "entries": [] })));
    };
    Ok(HttpResponse::Ok().json(entry_at(&state, &repo, &commit, "")?))
}

async fn contents(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    query: web::Query<RefQuery>,
    state: web::Data<State>,
) -> Answer {
    let repo = readable(&req, &state, &path.0, &path.1)?;
    let file = clean_path(&path.2)?;
    let commit =
        commit_for(&state, &repo, query.rev.as_deref())?.ok_or_else(|| Fail::not_found("file"))?;
    Ok(HttpResponse::Ok().json(entry_at(&state, &repo, &commit, &file)?))
}

/// The README at the top of a commit: the first of the usual names.
async fn readme(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    query: web::Query<RefQuery>,
    state: web::Data<State>,
) -> Answer {
    let repo = readable(&req, &state, &path.0, &path.1)?;
    let commit = commit_for(&state, &repo, query.rev.as_deref())?
        .ok_or_else(|| Fail::not_found("README"))?;
    let dir = dir_of(&state, &repo);
    let entries = state.git.ls_tree(&dir, &commit, "")?;
    let pick = entries
        .iter()
        .filter(|e| e.kind == "blob")
        .find(|e| {
            let n = e.name.to_lowercase();
            n == "readme.md" || n == "readme" || n == "readme.txt" || n == "readme.markdown"
        })
        .ok_or_else(|| Fail::not_found("README"))?;
    Ok(HttpResponse::Ok().json(entry_at(&state, &repo, &commit, &pick.path)?))
}

#[derive(Deserialize)]
struct LogQuery {
    #[serde(rename = "sha")]
    rev: Option<String>,
    limit: Option<usize>,
}

async fn commits(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    query: web::Query<LogQuery>,
    state: web::Data<State>,
) -> Answer {
    let repo = readable(&req, &state, &path.0, &path.1)?;
    let Some(commit) = commit_for(&state, &repo, query.rev.as_deref())? else {
        return Ok(HttpResponse::Ok().json(Vec::<Value>::new()));
    };
    let limit = query.limit.unwrap_or(30).clamp(1, LOG_LIMIT);
    let list: Vec<Value> = state
        .git
        .log(&dir_of(&state, &repo), None, &commit, limit)?
        .iter()
        .map(|c| {
            json!({
                "sha": c.sha,
                "author": { "name": c.author_name, "email": c.author_email },
                "date": c.date,
                "subject": c.subject,
                "body": c.body,
            })
        })
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_the_repository() {
        assert_eq!(clean_path("/src/main.rs/").unwrap(), "src/main.rs");
        assert_eq!(clean_path("").unwrap(), "");
        for bad in ["../etc/passwd", "a/../b", "a//b", "a/./b", "a\0b"] {
            assert!(clean_path(bad).is_err(), "{bad}");
        }
    }
}
