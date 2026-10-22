//! The tracker's JSON API, under `/git/api/v1/tracker/`, for the Issues
//! pages. Only people signed in (and the administrator) use it; a visitor
//! gets 401. An issue is named by its key everywhere (`LOS-12`), a project
//! by its key (`LOS`).

use super::query::{Context, Filter, FilterQuery};
use super::store::{Actor, Change, NewTicket, Project, Ticket};
use super::{parse_key, valid_date, valid_project_key, Category};
use crate::forge::api::{need_scope, Answer};
use crate::forge::auth::{Caller, Who};
use crate::forge::db::{Permission, User};
use crate::forge::server::{self, Fail, State};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .route("/tracker", web::get().to(home))
        .route("/tracker/issues", web::get().to(list))
        .route("/tracker/issues", web::post().to(create))
        .route("/tracker/issues/bulk", web::post().to(bulk))
        .route("/tracker/calendar.ics", web::get().to(my_calendar))
        .route("/tracker/issues/{key}", web::get().to(get))
        .route("/tracker/issues/{key}", web::patch().to(edit))
        .route("/tracker/issues/{key}", web::delete().to(delete))
        .route("/tracker/issues/{key}/comments", web::post().to(comment))
        .route("/tracker/issues/{key}/relations", web::post().to(relate))
        .route(
            "/tracker/issues/{key}/subscription",
            web::put().to(subscribe),
        )
        .route(
            "/tracker/issues/{key}/subscription",
            web::delete().to(unsubscribe),
        )
        .route(
            "/tracker/issues/{key}/calendar.ics",
            web::get().to(issue_calendar),
        )
        .route("/tracker/issues/{key}/ai", web::post().to(ai))
        .route("/tracker/comments/{id}", web::patch().to(edit_comment))
        .route("/tracker/comments/{id}", web::delete().to(delete_comment))
        .route("/tracker/relations/{id}", web::delete().to(unrelate))
        .route("/tracker/projects", web::post().to(create_project))
        .route("/tracker/projects/{key}", web::get().to(project))
        .route("/tracker/projects/{key}", web::patch().to(edit_project))
        .route(
            "/tracker/projects/{key}/members/{login}",
            web::put().to(put_member),
        )
        .route(
            "/tracker/projects/{key}/members/{login}",
            web::delete().to(delete_member),
        )
        .route("/tracker/repos/{owner}/{repo}", web::get().to(repo_project))
        .route(
            "/tracker/projects/{key}/repos/{owner}/{repo}",
            web::put().to(link_repo),
        )
        .route(
            "/tracker/projects/{key}/repos/{owner}/{repo}",
            web::delete().to(unlink_repo),
        )
        .route("/tracker/projects/{key}/states", web::post().to(add_state))
        .route("/tracker/projects/{key}/labels", web::post().to(add_label))
        .route("/tracker/projects/{key}/cycles", web::post().to(add_cycle))
        .route("/tracker/states/{id}", web::patch().to(edit_state))
        .route("/tracker/states/{id}", web::delete().to(delete_state))
        .route("/tracker/labels/{id}", web::patch().to(edit_label))
        .route("/tracker/labels/{id}", web::delete().to(delete_label))
        .route("/tracker/cycles/{id}", web::patch().to(edit_cycle))
        .route("/tracker/cycles/{id}", web::delete().to(delete_cycle))
        .route("/tracker/views", web::get().to(views))
        .route("/tracker/views", web::post().to(save_view))
        .route("/tracker/views/{id}", web::patch().to(update_view))
        .route("/tracker/views/{id}", web::delete().to(delete_view))
        .route("/tracker/inbox", web::get().to(inbox))
        .route("/tracker/inbox/read", web::post().to(read_inbox))
}

// ── Who is asking ───────────────────────────────────────────────────────────

/// Someone signed in, or the administrator.
fn caller(req: &HttpRequest, state: &State, write: bool) -> Result<Caller, Fail> {
    let c = server::caller(req, state)?;
    if matches!(c.who, Who::Nobody) {
        return Err(Fail::unauthorized("Sign in to use the issues."));
    }
    need_scope(&c, "issue", write)?;
    Ok(c)
}

fn actor(c: &Caller) -> Actor {
    match c.user() {
        Some(u) => Actor::of(u),
        None => Actor::named("LosOS"),
    }
}

fn me(c: &Caller) -> Option<&User> {
    c.user()
}

/// The projects `c` may read, with what it may do in each.
fn readable(state: &State, c: &Caller) -> Result<Vec<(Project, Permission)>, Fail> {
    let mut out = Vec::new();
    for p in state.store.projects()? {
        let perm = state.store.project_permission(c, &p)?;
        if perm >= Permission::Read {
            out.push((p, perm));
        }
    }
    Ok(out)
}

fn find_project(
    state: &State,
    c: &Caller,
    key: &str,
    need: Permission,
) -> Result<(Project, Permission), Fail> {
    let p = state
        .store
        .project(key)?
        .ok_or_else(|| Fail::not_found("project"))?;
    let perm = state.store.project_permission(c, &p)?;
    if perm == Permission::None {
        return Err(Fail::not_found("project"));
    }
    if perm < need {
        return Err(Fail::forbidden(match need {
            Permission::Admin => "Only the project's administrators may do this.",
            _ => "You may read this project but not change it.",
        }));
    }
    Ok((p, perm))
}

