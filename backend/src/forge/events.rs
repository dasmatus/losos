//! What happened in LosOS Git, for whoever wants to know: a push, a pull
//! request opened or moved on.
//!
//! Two kinds of consumer. Code inside `losos-git` (the AI reviewer)
//! registers a callback with [`subscribe`]. Services outside it get the
//! event as JSON on a Unix socket: Mesh CI's `/run/losos-ci/events.sock`
//! (`--events-socket`, backend/src/ci/). Delivery to a socket is fire and
//! forget, on a thread of its own, with a two-second budget: a missing
//! socket (CI is off on this box) or a slow one is a line in the debug log
//! and never holds a push up.

use super::db::Repo;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// The all-zero id git uses for "no commit".
pub const ZERO: &str = "0000000000000000000000000000000000000000";

/// One event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A ref moved. Deletions are not sent.
    Push {
        owner: String,
        repo: String,
        private: bool,
        r#ref: String,
        before: String,
        after: String,
        pusher: String,
        chat_room: Option<String>,
    },
    /// A pull request was opened, reopened, closed or merged, its head
    /// moved (`synchronize`), or someone reviewed it or wrote in it
    /// (`reviewed`, `commented`).
    PullRequest {
        action: String,
        owner: String,
        repo: String,
        private: bool,
        pr: PullRequest,
        chat_room: Option<String>,
    },
    /// An issue was opened, closed, reopened or written in.
    Issue {
        action: String,
        owner: String,
        repo: String,
        private: bool,
        number: i64,
        /// The issue tracker's key (`LOS-12`), when it has one.
        key: Option<String>,
        title: String,
        /// Who did it.
        author: String,
        chat_room: Option<String>,
    },
    /// A check on a commit finished: `success`, `failure` or `error`.
    Status {
        owner: String,
        repo: String,
        private: bool,
        sha: String,
        context: String,
        state: String,
        description: String,
        chat_room: Option<String>,
    },
}

/// What an event says about a pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub number: i64,
    pub title: String,
    /// `refs/heads/fix`.
    pub head_ref: String,
    pub head_sha: String,
    /// `refs/heads/main`.
    pub base_ref: String,
    pub author: String,
    /// Who did what the event says, when a person did (not a push).
    pub actor: String,
}

impl Event {
    /// The JSON the socket consumers get (the contract in
    /// backend/src/ci/events.rs).
    pub fn to_json(&self) -> Value {
        match self {
            Event::Push {
                owner,
                repo,
                private,
                r#ref,
                before,
                after,
                pusher,
                chat_room,
            } => json!({
                "kind": "push",
                "owner": owner,
                "repo": repo,
                "private": private,
                "ref": r#ref,
                "before": before,
                "after": after,
                "pusher": pusher,
                "chatRoom": chat_room,
            }),
            Event::PullRequest {
                action,
                owner,
                repo,
                private,
                pr,
                chat_room,
            } => json!({
                "kind": "pull_request",
                "action": action,
                "owner": owner,
                "repo": repo,
                "private": private,
                "number": pr.number,
                "title": pr.title,
                "headRef": pr.head_ref,
                "headSha": pr.head_sha,
                "baseRef": pr.base_ref,
                "author": pr.author,
                "actor": pr.actor,
                "chatRoom": chat_room,
            }),
            Event::Issue {
                action,
                owner,
                repo,
                private,
                number,
                key,
                title,
                author,
                chat_room,
            } => json!({
                "kind": "issue",
                "action": action,
                "owner": owner,
                "repo": repo,
                "private": private,
                "number": number,
                "key": key,
                "title": title,
                "author": author,
                "chatRoom": chat_room,
            }),
            Event::Status {
                owner,
                repo,
                private,
                sha,
                context,
                state,
                description,
                chat_room,
            } => json!({
                "kind": "status",
                "owner": owner,
                "repo": repo,
                "private": private,
                "sha": sha,
                "context": context,
                "state": state,
                "description": description,
                "chatRoom": chat_room,
            }),
        }
    }
}

type Consumer = Box<dyn Fn(&Event) + Send + Sync>;

fn consumers() -> &'static Mutex<Vec<Consumer>> {
    static C: OnceLock<Mutex<Vec<Consumer>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Vec::new()))
}

fn sockets() -> &'static Mutex<Vec<PathBuf>> {
    static S: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}

/// Call `f` with every event from now on, on the thread that made it.
pub fn subscribe(f: Consumer) {
    consumers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(f);
}

/// Send every event from now on to the socket at `path` as well.
pub fn deliver_to(path: PathBuf) {
    sockets()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(path);
}

