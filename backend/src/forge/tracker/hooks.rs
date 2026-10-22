//! What pushes and pull requests do to issues.
//!
//!   * A commit that names an issue (`LOS-12`, `#12`, `owner/repo#12`) is
//!     linked to it. On the repository's default branch, a closing word
//!     before the name (`Fixes LOS-12`) also moves the issue to Done.
//!   * A pull request whose title, description or branch names an issue is
//!     linked to it, and moves an issue nobody started to In Review (the
//!     project's first started state when it has no In Review). When it is
//!     merged, the issues it said it closes move to Done.
//!
//! The pusher or the pull request's author must be able to read the issue
//! to link it and to write it to close it. The work runs on a thread of its
//! own: a push never waits for it.

use super::refs::{references, Reference, Target};
use super::store::{Actor, Change, Ticket};
use super::Category;
use crate::forge::auth::Caller;
use crate::forge::db::{Permission, Repo, User};
use crate::forge::events::{self, Event};
use crate::forge::server::State;
use actix_web::web;
use std::sync::mpsc;

/// How many commits of one push are read for references.
const MAX_COMMITS: usize = 200;

/// Listen for pushes and pull requests.
pub fn start(state: web::Data<State>) -> std::io::Result<()> {
    let (tx, rx) = mpsc::channel::<Event>();
    let tx = std::sync::Mutex::new(tx);
    events::subscribe(Box::new(move |e| {
        let _ = tx.lock().unwrap_or_else(|p| p.into_inner()).send(e.clone());
    }));
    std::thread::Builder::new()
        .name("tracker".into())
        .spawn(move || {
            for e in rx {
                if let Err(err) = handle(&state, &e) {
                    tracing::warn!(error = ?err, "the tracker could not take in an event");
                }
            }
        })?;
    Ok(())
}

pub fn handle(state: &State, e: &Event) -> anyhow::Result<()> {
    match e {
        Event::Push {
            owner,
            repo,
            r#ref,
            before,
            after,
            pusher,
            ..
        } => {
            let Some(repo) = state.store.repo(owner, repo)? else {
                return Ok(());
            };
            let Some(pusher) = state.store.user(pusher)? else {
                return Ok(());
            };
            pushed(state, &repo, &pusher, r#ref, before, after)
        }
        Event::PullRequest {
            action,
            owner,
            repo,
            pr,
            ..
        } => {
            let Some(repo) = state.store.repo(owner, repo)? else {
                return Ok(());
            };
            pull(state, &repo, action, pr.number)
        }
    }
}

/// The issue a reference names, if the person may see it.
fn resolve(
    state: &State,
    repo: &Repo,
    target: &Target,
    caller: &Caller,
) -> anyhow::Result<Option<(Ticket, Permission)>> {
    let ticket = match target {
        Target::Key(k, n) => state.store.ticket(k, *n)?,
        Target::Local(n) => state.store.ticket_of_repo(repo.id, *n)?,
        Target::Repo(o, name, n) => match state.store.repo(o, name)? {
            Some(other) => state.store.ticket_of_repo(other.id, *n)?,
            None => None,
        },
    };
    let Some(ticket) = ticket else {
        return Ok(None);
    };
    let Some(project) = state.store.project_by_id(ticket.project_id)? else {
        return Ok(None);
    };
    let p = state.store.project_permission(caller, &project)?;
    Ok((p >= Permission::Read).then_some((ticket, p)))
}

fn close(state: &State, t: &Ticket, actor: &Actor) -> anyhow::Result<bool> {
    if t.state.category.closed() {
        return Ok(false);
    }
    let Some(done) = state.store.first_state(t.project_id, Category::Done)? else {
        return Ok(false);
    };
    state.store.change_ticket(
        t.id,
        &Change {
            state_id: Some(done.id),
            ..Change::default()
        },
        actor,
    )?;
    Ok(true)
}

fn pushed(
    state: &State,
    repo: &Repo,
    pusher: &User,
    r#ref: &str,
    before: &str,
    after: &str,
) -> anyhow::Result<()> {
    let Some(branch) = r#ref.strip_prefix("refs/heads/") else {
        return Ok(());
    };
    let on_default = branch == repo.default_branch;
    let dir = state.config.repo_path(&repo.owner, &repo.name);
    let from = (before != events::ZERO).then_some(before);
    // A new branch: only what the default branch does not have yet.
    let from = match from {
        Some(f) => Some(f.to_string()),
        None if !on_default => state
            .git
            .commit_of(&dir, &format!("refs/heads/{}", repo.default_branch))?,
        None => None,
    };
    let commits = state.git.log(&dir, from.as_deref(), after, MAX_COMMITS)?;
    let caller = Caller::person(pusher.clone());
    let actor = Actor::of(pusher);
    // Oldest first, so the history reads in the order things happened.
    for c in commits.iter().rev() {
        let message = format!("{}\n\n{}", c.subject, c.body);
        for Reference { target, closes } in references(&message) {
            let Some((t, p)) = resolve(state, repo, &target, &caller)? else {
                continue;
            };
            let closes = closes && on_default && p >= Permission::Write;
            state.store.add_link(
                t.id,
                repo.id,
                "commit",
                &c.sha,
                &c.subject,
                closes,
                &Actor::named(&c.author_name),
            )?;
            if closes && close(state, &t, &actor)? {
                tracing::info!(
                    issue = t.key.as_str(),
                    commit = &c.sha[..12],
                    "a commit closed an issue"
                );
            }
        }
    }
    Ok(())
}