fn find_ticket(
    state: &State,
    c: &Caller,
    key: &str,
    need: Permission,
) -> Result<(Ticket, Project, Permission), Fail> {
    let (k, n) = parse_key(key).ok_or_else(|| Fail::not_found("issue"))?;
    let t = state
        .store
        .ticket(&k, n)?
        .ok_or_else(|| Fail::not_found("issue"))?;
    let p = state
        .store
        .project_by_id(t.project_id)?
        .ok_or_else(|| Fail::not_found("issue"))?;
    let perm = state.store.project_permission(c, &p)?;
    if perm == Permission::None {
        return Err(Fail::not_found("issue"));
    }
    if perm < need {
        return Err(Fail::forbidden(
            "You may read this issue but not change it.",
        ));
    }
    Ok((t, p, perm))
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn base(state: &State) -> String {
    crate::forge::api::base(state)
}

// ── Shapes ──────────────────────────────────────────────────────────────────

fn ticket_json(state: &State, t: &Ticket) -> Value {
    let mut v = serde_json::to_value(t).unwrap_or(Value::Null);
    v["url"] = json!(format!("{}/issues/{}", base(state), t.key));
    v
}

fn project_json(state: &State, p: &Project, perm: Permission) -> Result<Value, Fail> {
    let mut v = serde_json::to_value(p).unwrap_or(Value::Null);
    let mut repos = Vec::new();
    for id in state.store.project_repo_ids(p.id)? {
        if let Some(r) = state.store.repo_by_id(id)? {
            repos.push(
                json!({ "owner": r.owner, "name": r.name, "defaultBranch": r.default_branch }),
            );
        }
    }
    v["repos"] = json!(repos);
    v["permission"] = json!(perm.as_str());
    v["states"] = json!(state.store.workflow(p.id)?);
    v["labels"] = json!(state.store.project_labels(p.id)?);
    v["cycles"] = json!(state.store.cycles(p.id)?);
    Ok(v)
}

/// People one may give an issue to: the box's accounts that are people.
fn people(state: &State) -> Result<Vec<Value>, Fail> {
    Ok(state
        .store
        .users()?
        .into_iter()
        .filter(|u| !u.is_bot)
        .map(|u| json!({ "login": u.login, "fullName": u.full_name }))
        .collect())
}

// ── Home, lists ─────────────────────────────────────────────────────────────

async fn home(req: HttpRequest, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, false)?;
    let mut projects = Vec::new();
    let all = readable(&state, &c)?;
    let ids: Vec<i64> = all.iter().map(|(p, _)| p.id).collect();
    let tickets = state.store.tickets_of(&ids)?;
    for (p, perm) in &all {
        let mut v = project_json(&state, p, *perm)?;
        let mine: Vec<&Ticket> = tickets.iter().filter(|t| t.project_id == p.id).collect();
        v["counts"] = json!({
            "total": mine.len(),
            "open": mine.iter().filter(|t| !t.state.category.closed()).count(),
            "started": mine.iter().filter(|t| t.state.category == Category::Started).count(),
        });
        projects.push(v);
    }
    let (views, unread, assigned) = match me(&c) {
        Some(u) => (
            state.store.views_for(u.id)?,
            state.store.unread(u.id)?,
            tickets
                .iter()
                .filter(|t| {
                    t.assignee.as_deref() == Some(u.login.as_str()) && !t.state.category.closed()
                })
                .count(),
        ),
        None => (Vec::new(), 0, 0),
    };
    Ok(HttpResponse::Ok().json(json!({
        "me": me(&c).map(|u| u.login.clone()),
        "projects": projects,
        "views": views,
        "unread": unread,
        "assigned": assigned,
        "people": people(&state)?,
        "ai": state.reviewer.available(),
        "today": today(),
    })))
}

async fn list(req: HttpRequest, query: web::Query<FilterQuery>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, false)?;
    let filter = query.into_inner().into_filter().map_err(Fail::bad)?;
    Ok(HttpResponse::Ok().json(json!({ "issues": filtered(&state, &c, &filter)? })))
}

fn filtered(state: &State, c: &Caller, filter: &Filter) -> Result<Vec<Value>, Fail> {
    let all = readable(state, c)?;
    let keys: HashMap<i64, String> = all.iter().map(|(p, _)| (p.id, p.key.clone())).collect();
    let ids: Vec<i64> = all
        .iter()
        .filter(|(p, _)| filter.project.is_empty() || filter.project.contains(&p.key))
        .map(|(p, _)| p.id)
        .collect();
    let mut cycles = Vec::new();
    for id in &ids {
        cycles.extend(state.store.cycles(*id)?);
    }
    let today = today();
    let cx = Context {
        me: me(c).map(|u| u.login.as_str()),
        today: &today,
        cycles: &cycles,
    };
    let mut list: Vec<Ticket> = state
        .store
        .tickets_of(&ids)?
        .into_iter()
        .filter(|t| filter.matches(t, keys.get(&t.project_id).map_or("", String::as_str), &cx))
        .collect();
    filter.sort(&mut list);
    Ok(list.iter().map(|t| ticket_json(state, t)).collect())
}

// ── One issue ───────────────────────────────────────────────────────────────

fn summary(state: &State, id: i64) -> Result<Option<Value>, Fail> {
    Ok(state.store.ticket_by_id(id)?.map(|t| {
        json!({
            "key": t.key, "title": t.title, "state": t.state, "priority": t.priority,
            "assignee": t.assignee, "url": format!("{}/issues/{}", base(state), t.key),
        })
    }))
}

async fn get(req: HttpRequest, path: web::Path<String>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, false)?;
    let (t, p, perm) = find_ticket(&state, &c, &path, Permission::Read)?;
    full(&state, &c, &t, &p, perm)
}

fn full(state: &State, c: &Caller, t: &Ticket, p: &Project, perm: Permission) -> Answer {
    let mut v = ticket_json(state, t);
    v["project"] = project_json(state, p, perm)?;
    v["comments"] = json!(state.store.ticket_comments(t.id)?);
    v["activity"] = json!(state.store.ticket_activity(t.id)?);
    let mut relations = Vec::new();
    for r in state.store.relations(t.id)? {
        // An issue of a project the reader cannot see is left out.
        if let Some(other) = state.store.ticket_by_id(r.other_id)? {
            if let Some(op) = state.store.project_by_id(other.project_id)? {
                if state.store.project_permission(c, &op)? >= Permission::Read {
                    relations.push(
                        json!({ "id": r.id, "kind": r.kind, "issue": summary(state, r.other_id)? }),
                    );
                }
            }
        }
    }
    v["relations"] = json!(relations);
    let links: Vec<Value> = state
        .store
        .links(t.id)?
        .into_iter()
        .map(|l| {
            let url = match l.kind.as_str() {
                "pull" => format!("{}/{}/{}/pulls/{}", base(state), l.owner, l.repo, l.target),
                _ => format!(
                    "{}/{}/{}/commits?sha={}",
                    base(state),
                    l.owner,
                    l.repo,
                    l.target
                ),
            };
            let mut j = serde_json::to_value(&l).unwrap_or(Value::Null);
            j["url"] = json!(url);
            j
        })
        .collect();
    v["links"] = json!(links);
    v["children"] = json!(state
        .store
        .children(t.id)?
        .iter()
        .map(|ch| ticket_json(state, ch))
        .collect::<Vec<_>>());
    if let Some(parent) = t.parent_id {
        v["parent"] = summary(state, parent)?.unwrap_or(Value::Null);
    }
    let subscribers = state.store.subscribers(t.id)?;
    v["subscribed"] =
        json!(me(c).is_some_and(|u| subscribers.iter().any(|s| s.eq_ignore_ascii_case(&u.login))));
    v["subscribers"] = json!(subscribers);
    v["permission"] = json!(perm.as_str());
    Ok(HttpResponse::Ok().json(v))
}

