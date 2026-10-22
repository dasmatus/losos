//! The JSON API under `/git/api/v1/`, for the pages, the terminal client and
//! lososd.
//!
//! The shapes follow Forgejo's where LosOS used them (`login`, `full_name`,
//! `default_branch`, `private`), so a person who scripted Forgejo recognises
//! them; they are LosOS's own otherwise. Errors are `{"message": "..."}`
//! with the status saying which kind.
//!
//! The administrator's routes under `/api/v1/admin/` are what lososd calls
//! with the admin token: they make sure an account or a repository exists
//! (`PUT`, idempotent), and mint a token for one of the owner's computers
//! after lososd checked the owner's password itself.

use super::auth::{self, Caller};
use super::db::{Attachment, Exists, Issue, IssueFilter, Permission, Repo, User, Visibility};
use super::gitcmd::Signature;
use super::server::{self, Fail, State};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Value};

/// What the version route says.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn routes(cfg: &mut web::ServiceConfig) {
    let scope = web::scope("/api/v1")
        .route("/version", web::get().to(version))
        .route("/user", web::get().to(me))
        .route("/user/repos", web::get().to(my_repos))
        .route("/user/repos", web::post().to(create_my_repo))
        .route("/user/tokens", web::get().to(my_tokens))
        .route("/user/tokens", web::post().to(create_my_token))
        .route("/user/tokens/{id}", web::delete().to(delete_my_token))
        .route("/users/{login}", web::get().to(user))
        .route("/users/{login}/repos", web::get().to(user_repos))
        .route("/repos/search", web::get().to(search_repos))
        .route("/repos/{owner}/{repo}", web::get().to(get_repo))
        .route("/repos/{owner}/{repo}", web::patch().to(edit_repo))
        .route("/repos/{owner}/{repo}", web::delete().to(delete_repo))
        .route(
            "/repos/{owner}/{repo}/collaborators",
            web::get().to(collaborators),
        )
        .route(
            "/repos/{owner}/{repo}/collaborators/{login}",
            web::put().to(put_collaborator),
        )
        .route(
            "/repos/{owner}/{repo}/collaborators/{login}",
            web::delete().to(delete_collaborator),
        )
        .route("/repos/{owner}/{repo}/issues", web::get().to(list_issues))
        .route("/repos/{owner}/{repo}/pulls", web::get().to(list_pulls))
        .route(
            "/repos/{owner}/{repo}/issues/{number}",
            web::get().to(get_issue),
        )
        .route(
            "/repos/{owner}/{repo}/issues/{number}/comments",
            web::get().to(issue_comments),
        )
        .route("/repos/{owner}/{repo}/labels", web::get().to(repo_labels))
        .route(
            "/repos/{owner}/{repo}/commits/{sha}/statuses",
            web::get().to(commit_statuses),
        )
        .route(
            "/repos/{owner}/{repo}/statuses/{sha}",
            web::post().to(create_status),
        )
        .route("/attachments/{uuid}", web::get().to(get_attachment))
        .route("/admin/users", web::get().to(admin_users))
        .route("/admin/users/{login}", web::put().to(admin_ensure_user))
        .route(
            "/admin/users/{login}/repos/{repo}",
            web::put().to(admin_ensure_repo),
        )
        .route(
            "/admin/users/{login}/tokens",
            web::post().to(admin_create_token),
        )
        .route(
            "/admin/users/{login}/tokens/{name}",
            web::delete().to(admin_delete_token),
        );
    // Pull requests and review, the code, the worktrees: one scope, as a
    // scope that matches the prefix answers 404 for the rest.
    let scope = super::pulls::routes(scope);
    let scope = super::code::routes(scope);
    let scope = super::reviewer::api::routes(scope);
    cfg.service(super::worktrees::routes(scope));
}

// ── Answers ─────────────────────────────────────────────────────────────────

pub(super) type Answer = Result<HttpResponse, Fail>;

