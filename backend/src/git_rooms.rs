//! Chat rooms of LosOS Git repositories.
//!
//! An administrator of a repository may give it a room in LosOS Chat
//! (`POST /api/chat/git-room`, from the repository's settings page). lososd
//! makes the room as that person, puts LosOS Git's account
//! (`@losos-git:<server>`, [`crate::chat::GIT_BOT`]) in it, tells LosOS Git
//! the room's ID, which the repository's pages link to, and keeps which room
//! is whose in `git-rooms.json` beside its state.
//!
//! LosOS Git sends what happens (`crate::forge::events`) to a socket of
//! lososd's whose directory only LosOS Git's group may enter
//! (`modules/chat.nix`). An event of a repository whose room lososd made is
//! posted there as a notice; any other is dropped, so a room ID set by hand
//! through LosOS Git's API never gets anything.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

use crate::chat::Notice;

/// The largest event body taken.
const MAX_EVENT: usize = 64 * 1024;

/// Which room lososd made for which repository.
pub struct Rooms {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Rooms {
    #[must_use]
    pub fn from_env() -> Rooms {
        Rooms::at(
            std::env::var_os("LOSOS_GIT_ROOMS_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/var/lib/losos/git-rooms.json")),
        )
    }

    #[must_use]
    pub fn at(path: PathBuf) -> Rooms {
        Rooms {
            path,
            lock: Mutex::new(()),
        }
    }