fn double<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(d)?))
}

/// A new issue or a change. Absent leaves a field; `null` clears it.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Fields {
    project: Option<String>,
    title: Option<String>,
    body: Option<String>,
    /// A state id.
    state: Option<i64>,
    /// The first state of a category: `done`, `started`, ...
    category: Option<String>,
    priority: Option<i64>,
    #[serde(deserialize_with = "double")]
    estimate: Option<Option<i64>>,
    /// A login, `me`, or `null`.
    #[serde(deserialize_with = "double")]
    assignee: Option<Option<String>>,
    #[serde(deserialize_with = "double")]
    cycle: Option<Option<i64>>,
    /// A key, or `null`.
    #[serde(deserialize_with = "double")]
    parent: Option<Option<String>>,
    #[serde(deserialize_with = "double")]
    due: Option<Option<String>>,
    sort: Option<f64>,
    /// Label names; a missing one is made.
    labels: Option<Vec<String>>,
    add_labels: Vec<String>,
    remove_labels: Vec<String>,
    /// `owner/name`: give a new issue that repository's next number too.
    /// Left out in a project of one repository, that one; `""`, none.
    repo: Option<String>,
}

fn change_of(
    state: &State,
    c: &Caller,
    project: &Project,
    f: &Fields,
    own: Option<i64>,
) -> Result<Change, Fail> {
    let mut change = Change::default();
    if let Some(t) = &f.title {
        let t = t.trim();
        if t.is_empty() {
            return Err(Fail::bad("An issue needs a title."));
        }
        if t.chars().count() > 255 {
            return Err(Fail::bad("A title is at most 255 characters."));
        }
        change.title = Some(t.to_string());
    }
    if let Some(b) = &f.body {
        if b.len() > 200_000 {
            return Err(Fail::bad("The description is too long."));
        }
        change.body = Some(b.clone());
    }
    let flow = state.store.workflow(project.id)?;
    if let Some(s) = f.state {
        if !flow.iter().any(|w| w.id == s) {
            return Err(Fail::bad("That state is not in this project's workflow."));
        }
        change.state_id = Some(s);
    } else if let Some(cat) = &f.category {
        let cat = Category::parse(cat)
            .ok_or_else(|| Fail::bad(format!("\"{cat}\" is not a category.")))?;
        let s = flow
            .iter()
            .find(|w| w.category == cat)
            .ok_or_else(|| Fail::bad("This project's workflow has no such state."))?;
        change.state_id = Some(s.id);
    }
    if let Some(p) = f.priority {
        if !(0..=4).contains(&p) {
            return Err(Fail::bad("A priority is 0 (none) to 4 (low)."));
        }
        change.priority = Some(p);
    }
    if let Some(e) = f.estimate {
        if e.is_some_and(|e| !(0..=1000).contains(&e)) {
            return Err(Fail::bad("An estimate is 0 to 1000 points."));
        }
        change.estimate = Some(e);
    }
    if let Some(a) = &f.assignee {
        change.assignee_id = Some(match a.as_deref() {
            None | Some("") => None,
            Some("me") => Some(
                me(c)
                    .ok_or_else(|| Fail::bad("The administrator is nobody to give it to."))?
                    .id,
            ),
            Some(login) => {
                let u = state
                    .store
                    .user(login)?
                    .filter(|u| !u.is_bot)
                    .ok_or_else(|| {
                        Fail::bad(format!("There is no one called {login} on this box."))
                    })?;
                Some(u.id)
            }
        });
    }
    if let Some(cy) = f.cycle {
        if let Some(id) = cy {
            let ok = state
                .store
                .cycle(id)?
                .is_some_and(|x| x.project_id == project.id);
            if !ok {
                return Err(Fail::bad("That cycle is not this project's."));
            }
        }
        change.cycle_id = Some(cy);
    }
    if let Some(parent) = &f.parent {
        change.parent_id = Some(match parent.as_deref() {
            None | Some("") => None,
            Some(key) => {
                let (pt, _, _) = find_ticket(state, c, key, Permission::Read)
                    .map_err(|_| Fail::bad(format!("There is no issue {key} to put it under.")))?;
                if Some(pt.id) == own {
                    return Err(Fail::bad("An issue cannot be under itself."));
                }
                Some(pt.id)
            }
        });
    }
    if let Some(d) = &f.due {
        if let Some(d) = d {
            if !valid_date(d) {
                return Err(Fail::bad("A due date is a day, YYYY-MM-DD."));
            }
        }
        change.due = Some(d.clone().filter(|d| !d.is_empty()));
    }
    change.sort = f.sort.filter(|s| s.is_finite());
    let label_ids = |names: &[String]| -> Result<Vec<i64>, Fail> {
        let mut out = Vec::new();
        for n in names {
            let n = n.trim();
            if n.is_empty() || n.chars().count() > 50 {
                return Err(Fail::bad("A label is 1 to 50 characters."));
            }
            out.push(state.store.ensure_label(project.id, n, "")?);
        }
        Ok(out)
    };
    if let Some(ls) = &f.labels {
        change.labels = Some(label_ids(ls)?);
    }
    change.add_labels = label_ids(&f.add_labels)?;
    // Removing a label the project lacks removes nothing.
    let known = state.store.project_labels(project.id)?;
    change.remove_labels = f
        .remove_labels
        .iter()
        .filter_map(|n| {
            known
                .iter()
                .find(|l| l.name.eq_ignore_ascii_case(n))
                .map(|l| l.id)
        })
        .collect();
    Ok(change)
}