pub(super) fn need_person(c: &Caller) -> Result<&User, Fail> {
    c.user()
        .ok_or_else(|| Fail::unauthorized("Sign in, or send a token."))
}

pub(super) fn need_admin(c: &Caller) -> Result<(), Fail> {
    if c.is_site_admin() {
        Ok(())
    } else {
        Err(Fail::forbidden("Only the box's administrator may do this."))
    }
}

pub(super) fn need_scope(c: &Caller, family: &str, write: bool) -> Result<(), Fail> {
    if c.allows(family, write) {
        Ok(())
    } else {
        Err(Fail::forbidden(format!(
            "The token does not allow {} {family}.",
            if write { "writing" } else { "reading" }
        )))
    }
}

pub(super) fn seen_by_nobody(state: &State, c: &Caller) -> Result<(), Fail> {
    if state.config.require_sign_in && c.user().is_none() && !c.is_site_admin() {
        Err(Fail::unauthorized("Sign in to use LosOS Git."))
    } else {
        Ok(())
    }
}

// ── Views ───────────────────────────────────────────────────────────────────

/// An account as others see it; the address only for the account itself
/// and the administrator.
pub fn user_json(u: &User, viewer: &Caller) -> Value {
    let own = viewer.user().is_some_and(|v| v.id == u.id) || viewer.is_site_admin();
    json!({
        "id": u.id,
        "login": u.login,
        "full_name": u.full_name,
        "email": if own { u.email.as_str() } else { "" },
        "is_admin": u.is_admin,
        "is_bot": u.is_bot,
        "created": u.created,
    })
}

/// The address a clone uses, as the caller reached the box.
fn clone_url(req: &HttpRequest, state: &State, repo: &Repo) -> String {
    let info = req.connection_info();
    format!(
        "{}://{}{}{}/{}.git",
        info.scheme(),
        info.host(),
        state.config.base_path.trim_end_matches('/'),
        format_args!("/{}", repo.owner),
        repo.name
    )
}

pub fn repo_json(req: &HttpRequest, state: &State, repo: &Repo, permission: Permission) -> Value {
    let dir = state.config.repo_path(&repo.owner, &repo.name);
    let empty = state.git.refs(&dir).map(|r| r.is_empty()).unwrap_or(true);
    let base = state.config.base_path.trim_end_matches('/');
    json!({
        "id": repo.id,
        "owner": { "login": repo.owner },
        "name": repo.name,
        "full_name": format!("{}/{}", repo.owner, repo.name),
        "description": repo.description,
        "private": repo.private,
        "visibility": repo.visibility,
        "default_branch": repo.default_branch,
        "ai_review": state.store.ai_review(repo.id).unwrap_or(false),
        "empty": empty,
        "fork": repo.fork_of.is_some(),
        "parent_id": repo.fork_of,
        "html_url": format!("{base}/{}/{}", repo.owner, repo.name),
        "clone_url": clone_url(req, state, repo),
        "created": repo.created,
        "updated": repo.updated,
        "permission": permission,
        "permissions": {
            "admin": permission >= Permission::Admin,
            "push": permission >= Permission::Write,
            "pull": permission >= Permission::Read,
        },
    })
}

fn token_json(t: &super::db::Token) -> Value {
    json!({
        "id": t.id,
        "name": t.name,
        "scopes": t.scopes,
        "last_eight": t.last_eight,
        "created": t.created,
        "last_used": t.last_used,
    })
}

// ── Making and finding repositories ─────────────────────────────────────────

