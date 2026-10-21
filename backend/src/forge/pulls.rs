//! Pull requests and code review.
//!
//! A pull request asks to bring one branch (the head) into another (the
//! base) of the same repository. Its diff is the head against the point it
//! left the base (`git diff base...head`), its commits are those the base
//! lacks, and whether it can merge is `git merge-tree`'s answer, so no
//! worktree is ever checked out on the box. A review is one verdict
//! (approve, ask for changes, or only comment) on the head as it was, with
//! comments on lines of the diff; a line comment starts a thread that
//! anyone may answer and resolve.
//!
//! Merging writes the commit with `git commit-tree` and moves the base with
//! `git update-ref <new> <old>`, which refuses if the base moved meanwhile.
//! Three ways: a merge commit, one squashed commit, or a fast-forward when
//! the head already contains the base. A push that puts a pull request's
//! head into its base (a merge made on someone's computer) marks it merged
//! too ([`after_push`]).

use super::api::{self, find_repo, need_person, need_scope, seen_by_nobody, Answer};
use super::auth::Caller;
use super::db::{
    Issue, LineComment, NewLineComment, Permission, Repo, Review, ReviewState, Side, User,
};
use super::gitcmd::{Commit, FileDiff, Signature};
use super::server::{self, Fail, State};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// The most commits a pull request's page lists.
const COMMIT_LIMIT: usize = 250;

pub fn routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .route("/repos/{owner}/{repo}/pulls", web::post().to(create_pull))
        .route(
            "/repos/{owner}/{repo}/pulls/{number}",
            web::get().to(get_pull),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}",
            web::patch().to(edit_pull),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/commits",
            web::get().to(pull_commits),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/files",
            web::get().to(pull_files),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/reviews",
            web::get().to(list_reviews),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/reviews",
            web::post().to(create_review),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/comments",
            web::get().to(list_line_comments),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/comments",
            web::post().to(reply_line_comment),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/comments/{id}",
            web::patch().to(resolve_line_comment),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/requested_reviewers",
            web::post().to(request_reviewers),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/requested_reviewers/{login}",
            web::delete().to(unrequest_reviewer),
        )
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/merge",
            web::post().to(merge_pull),
        )
        .route(
            "/repos/{owner}/{repo}/issues/{number}/comments",
            web::post().to(create_comment),
        )
        .route(
            "/repos/{owner}/{repo}/compare/{spec:.*}",
            web::get().to(compare),
        )
}

// ── Where a pull request stands ─────────────────────────────────────────────

/// The commits a pull request is between, now (open) or when it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ends {
    /// `None` when the head branch is gone.
    pub head: Option<String>,
    /// `None` when the base branch is gone.
    pub base: Option<String>,
}

pub fn ends(state: &State, dir: &std::path::Path, pr: &Issue) -> anyhow::Result<Ends> {
    if pr.closed {
        return Ok(Ends {
            head: pr.head_sha.clone(),
            base: pr.base_sha.clone(),
        });
    }
    let head = match pr.head_branch.as_deref() {
        Some(b) => state.git.commit_of(dir, b)?,
        None => None,
    };
    let base = match pr.base_branch.as_deref() {
        Some(b) => state.git.commit_of(dir, b)?,
        None => None,
    };
    Ok(Ends { head, base })
}

/// What the page shows about a pull request beside its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub ends: Ends,
    pub merge_base: Option<String>,
    pub ahead: i64,
    pub behind: i64,
    /// `None` while it cannot be told (a branch is gone, or it is closed).
    pub mergeable: Option<bool>,
    pub conflicts: Vec<String>,
    /// The head already contains the base: a fast-forward is possible.
    pub fast_forward: bool,
}

pub fn standing(state: &State, dir: &std::path::Path, pr: &Issue) -> anyhow::Result<Standing> {
    let ends = ends(state, dir, pr)?;
    let mut s = Standing {
        ends: ends.clone(),
        merge_base: None,
        ahead: 0,
        behind: 0,
        mergeable: None,
        conflicts: Vec::new(),
        fast_forward: false,
    };
    let (Some(head), Some(base)) = (ends.head.as_deref(), ends.base.as_deref()) else {
        return Ok(s);
    };
    s.merge_base = state.git.merge_base(dir, base, head)?;
    let (ahead, behind) = state.git.ahead_behind(dir, base, head)?;
    s.ahead = ahead;
    s.behind = behind;
    if !pr.closed {
        s.fast_forward = state.git.is_ancestor(dir, base, head)?;
        if s.merge_base.is_some() {
            let m = state.git.merge_tree(dir, base, head)?;
            s.mergeable = Some(m.conflicts.is_empty());
            s.conflicts = m.conflicts;
        } else {
            s.mergeable = Some(false);
        }
    }
    Ok(s)
}

/// The diff a pull request shows: its head against where it left the base.
pub fn pull_diff(state: &State, repo: &Repo, pr: &Issue) -> anyhow::Result<Vec<FileDiff>> {
    let dir = repo_dir(state, repo);
    let ends = ends(state, &dir, pr)?;
    let (Some(head), Some(base)) = (ends.head, ends.base) else {
        return Ok(Vec::new());
    };
    let from = state.git.merge_base(&dir, &base, &head)?.unwrap_or(base);
    state.git.diff(&dir, &from, &head)
}

