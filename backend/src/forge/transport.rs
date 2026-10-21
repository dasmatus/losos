//! Git's smart HTTP protocol: clone, fetch and push.
//!
//! Three addresses per repository, all below `/git/<owner>/<name>.git/` on
//! the box (nginx takes `/git` off before they arrive here):
//!
//!   * `GET  info/refs?service=git-upload-pack|git-receive-pack`, the refs;
//!   * `POST git-upload-pack`, a clone or a fetch;
//!   * `POST git-receive-pack`, a push.
//!
//! git does the work: each request runs `git upload-pack` or `git
//! receive-pack` with `--stateless-rpc`, the request body on its stdin and
//! its stdout streamed back as the answer, as `git http-backend` and Forgejo
//! do. Protocol version 2 passes through in `Git-Protocol`.
//!
//! What this file adds is who may: reading needs read access, pushing write
//! access, and nobody (no token, no LosOS Login) is asked for credentials
//! with a 401, which is what makes git call its credential helper.

use super::db::{Permission, Repo};
use super::server::{self, Fail, State};
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    UploadPack,
    ReceivePack,
}

impl Service {
    pub fn parse(s: &str) -> Option<Service> {
        match s {
            "git-upload-pack" => Some(Service::UploadPack),
            "git-receive-pack" => Some(Service::ReceivePack),
            _ => None,
        }
    }

    /// The git subcommand.
    pub fn command(self) -> &'static str {
        match self {
            Service::UploadPack => "upload-pack",
            Service::ReceivePack => "receive-pack",
        }
    }

    fn needs(self) -> Permission {
        match self {
            Service::UploadPack => Permission::Read,
            Service::ReceivePack => Permission::Write,
        }
    }
}

/// One pkt-line: four hex digits of length, then the payload.
pub fn pkt_line(payload: &str) -> Vec<u8> {
    format!("{:04x}{payload}", payload.len() + 4).into_bytes()
}

/// The flush packet.
pub const FLUSH: &[u8] = b"0000";

/// Whether the client asked for protocol version 2.
pub fn wants_v2(protocol: Option<&str>) -> bool {
    protocol.is_some_and(|p| p.split(':').any(|kv| kv.trim() == "version=2"))
}

/// A `Git-Protocol` value safe to hand git in its environment: what clients
/// send is `key=value` pairs joined by colons.
pub fn clean_protocol(value: &str) -> Option<String> {
    let ok = !value.is_empty()
        && value.len() < 256
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '=' | ':' | '.' | '-' | '_'));
    ok.then(|| value.to_string())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/{owner}/{repo}/info/refs", web::get().to(info_refs))
        .route(
            "/{owner}/{repo}/git-upload-pack",
            web::post().to(upload_pack),
        )
        .route(
            "/{owner}/{repo}/git-receive-pack",
            web::post().to(receive_pack),
        );
}

#[derive(serde::Deserialize)]
struct InfoRefsQuery {
    service: Option<String>,
}

/// The repository `owner`/`repo` names and what the caller may do there,
/// or the answer for a request that may not go on. A push to a repository
/// that does not exist makes it when the box allows push-to-create and it
/// is in the pusher's own namespace.
fn authorize(
    req: &HttpRequest,
    state: &State,
    owner: &str,
    repo: &str,
    service: Service,
    may_create: bool,
) -> Result<(Repo, PathBuf), Fail> {
    let caller = server::caller(req, state)?;
    let name = super::names::strip_git(repo);
    let (Ok(owner), Ok(name)) = (super::names::owner(owner), super::names::repo(name)) else {
        return Err(Fail::not_found("repository"));
    };
    if caller.user().is_none() && !caller.is_site_admin() && state.config.require_sign_in {
        return Err(Fail::unauthorized("Sign in to use LosOS Git."));
    }
    let found = state.store.repo(owner, name)?;
    let repo = match found {
        Some(r) => r,
        None => {
            let own_namespace = caller
                .user()
                .is_some_and(|u| u.login.eq_ignore_ascii_case(owner));
            if may_create
                && state.config.push_create
                && own_namespace
                && caller.allows("repository", true)
            {
                let user = caller
                    .user()
                    .cloned()
                    .ok_or_else(|| Fail::unauthorized("Sign in to push."))?;
                super::api::create_repo(
                    state,
                    &user,
                    name,
                    "",
                    state.config.default_private,
                    "main",
                    false,
                )
                .map_err(|e| Fail::bad(e.to_string()))?
            } else if caller.user().is_none() && !caller.is_site_admin() {
                // Nobody: maybe a private repository; ask who they are
                // rather than say whether it exists.
                return Err(Fail::unauthorized("Sign in to see this repository."));
            } else {
                return Err(Fail::not_found("repository"));
            }
        }
    };
    let access = caller.repo_access(&state.store, &repo)?;
    if access < service.needs() {
        if caller.user().is_none() && !caller.is_site_admin() {
            return Err(Fail::unauthorized("Sign in to use this repository."));
        }
        if access == Permission::None {
            return Err(Fail::not_found("repository"));
        }
        return Err(Fail::forbidden(
            "You may read this repository but not push to it.",
        ));
    }
    let path = state.config.repo_path(&repo.owner, &repo.name);
    if !path.join("HEAD").exists() {
        tracing::error!(path = %path.display(), "a repository on record is missing on disk");
        return Err(Fail::not_found("repository"));
    }
    Ok((repo, path))
}

