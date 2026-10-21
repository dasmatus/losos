//! The internal socket's routes, for services on the box that work with
//! LosOS Git rather than for people: Mesh CI reads a commit's files and
//! writes its checks here (backend/src/ci/).
//!
//! The socket is the gate: `/run/losos-git-peers/internal.sock`, in a
//! directory only `losos-git` and the members of `losos-git-peers` may
//! enter (modules/git.nix). Nothing here asks who is calling.

use super::server::{Fail, State};
use super::statuses::{self, NewStatus};
use actix_web::{web, HttpResponse};

type Answer = Result<HttpResponse, Fail>;

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route(
        "/internal/repos/{owner}/{repo}/archive/{sha}",
        web::get().to(archive),
    )
    .route(
        "/internal/repos/{owner}/{repo}/statuses/{sha}",
        web::post().to(post_status),
    );
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
    Ok(HttpResponse::NoContent().finish())
}