fn pull_commit_list(state: &State, repo: &Repo, pr: &Issue) -> anyhow::Result<Vec<Commit>> {
    let dir = repo_dir(state, repo);
    let ends = ends(state, &dir, pr)?;
    let Some(head) = ends.head else {
        return Ok(Vec::new());
    };
    let mut list = state
        .git
        .log(&dir, ends.base.as_deref(), &head, COMMIT_LIMIT)?;
    // Oldest first, the order they were written in.
    list.reverse();
    Ok(list)
}

fn repo_dir(state: &State, repo: &Repo) -> PathBuf {
    state.config.repo_path(&repo.owner, &repo.name)
}

/// Tell whoever listens (the CI, the code reviewer, `super::events`) that a
/// pull request was opened, synchronised, reviewed, closed, reopened or
/// merged.
pub fn notify(state: &State, action: &str, repo: &Repo, pr: &Issue) {
    let head_sha = match pr.head_sha.clone() {
        Some(sha) => sha,
        None => pr
            .head_branch
            .as_deref()
            .and_then(|b| {
                state
                    .git
                    .commit_of(&repo_dir(state, repo), b)
                    .ok()
                    .flatten()
            })
            .unwrap_or_default(),
    };
    super::events::pull_request(
        action,
        repo,
        super::events::PullRequest {
            number: pr.number,
            title: pr.title.clone(),
            head_ref: format!(
                "refs/heads/{}",
                pr.head_branch.as_deref().unwrap_or_default()
            ),
            head_sha,
            base_ref: format!(
                "refs/heads/{}",
                pr.base_branch.as_deref().unwrap_or_default()
            ),
            author: pr.poster.clone(),
        },
    );
}

/// After a push: open pull requests whose head moved are synchronised, and
/// one whose head the base now contains was merged on someone's computer.
pub fn after_push(state: &State, repo: &Repo, changed: &[(String, String, String)]) {
    let moved = |branch: &str| {
        changed
            .iter()
            .any(|(name, _, _)| name.strip_prefix("refs/heads/") == Some(branch))
    };
    let open = match state.store.issues(
        repo.id,
        super::db::IssueFilter {
            closed: Some(false),
            pulls: Some(true),
        },
    ) {
        Ok(list) => list,
        Err(e) => {
            tracing::warn!(error = ?e, "could not read the open pull requests");
            return;
        }
    };
    let dir = repo_dir(state, repo);
    for pr in open {
        let (Some(head_branch), Some(base_branch)) =
            (pr.head_branch.as_deref(), pr.base_branch.as_deref())
        else {
            continue;
        };
        if !moved(head_branch) && !moved(base_branch) {
            continue;
        }
        let check = || -> anyhow::Result<()> {
            let ends = ends(state, &dir, &pr)?;
            if let (Some(head), Some(base)) = (ends.head.as_deref(), ends.base.as_deref()) {
                if moved(base_branch) && head != base && state.git.is_ancestor(&dir, head, base)? {
                    state.store.mark_merged(pr.id, base, head, base, None)?;
                    notify(state, "merged", repo, &pr);
                    return Ok(());
                }
            }
            if moved(head_branch) {
                notify(state, "synchronize", repo, &pr);
            }
            Ok(())
        };
        if let Err(e) = check() {
            tracing::warn!(error = ?e, number = pr.number, "could not look at a pull request after a push");
        }
    }
}

// ── Views ───────────────────────────────────────────────────────────────────

fn commit_json(c: &Commit) -> Value {
    json!({
        "sha": c.sha,
        "author": { "name": c.author_name, "email": c.author_email },
        "date": c.date,
        "subject": c.subject,
        "body": c.body,
    })
}

/// LosOS Reviewer is the box's one bot account; its reviews and comments
/// carry the mark the pages draw beside its name.
fn is_bot(login: &str) -> bool {
    login == super::reviewer::worker::BOT_LOGIN
}

fn review_json(r: &Review) -> Value {
    json!({
        "id": r.id,
        "user": { "login": r.reviewer, "is_bot": is_bot(&r.reviewer) },
        "state": r.state,
        "body": r.body,
        "commit_id": r.commit_id,
        "submitted_at": r.created,
    })
}

fn line_json(c: &LineComment) -> Value {
    json!({
        "id": c.id,
        "review_id": c.review_id,
        "in_reply_to": c.reply_to,
        "user": { "login": c.poster, "is_bot": is_bot(&c.poster) },
        "path": c.path,
        "line": c.line,
        "side": c.side,
        "commit_id": c.commit_id,
        "body": c.body,
        "resolved": c.resolved,
        "resolved_by": c.resolved_by.as_ref().map(|l| json!({ "login": l })),
        "created_at": c.created,
    })
}

fn file_json(f: &FileDiff) -> Value {
    json!({
        "filename": f.path,
        "previous_filename": f.old_path,
        "status": f.status,
        "additions": f.additions,
        "deletions": f.deletions,
        "binary": f.binary,
        "patch": f.patch,
        "too_large": f.too_large,
    })
}