async fn create(req: HttpRequest, body: web::Json<Fields>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, true)?;
    let key = body
        .project
        .as_deref()
        .ok_or_else(|| Fail::bad("Say which project the issue is in."))?;
    let (p, perm) = find_project(&state, &c, key, Permission::Read)?;
    // Anyone who may read a repository may open an issue on it, as on any
    // forge; what the issue gets beyond its words is for its writers.
    let linked = !state.store.project_repo_ids(p.id)?.is_empty();
    if perm < Permission::Write && !(linked && c.allows("issue", true)) {
        find_project(&state, &c, key, Permission::Write)?;
    }
    if body.title.as_deref().is_none_or(|t| t.trim().is_empty()) {
        return Err(Fail::bad("An issue needs a title."));
    }
    let words = Fields {
        project: body.project.clone(),
        title: body.title.clone(),
        body: body.body.clone(),
        repo: body.repo.clone(),
        ..Fields::default()
    };
    let body = if perm < Permission::Write {
        &words
    } else {
        &*body
    };
    let change = change_of(&state, &c, &p, body, None)?;
    let repo_id = match body.repo.as_deref() {
        // A project of one repository numbers its issues in it.
        None => match state.store.project_repo_ids(p.id)?.as_slice() {
            [only] => Some(*only),
            _ => None,
        },
        Some("") => None,
        Some(r) => {
            let (o, n) = r
                .split_once('/')
                .ok_or_else(|| Fail::bad("A repository is owner/name."))?;
            let repo = state
                .store
                .repo(o, n)?
                .ok_or_else(|| Fail::bad("No such repository."))?;
            if !state.store.project_repo_ids(p.id)?.contains(&repo.id) {
                return Err(Fail::bad("That repository is not linked to this project."));
            }
            Some(repo.id)
        }
    };
    let new = NewTicket {
        title: change.title.clone().unwrap_or_default(),
        body: change.body.clone().unwrap_or_default(),
        state_id: change.state_id,
        priority: change.priority.unwrap_or(0),
        estimate: change.estimate.flatten(),
        assignee_id: change.assignee_id.flatten(),
        labels: change
            .labels
            .clone()
            .unwrap_or_default()
            .into_iter()
            .chain(change.add_labels.clone())
            .collect(),
        cycle_id: change.cycle_id.flatten(),
        parent_id: change.parent_id.flatten(),
        due: change.due.clone().flatten(),
        repo_id,
    };
    let who = actor(&c);
    let t = state.store.create_ticket(p.id, &new, &who)?;
    super::announce(&state.store, &t, "opened", &who.name);
    let perm = state.store.project_permission(&c, &p)?;
    let mut answer = full(&state, &c, &t, &p, perm)?;
    *answer.status_mut() = actix_web::http::StatusCode::CREATED;
    Ok(answer)
}

fn apply(state: &State, c: &Caller, t: &Ticket, p: &Project, f: &Fields) -> Result<(), Fail> {
    let change = change_of(state, c, p, f, Some(t.id))?;
    if !change.is_empty() {
        let who = actor(c);
        state
            .store
            .change_ticket(t.id, &change, &who)
            .map_err(bad_change)?;
        if let Some(now) = state.store.ticket_by_id(t.id)? {
            if let Some(action) = super::closing(t.state.category, now.state.category) {
                super::announce(&state.store, &now, action, &who.name);
            }
        }
    }
    if let Some(to) = f.project.as_deref() {
        if !to.eq_ignore_ascii_case(&p.key) {
            let (target, _) = find_project(state, c, to, Permission::Write)?;
            state.store.move_ticket(t.id, target.id, &actor(c))?;
        }
    }
    Ok(())
}

/// A change the store refused because of what it asked for.
fn bad_change(e: anyhow::Error) -> Fail {
    let text = e.to_string();
    if text.contains("cannot be under itself")
        || text.contains("another project")
        || text.contains("YYYY")
    {
        Fail::bad(text)
    } else {
        e.into()
    }
}

async fn edit(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<Fields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (t, p, _) = find_ticket(&state, &c, &path, Permission::Write)?;
    apply(&state, &c, &t, &p, &body)?;
    let t = state
        .store
        .ticket_by_id(t.id)?
        .ok_or_else(|| Fail::not_found("issue"))?;
    let p = state
        .store
        .project_by_id(t.project_id)?
        .ok_or_else(|| Fail::not_found("project"))?;
    let perm = state.store.project_permission(&c, &p)?;
    full(&state, &c, &t, &p, perm)
}

#[derive(Deserialize)]
struct Bulk {
    keys: Vec<String>,
    change: Fields,
}

async fn bulk(req: HttpRequest, body: web::Json<Bulk>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, true)?;
    if body.keys.len() > 500 {
        return Err(Fail::bad("At most 500 issues at a time."));
    }
    let mut changed = Vec::new();
    for key in &body.keys {
        let (t, p, _) = find_ticket(&state, &c, key, Permission::Write)?;
        // A state or label is named by category or name, which every
        // project has its own of.
        apply(&state, &c, &t, &p, &body.change)?;
        changed.push(key.clone());
    }
    Ok(HttpResponse::Ok().json(json!({ "changed": changed })))
}

async fn delete(req: HttpRequest, path: web::Path<String>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, true)?;
    let (t, _, _) = find_ticket(&state, &c, &path, Permission::Admin)?;
    state.store.delete_ticket(t.id)?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize)]
struct Text {
    body: String,
}

async fn comment(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<Text>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (t, _, _) = find_ticket(&state, &c, &path, Permission::Read)?;
    // Anyone who may read an issue may say something on it.
    if body.body.trim().is_empty() {
        return Err(Fail::bad("Write something first."));
    }
    if body.body.len() > 100_000 {
        return Err(Fail::bad("The comment is too long."));
    }
    let who = actor(&c);
    let comment = state.store.comment_ticket(t.id, &body.body, &who)?;
    super::announce(&state.store, &t, "commented", &who.name);
    Ok(HttpResponse::Created().json(comment))
}

fn own_comment(state: &State, c: &Caller, id: i64) -> Result<(), Fail> {
    let (ticket_id, author) = state
        .store
        .ticket_comment_owner(id)?
        .ok_or_else(|| Fail::not_found("comment"))?;
    let t = state
        .store
        .ticket_by_id(ticket_id)?
        .ok_or_else(|| Fail::not_found("comment"))?;
    let p = state
        .store
        .project_by_id(t.project_id)?
        .ok_or_else(|| Fail::not_found("comment"))?;
    let perm = state.store.project_permission(c, &p)?;
    if perm == Permission::None {
        return Err(Fail::not_found("comment"));
    }
    if author != me(c).map(|u| u.id) && perm < Permission::Admin {
        return Err(Fail::forbidden("Only who wrote a comment may change it."));
    }
    Ok(())
}