/// Make a repository under `owner`: the record, then the bare repository,
/// then (with `readme`) a first commit. A failure on disk takes the record
/// back out, so the two never disagree.
pub fn create_repo(
    state: &State,
    owner: &User,
    name: &str,
    description: &str,
    visibility: Visibility,
    default_branch: &str,
    readme: bool,
) -> anyhow::Result<Repo> {
    super::names::repo(name).map_err(|e| anyhow::anyhow!(e))?;
    super::names::branch(default_branch).map_err(|e| anyhow::anyhow!(e))?;
    let repo = state.store.create_repo(
        owner.id,
        name,
        description,
        visibility,
        default_branch,
        None,
    )?;
    let dir = state.config.repo_path(&owner.login, name);
    let existed = dir.exists();
    let made = (|| -> anyhow::Result<()> {
        if existed {
            anyhow::bail!("{} is already on disk", dir.display());
        }
        state.git.init_bare(&dir, default_branch)?;
        if readme {
            let mut text = format!("# {name}\n");
            if !description.is_empty() {
                text.push_str(&format!("\n{description}\n"));
            }
            let who = Signature {
                name: if owner.full_name.is_empty() {
                    owner.login.clone()
                } else {
                    owner.full_name.clone()
                },
                email: if owner.email.is_empty() {
                    format!("{}@localhost", owner.login)
                } else {
                    owner.email.clone()
                },
            };
            state
                .git
                .initial_commit(&dir, default_branch, &text, &who)?;
        }
        Ok(())
    })();
    if let Err(e) = made {
        let _ = state.store.delete_repo(repo.id);
        // Never remove a folder this call did not make.
        if !existed {
            let _ = std::fs::remove_dir_all(&dir);
        }
        return Err(e);
    }
    tracing::info!(repo = format!("{}/{name}", owner.login), "repository made");
    Ok(repo)
}

/// The repository a route names and what the caller may do there. Nobody
/// learns that a private repository exists: it is "no such repository".
pub(super) fn find_repo(
    state: &State,
    caller: &Caller,
    owner: &str,
    name: &str,
) -> Result<(Repo, Permission), Fail> {
    let name = super::names::strip_git(name);
    let repo = state
        .store
        .repo(owner, name)?
        .ok_or_else(|| Fail::not_found("repository"))?;
    let p = caller.permission(&state.store, &repo)?;
    if p == Permission::None {
        return Err(Fail::not_found("repository"));
    }
    Ok((repo, p))
}

fn visible(
    state: &State,
    caller: &Caller,
    repos: Vec<Repo>,
) -> anyhow::Result<Vec<(Repo, Permission)>> {
    let mut out = Vec::new();
    for r in repos {
        let p = caller.permission(&state.store, &r)?;
        if p > Permission::None {
            out.push((r, p));
        }
    }
    Ok(out)
}

// ── Handlers ────────────────────────────────────────────────────────────────

async fn version() -> HttpResponse {
    HttpResponse::Ok().json(json!({ "version": VERSION, "server": "LosOS Git" }))
}

async fn me(req: HttpRequest, state: web::Data<State>) -> Answer {
    let c = server::caller(&req, &state)?;
    let u = need_person(&c)?;
    need_scope(&c, "user", false)?;
    Ok(HttpResponse::Ok().json(user_json(u, &c)))
}

async fn user(req: HttpRequest, path: web::Path<String>, state: web::Data<State>) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    let u = state
        .store
        .user(&path)?
        .ok_or_else(|| Fail::not_found("account"))?;
    Ok(HttpResponse::Ok().json(user_json(&u, &c)))
}

async fn user_repos(req: HttpRequest, path: web::Path<String>, state: web::Data<State>) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", false)?;
    let u = state
        .store
        .user(&path)?
        .ok_or_else(|| Fail::not_found("account"))?;
    let repos = state.store.repos_of(u.id)?;
    let list: Vec<Value> = visible(&state, &c, repos)?
        .iter()
        .map(|(r, p)| repo_json(&req, &state, r, *p))
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