/// Tell everyone.
pub fn emit(event: Event) {
    for c in consumers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
    {
        c(&event);
    }
    let targets = sockets()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if targets.is_empty() {
        return;
    }
    let body = event.to_json().to_string();
    std::thread::spawn(move || {
        for path in targets {
            if let Err(e) = post(&path, &body) {
                tracing::debug!(socket = %path.display(), error = %e, "an event was not delivered");
            }
        }
    });
}

/// `POST /events` over a Unix socket. Only the status line is read.
fn post(path: &std::path::Path, body: &str) -> std::io::Result<()> {
    let mut s = std::os::unix::net::UnixStream::connect(path)?;
    s.set_write_timeout(Some(Duration::from_secs(2)))?;
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        s,
        "POST /events HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut head = [0u8; 12];
    s.read_exact(&mut head)?;
    let status = String::from_utf8_lossy(&head[9..12]).to_string();
    if status.starts_with('2') {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("answered {status}")))
    }
}

/// A push moved `changes` (`(ref, old, new)`, git's hook shape).
pub fn pushed(repo: &Repo, pusher: &str, changes: &[(String, String, String)]) {
    for (r, before, after) in changes {
        if after == ZERO {
            continue;
        }
        emit(Event::Push {
            owner: repo.owner.clone(),
            repo: repo.name.clone(),
            private: repo.private,
            r#ref: r.clone(),
            before: before.clone(),
            after: after.clone(),
            pusher: pusher.to_string(),
            chat_room: repo.chat_room.clone(),
        });
    }
}

/// A pull request changed: `action` is opened, reopened, synchronize,
/// closed or merged.
pub fn pull_request(action: &str, repo: &Repo, pr: PullRequest) {
    emit(Event::PullRequest {
        action: action.to_string(),
        owner: repo.owner.clone(),
        repo: repo.name.clone(),
        private: repo.private,
        pr,
        chat_room: repo.chat_room.clone(),
    });
}

/// An issue changed: `action` is opened, closed, reopened or commented.
pub fn issue(action: &str, repo: &Repo, number: i64, key: Option<&str>, title: &str, author: &str) {
    emit(Event::Issue {
        action: action.to_string(),
        owner: repo.owner.clone(),
        repo: repo.name.clone(),
        private: repo.private,
        number,
        key: key.map(str::to_string),
        title: title.to_string(),
        author: author.to_string(),
        chat_room: repo.chat_room.clone(),
    });
}

/// A check on `sha` reached `state`. Only a finished check is an event:
/// `pending` says nothing yet.
pub fn status(repo: &Repo, sha: &str, context: &str, state: &str, description: &str) {
    if !matches!(state, "success" | "failure" | "error") {
        return;
    }
    emit(Event::Status {
        owner: repo.owner.clone(),
        repo: repo.name.clone(),
        private: repo.private,
        sha: sha.to_string(),
        context: context.to_string(),
        state: state.to_string(),
        description: description.to_string(),
        chat_room: repo.chat_room.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn repo() -> Repo {
        Repo {
            id: 1,
            owner_id: 1,
            owner: "matus".into(),
            name: "site".into(),
            description: String::new(),
            private: true,
            visibility: crate::forge::db::Visibility::Private,
            default_branch: "main".into(),
            fork_of: None,
            created: String::new(),
            updated: String::new(),
            chat_room: Some("!room:mattbox.local".into()),
        }
    }

    #[test]
    fn pushes_reach_subscribers_and_the_socket() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        subscribe(Box::new(move |e| s2.lock().unwrap().push(e.clone())));

        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("events.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        deliver_to(sock);
        let server = std::thread::spawn(move || {
            let (mut c, _) = listener.accept().unwrap();
            // The head and the body can arrive in separate reads on a busy
            // machine: read until the body is as long as its header says.
            let mut request = Vec::new();
            let mut buf = vec![0u8; 4096];
            loop {
                let n = c.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request);
                let done = text.split_once("\r\n\r\n").is_some_and(|(head, body)| {
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    body.len() >= length
                });
                if done || n == 0 {
                    break;
                }
            }
            c.write_all(b"HTTP/1.1 204 No Content\r\n\r\n").unwrap();
            String::from_utf8_lossy(&request).to_string()
        });

        let a = "a".repeat(40);
        pushed(
            &repo(),
            "matus",
            &[
                ("refs/heads/main".into(), ZERO.into(), a.clone()),
                ("refs/heads/old".into(), a.clone(), ZERO.into()),
            ],
        );
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /events HTTP/1.1"));
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["kind"], "push");
        assert_eq!(body["ref"], "refs/heads/main");
        assert_eq!(body["after"], a);
        assert_eq!(body["private"], true);
        assert_eq!(body["chatRoom"], "!room:mattbox.local");
        // The deletion is not an event.
        let seen = seen.lock().unwrap();
        assert!(seen
            .iter()
            .all(|e| !matches!(e, Event::Push { after, .. } if after == ZERO)));
    }
}