async fn edit_comment(
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<Text>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    own_comment(&state, &c, *path)?;
    if body.body.trim().is_empty() {
        return Err(Fail::bad("A comment cannot be empty."));
    }
    state.store.edit_ticket_comment(*path, &body.body)?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_comment(req: HttpRequest, path: web::Path<i64>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, true)?;
    own_comment(&state, &c, *path)?;
    state.store.delete_ticket_comment(*path)?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize)]
struct NewRelation {
    kind: String,
    other: String,
}

async fn relate(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<NewRelation>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (t, _, _) = find_ticket(&state, &c, &path, Permission::Write)?;
    let (other, _, _) = find_ticket(&state, &c, &body.other, Permission::Read)
        .map_err(|_| Fail::bad(format!("There is no issue {}.", body.other)))?;
    if other.id == t.id {
        return Err(Fail::bad("An issue cannot be related to itself."));
    }
    if !["blocks", "blocked_by", "relates", "duplicates"].contains(&body.kind.as_str()) {
        return Err(Fail::bad(
            "A relation is blocks, blocked_by, relates or duplicates.",
        ));
    }
    state.store.relate(t.id, &body.kind, other.id, &actor(&c))?;
    Ok(HttpResponse::NoContent().finish())
}

async fn unrelate(req: HttpRequest, path: web::Path<i64>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, true)?;
    let (from, _) = state
        .store
        .relation_ends(*path)?
        .ok_or_else(|| Fail::not_found("relation"))?;
    let t = state
        .store
        .ticket_by_id(from)?
        .ok_or_else(|| Fail::not_found("relation"))?;
    let p = state
        .store
        .project_by_id(t.project_id)?
        .ok_or_else(|| Fail::not_found("relation"))?;
    if state.store.project_permission(&c, &p)? < Permission::Write {
        return Err(Fail::not_found("relation"));
    }
    state.store.unrelate(*path)?;
    Ok(HttpResponse::NoContent().finish())
}

async fn subscribe(req: HttpRequest, path: web::Path<String>, state: web::Data<State>) -> Answer {
    follow(req, path, state, true).await
}

async fn unsubscribe(req: HttpRequest, path: web::Path<String>, state: web::Data<State>) -> Answer {
    follow(req, path, state, false).await
}

async fn follow(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<State>,
    on: bool,
) -> Answer {
    let c = caller(&req, &state, false)?;
    let u = me(&c)
        .ok_or_else(|| Fail::bad("The administrator follows nothing."))?
        .id;
    let (t, _, _) = find_ticket(&state, &c, &path, Permission::Read)?;
    state.store.set_subscribed(t.id, u, on)?;
    Ok(HttpResponse::NoContent().finish())
}

fn ics(body: String, name: &str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/calendar; charset=utf-8")
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{name}.ics\""),
        ))
        .body(body)
}

fn host(req: &HttpRequest) -> String {
    req.connection_info().host().to_string()
}

async fn issue_calendar(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, false)?;
    let (t, _, _) = find_ticket(&state, &c, &path, Permission::Read)?;
    if t.due.is_none() {
        return Err(Fail::not_found("due date"));
    }
    let body = super::ics::calendar(std::slice::from_ref(&t), &t.key, &host(&req), &base(&state));
    Ok(ics(body, &t.key))
}

/// Everything given to the caller that has a due date: a calendar to
/// subscribe to with a token.
async fn my_calendar(req: HttpRequest, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, false)?;
    let login = me(&c).map(|u| u.login.clone()).unwrap_or_default();
    let filter = Filter {
        assignee: vec!["me".into()],
        due: Some("any".into()),
        ..Filter::default()
    };
    let all = readable(&state, &c)?;
    let ids: Vec<i64> = all.iter().map(|(p, _)| p.id).collect();
    let today = today();
    let cx = Context {
        me: Some(&login),
        today: &today,
        cycles: &[],
    };
    let list: Vec<Ticket> = state
        .store
        .tickets_of(&ids)?
        .into_iter()
        .filter(|t| filter.matches(t, "", &cx))
        .collect();
    let body = super::ics::calendar(&list, "LosOS Git issues", &host(&req), &base(&state));
    Ok(ics(body, "issues"))
}

#[derive(Deserialize)]
struct AiAsk {
    /// `breakdown`: sub-issues for it; `summary`: what was said, short.
    task: String,
}

/// The box's code model on one issue. It suggests; the person decides.
async fn ai(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<AiAsk>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, false)?;
    let (t, _, _) = find_ticket(&state, &c, &path, Permission::Read)?;
    let Some(config) = state.reviewer.config.clone() else {
        return Err(Fail::new(
            actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
            "The box's AI is off. Turn on Build with AI to use it here.",
        ));
    };
    let comments = state.store.ticket_comments(t.id)?;
    let (system, user, max) = match body.task.as_str() {
        "breakdown" => (
            "You split one task into smaller steps someone can do one at a time. \
             Answer with three to six steps, one per line, each line starting with \"- \". \
             Each step is a short imperative sentence. Write nothing else."
                .to_string(),
            format!("Task: {}\n\n{}", t.title, trim_to(&t.body, 3000)),
            200,
        ),
        "summary" => {
            let mut talk = format!("Issue: {}\n\n{}\n", t.title, trim_to(&t.body, 2000));
            for cm in comments.iter().rev().take(20).rev() {
                talk.push_str(&format!("\n{}: {}\n", cm.author, trim_to(&cm.body, 600)));
            }
            (
                "You summarise an issue and its discussion for someone who has not read it. \
                 Answer in at most three short sentences: what the problem is, what was decided, \
                 what is left to do. Write nothing else."
                    .to_string(),
                talk,
                160,
            )
        }
        other => {
            return Err(Fail::bad(format!(
                "\"{other}\" is not breakdown or summary."
            )))
        }
    };
    let reply =
        web::block(move || crate::forge::reviewer::model::ask(&config, &system, &user, max, false))
            .await
            .map_err(|e| Fail::from(anyhow::anyhow!("{e}")))?
            .map_err(|e| {
                tracing::warn!(error = ?e, "the box's AI did not answer");
                Fail::new(
                    actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                    "The box's AI did not answer. It may still be starting; try again in a minute.",
                )
            })?;
    let items: Vec<String> = if body.task == "breakdown" {
        steps(&reply.text)
    } else {
        Vec::new()
    };
    Ok(HttpResponse::Ok().json(json!({ "text": reply.text.trim(), "items": items })))
}