async fn my_repos(req: HttpRequest, state: web::Data<State>) -> Answer {
    let c = server::caller(&req, &state)?;
    let u = need_person(&c)?;
    need_scope(&c, "repository", false)?;
    // Their own, the ones shared with them, and the ones the box shares
    // with everyone on it.
    let mut repos = state.store.repos_of(u.id)?;
    repos.extend(state.store.repos_shared_with(u.id)?);
    let open: Vec<Repo> = state
        .store
        .all_repos()?
        .into_iter()
        .filter(|r| r.visibility != Visibility::Private)
        .collect();
    repos.extend(open);
    let mut seen = std::collections::HashSet::new();
    repos.retain(|r| seen.insert(r.id));
    let list: Vec<Value> = visible(&state, &c, repos)?
        .iter()
        .map(|(r, p)| repo_json(&req, &state, r, *p))
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

/// Who may read a repository, as a call asks for it: `visibility`
/// (`private`, `internal` or `public`), or Forgejo's `private` flag alone.
/// Both, when they disagree, are refused rather than one picked.
fn asked_visibility(
    visibility: Option<&str>,
    private: Option<bool>,
) -> Result<Option<Visibility>, Fail> {
    let named = match visibility {
        None => None,
        Some(v) => Some(Visibility::parse(v.trim()).ok_or_else(|| {
            Fail::bad(format!(
                "\"{v}\" is not a visibility: private, internal or public."
            ))
        })?),
    };
    match (named, private) {
        (Some(v), Some(p)) if p != (v != Visibility::Public) => Err(Fail::bad(format!(
            "A {} repository cannot have private set to {p}.",
            v.as_str()
        ))),
        (Some(v), _) => Ok(Some(v)),
        (None, Some(p)) => Ok(Some(Visibility::from_private(p))),
        (None, None) => Ok(None),
    }
}

#[derive(Deserialize)]
struct NewRepo {
    name: String,
    #[serde(default)]
    description: String,
    private: Option<bool>,
    visibility: Option<String>,
    default_branch: Option<String>,
    #[serde(default)]
    auto_init: bool,
}

async fn create_my_repo(
    req: HttpRequest,
    body: web::Json<NewRepo>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    let u = need_person(&c)?.clone();
    need_scope(&c, "repository", true)?;
    let visibility = asked_visibility(body.visibility.as_deref(), body.private)?
        .unwrap_or(Visibility::from_private(state.config.default_private));
    let repo = create_repo(
        &state,
        &u,
        body.name.trim(),
        body.description.trim(),
        visibility,
        body.default_branch.as_deref().unwrap_or("main"),
        body.auto_init,
    )
    .map_err(|e| {
        if e.downcast_ref::<Exists>().is_some() {
            Fail::from(e)
        } else {
            Fail::bad(e.to_string())
        }
    })?;
    Ok(HttpResponse::Created().json(repo_json(&req, &state, &repo, Permission::Admin)))
}

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    limit: Option<usize>,
}

async fn search_repos(
    req: HttpRequest,
    query: web::Query<SearchQuery>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", false)?;
    let q = query.q.trim().to_lowercase();
    let repos = state.store.all_repos()?;
    let mut list: Vec<(Repo, Permission)> = visible(&state, &c, repos)?
        .into_iter()
        .filter(|(r, _)| {
            q.is_empty()
                || r.name.to_lowercase().contains(&q)
                || r.owner.to_lowercase().contains(&q)
                || r.description.to_lowercase().contains(&q)
        })
        .collect();
    list.sort_by(|a, b| b.0.updated.cmp(&a.0.updated));
    list.truncate(query.limit.unwrap_or(50).min(200));
    let docs: Vec<Value> = list
        .iter()
        .map(|(r, p)| repo_json(&req, &state, r, *p))
        .collect();
    Ok(HttpResponse::Ok().json(json!({ "ok": true, "data": docs })))
}

async fn get_repo(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", false)?;
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    Ok(HttpResponse::Ok().json(repo_json(&req, &state, &repo, p)))
}

#[derive(Deserialize)]
struct EditRepo {
    name: Option<String>,
    description: Option<String>,
    private: Option<bool>,
    visibility: Option<String>,
    default_branch: Option<String>,
}