    fn read(&self) -> BTreeMap<String, String> {
        std::fs::read(&self.path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// The room lososd made for `owner/repo`.
    pub fn get(&self, owner: &str, repo: &str) -> Option<String> {
        let _held = self.lock.lock();
        self.read().get(&key(owner, repo)).cloned()
    }

    /// Remember the room of `owner/repo`.
    pub fn set(&self, owner: &str, repo: &str, room: &str) -> anyhow::Result<()> {
        let _held = self.lock.lock();
        let mut all = self.read();
        all.insert(key(owner, repo), room.to_string());
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&all)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

fn key(owner: &str, repo: &str) -> String {
    format!("{owner}/{repo}")
}

/// A name LosOS Git uses for an account or a repository: what may travel
/// in its paths.
#[must_use]
pub fn plain_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && !s.starts_with('.')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

/// A LosOS Git login as a chat address: `eva@kitchenbox.local`, someone of
/// another box, is `@eva:kitchenbox.local`; `owner_account` is the box
/// owner's (`notshared`), shown as `owner`; anyone else is on this server.
#[must_use]
pub fn address(login: &str, server: &str, owner_account: &str, owner: &str) -> String {
    if let Some((name, home)) = login.split_once('@') {
        return format!("@{name}:{home}");
    }
    if login == owner_account && !owner.is_empty() {
        return owner.to_string();
    }
    format!("@{login}:{server}")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn field<'a>(event: &'a Value, k: &str) -> &'a str {
    event.get(k).and_then(Value::as_str).unwrap_or("")
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// What a LosOS Git event says in its repository's room, or `None` for an
/// event nobody needs to read there. `who` turns a login into a chat
/// address ([`address`]); `base` is `https://<box>`.
pub fn notice(event: &Value, base: &str, who: &dyn Fn(&str) -> String) -> Option<Notice> {
    let (owner, repo) = (field(event, "owner"), field(event, "repo"));
    if !plain_name(owner) || !plain_name(repo) {
        return None;
    }
    let page = format!("{base}/git/{owner}/{repo}");
    let person = |login: &str| {
        let id = who(login);
        (
            id.clone(),
            format!(
                "<a href=\"https://matrix.to/#/{}\">{}</a>",
                esc(&id),
                esc(&id)
            ),
        )
    };
    let (text, html, link) = match field(event, "kind") {
        "push" => {
            let r = field(event, "ref");
            let after = field(event, "after");
            let (pusher, pusher_html) = person(field(event, "pusher"));
            let what = if let Some(tag) = r.strip_prefix("refs/tags/") {
                format!("tagged {tag}")
            } else {
                format!("pushed to {}", r.strip_prefix("refs/heads/")?)
            };
            (
                format!("{pusher} {what}: {}", short(after)),
                format!(
                    "{pusher_html} {}: <code>{}</code>",
                    esc(&what),
                    esc(short(after))
                ),
                format!("{page}/commits"),
            )
        }
        "pull_request" => {
            let n = event.get("number").and_then(Value::as_i64)?;
            let title = field(event, "title");
            let actor = match field(event, "actor") {
                "" => field(event, "author"),
                a => a,
            };
            let did = match field(event, "action") {
                "opened" => "opened",
                "closed" => "closed",
                "reopened" => "reopened",
                "merged" => "merged",
                "reviewed" => "reviewed",
                "commented" => "commented on",
                "synchronize" => "pushed to",
                _ => return None,
            };
            let (actor, actor_html) = person(actor);
            let link = format!("{page}/pulls/{n}");
            (
                format!("{actor} {did} pull request #{n}: {title}"),
                format!(
                    "{actor_html} {did} <a href=\"{}\">pull request #{n}</a>: {}",
                    esc(&link),
                    esc(title)
                ),
                link,
            )
        }
        "issue" => {
            let n = event.get("number").and_then(Value::as_i64)?;
            let title = field(event, "title");
            let did = match field(event, "action") {
                "opened" => "opened",
                "closed" => "closed",
                "reopened" => "reopened",
                "commented" => "commented on",
                _ => return None,
            };
            let name = match field(event, "key") {
                "" => format!("issue #{n}"),
                k => format!("issue {k}"),
            };
            let (author, author_html) = person(field(event, "author"));
            (
                format!("{author} {did} {name}: {title}"),
                format!("{author_html} {did} {}: {}", esc(&name), esc(title)),
                page.clone(),
            )
        }
        "status" => {
            let did = match field(event, "state") {
                "success" => "passed",
                "failure" => "failed",
                "error" => "could not run",
                _ => return None,
            };
            let (context, sha) = (field(event, "context"), field(event, "sha"));
            let description = field(event, "description");
            let tail = if description.is_empty() {
                String::new()
            } else {
                format!(": {description}")
            };
            (
                format!("{context} {did} on {}{tail}", short(sha)),
                format!(
                    "{} {did} on <code>{}</code>{}",
                    esc(context),
                    esc(short(sha)),
                    esc(&tail)
                ),
                format!("{page}/commits"),
            )
        }
        _ => return None,
    };
    Some(Notice {
        text: format!("{text}\n{link}"),
        html,
    })
}

/// Take LosOS Git's events on the Unix socket at `path`, one `POST /events`
/// per connection, answer 204 at once and hand each body to `handle`, on a
/// thread of its own. The socket is open to anyone who reaches it: its
/// directory is the gate.
pub fn listen(
    path: &Path,
    handle: impl Fn(Value) + Send + 'static,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::remove_file(path);
    let listener = std::os::unix::net::UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
    tracing::info!(socket = %path.display(), "taking LosOS Git's events for chat rooms");
    Ok(std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            match read_event(conn) {
                Ok(Some(event)) => handle(event),
                Ok(None) => {}
                Err(e) => tracing::debug!(error = %e, "a LosOS Git event was not read"),
            }
        }
    }))
}