fn trim_to(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// The steps of a model's list: `- x`, `* x`, `1. x`, at most eight.
pub fn steps(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l
                .strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| {
                    let digits = l.find(|c: char| !c.is_ascii_digit())?;
                    (digits > 0).then(|| l[digits..].trim_start_matches(['.', ')']).trim_start())
                })?;
            let rest = rest.trim().trim_end_matches('.').trim();
            (!rest.is_empty()).then(|| rest.chars().take(120).collect())
        })
        .take(8)
        .collect()
}

// ── Projects ────────────────────────────────────────────────────────────────

async fn project(req: HttpRequest, path: web::Path<String>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, false)?;
    let (p, perm) = find_project(&state, &c, &path, Permission::Read)?;
    let mut v = project_json(&state, &p, perm)?;
    v["members"] = json!(state
        .store
        .project_members(p.id)?
        .into_iter()
        .map(|(l, r)| json!({ "login": l, "role": r.as_str() }))
        .collect::<Vec<_>>());
    Ok(HttpResponse::Ok().json(v))
}

/// The project a repository's issues are in. One who may write the
/// repository gets one made on first ask; a reader of a repository with no
/// issues yet gets 404.
async fn repo_project(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, false)?;
    let (repo, repo_perm) = crate::forge::api::find_repo(&state, &c, &path.0, &path.1)?;
    let p = match state.store.project_of_repo(repo.id)? {
        Some(p) => p,
        None if repo_perm >= Permission::Write => {
            super::adopt::project_for_repo(&state.store, &repo)?
        }
        None => return Err(Fail::not_found("project")),
    };
    let perm = state.store.project_permission(&c, &p)?;
    if perm == Permission::None {
        return Err(Fail::not_found("project"));
    }
    Ok(HttpResponse::Ok().json(project_json(&state, &p, perm)?))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ProjectFields {
    key: Option<String>,
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    visibility: Option<String>,
    #[serde(deserialize_with = "double")]
    lead: Option<Option<String>>,
    archived: Option<bool>,
    /// `owner/name` to link when making it.
    repo: Option<String>,
}

fn check_color(c: &Option<String>) -> Result<(), Fail> {
    if let Some(c) = c {
        let ok = c.is_empty()
            || (c.len() == 7
                && c.starts_with('#')
                && c[1..].chars().all(|h| h.is_ascii_hexdigit()));
        if !ok {
            return Err(Fail::bad("A colour is #rrggbb."));
        }
    }
    Ok(())
}

fn check_visibility(v: &Option<String>) -> Result<(), Fail> {
    match v.as_deref() {
        None | Some("private") | Some("internal") => Ok(()),
        Some(_) => Err(Fail::bad("A project is private or internal.")),
    }
}

async fn create_project(
    req: HttpRequest,
    body: web::Json<ProjectFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let name = body.name.as_deref().map(str::trim).unwrap_or("");
    if name.is_empty() || name.chars().count() > 80 {
        return Err(Fail::bad("A project needs a name of up to 80 characters."));
    }
    check_color(&body.color)?;
    check_visibility(&body.visibility)?;
    let key = match body.key.as_deref() {
        Some(k) if !k.trim().is_empty() => {
            let k = k.trim().to_ascii_uppercase();
            if !valid_project_key(&k) {
                return Err(Fail::bad(
                    "A key is 2 to 6 letters or digits, a letter first.",
                ));
            }
            k
        }
        _ => state.store.free_project_key(&super::key_for(name))?,
    };
    let repo = match body.repo.as_deref() {
        None | Some("") => None,
        Some(r) => {
            let (o, n) = r
                .split_once('/')
                .ok_or_else(|| Fail::bad("A repository is owner/name."))?;
            let repo = state
                .store
                .repo(o, n)?
                .ok_or_else(|| Fail::bad("No such repository."))?;
            if c.permission(&state.store, &repo)? < Permission::Admin {
                return Err(Fail::forbidden(
                    "Only the repository's administrators may link it.",
                ));
            }
            Some(repo)
        }
    };
    let p = state.store.create_project(
        &key,
        name,
        body.description.as_deref().unwrap_or(""),
        body.color.as_deref().unwrap_or(""),
        body.visibility.as_deref().unwrap_or("private"),
        me(&c).map(|u| u.id),
    )?;
    if let Some(repo) = repo {
        state.store.link_repo(p.id, repo.id, true)?;
    }
    let perm = state.store.project_permission(&c, &p)?;
    Ok(HttpResponse::Created().json(project_json(&state, &p, perm)?))
}

async fn edit_project(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<ProjectFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (p, _) = find_project(&state, &c, &path, Permission::Admin)?;
    check_color(&body.color)?;
    check_visibility(&body.visibility)?;
    if let Some(n) = &body.name {
        if n.trim().is_empty() || n.chars().count() > 80 {
            return Err(Fail::bad("A project needs a name of up to 80 characters."));
        }
    }
    let lead = match &body.lead {
        None => None,
        Some(None) => Some(None),
        Some(Some(l)) => Some(Some(
            state
                .store
                .user(l)?
                .ok_or_else(|| Fail::bad(format!("There is no one called {l}.")))?
                .id,
        )),
    };
    state.store.update_project(
        p.id,
        body.name.as_deref().map(str::trim),
        body.description.as_deref(),
        body.color.as_deref(),
        body.visibility.as_deref(),
        lead,
        body.archived,
    )?;
    let p = state
        .store
        .project_by_id(p.id)?
        .ok_or_else(|| Fail::not_found("project"))?;
    let perm = state.store.project_permission(&c, &p)?;
    Ok(HttpResponse::Ok().json(project_json(&state, &p, perm)?))
}

#[derive(Deserialize)]
struct Role {
    role: String,
}