async fn edit_repo(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<EditRepo>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_scope(&c, "repository", true)?;
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    if p < Permission::Admin {
        return Err(Fail::forbidden(
            "Only the repository's administrators may change it.",
        ));
    }
    let new_name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| *n != repo.name);
    if let Some(n) = new_name {
        super::names::repo(n).map_err(Fail::bad)?;
    }
    let visibility = asked_visibility(body.visibility.as_deref(), body.private)?;
    let branch = body.default_branch.as_deref().map(str::trim);
    let dir = state.config.repo_path(&repo.owner, &repo.name);
    if let Some(b) = branch {
        super::names::branch(b).map_err(Fail::bad)?;
        let exists = state
            .git
            .refs(&dir)?
            .iter()
            .any(|r| r.name == format!("refs/heads/{b}"));
        if !exists {
            return Err(Fail::bad(format!("There is no branch {b}.")));
        }
    }
    state.store.update_repo(
        repo.id,
        new_name,
        body.description.as_deref().map(str::trim),
        visibility,
        branch,
    )?;
    if let Some(b) = branch {
        state.git.set_head(&dir, b)?;
    }
    if let Some(n) = new_name {
        let to = state.config.repo_path(&repo.owner, n);
        // A change of case alone is the same folder.
        if to != dir {
            if let Err(e) = std::fs::rename(&dir, &to) {
                let _ = state
                    .store
                    .update_repo(repo.id, Some(&repo.name), None, None, None);
                return Err(e.into());
            }
        }
    }
    let repo = state
        .store
        .repo_by_id(repo.id)?
        .ok_or_else(|| Fail::not_found("repository"))?;
    Ok(HttpResponse::Ok().json(repo_json(&req, &state, &repo, p)))
}

async fn delete_repo(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_scope(&c, "repository", true)?;
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    if p < Permission::Admin {
        return Err(Fail::forbidden(
            "Only the repository's administrators may delete it.",
        ));
    }
    state.store.delete_repo(repo.id)?;
    let dir = state.config.repo_path(&repo.owner, &repo.name);
    if let Err(e) = std::fs::remove_dir_all(&dir) {
        tracing::warn!(dir = %dir.display(), error = ?e, "the repository's folder stayed");
    }
    tracing::info!(
        repo = format!("{}/{}", repo.owner, repo.name),
        "repository deleted"
    );
    Ok(HttpResponse::NoContent().finish())
}

async fn collaborators(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_scope(&c, "repository", false)?;
    let (repo, _) = find_repo(&state, &c, &path.0, &path.1)?;
    let list: Vec<Value> = state
        .store
        .collaborators(repo.id)?
        .iter()
        .map(|(u, p)| {
            let mut doc = user_json(u, &c);
            doc["permission"] = json!(p);
            doc
        })
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

#[derive(Deserialize)]
struct CollaboratorBody {
    permission: Option<String>,
}

async fn put_collaborator(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    body: Option<web::Json<CollaboratorBody>>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_scope(&c, "repository", true)?;
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    if p < Permission::Admin {
        return Err(Fail::forbidden(
            "Only the repository's administrators may add people.",
        ));
    }
    let who = state
        .store
        .user(&path.2)?
        .ok_or_else(|| Fail::not_found("account"))?;
    if who.id == repo.owner_id {
        return Err(Fail::bad("The owner already has every right."));
    }
    let level = body
        .as_ref()
        .and_then(|b| b.permission.as_deref())
        .unwrap_or("write");
    let level =
        Permission::parse(level).ok_or_else(|| Fail::bad("permission is read, write or admin"))?;
    state.store.set_collaborator(repo.id, who.id, level)?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_collaborator(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_scope(&c, "repository", true)?;
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    let who = state
        .store
        .user(&path.2)?
        .ok_or_else(|| Fail::not_found("account"))?;
    // Anyone may leave; only administrators may remove someone else.
    let leaving = c.user().is_some_and(|u| u.id == who.id);
    if p < Permission::Admin && !leaving {
        return Err(Fail::forbidden(
            "Only the repository's administrators may remove people.",
        ));
    }
    state
        .store
        .set_collaborator(repo.id, who.id, Permission::None)?;
    Ok(HttpResponse::NoContent().finish())
}

// ── Tokens ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct NewToken {
    name: String,
    scopes: Vec<String>,
    /// Replace a token of the same name instead of refusing (a computer set
    /// up a second time).
    #[serde(default)]
    replace: bool,
}

fn mint(state: &State, user: &User, body: &NewToken) -> Answer {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err(Fail::bad("A token needs a name of at most 100 characters."));
    }
    let scopes = auth::validate_scopes(&body.scopes).map_err(Fail::bad)?;
    if body.replace {
        state.store.delete_token(user.id, name)?;
    }
    let (token, value) = state.store.create_token(user.id, name, &scopes)?;
    let mut doc = token_json(&token);
    doc["token"] = json!(value);
    Ok(HttpResponse::Created().json(doc))
}