/// Each reviewer's latest verdict that counts (a comment changes none),
/// and who has been asked and not yet answered.
fn reviewers_json(state: &State, pr: &Issue) -> anyhow::Result<Value> {
    let reviews = state.store.reviews(pr.id)?;
    let mut latest: Vec<(String, ReviewState, String)> = Vec::new();
    for r in &reviews {
        if r.state == ReviewState::Commented {
            if !latest.iter().any(|(l, _, _)| l == &r.reviewer) {
                latest.push((r.reviewer.clone(), r.state, r.commit_id.clone()));
            }
            continue;
        }
        match latest.iter_mut().find(|(l, _, _)| l == &r.reviewer) {
            Some(entry) => *entry = (r.reviewer.clone(), r.state, r.commit_id.clone()),
            None => latest.push((r.reviewer.clone(), r.state, r.commit_id.clone())),
        }
    }
    let requested = state.store.review_requests(pr.id)?;
    let mut list = Vec::new();
    for (login, s, commit) in &latest {
        if requested.iter().any(|u| &u.login == login) {
            continue;
        }
        // A reviewer who answered is still a person or a program.
        let is_bot = state.store.user(login)?.is_some_and(|u| u.is_bot);
        list.push(json!({
            "login": login,
            "is_bot": is_bot,
            "state": s,
            "commit_id": commit,
            "requested": false,
        }));
    }
    for u in &requested {
        let before = latest.iter().find(|(l, _, _)| l == &u.login);
        list.push(json!({
            "login": u.login,
            "is_bot": u.is_bot,
            "state": "requested",
            "previous": before.map(|(_, s, _)| *s),
            "requested": true,
        }));
    }
    Ok(Value::Array(list))
}

fn worktree_json(state: &State, repo: &Repo, branch: Option<&str>) -> anyhow::Result<Value> {
    let Some(branch) = branch else {
        return Ok(Value::Null);
    };
    Ok(state
        .store
        .worktrees(repo.id)?
        .into_iter()
        .find(|w| w.branch == branch)
        .map(|w| {
            json!({
                "id": w.id,
                "owner": { "login": w.owner },
                "computer": w.computer,
                "name": w.name,
                "head": w.head,
                "updated_at": w.updated,
            })
        })
        .unwrap_or(Value::Null))
}

pub fn pull_json(
    state: &State,
    repo: &Repo,
    pr: &Issue,
    permission: Permission,
) -> anyhow::Result<Value> {
    let dir = repo_dir(state, repo);
    let s = standing(state, &dir, pr)?;
    let mut doc = api::issue_json(&api::base(state), repo, pr, &[]);
    doc["merged"] = json!(pr.merged);
    doc["merged_by"] = json!(pr.merged_by.as_ref().map(|l| json!({ "login": l })));
    doc["merged_at"] = json!(pr.merged_at);
    doc["merge_commit_sha"] = json!(pr.merge_commit);
    doc["head"] = json!({ "ref": pr.head_branch, "sha": s.ends.head });
    doc["base"] = json!({ "ref": pr.base_branch, "sha": s.ends.base });
    doc["merge_base"] = json!(s.merge_base);
    doc["ahead_by"] = json!(s.ahead);
    doc["behind_by"] = json!(s.behind);
    doc["mergeable"] = json!(s.mergeable);
    doc["conflicts"] = json!(s.conflicts);
    doc["fast_forward"] = json!(s.fast_forward);
    doc["reviewers"] = reviewers_json(state, pr)?;
    doc["worktree"] = worktree_json(state, repo, pr.head_branch.as_deref())?;
    doc["comments"] = json!(state.store.comments(pr.id)?.len());
    doc["review_comments"] = json!(state.store.review_comments(pr.id)?.len());
    doc["can_merge"] = json!(permission >= Permission::Write && !pr.closed);
    doc["permission"] = json!(permission);
    Ok(doc)
}

// ── Finding one ─────────────────────────────────────────────────────────────

/// The pull request a route names, its repository, the caller and what
/// they may do there. A token needs the `issue` scope family, and to write
/// `write:issue`.
fn find_pull(
    req: &HttpRequest,
    state: &State,
    owner: &str,
    name: &str,
    number: i64,
    write: bool,
) -> Result<(Repo, Issue, Caller, Permission), Fail> {
    let c = server::caller(req, state)?;
    seen_by_nobody(state, &c)?;
    need_scope(&c, "issue", write)?;
    let (repo, p) = find_repo(state, &c, owner, name)?;
    let pr = state
        .store
        .issue(repo.id, number)?
        .filter(|i| i.is_pull)
        .ok_or_else(|| Fail::not_found("pull request"))?;
    Ok((repo, pr, c, p))
}

fn signature(u: &User) -> Signature {
    Signature {
        name: if u.full_name.is_empty() {
            u.login.clone()
        } else {
            u.full_name.clone()
        },
        email: if u.email.is_empty() {
            format!("{}@localhost", u.login.to_lowercase())
        } else {
            u.email.clone()
        },
    }
}

fn text(field: &str, value: &str, max: usize) -> Result<(), Fail> {
    if value.chars().count() > max {
        return Err(Fail::bad(format!(
            "The {field} is longer than {max} characters."
        )));
    }
    Ok(())
}

const TITLE_MAX: usize = 255;
const BODY_MAX: usize = 65_536;

// ── Handlers ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct NewPull {
    title: String,
    #[serde(default)]
    body: String,
    head: String,
    base: String,
}