fn read_event(conn: std::os::unix::net::UnixStream) -> std::io::Result<Option<Value>> {
    conn.set_read_timeout(Some(Duration::from_secs(2)))?;
    conn.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut reader = BufReader::new(conn.try_clone()?);
    let mut length = None;
    let mut line = String::new();
    let mut first = true;
    loop {
        line.clear();
        if reader.by_ref().take(8192).read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let l = line.trim_end();
        if first {
            first = false;
            if !l.starts_with("POST /events ") {
                (&conn).write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")?;
                return Ok(None);
            }
            continue;
        }
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                length = v.trim().parse::<usize>().ok();
            }
        }
    }
    let Some(length) = length.filter(|n| *n <= MAX_EVENT) else {
        (&conn).write_all(b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n")?;
        return Ok(None);
    };
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    (&conn).write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")?;
    Ok(serde_json::from_slice(&body).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn who(login: &str) -> String {
        address(login, "mattbox.local", "notshared", "@matus:mattbox.local")
    }

    #[test]
    fn logins_become_chat_addresses() {
        assert_eq!(who("eva@kitchenbox.local"), "@eva:kitchenbox.local");
        assert_eq!(who("notshared"), "@matus:mattbox.local");
        assert_eq!(who("jana"), "@jana:mattbox.local");
    }

    #[test]
    fn each_kind_of_event_reads_as_one_line_with_a_link() {
        let base = "https://mattbox.local";
        let pr = json!({
            "kind": "pull_request", "action": "opened", "owner": "notshared", "repo": "site",
            "number": 3, "title": "Hours <b>", "author": "eva@kitchenbox.local",
            "actor": "eva@kitchenbox.local", "chatRoom": "!r:mattbox.local",
        });
        let n = notice(&pr, base, &who).expect("a notice");
        assert_eq!(
            n.text,
            "@eva:kitchenbox.local opened pull request #3: Hours <b>\nhttps://mattbox.local/git/notshared/site/pulls/3"
        );
        assert!(n.html.contains("Hours &lt;b&gt;"), "{}", n.html);
        assert!(n.html.contains("https://matrix.to/#/@eva:kitchenbox.local"));

        let push = json!({
            "kind": "push", "owner": "notshared", "repo": "site", "ref": "refs/heads/main",
            "before": "0", "after": "abcdef0123456789", "pusher": "notshared",
        });
        assert_eq!(
            notice(&push, base, &who).unwrap().text,
            "@matus:mattbox.local pushed to main: abcdef0\nhttps://mattbox.local/git/notshared/site/commits"
        );

        let issue = json!({
            "kind": "issue", "action": "commented", "owner": "notshared", "repo": "site",
            "number": 7, "key": "LOS-7", "title": "Menu", "author": "eva@kitchenbox.local",
        });
        assert!(notice(&issue, base, &who)
            .unwrap()
            .text
            .starts_with("@eva:kitchenbox.local commented on issue LOS-7: Menu"));

        let status = json!({
            "kind": "status", "owner": "notshared", "repo": "site", "sha": "1234567890",
            "context": "build", "state": "failure", "description": "2 tests failed",
        });
        assert!(notice(&status, base, &who)
            .unwrap()
            .text
            .starts_with("build failed on 1234567: 2 tests failed"));
    }

    #[test]
    fn what_nobody_reads_in_a_room_is_left_out() {
        let base = "https://mattbox.local";
        for event in [
            json!({ "kind": "status", "owner": "o", "repo": "r", "state": "pending" }),
            json!({ "kind": "pull_request", "owner": "o", "repo": "r", "number": 1, "action": "labeled" }),
            json!({ "kind": "push", "owner": "o", "repo": "r", "ref": "refs/notes/x" }),
            json!({ "kind": "push", "owner": "../o", "repo": "r", "ref": "refs/heads/x" }),
            json!({ "kind": "other", "owner": "o", "repo": "r" }),
        ] {
            assert_eq!(notice(&event, base, &who), None, "{event}");
        }
    }

    #[test]
    fn rooms_are_remembered_per_repository() {
        let dir = tempfile::tempdir().unwrap();
        let rooms = Rooms::at(dir.path().join("git-rooms.json"));
        assert_eq!(rooms.get("notshared", "site"), None);
        rooms.set("notshared", "site", "!a:mattbox.local").unwrap();
        rooms.set("jana", "notes", "!b:mattbox.local").unwrap();
        assert_eq!(
            rooms.get("notshared", "site").as_deref(),
            Some("!a:mattbox.local")
        );
        assert_eq!(rooms.get("notshared", "notes"), None);
    }

    #[test]
    fn the_socket_takes_one_event_per_connection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.sock");
        let (tx, rx) = std::sync::mpsc::channel();
        listen(&path, move |v| {
            let _ = tx.send(v);
        })
        .unwrap();
        let body = r#"{"kind":"push"}"#;
        let mut s = std::os::unix::net::UnixStream::connect(&path).unwrap();
        write!(
            s,
            "POST /events HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut answer = String::new();
        s.read_to_string(&mut answer).unwrap();
        assert!(answer.starts_with("HTTP/1.1 204"), "{answer}");
        let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(got["kind"], "push");

        let mut s = std::os::unix::net::UnixStream::connect(&path).unwrap();
        write!(s, "POST /events HTTP/1.1\r\nContent-Length: 999999\r\n\r\n").unwrap();
        let mut answer = String::new();
        s.read_to_string(&mut answer).unwrap();
        assert!(answer.starts_with("HTTP/1.1 413"), "{answer}");
    }
}