async fn my_tokens(req: HttpRequest, state: web::Data<State>) -> Answer {
    let c = server::caller(&req, &state)?;
    let u = need_person(&c)?;
    need_scope(&c, "user", false)?;
    let list: Vec<Value> = state.store.tokens(u.id)?.iter().map(token_json).collect();
    Ok(HttpResponse::Ok().json(list))
}

async fn create_my_token(
    req: HttpRequest,
    body: web::Json<NewToken>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    let u = need_person(&c)?.clone();
    // A token cannot mint a token: only a person signed in through
    // LosOS Login (or lososd for them) can.
    if c.scopes.is_some() {
        return Err(Fail::forbidden(
            "Make tokens on the LosOS Git page, signed in.",
        ));
    }
    mint(&state, &u, &body)
}

async fn delete_my_token(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    let u = need_person(&c)?;
    need_scope(&c, "user", true)?;
    if state.store.delete_token(u.id, &path)? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(Fail::not_found("token"))
    }
}

// ── The administrator's routes ──────────────────────────────────────────────

async fn admin_users(req: HttpRequest, state: web::Data<State>) -> Answer {
    let c = server::caller(&req, &state)?;
    need_admin(&c)?;
    let list: Vec<Value> = state
        .store
        .users()?
        .iter()
        .map(|u| user_json(u, &c))
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

#[derive(Deserialize)]
struct EnsureUser {
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    email: String,
    admin: Option<bool>,
}

async fn admin_ensure_user(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<EnsureUser>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_admin(&c)?;
    let u = state
        .store
        .ensure_user(&path, body.full_name.trim(), body.email.trim())
        .map_err(|e| Fail::bad(e.to_string()))?;
    if let Some(admin) = body.admin {
        state.store.set_admin(u.id, admin)?;
    }
    let u = state
        .store
        .user_by_id(u.id)?
        .ok_or_else(|| Fail::not_found("account"))?;
    Ok(HttpResponse::Ok().json(user_json(&u, &c)))
}

#[derive(Deserialize)]
struct EnsureRepo {
    #[serde(default)]
    description: String,
    private: Option<bool>,
    visibility: Option<String>,
    default_branch: Option<String>,
}

async fn admin_ensure_repo(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<EnsureRepo>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_admin(&c)?;
    let owner = state
        .store
        .user(&path.0)?
        .ok_or_else(|| Fail::not_found("account"))?;
    let visibility = asked_visibility(body.visibility.as_deref(), body.private)?;
    if let Some(repo) = state.store.repo(&owner.login, &path.1)? {
        return Ok(HttpResponse::Ok().json(repo_json(&req, &state, &repo, Permission::Admin)));
    }
    let repo = create_repo(
        &state,
        &owner,
        &path.1,
        body.description.trim(),
        visibility.unwrap_or(Visibility::Private),
        body.default_branch.as_deref().unwrap_or("main"),
        false,
    )
    .map_err(|e| Fail::bad(e.to_string()))?;
    Ok(HttpResponse::Created().json(repo_json(&req, &state, &repo, Permission::Admin)))
}

async fn admin_create_token(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<NewToken>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_admin(&c)?;
    let u = state
        .store
        .user(&path)?
        .ok_or_else(|| Fail::not_found("account"))?;
    mint(&state, &u, &body)
}

async fn admin_delete_token(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_admin(&c)?;
    let u = state
        .store
        .user(&path.0)?
        .ok_or_else(|| Fail::not_found("account"))?;
    if state.store.delete_token(u.id, &path.1)? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(Fail::not_found("token"))
    }
}