fn internal<E: std::fmt::Debug>(e: E) -> HttpResponse {
    use actix_web::ResponseError;
    tracing::error!(error = ?e, "LosOS Git could not answer");
    Fail::new(
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        "LosOS Git could not answer.",
    )
    .error_response()
}

fn protocol(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("git-protocol")
        .and_then(|v| v.to_str().ok())
        .and_then(clean_protocol)
}

async fn info_refs(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    query: web::Query<InfoRefsQuery>,
    state: web::Data<State>,
) -> HttpResponse {
    let (owner, repo) = path.into_inner();
    // The dumb protocol (no service) is not offered.
    let Some(service) = query.service.as_deref().and_then(Service::parse) else {
        return HttpResponse::Forbidden().json(server::message(
            "LosOS Git speaks git's smart protocol only. Update git.",
        ));
    };
    let (_, dir) = match authorize(&req, &state, &owner, &repo, service, true) {
        Ok(v) => v,
        Err(fail) => return actix_web::ResponseError::error_response(&fail),
    };
    let protocol = protocol(&req);
    let mut cmd = state.git.async_command();
    cmd.arg(service.command())
        .args(["--stateless-rpc", "--advertise-refs"])
        .arg(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit());
    if let Some(p) = &protocol {
        cmd.env("GIT_PROTOCOL", p);
    }
    let out = match cmd.output().await {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => return internal(format!("git {} exited {}", service.command(), o.status)),
        Err(e) => return internal(e),
    };
    let mut body = Vec::new();
    if !wants_v2(protocol.as_deref()) {
        body.extend(pkt_line(&format!("# service=git-{}\n", service.command())));
        body.extend(FLUSH);
    }
    body.extend(out);
    HttpResponse::Ok()
        .content_type(format!(
            "application/x-git-{}-advertisement",
            service.command()
        ))
        .insert_header(("Cache-Control", "no-cache, max-age=0, must-revalidate"))
        .body(body)
}

async fn upload_pack(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    payload: web::Payload,
    state: web::Data<State>,
) -> HttpResponse {
    rpc(req, path, payload, state, Service::UploadPack).await
}

async fn receive_pack(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    payload: web::Payload,
    state: web::Data<State>,
) -> HttpResponse {
    rpc(req, path, payload, state, Service::ReceivePack).await
}

async fn rpc(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    mut payload: web::Payload,
    state: web::Data<State>,
    service: Service,
) -> HttpResponse {
    let (owner, repo_name) = path.into_inner();
    let (repo, dir) = match authorize(&req, &state, &owner, &repo_name, service, false) {
        Ok(v) => v,
        Err(fail) => return actix_web::ResponseError::error_response(&fail),
    };
    let expected = format!("application/x-git-{}-request", service.command());
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if content_type != expected {
        return HttpResponse::UnsupportedMediaType()
            .json(server::message(&format!("Expected {expected}.")));
    }
    let gzip = req
        .headers()
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("gzip") || v.eq_ignore_ascii_case("x-gzip"));

    let before = if service == Service::ReceivePack {
        match state.git.refs(&dir) {
            Ok(r) => Some(r),
            Err(e) => return internal(e),
        }
    } else {
        None
    };

    let mut cmd = state.git.async_command();
    cmd.arg(service.command())
        .arg("--stateless-rpc")
        .arg(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true);
    if let Some(p) = protocol(&req) {
        cmd.env("GIT_PROTOCOL", p);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return internal(e),
    };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return internal("git has no pipes");
    };

    // The request body into git, unzipped on the way when it came gzipped.
    actix_web::rt::spawn(async move {
        let mut decoder = gzip.then(|| flate2::write::GzDecoder::new(Vec::new()));
        while let Some(chunk) = payload.next().await {
            let Ok(chunk) = chunk else { return };
            let bytes = match decoder.as_mut() {
                Some(d) => {
                    use std::io::Write;
                    if d.write_all(&chunk).is_err() {
                        return;
                    }
                    std::mem::take(d.get_mut())
                }
                None => chunk.to_vec(),
            };
            if stdin.write_all(&bytes).await.is_err() {
                return;
            }
        }
        if let Some(d) = decoder {
            if let Ok(rest) = d.finish() {
                let _ = stdin.write_all(&rest).await;
            }
        }
        let _ = stdin.shutdown().await;
    });

    // git's answer out as it is written, then the bookkeeping of a push.
    let state = state.into_inner();
    let after_push = before.map(|before| {
        let state = state.clone();
        let dir = dir.clone();
        move || pushed(&state, &repo, &dir, &before)
    });
    let body = Answer {
        stdout,
        child: Some(child),
        after: after_push.map(|f| Box::new(f) as Box<dyn FnOnce() + Send>),
    }
    .into_stream();
    HttpResponse::Ok()
        .content_type(format!("application/x-git-{}-result", service.command()))
        .insert_header(("Cache-Control", "no-cache, max-age=0, must-revalidate"))
        .streaming(body)
}