async fn put_member(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    body: web::Json<Role>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (p, _) = find_project(&state, &c, &path.0, Permission::Admin)?;
    let role = Permission::parse(&body.role)
        .ok_or_else(|| Fail::bad("A role is read, write or admin."))?;
    let u = state
        .store
        .user(&path.1)?
        .ok_or_else(|| Fail::not_found("account"))?;
    state.store.set_project_member(p.id, u.id, Some(role))?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_member(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (p, _) = find_project(&state, &c, &path.0, Permission::Admin)?;
    let u = state
        .store
        .user(&path.1)?
        .ok_or_else(|| Fail::not_found("account"))?;
    state.store.set_project_member(p.id, u.id, None)?;
    Ok(HttpResponse::NoContent().finish())
}

async fn link_repo(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    state: web::Data<State>,
) -> Answer {
    link(req, path, state, true).await
}

async fn unlink_repo(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    state: web::Data<State>,
) -> Answer {
    link(req, path, state, false).await
}

async fn link(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    state: web::Data<State>,
    on: bool,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (p, _) = find_project(&state, &c, &path.0, Permission::Admin)?;
    let repo = state
        .store
        .repo(&path.1, &path.2)?
        .ok_or_else(|| Fail::not_found("repository"))?;
    if c.permission(&state.store, &repo)? < Permission::Admin {
        return Err(Fail::forbidden(
            "Only the repository's administrators may link it.",
        ));
    }
    state.store.link_repo(p.id, repo.id, on)?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StateFields {
    name: Option<String>,
    category: Option<String>,
    color: Option<String>,
    position: Option<i64>,
    /// Where a deleted state's issues go.
    into: Option<i64>,
}

fn short_name(n: &Option<String>, what: &str) -> Result<Option<String>, Fail> {
    match n {
        None => Ok(None),
        Some(n) if n.trim().is_empty() || n.chars().count() > 40 => {
            Err(Fail::bad(format!("A {what} name is 1 to 40 characters.")))
        }
        Some(n) => Ok(Some(n.trim().to_string())),
    }
}

async fn add_state(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<StateFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (p, _) = find_project(&state, &c, &path, Permission::Admin)?;
    let name =
        short_name(&body.name, "state")?.ok_or_else(|| Fail::bad("A state needs a name."))?;
    let cat = body
        .category
        .as_deref()
        .and_then(Category::parse)
        .ok_or_else(|| Fail::bad("A state is backlog, unstarted, started, done or canceled."))?;
    check_color(&body.color)?;
    let id = state
        .store
        .add_state(p.id, &name, cat, body.color.as_deref().unwrap_or(""))?;
    Ok(HttpResponse::Created().json(json!({ "id": id })))
}

fn state_project(state: &State, c: &Caller, id: i64) -> Result<Project, Fail> {
    let pid: i64 = state
        .store
        .conn()
        .query_row(
            "SELECT project_id FROM workflow_states WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| Fail::not_found("state"))?;
    let p = state
        .store
        .project_by_id(pid)?
        .ok_or_else(|| Fail::not_found("state"))?;
    if state.store.project_permission(c, &p)? < Permission::Admin {
        return Err(Fail::forbidden(
            "Only the project's administrators may change its workflow.",
        ));
    }
    Ok(p)
}

async fn edit_state(
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<StateFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    state_project(&state, &c, *path)?;
    check_color(&body.color)?;
    state.store.update_state(
        *path,
        short_name(&body.name, "state")?.as_deref(),
        body.color.as_deref(),
        body.position,
    )?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_state(
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<StateFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let p = state_project(&state, &c, *path)?;
    let flow = state.store.workflow(p.id)?;
    let gone = flow
        .iter()
        .find(|s| s.id == *path)
        .ok_or_else(|| Fail::not_found("state"))?;
    let into = match query.into {
        Some(i) => flow.iter().find(|s| s.id == i && s.id != *path),
        None => flow
            .iter()
            .find(|s| s.id != *path && s.category == gone.category),
    }
    .ok_or_else(|| Fail::bad("Say which state its issues move to."))?;
    if !flow
        .iter()
        .any(|s| s.id != *path && s.category == gone.category)
        && gone.category.closed()
    {
        return Err(Fail::bad("A workflow keeps one state of each closed kind."));
    }
    state.store.delete_state(*path, into.id)?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LabelFields {
    name: Option<String>,
    color: Option<String>,
    description: Option<String>,
}

async fn add_label(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<LabelFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (p, _) = find_project(&state, &c, &path, Permission::Write)?;
    let name =
        short_name(&body.name, "label")?.ok_or_else(|| Fail::bad("A label needs a name."))?;
    check_color(&body.color)?;
    let id = state
        .store
        .ensure_label(p.id, &name, body.color.as_deref().unwrap_or(""))?;
    if body.color.is_some() || body.description.is_some() {
        state
            .store
            .update_label(id, None, body.color.as_deref(), body.description.as_deref())?;
    }
    Ok(HttpResponse::Created().json(json!({ "id": id })))
}

fn label_project(state: &State, c: &Caller, id: i64) -> Result<Project, Fail> {
    let pid: i64 = state
        .store
        .conn()
        .query_row(
            "SELECT project_id FROM project_labels WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| Fail::not_found("label"))?;
    let p = state
        .store
        .project_by_id(pid)?
        .ok_or_else(|| Fail::not_found("label"))?;
    if state.store.project_permission(c, &p)? < Permission::Write {
        return Err(Fail::not_found("label"));
    }
    Ok(p)
}

async fn edit_label(
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<LabelFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    label_project(&state, &c, *path)?;
    check_color(&body.color)?;
    state.store.update_label(
        *path,
        short_name(&body.name, "label")?.as_deref(),
        body.color.as_deref(),
        body.description.as_deref(),
    )?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_label(req: HttpRequest, path: web::Path<i64>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, true)?;
    label_project(&state, &c, *path)?;
    state.store.delete_label(*path)?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CycleFields {
    name: Option<String>,
    starts: Option<String>,
    ends: Option<String>,
}

fn check_cycle(starts: Option<&str>, ends: Option<&str>) -> Result<(), Fail> {
    for d in [starts, ends].into_iter().flatten() {
        if !valid_date(d) {
            return Err(Fail::bad("A cycle starts and ends on a day, YYYY-MM-DD."));
        }
    }
    if let (Some(s), Some(e)) = (starts, ends) {
        if e < s {
            return Err(Fail::bad("A cycle ends after it starts."));
        }
    }
    Ok(())
}

async fn add_cycle(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<CycleFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let (p, _) = find_project(&state, &c, &path, Permission::Write)?;
    let (Some(starts), Some(ends)) = (body.starts.as_deref(), body.ends.as_deref()) else {
        return Err(Fail::bad("A cycle has a first and a last day."));
    };
    check_cycle(Some(starts), Some(ends))?;
    let cycle = state.store.add_cycle(
        p.id,
        body.name.as_deref().unwrap_or("").trim(),
        starts,
        ends,
    )?;
    Ok(HttpResponse::Created().json(cycle))
}

fn cycle_project(state: &State, c: &Caller, id: i64) -> Result<super::store::Cycle, Fail> {
    let cy = state
        .store
        .cycle(id)?
        .ok_or_else(|| Fail::not_found("cycle"))?;
    let p = state
        .store
        .project_by_id(cy.project_id)?
        .ok_or_else(|| Fail::not_found("cycle"))?;
    if state.store.project_permission(c, &p)? < Permission::Write {
        return Err(Fail::not_found("cycle"));
    }
    Ok(cy)
}

async fn edit_cycle(
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<CycleFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, true)?;
    let cy = cycle_project(&state, &c, *path)?;
    check_cycle(
        Some(body.starts.as_deref().unwrap_or(&cy.starts)),
        Some(body.ends.as_deref().unwrap_or(&cy.ends)),
    )?;
    state.store.update_cycle(
        *path,
        body.name.as_deref(),
        body.starts.as_deref(),
        body.ends.as_deref(),
    )?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_cycle(req: HttpRequest, path: web::Path<i64>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, true)?;
    cycle_project(&state, &c, *path)?;
    state.store.delete_cycle(*path)?;
    Ok(HttpResponse::NoContent().finish())
}

// ── Views and the inbox ─────────────────────────────────────────────────────

fn person(c: &Caller) -> Result<&User, Fail> {
    me(c).ok_or_else(|| Fail::bad("Views and the inbox are a person's."))
}

async fn views(req: HttpRequest, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, false)?;
    Ok(HttpResponse::Ok().json(state.store.views_for(person(&c)?.id)?))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ViewFields {
    name: Option<String>,
    query: Option<Filter>,
    layout: Option<String>,
    group_by: Option<String>,
    shared: Option<bool>,
}

fn check_view(v: &ViewFields) -> Result<(), Fail> {
    short_name(&v.name, "view")?;
    if !matches!(v.layout.as_deref(), None | Some("list") | Some("board")) {
        return Err(Fail::bad("A view is a list or a board."));
    }
    if !matches!(
        v.group_by.as_deref(),
        None | Some("state")
            | Some("assignee")
            | Some("priority")
            | Some("project")
            | Some("cycle")
            | Some("none")
    ) {
        return Err(Fail::bad(
            "A view groups by state, assignee, priority, project, cycle or none.",
        ));
    }
    Ok(())
}

async fn save_view(
    req: HttpRequest,
    body: web::Json<ViewFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, false)?;
    let u = person(&c)?;
    check_view(&body)?;
    let name = short_name(&body.name, "view")?.ok_or_else(|| Fail::bad("A view needs a name."))?;
    let id = state.store.save_view(
        None,
        u.id,
        &name,
        &serde_json::to_value(body.query.clone().unwrap_or_default()).unwrap_or(Value::Null),
        body.layout.as_deref().unwrap_or("list"),
        body.group_by.as_deref().unwrap_or("state"),
        body.shared.unwrap_or(false),
    )?;
    let (view, _) = state
        .store
        .view(id)?
        .ok_or_else(|| Fail::not_found("view"))?;
    Ok(HttpResponse::Created().json(view))
}

async fn update_view(
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<ViewFields>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, false)?;
    let u = person(&c)?;
    check_view(&body)?;
    let (old, owner) = state
        .store
        .view(*path)?
        .ok_or_else(|| Fail::not_found("view"))?;
    if owner != u.id {
        return Err(Fail::forbidden("Only who saved a view may change it."));
    }
    let query = match &body.query {
        Some(q) => serde_json::to_value(q).unwrap_or(Value::Null),
        None => old.query.clone(),
    };
    state.store.save_view(
        Some(old.id),
        u.id,
        short_name(&body.name, "view")?
            .as_deref()
            .unwrap_or(&old.name),
        &query,
        body.layout.as_deref().unwrap_or(&old.layout),
        body.group_by.as_deref().unwrap_or(&old.group_by),
        body.shared.unwrap_or(old.shared),
    )?;
    let (view, _) = state
        .store
        .view(old.id)?
        .ok_or_else(|| Fail::not_found("view"))?;
    Ok(HttpResponse::Ok().json(view))
}

async fn delete_view(req: HttpRequest, path: web::Path<i64>, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, false)?;
    let u = person(&c)?;
    let (_, owner) = state
        .store
        .view(*path)?
        .ok_or_else(|| Fail::not_found("view"))?;
    if owner != u.id {
        return Err(Fail::forbidden("Only who saved a view may remove it."));
    }
    state.store.delete_view(*path)?;
    Ok(HttpResponse::NoContent().finish())
}

async fn inbox(req: HttpRequest, state: web::Data<State>) -> Answer {
    let c = caller(&req, &state, false)?;
    let u = person(&c)?;
    let mut items = Vec::new();
    for item in state.store.inbox(u.id, 200)? {
        // An issue of a project the person lost sight of is left out.
        let Some(t) = state.store.ticket_by_id(item.ticket_id)? else {
            continue;
        };
        let Some(p) = state.store.project_by_id(t.project_id)? else {
            continue;
        };
        if state.store.project_permission(&c, &p)? < Permission::Read {
            continue;
        }
        let mut v = serde_json::to_value(&item).unwrap_or(Value::Null);
        v["issue"] = ticket_json(&state, &t);
        items.push(v);
    }
    Ok(HttpResponse::Ok().json(json!({ "items": items, "unread": state.store.unread(u.id)? })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReadMark {
    /// Leave out for all of them.
    ids: Option<Vec<i64>>,
    /// `false` marks them unread again.
    read: Option<bool>,
}

async fn read_inbox(
    req: HttpRequest,
    body: web::Json<ReadMark>,
    state: web::Data<State>,
) -> Answer {
    let c = caller(&req, &state, false)?;
    let u = person(&c)?;
    state
        .store
        .mark_read(u.id, body.ids.as_deref(), body.read.unwrap_or(true))?;
    Ok(HttpResponse::Ok().json(json!({ "unread": state.store.unread(u.id)? })))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_models_list_is_read_line_by_line() {
        let text = "Here you go:\n- Find the log.\n* Read the error\n3. Fix it\n\nThat is all.";
        assert_eq!(
            super::steps(text),
            ["Find the log", "Read the error", "Fix it"]
        );
    }
}