fn pull(state: &State, repo: &Repo, action: &str, number: i64) -> anyhow::Result<()> {
    let Some(pr) = state.store.issue(repo.id, number)? else {
        return Ok(());
    };
    let Some(author) = state.store.user(&pr.poster)? else {
        return Ok(());
    };
    let caller = Caller::person(author.clone());
    let actor = Actor::of(&author);
    match action {
        "opened" | "reopened" | "synchronize" | "edited" => {
            let mut refs = references(&format!("{}\n\n{}", pr.title, pr.body));
            // `los-12-backup` and `matus/LOS-12` name an issue too.
            if let Some(branch) = pr.head_branch.as_deref() {
                let leaf = branch.rsplit(['/', ':']).next().unwrap_or(branch);
                let mut parts = leaf.split(['-', '_']);
                if let (Some(k), Some(n)) = (parts.next(), parts.next()) {
                    if let Some((k, n)) = super::parse_key(&format!("{k}-{n}")) {
                        let t = Target::Key(k, n);
                        if !refs.iter().any(|r| r.target == t) {
                            refs.push(Reference {
                                target: t,
                                closes: false,
                            });
                        }
                    }
                }
            }
            for Reference { target, closes } in refs {
                let Some((t, p)) = resolve(state, repo, &target, &caller)? else {
                    continue;
                };
                let closes = closes && p >= Permission::Write;
                state.store.add_link(
                    t.id,
                    repo.id,
                    "pull",
                    &number.to_string(),
                    &pr.title,
                    closes,
                    &actor,
                )?;
                if p >= Permission::Write
                    && matches!(t.state.category, Category::Backlog | Category::Unstarted)
                {
                    let flow = state.store.workflow(t.project_id)?;
                    let review = flow
                        .iter()
                        .find(|s| {
                            s.category == Category::Started
                                && s.name.eq_ignore_ascii_case("in review")
                        })
                        .or_else(|| flow.iter().find(|s| s.category == Category::Started));
                    if let Some(review) = review {
                        state.store.change_ticket(
                            t.id,
                            &Change {
                                state_id: Some(review.id),
                                ..Change::default()
                            },
                            &actor,
                        )?;
                    }
                }
            }
        }
        "merged" => {
            let merger = pr
                .merged_by
                .as_deref()
                .and_then(|m| state.store.user(m).ok().flatten())
                .map(|u| Actor::of(&u))
                .unwrap_or(actor);
            for id in state.store.closed_by_pull(repo.id, number)? {
                if let Some(t) = state.store.ticket_by_id(id)? {
                    if close(state, &t, &merger)? {
                        tracing::info!(
                            issue = t.key.as_str(),
                            pull = number,
                            "a merged pull request closed an issue"
                        );
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