async fn create_pull(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<NewPull>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "issue", true)?;
    let user = need_person(&c)?.clone();
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    if p < Permission::Read {
        return Err(Fail::not_found("repository"));
    }
    let title = body.title.trim();
    if title.is_empty() {
        return Err(Fail::bad("A pull request needs a title."));
    }
    text("title", title, TITLE_MAX)?;
    text("description", &body.body, BODY_MAX)?;
    for b in [&body.head, &body.base] {
        super::names::branch(b).map_err(Fail::bad)?;
    }
    if body.head == body.base {
        return Err(Fail::bad("The two branches are the same branch."));
    }
    let dir = repo_dir(&state, &repo);
    let head = state
        .git
        .commit_of(&dir, &body.head)?
        .ok_or_else(|| Fail::bad(format!("There is no branch {}.", body.head)))?;
    let base = state
        .git
        .commit_of(&dir, &body.base)?
        .ok_or_else(|| Fail::bad(format!("There is no branch {}.", body.base)))?;
    if state.git.is_ancestor(&dir, &head, &base)? {
        return Err(Fail::bad(format!(
            "{} has nothing that {} does not have already.",
            body.head, body.base
        )));
    }
    let open = state.store.issues(
        repo.id,
        super::db::IssueFilter {
            closed: Some(false),
            pulls: Some(true),
        },
    )?;
    if let Some(same) = open.iter().find(|i| {
        i.head_branch.as_deref() == Some(body.head.as_str())
            && i.base_branch.as_deref() == Some(body.base.as_str())
    }) {
        return Err(Fail::new(
            actix_web::http::StatusCode::CONFLICT,
            format!("Pull request #{} already asks for this.", same.number),
        ));
    }
    let pr = state.store.create_issue(
        repo.id,
        user.id,
        title,
        &body.body,
        Some((&body.head, &body.base)),
    )?;
    notify(&state, "opened", &repo, &pr);
    Ok(HttpResponse::Created().json(pull_json(&state, &repo, &pr, p)?))
}

async fn get_pull(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let (repo, pr, _, p) = find_pull(&req, &state, &path.0, &path.1, path.2, false)?;
    Ok(HttpResponse::Ok().json(pull_json(&state, &repo, &pr, p)?))
}

#[derive(Deserialize)]
struct EditPull {
    title: Option<String>,
    body: Option<String>,
    base: Option<String>,
    /// `open` or `closed`.
    state: Option<String>,
}

async fn edit_pull(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    body: web::Json<EditPull>,
    state: web::Data<State>,
) -> Answer {
    let (repo, pr, c, p) = find_pull(&req, &state, &path.0, &path.1, path.2, true)?;
    let user = need_person(&c)?;
    let author = user.login == pr.poster;
    if !author && p < Permission::Write {
        return Err(Fail::forbidden(
            "Only its author and people who may push can change a pull request.",
        ));
    }
    if pr.merged {
        return Err(Fail::bad("The pull request is merged."));
    }
    if let Some(t) = &body.title {
        if t.trim().is_empty() {
            return Err(Fail::bad("A pull request needs a title."));
        }
        text("title", t, TITLE_MAX)?;
    }
    if let Some(b) = &body.body {
        text("description", b, BODY_MAX)?;
    }
    let dir = repo_dir(&state, &repo);
    if let Some(base) = &body.base {
        super::names::branch(base).map_err(Fail::bad)?;
        if state.git.commit_of(&dir, base)?.is_none() {
            return Err(Fail::bad(format!("There is no branch {base}.")));
        }
        if pr.head_branch.as_deref() == Some(base.as_str()) {
            return Err(Fail::bad("The two branches are the same branch."));
        }
    }
    state.store.edit_issue(
        pr.id,
        body.title.as_deref().map(str::trim),
        body.body.as_deref(),
        body.base.as_deref(),
    )?;
    match body.state.as_deref() {
        None => {}
        Some("closed") if !pr.closed => {
            let e = ends(&state, &dir, &pr)?;
            let shas = match (e.head.as_deref(), e.base.as_deref()) {
                (Some(h), Some(b)) => Some((h, b)),
                _ => None,
            };
            state.store.set_closed(pr.id, true, shas)?;
            notify(&state, "closed", &repo, &pr);
        }
        Some("open") if pr.closed => {
            let head_there = match pr.head_branch.as_deref() {
                Some(b) => state.git.commit_of(&dir, b)?.is_some(),
                None => false,
            };
            if !head_there {
                return Err(Fail::bad("The branch it came from is gone."));
            }
            state.store.set_closed(pr.id, false, None)?;
            notify(&state, "reopened", &repo, &pr);
        }
        Some("open" | "closed") => {}
        Some(other) => return Err(Fail::bad(format!("\"{other}\" is not open or closed."))),
    }
    let pr = state
        .store
        .issue(repo.id, pr.number)?
        .ok_or_else(|| Fail::not_found("pull request"))?;
    Ok(HttpResponse::Ok().json(pull_json(&state, &repo, &pr, p)?))
}