// ── Issues and pull requests ────────────────────────────────────────────────

#[derive(Deserialize)]
struct IssueQuery {
    /// `open` (the default), `closed` or `all`.
    #[serde(default)]
    state: Option<String>,
    /// `issues`, `pulls`, or both when left out.
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

pub(super) fn issue_json(base: &str, repo: &Repo, i: &Issue, attachments: &[Attachment]) -> Value {
    let kind = if i.is_pull { "pulls" } else { "issues" };
    let mut doc = json!({
        "number": i.number,
        "title": i.title,
        "body": i.body,
        "user": { "login": i.poster },
        "state": if i.closed { "closed" } else { "open" },
        "labels": i.labels,
        "assignees": i.assignees.iter().map(|a| json!({ "login": a })).collect::<Vec<_>>(),
        "created_at": i.created,
        "updated_at": i.updated,
        "closed_at": i.closed_at,
        "html_url": format!("{base}/{}/{}/{kind}/{}", repo.owner, repo.name, i.number),
        "assets": attachments.iter().map(|a| json!({
            "uuid": a.uuid,
            "name": a.name,
            "size": a.size,
            "created_at": a.created,
            "browser_download_url": format!("{base}/api/v1/attachments/{}", a.uuid),
        })).collect::<Vec<_>>(),
    });
    if i.is_pull {
        doc["pull_request"] = json!({
            "head": i.head_branch,
            "base": i.base_branch,
            "merged": i.merged,
            "merge_commit_sha": i.merge_commit,
        });
    }
    doc
}

pub(super) fn base(state: &State) -> String {
    state.config.base_path.trim_end_matches('/').to_string()
}

async fn issues_of(
    req: &HttpRequest,
    path: &(String, String),
    query: &IssueQuery,
    state: &State,
    pulls: Option<bool>,
) -> Answer {
    let c = server::caller(req, state)?;
    seen_by_nobody(state, &c)?;
    need_scope(&c, "issue", false)?;
    let (repo, _) = find_repo(state, &c, &path.0, &path.1)?;
    let closed = match query.state.as_deref() {
        None | Some("open") => Some(false),
        Some("closed") => Some(true),
        Some("all") => None,
        Some(other) => {
            return Err(Fail::bad(format!(
                "\"{other}\" is not open, closed or all."
            )))
        }
    };
    let pulls = match (pulls, query.kind.as_deref()) {
        (Some(p), _) => Some(p),
        (None, None) => None,
        (None, Some("issues")) => Some(false),
        (None, Some("pulls")) => Some(true),
        (None, Some(other)) => {
            return Err(Fail::bad(format!("\"{other}\" is not issues or pulls.")))
        }
    };
    let base = base(state);
    let list: Vec<Value> = state
        .store
        .issues(repo.id, IssueFilter { closed, pulls })?
        .iter()
        .map(|i| issue_json(&base, &repo, i, &[]))
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

async fn list_issues(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    query: web::Query<IssueQuery>,
    state: web::Data<State>,
) -> Answer {
    issues_of(&req, &path, &query, &state, None).await
}

async fn list_pulls(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    query: web::Query<IssueQuery>,
    state: web::Data<State>,
) -> Answer {
    issues_of(&req, &path, &query, &state, Some(true)).await
}

fn find_issue(
    req: &HttpRequest,
    state: &State,
    owner: &str,
    name: &str,
    number: i64,
) -> Result<(Repo, Issue), Fail> {
    let c = server::caller(req, state)?;
    seen_by_nobody(state, &c)?;
    need_scope(&c, "issue", false)?;
    let (repo, _) = find_repo(state, &c, owner, name)?;
    let issue = state
        .store
        .issue(repo.id, number)?
        .ok_or_else(|| Fail::not_found("issue"))?;
    Ok((repo, issue))
}

async fn get_issue(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let (repo, issue) = find_issue(&req, &state, &path.0, &path.1, path.2)?;
    let attachments = state.store.attachments(issue.id)?;
    Ok(HttpResponse::Ok().json(issue_json(&base(&state), &repo, &issue, &attachments)))
}

async fn issue_comments(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let (_, issue) = find_issue(&req, &state, &path.0, &path.1, path.2)?;
    let list: Vec<Value> = state
        .store
        .comments(issue.id)?
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "user": { "login": c.poster },
                "body": c.body,
                "created_at": c.created,
                "updated_at": c.updated,
            })
        })
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

