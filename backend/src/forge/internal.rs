//! The internal socket's routes, for services on the box that work with
//! LosOS Git rather than for people: Mesh CI reads a commit's files and
//! writes its checks here (backend/src/ci/).
//!
//! The socket is the gate: `/run/losos-git-peers/internal.sock`, in a
//! directory only `losos-git` and the members of `losos-git-peers` may
//! enter (modules/git.nix). Nothing here asks who is calling.

use super::auth::Caller;
use super::server::{Fail, State};
use super::statuses::{self, NewStatus};
use actix_web::{web, HttpResponse};
use serde::Deserialize;

type Answer = Result<HttpResponse, Fail>;

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route(
        "/internal/repos/{owner}/{repo}/archive/{sha}",
        web::get().to(archive),
    )
    .route(
        "/internal/repos/{owner}/{repo}/statuses/{sha}",
        web::post().to(post_status),
    )
    .route("/internal/repos/{owner}/{repo}/info", web::get().to(info));
}

fn repo_and_sha(
    state: &State,
    owner: &str,
    name: &str,
    sha: &str,
) -> Result<(super::db::Repo, String), Fail> {
    if !statuses::is_sha(sha) {
        return Err(Fail::bad("A commit is 40 hex characters."));
    }
    let repo = state
        .store
        .repo(owner, super::names::strip_git(name))?
        .ok_or_else(|| Fail::not_found("repository"))?;
    Ok((repo, sha.to_ascii_lowercase()))
}

/// `GET /internal/repos/{owner}/{repo}/archive/{sha}`: the commit's tree as
/// a gzipped tar, `git archive`'s.
async fn archive(path: web::Path<(String, String, String)>, state: web::Data<State>) -> Answer {
    let (owner, name, sha) = path.into_inner();
    let (repo, sha) = repo_and_sha(&state, &owner, &name, &sha)?;
    let dir = state.config.repo_path(&repo.owner, &repo.name);
    let out = state
        .git
        .async_command()
        .arg("--git-dir")
        .arg(&dir)
        .args(["archive", "--format=tar.gz", &sha])
        .stdin(std::process::Stdio::null())
        .output()
        .await?;
    if !out.status.success() {
        return Err(Fail::not_found("commit"));
    }
    Ok(HttpResponse::Ok()
        .content_type("application/gzip")
        .body(out.stdout))
}

/// `POST /internal/repos/{owner}/{repo}/statuses/{sha}`: a check's state.
async fn post_status(
    path: web::Path<(String, String, String)>,
    body: web::Json<NewStatus>,
    state: web::Data<State>,
) -> Answer {
    let (owner, name, sha) = path.into_inner();
    let (repo, sha) = repo_and_sha(&state, &owner, &name, &sha)?;
    if let Some(why) = body.fault() {
        return Err(Fail::bad(why));
    }
    statuses::set(&state.store, repo.id, &sha, &body, "LosOS CI")?;
    super::events::status(&repo, &sha, &body.context, &body.state, &body.description);
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Debug, Deserialize)]
struct InfoQuery {
    /// A LosOS Login name: the answer says what they may do here.
    #[serde(default)]
    user: String,
    /// A branch, tag or full ref to resolve; the default branch when absent.
    #[serde(default, rename = "ref")]
    git_ref: String,
}

/// `GET /internal/repos/{owner}/{repo}/info?user=&ref=`: whether the
/// repository is private, what `user` may do in it, and the commit `ref`
/// (or the default branch) points at, `null` when it names none.
async fn info(
    path: web::Path<(String, String)>,
    query: web::Query<InfoQuery>,
    state: web::Data<State>,
) -> Answer {
    let (owner, name) = path.into_inner();
    let repo = state
        .store
        .repo(&owner, super::names::strip_git(&name))?
        .ok_or_else(|| Fail::not_found("repository"))?;
    let caller = match state.store.user(query.user.trim())? {
        Some(user) if !query.user.trim().is_empty() => Caller::person(user),
        _ => Caller::nobody(),
    };
    let permission = caller.permission(&state.store, &repo)?;
    let wanted = match query.git_ref.trim() {
        "" => format!("refs/heads/{}", repo.default_branch),
        r if r.starts_with("refs/") => r.to_string(),
        r => format!("refs/heads/{r}"),
    };
    // A ref, never an option or a revision expression.
    let sha = if wanted
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c))
        && !wanted.contains("..")
    {
        let dir = state.config.repo_path(&repo.owner, &repo.name);
        let out = state
            .git
            .async_command()
            .arg("--git-dir")
            .arg(&dir)
            .args(["rev-parse", "--verify", "--quiet", "--end-of-options"])
            .arg(format!("{wanted}^{{commit}}"))
            .stdin(std::process::Stdio::null())
            .output()
            .await?;
        let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (out.status.success() && statuses::is_sha(&sha)).then_some(sha)
    } else {
        None
    };
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "owner": repo.owner,
        "name": repo.name,
        "private": repo.private,
        "defaultBranch": repo.default_branch,
        "permission": permission,
        "ref": wanted,
        "sha": sha,
    })))
}