async fn pull_commits(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let (repo, pr, _, _) = find_pull(&req, &state, &path.0, &path.1, path.2, false)?;
    let list: Vec<Value> = pull_commit_list(&state, &repo, &pr)?
        .iter()
        .map(commit_json)
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

async fn pull_files(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let (repo, pr, _, _) = find_pull(&req, &state, &path.0, &path.1, path.2, false)?;
    let files = web::block({
        let state = state.clone();
        move || pull_diff(&state, &repo, &pr)
    })
    .await
    .map_err(|e| Fail::from(anyhow::anyhow!("{e}")))??;
    Ok(HttpResponse::Ok().json(files.iter().map(file_json).collect::<Vec<_>>()))
}

async fn list_reviews(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let (_, pr, _, _) = find_pull(&req, &state, &path.0, &path.1, path.2, false)?;
    let list: Vec<Value> = state
        .store
        .reviews(pr.id)?
        .iter()
        .map(review_json)
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

#[derive(Deserialize)]
struct NewLine {
    path: String,
    line: i64,
    #[serde(default = "new_side")]
    side: String,
    body: String,
}

fn new_side() -> String {
    "new".to_string()
}

#[derive(Deserialize)]
struct NewReview {
    /// `APPROVE`, `REQUEST_CHANGES` or `COMMENT`, Forgejo's words.
    event: String,
    #[serde(default)]
    body: String,
    /// The head the reviewer looked at; the current one when left out.
    #[serde(default)]
    commit_id: Option<String>,
    #[serde(default)]
    comments: Vec<NewLine>,
}

/// The lines a line comment may sit on: those the diff shows, per side.
fn commentable(files: &[FileDiff], path: &str, side: Side, line: i64) -> bool {
    let Some(f) = files.iter().find(|f| f.path == path) else {
        return false;
    };
    let Some(patch) = f.patch.as_deref() else {
        return false;
    };
    let (mut old, mut new) = (0i64, 0i64);
    for l in patch.lines() {
        if let Some(h) = l.strip_prefix("@@ ") {
            let mut parts = h.split_whitespace();
            let start = |p: Option<&str>| {
                p.and_then(|v| v[1..].split(',').next())
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(0)
            };
            old = start(parts.next());
            new = start(parts.next());
            continue;
        }
        match l.chars().next() {
            Some('+') => {
                if side == Side::New && new == line {
                    return true;
                }
                new += 1;
            }
            Some('-') => {
                if side == Side::Old && old == line {
                    return true;
                }
                old += 1;
            }
            Some('\\') => {}
            _ => {
                if (side == Side::New && new == line) || (side == Side::Old && old == line) {
                    return true;
                }
                old += 1;
                new += 1;
            }
        }
    }
    false
}

async fn create_review(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    body: web::Json<NewReview>,
    state: web::Data<State>,
) -> Answer {
    let (repo, pr, c, _) = find_pull(&req, &state, &path.0, &path.1, path.2, true)?;
    let user = need_person(&c)?.clone();
    let review_state = match body.event.as_str() {
        "APPROVE" => ReviewState::Approved,
        "REQUEST_CHANGES" => ReviewState::ChangesRequested,
        "COMMENT" => ReviewState::Commented,
        other => {
            return Err(Fail::bad(format!(
                "\"{other}\" is not APPROVE, REQUEST_CHANGES or COMMENT."
            )))
        }
    };
    if review_state != ReviewState::Commented && user.login == pr.poster {
        return Err(Fail::bad(
            "You cannot approve or ask for changes on your own pull request; comment instead.",
        ));
    }
    if review_state == ReviewState::ChangesRequested
        && body.body.trim().is_empty()
        && body.comments.is_empty()
    {
        return Err(Fail::bad("Say what should change."));
    }
    if review_state == ReviewState::Commented
        && body.body.trim().is_empty()
        && body.comments.is_empty()
    {
        return Err(Fail::bad("A review of only comments needs a comment."));
    }
    if pr.closed {
        return Err(Fail::bad("The pull request is closed."));
    }
    text("review", &body.body, BODY_MAX)?;
    let dir = repo_dir(&state, &repo);
    let head = ends(&state, &dir, &pr)?
        .head
        .ok_or_else(|| Fail::bad("The branch it came from is gone."))?;
    if let Some(seen) = &body.commit_id {
        if seen != &head {
            return Err(Fail::new(
                actix_web::http::StatusCode::CONFLICT,
                "New commits were pushed while you reviewed. Look at them, then send the review.",
            ));
        }
    }
    let mut lines = Vec::new();
    if !body.comments.is_empty() {
        let files = pull_diff(&state, &repo, &pr)?;
        for l in &body.comments {
            let side = Side::parse(&l.side)
                .ok_or_else(|| Fail::bad(format!("\"{}\" is not old or new.", l.side)))?;
            if l.body.trim().is_empty() {
                return Err(Fail::bad("A line comment is empty."));
            }
            text("comment", &l.body, BODY_MAX)?;
            if !commentable(&files, &l.path, side, l.line) {
                return Err(Fail::bad(format!(
                    "Line {} of {} is not in the diff.",
                    l.line, l.path
                )));
            }
            lines.push(NewLineComment {
                path: l.path.clone(),
                line: l.line,
                side,
                body: l.body.clone(),
            });
        }
    }
    let review =
        state
            .store
            .create_review(pr.id, user.id, review_state, &body.body, &head, &lines)?;
    notify(&state, "reviewed", &repo, &pr);
    Ok(HttpResponse::Created().json(review_json(&review)))
}

async fn list_line_comments(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    state: web::Data<State>,
) -> Answer {
    let (_, pr, _, _) = find_pull(&req, &state, &path.0, &path.1, path.2, false)?;
    let list: Vec<Value> = state
        .store
        .review_comments(pr.id)?
        .iter()
        .map(line_json)
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

#[derive(Deserialize)]
struct Reply {
    in_reply_to: i64,
    body: String,
}

async fn reply_line_comment(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    body: web::Json<Reply>,
    state: web::Data<State>,
) -> Answer {
    let (_, pr, c, _) = find_pull(&req, &state, &path.0, &path.1, path.2, true)?;
    let user = need_person(&c)?;
    if body.body.trim().is_empty() {
        return Err(Fail::bad("The reply is empty."));
    }
    text("reply", &body.body, BODY_MAX)?;
    let parent = state
        .store
        .review_comment(body.in_reply_to)?
        .filter(|p| p.issue_id == pr.id)
        .ok_or_else(|| Fail::not_found("comment"))?;
    let reply = state
        .store
        .reply_review_comment(parent.id, user.id, &body.body)?;
    Ok(HttpResponse::Created().json(line_json(&reply)))
}

#[derive(Deserialize)]
struct Resolve {
    resolved: bool,
}

async fn resolve_line_comment(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    body: web::Json<Resolve>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "issue", true)?;
    let user = need_person(&c)?.clone();
    let (repo, p) = find_repo(&state, &c, &path.0, &path.1)?;
    let comment = state
        .store
        .review_comment(path.2)?
        .ok_or_else(|| Fail::not_found("comment"))?;
    let pr = state
        .store
        .issue_by_id(comment.issue_id)?
        .filter(|i| i.repo_id == repo.id)
        .ok_or_else(|| Fail::not_found("comment"))?;
    if p < Permission::Write && user.login != pr.poster && user.login != comment.poster {
        return Err(Fail::forbidden(
            "Only the pull request's author, the comment's and people who may push can resolve it.",
        ));
    }
    state
        .store
        .set_resolved(comment.id, Some(user.id), body.resolved)?;
    let c = state
        .store
        .review_comment(comment.reply_to.unwrap_or(comment.id))?
        .ok_or_else(|| Fail::not_found("comment"))?;
    Ok(HttpResponse::Ok().json(line_json(&c)))
}

#[derive(Deserialize)]
struct Reviewers {
    reviewers: Vec<String>,
}

async fn request_reviewers(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    body: web::Json<Reviewers>,
    state: web::Data<State>,
) -> Answer {
    let (repo, pr, c, p) = find_pull(&req, &state, &path.0, &path.1, path.2, true)?;
    let user = need_person(&c)?.clone();
    if user.login != pr.poster && p < Permission::Write {
        return Err(Fail::forbidden(
            "Only its author and people who may push can ask for reviews.",
        ));
    }
    if pr.closed {
        return Err(Fail::bad("The pull request is closed."));
    }
    let mut asked = Vec::new();
    for login in &body.reviewers {
        let who = state
            .store
            .user(login)?
            .ok_or_else(|| Fail::bad(format!("There is no account {login}.")))?;
        if who.login == pr.poster {
            return Err(Fail::bad(
                "The author cannot review their own pull request.",
            ));
        }
        if who.login == super::reviewer::worker::BOT_LOGIN {
            // The box's own reviewer reads every repository that asked for
            // it, and none other.
            if !state.reviewer.available() {
                return Err(Fail::bad(
                    "LosOS Reviewer needs the box's code model: turn on Build with AI.",
                ));
            }
            if !state.store.ai_review(repo.id)? {
                return Err(Fail::bad(
                    "AI review is off for this repository. Turn it on in its settings.",
                ));
            }
            asked.push(who);
            continue;
        }
        if Caller::person(who.clone()).permission(&state.store, &repo)? == Permission::None {
            return Err(Fail::bad(format!(
                "{} cannot see this repository.",
                who.login
            )));
        }
        asked.push(who);
    }
    for who in &asked {
        state.store.request_review(pr.id, who.id, Some(user.id))?;
        notify_review_requested(&state, &repo, &pr, who);
    }
    Ok(HttpResponse::Ok().json(pull_json(&state, &repo, &pr, p)?))
}

/// Someone was asked to review: the code reviewer starts on it when it is
/// the one asked.
pub fn notify_review_requested(state: &State, repo: &Repo, pr: &Issue, who: &User) {
    tracing::info!(
        repo = format!("{}/{}", repo.owner, repo.name),
        number = pr.number,
        reviewer = who.login.as_str(),
        "review requested"
    );
    let head = pr.head_sha.clone().or_else(|| {
        pr.head_branch.as_deref().and_then(|b| {
            state
                .git
                .commit_of(&repo_dir(state, repo), b)
                .ok()
                .flatten()
        })
    });
    if let Some(head) = head {
        super::reviewer::worker::review_requested(state, repo.id, pr.id, &head, who.id);
    }
}

async fn unrequest_reviewer(
    req: HttpRequest,
    path: web::Path<(String, String, i64, String)>,
    state: web::Data<State>,
) -> Answer {
    let (repo, pr, c, p) = find_pull(&req, &state, &path.0, &path.1, path.2, true)?;
    let user = need_person(&c)?;
    if user.login != pr.poster && p < Permission::Write && !user.login.eq_ignore_ascii_case(&path.3)
    {
        return Err(Fail::forbidden(
            "Only its author and people who may push can take back a request.",
        ));
    }
    if let Some(who) = state.store.user(&path.3)? {
        state.store.drop_review_request(pr.id, who.id)?;
    }
    Ok(HttpResponse::Ok().json(pull_json(&state, &repo, &pr, p)?))
}

#[derive(Deserialize)]
struct Merge {
    /// `merge` (the default), `squash` or `fast-forward`.
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    message: Option<String>,
    /// The head the person saw; the merge is refused if it moved.
    #[serde(default)]
    head_commit_id: Option<String>,
    #[serde(default)]
    delete_branch: bool,
}

async fn merge_pull(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    body: web::Json<Merge>,
    state: web::Data<State>,
) -> Answer {
    let (repo, pr, c, p) = find_pull(&req, &state, &path.0, &path.1, path.2, true)?;
    let user = need_person(&c)?.clone();
    if p < Permission::Write || c.repo_access(&state.store, &repo)? < Permission::Write {
        return Err(Fail::forbidden("Only people who may push can merge."));
    }
    if pr.merged {
        return Err(Fail::new(
            actix_web::http::StatusCode::CONFLICT,
            "The pull request is merged already.",
        ));
    }
    if pr.closed {
        return Err(Fail::bad("The pull request is closed. Reopen it first."));
    }
    let style = body.style.as_deref().unwrap_or("merge");
    if !matches!(style, "merge" | "squash" | "fast-forward") {
        return Err(Fail::bad(format!(
            "\"{style}\" is not merge, squash or fast-forward."
        )));
    }
    if let Some(m) = &body.message {
        text("message", m, BODY_MAX)?;
    }
    let dir = repo_dir(&state, &repo);
    let state2 = state.clone();
    let repo2 = repo.clone();
    let pr2 = pr.clone();
    let style = style.to_string();
    let message = body.message.clone();
    let seen = body.head_commit_id.clone();
    let delete_branch = body.delete_branch;
    let merged = web::block(move || {
        merge(
            &state2,
            &repo2,
            &dir,
            &pr2,
            &user,
            &style,
            message.as_deref(),
            seen.as_deref(),
            delete_branch,
        )
    })
    .await
    .map_err(|e| Fail::from(anyhow::anyhow!("{e}")))??;
    let pr = state
        .store
        .issue(repo.id, pr.number)?
        .ok_or_else(|| Fail::not_found("pull request"))?;
    notify(&state, "merged", &repo, &pr);
    let mut doc = pull_json(&state, &repo, &pr, p)?;
    doc["sha"] = json!(merged);
    Ok(HttpResponse::Ok().json(doc))
}

/// The merge itself, off the request thread. Returns the base's new commit.
#[allow(clippy::too_many_arguments)]
fn merge(
    state: &State,
    repo: &Repo,
    dir: &std::path::Path,
    pr: &Issue,
    user: &User,
    style: &str,
    message: Option<&str>,
    seen: Option<&str>,
    delete_branch: bool,
) -> Result<String, Fail> {
    let (Some(head_branch), Some(base_branch)) =
        (pr.head_branch.as_deref(), pr.base_branch.as_deref())
    else {
        return Err(Fail::bad("The pull request names no branches."));
    };
    let head = state
        .git
        .commit_of(dir, head_branch)?
        .ok_or_else(|| Fail::bad("The branch it came from is gone."))?;
    let base = state
        .git
        .commit_of(dir, base_branch)?
        .ok_or_else(|| Fail::bad(format!("The branch {base_branch} is gone.")))?;
    if seen.is_some_and(|s| s != head) {
        return Err(Fail::new(
            actix_web::http::StatusCode::CONFLICT,
            "New commits were pushed. Look at them, then merge.",
        ));
    }
    if state.git.is_ancestor(dir, &head, &base)? {
        state
            .store
            .mark_merged(pr.id, &base, &head, &base, Some(user.id))?;
        return Ok(base);
    }
    let who = signature(user);
    let title = format!("{} (#{})", pr.title, pr.number);
    let new = match style {
        "fast-forward" => {
            if !state.git.is_ancestor(dir, &base, &head)? {
                return Err(Fail::bad(format!(
                    "{base_branch} has commits {head_branch} lacks, so it cannot fast-forward. Merge or squash instead."
                )));
            }
            head.clone()
        }
        _ => {
            let m = state.git.merge_tree(dir, &base, &head)?;
            if !m.conflicts.is_empty() {
                return Err(Fail::new(
                    actix_web::http::StatusCode::CONFLICT,
                    format!(
                        "These files conflict: {}. Merge {base_branch} into {head_branch} and push.",
                        m.conflicts.join(", ")
                    ),
                ));
            }
            if style == "squash" {
                let mut text = message.map(str::to_string).unwrap_or_else(|| {
                    let mut t = title.clone();
                    if let Ok(commits) = state.git.log(dir, Some(&base), &head, COMMIT_LIMIT) {
                        if !commits.is_empty() {
                            t.push_str("\n\n");
                            for c in commits.iter().rev() {
                                t.push_str(&format!("* {}\n", c.subject));
                            }
                        }
                    }
                    t
                });
                if text.trim().is_empty() {
                    text = title.clone();
                }
                state.git.commit_tree(dir, &m.tree, &[&base], &text, &who)?
            } else {
                let text = message.map(str::to_string).unwrap_or_else(|| {
                    format!(
                        "Merge pull request #{} from {head_branch}\n\n{}",
                        pr.number, pr.title
                    )
                });
                state
                    .git
                    .commit_tree(dir, &m.tree, &[&base, &head], &text, &who)?
            }
        }
    };
    state
        .git
        .move_branch(dir, base_branch, &new, &base)
        .map_err(|_| {
            Fail::new(
                actix_web::http::StatusCode::CONFLICT,
                format!("{base_branch} moved while merging. Try again."),
            )
        })?;
    state
        .store
        .mark_merged(pr.id, &new, &head, &base, Some(user.id))?;
    state.store.touch_repo(repo.id)?;
    if delete_branch && head_branch != repo.default_branch {
        if let Err(e) = state.git.delete_branch(dir, head_branch, &head) {
            tracing::warn!(error = ?e, "could not delete the merged branch");
        }
    }
    tracing::info!(
        repo = format!("{}/{}", repo.owner, repo.name),
        number = pr.number,
        style,
        by = user.login.as_str(),
        "merged"
    );
    Ok(new)
}

#[derive(Deserialize)]
struct NewComment {
    body: String,
}

/// A comment on an issue or a pull request's conversation.
async fn create_comment(
    req: HttpRequest,
    path: web::Path<(String, String, i64)>,
    body: web::Json<NewComment>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "issue", true)?;
    let user = need_person(&c)?.clone();
    let (repo, _) = find_repo(&state, &c, &path.0, &path.1)?;
    let issue = state
        .store
        .issue(repo.id, path.2)?
        .ok_or_else(|| Fail::not_found("issue"))?;
    if body.body.trim().is_empty() {
        return Err(Fail::bad("The comment is empty."));
    }
    text("comment", &body.body, BODY_MAX)?;
    let comment = state.store.add_comment(issue.id, user.id, &body.body)?;
    Ok(HttpResponse::Created().json(json!({
        "id": comment.id,
        "user": { "login": comment.poster },
        "body": comment.body,
        "created_at": comment.created,
        "updated_at": comment.updated,
    })))
}

/// `base...head`: what a pull request from `head` into `base` would hold,
/// for the page that opens one.
async fn compare(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", false)?;
    let (repo, _) = find_repo(&state, &c, &path.0, &path.1)?;
    let (base, head) = path
        .2
        .split_once("...")
        .ok_or_else(|| Fail::bad("Compare two branches as base...head."))?;
    for b in [base, head] {
        super::names::branch(b).map_err(Fail::bad)?;
    }
    let dir = repo_dir(&state, &repo);
    let base_sha = state
        .git
        .commit_of(&dir, base)?
        .ok_or_else(|| Fail::not_found("branch"))?;
    let head_sha = state
        .git
        .commit_of(&dir, head)?
        .ok_or_else(|| Fail::not_found("branch"))?;
    let (base_s, head_s) = (base.to_string(), head.to_string());
    let doc = web::block(move || -> anyhow::Result<Value> {
        let merge_base = state.git.merge_base(&dir, &base_sha, &head_sha)?;
        let (ahead, behind) = state.git.ahead_behind(&dir, &base_sha, &head_sha)?;
        let mut commits = state
            .git
            .log(&dir, Some(&base_sha), &head_sha, COMMIT_LIMIT)?;
        commits.reverse();
        let files = state
            .git
            .diff(&dir, merge_base.as_deref().unwrap_or(&base_sha), &head_sha)?;
        let mergeable = if merge_base.is_some() {
            state
                .git
                .merge_tree(&dir, &base_sha, &head_sha)?
                .conflicts
                .is_empty()
        } else {
            false
        };
        Ok(json!({
            "base": { "ref": base_s, "sha": base_sha },
            "head": { "ref": head_s, "sha": head_sha },
            "merge_base": merge_base,
            "ahead_by": ahead,
            "behind_by": behind,
            "mergeable": mergeable,
            "commits": commits.iter().map(commit_json).collect::<Vec<_>>(),
            "files": files.iter().map(file_json).collect::<Vec<_>>(),
        }))
    })
    .await
    .map_err(|e| Fail::from(anyhow::anyhow!("{e}")))??;
    Ok(HttpResponse::Ok().json(doc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(patch: &str) -> FileDiff {
        FileDiff {
            path: "a.txt".into(),
            old_path: None,
            status: "modified",
            additions: 0,
            deletions: 0,
            binary: false,
            patch: Some(patch.into()),
            too_large: false,
        }
    }

    #[test]
    fn only_lines_the_diff_shows_take_comments() {
        let f = [file(
            "@@ -3,3 +3,4 @@ fn\n one\n-two\n+zwei\n+drei\n three\n",
        )];
        // Context: old 3 / new 3.
        assert!(commentable(&f, "a.txt", Side::New, 3));
        assert!(commentable(&f, "a.txt", Side::Old, 3));
        // The removed line is old 4, the two added are new 4 and 5.
        assert!(commentable(&f, "a.txt", Side::Old, 4));
        assert!(commentable(&f, "a.txt", Side::New, 4));
        assert!(commentable(&f, "a.txt", Side::New, 5));
        // Trailing context: old 5 / new 6.
        assert!(commentable(&f, "a.txt", Side::New, 6));
        assert!(!commentable(&f, "a.txt", Side::New, 7));
        assert!(!commentable(&f, "a.txt", Side::Old, 1));
        assert!(!commentable(&f, "b.txt", Side::New, 3));
    }
}