/// git's stdout as a response body, and what to do once git is done.
struct Answer {
    stdout: tokio::process::ChildStdout,
    child: Option<tokio::process::Child>,
    after: Option<Box<dyn FnOnce() + Send>>,
}

impl Answer {
    fn into_stream(self) -> impl futures_util::Stream<Item = Result<web::Bytes, std::io::Error>> {
        futures_util::stream::unfold(Some(self), |answer| async move {
            let mut answer = answer?;
            let mut buf = vec![0u8; 64 * 1024];
            match answer.stdout.read(&mut buf).await {
                Ok(0) => {
                    if let Some(mut child) = answer.child.take() {
                        match child.wait().await {
                            Ok(status) if !status.success() => {
                                tracing::warn!(%status, "git ended with an error")
                            }
                            Err(e) => tracing::warn!(error = ?e, "could not wait for git"),
                            _ => {}
                        }
                    }
                    if let Some(after) = answer.after.take() {
                        // Before the answer ends, so the pusher's next call
                        // sees it; off the worker, as it reads git and SQLite.
                        if let Err(e) = web::block(after).await {
                            tracing::warn!(error = ?e, "the bookkeeping of a push failed");
                        }
                    }
                    None
                }
                Ok(n) => {
                    buf.truncate(n);
                    Some((Ok(web::Bytes::from(buf)), Some(answer)))
                }
                Err(e) => Some((Err(e), None)),
            }
        })
    }
}

/// After a push: the repository was touched, and a first push into an
/// empty repository whose default branch it did not create makes the
/// branch it did push the default, as a clone would otherwise check out
/// nothing.
pub fn pushed(state: &State, repo: &Repo, dir: &std::path::Path, before: &[super::gitcmd::Ref]) {
    let after = match state.git.refs(dir) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = ?e, "could not read the refs after a push");
            return;
        }
    };
    let changes = super::gitcmd::ref_changes(before, &after);
    if changes.is_empty() {
        return;
    }
    if let Err(e) = state.store.touch_repo(repo.id) {
        tracing::warn!(error = ?e, "could not record the push");
    }
    let default = format!("refs/heads/{}", repo.default_branch);
    if !after.iter().any(|r| r.name == default) {
        if let Some(first) = after.iter().find(|r| r.name.starts_with("refs/heads/")) {
            let branch = first.name.trim_start_matches("refs/heads/").to_string();
            let set = state.git.set_head(dir, &branch).and_then(|_| {
                state
                    .store
                    .update_repo(repo.id, None, None, None, Some(&branch))
            });
            if let Err(e) = set {
                tracing::warn!(error = ?e, "could not make the pushed branch the default");
            }
        }
    }
    tracing::info!(
        repo = format!("{}/{}", repo.owner, repo.name),
        refs = changes.len(),
        "push"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkt_lines_carry_their_own_length() {
        assert_eq!(
            pkt_line("# service=git-upload-pack\n"),
            b"001e# service=git-upload-pack\n".to_vec()
        );
        assert_eq!(pkt_line(""), b"0004".to_vec());
    }

    #[test]
    fn protocol_v2_is_seen_and_odd_values_dropped() {
        assert!(wants_v2(Some("version=2")));
        assert!(wants_v2(Some("object-format=sha1:version=2")));
        assert!(!wants_v2(Some("version=1")));
        assert!(!wants_v2(None));
        assert_eq!(clean_protocol("version=2").as_deref(), Some("version=2"));
        assert_eq!(clean_protocol("version=2\nX=1"), None);
        assert_eq!(clean_protocol("$(reboot)"), None);
        assert_eq!(clean_protocol(""), None);
    }

    #[test]
    fn services_are_the_two_smart_ones() {
        assert_eq!(Service::parse("git-upload-pack"), Some(Service::UploadPack));
        assert_eq!(
            Service::parse("git-receive-pack"),
            Some(Service::ReceivePack)
        );
        assert_eq!(Service::parse("git-upload-archive"), None);
        assert_eq!(Service::parse("upload-pack"), None);
    }
}