async fn repo_labels(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "issue", false)?;
    let (repo, _) = find_repo(&state, &c, &path.0, &path.1)?;
    Ok(HttpResponse::Ok().json(state.store.labels(repo.id)?))
}

/// A file attached to an issue, to whoever may read the issue.
async fn get_attachment(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "issue", false)?;
    let uuid = path.into_inner();
    if uuid.is_empty() || !uuid.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-') {
        return Err(Fail::not_found("attachment"));
    }
    let (attachment, repo_id) = state
        .store
        .attachment(&uuid)?
        .ok_or_else(|| Fail::not_found("attachment"))?;
    let repo = state
        .store
        .repo_by_id(repo_id)?
        .ok_or_else(|| Fail::not_found("attachment"))?;
    // The same answer for a file one may not see as for one that is not there.
    if c.permission(&state.store, &repo)? == Permission::None {
        return Err(Fail::not_found("attachment"));
    }
    let bytes = std::fs::read(
        state
            .config
            .state_dir
            .join("attachments")
            .join(&attachment.uuid),
    )
    .map_err(|_| Fail::not_found("attachment"))?;
    let name = attachment.name.replace(['"', '\\', '\r', '\n'], "_");
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{name}\""),
        ))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .body(bytes))
}

// ── Commit statuses (`super::statuses`) ────────────────────────────────────

/// `GET /api/v1/repos/{owner}/{repo}/commits/{sha}/statuses`: every check
/// on a commit and the state they add up to.
async fn commit_statuses(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", false)?;
    let (repo, _) = find_repo(&state, &c, &path.0, &path.1)?;
    if !super::statuses::is_sha(&path.2) {
        return Err(Fail::bad("A commit is 40 hex characters."));
    }
    let list = super::statuses::list(&state.store, repo.id, &path.2)?;
    Ok(HttpResponse::Ok().json(json!({
        "state": super::statuses::combined(&list),
        "statuses": list,
    })))
}

/// `POST /api/v1/repos/{owner}/{repo}/statuses/{sha}`: a check of one's
/// own on a commit, the body Gitea and GitHub take. Needs push access.
async fn create_status(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    body: web::Json<super::statuses::NewStatus>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    need_scope(&c, "repository", true)?;
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    if p < Permission::Write {
        return Err(Fail::forbidden(
            "Only people who may push to the repository may set a check.",
        ));
    }
    if !super::statuses::is_sha(&path.2) {
        return Err(Fail::bad("A commit is 40 hex characters."));
    }
    if let Some(why) = body.fault() {
        return Err(Fail::bad(why));
    }
    let who = c
        .user()
        .map(|u| u.login.clone())
        .unwrap_or_else(|| "admin".into());
    super::statuses::set(&state.store, repo.id, &path.2, &body, &who)?;
    let list = super::statuses::list(&state.store, repo.id, &path.2)?;
    Ok(HttpResponse::Created().json(json!({
        "state": super::statuses::combined(&list),
        "statuses": list,
    })))
}
