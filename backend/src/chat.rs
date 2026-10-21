//! LosOS Chat: the box's Matrix homeserver (Dendrite, `modules/chat.nix`)
//! and the parts of it lososd owns.
//!
//! Dendrite keeps the rooms, the messages, the devices and their keys.
//! lososd decides who may use it, because nobody signs in to chat with a
//! Matrix password: there are none. lososd is registered with Dendrite as
//! an application service whose namespace is every user on the box, and
//! that is the only way an account is made or a device signed in:
//!
//!   * `POST /api/chat/session` (behind LosOS Login, like every `/api/`
//!     route) makes the person's account the first time, `@<name>:<server>`,
//!     keeps their display name in step, and signs a device in for them with
//!     `m.login.application_service`. The page keeps the token; a page that
//!     has a device already names it, and gets the same device back, so its
//!     end-to-end keys stay valid.
//!   * nginx asks [`Chat::authorize`] about every request to the client and
//!     media APIs (`/internal/chat/authz`). It wants a live LosOS Login
//!     session (it asks Authelia with the request's own cookie), a person who
//!     is still on the box and not held to their own account, and, when the
//!     request carries a Matrix token, a token of that same person. So a
//!     person who signs out of LosOS Login, or is removed there, loses chat
//!     at their next request, and a token copied out of one person's browser
//!     opens nothing in another's.
//!   * Removing a person ([`Chat::close`]) also signs every one of their
//!     devices out of Dendrite, so nothing they held keeps working anywhere.
//!
//! Dendrite has no thread list (`GET /_matrix/client/v1/rooms/{room}/threads`)
//! and no thread summaries. [`Chat::threads`] answers that route from
//! Dendrite's own relations table, and fetches every event it returns
//! through Dendrite with the caller's token, so Dendrite still decides what
//! the caller may see.
//!
//! Chat belongs to the `shared` user: Dendrite runs as it and keeps
//! everything in the shared data domain, which is unlocked only while the
//! box shares its storage. lososd runs with `ProtectHome=true` and cannot
//! read that domain, so the relations table is read by `losos-chat-threads`
//! (`src/bin/losos-chat-threads.rs`), a socket-activated helper that runs as
//! `shared` and answers one question per connection ([`read_threads`]); its
//! socket is root's alone.
//!
//! Everything goes over loopback as one HTTP/1.0 exchange (Go answers 1.0
//! without chunked encoding), behind the two traits below so the logic is
//! tested without either server.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::Context;
use serde_json::{json, Value};

/// How long a token's owner is remembered. A token never changes owner, so
/// this only bounds the map.
const WHOAMI_TTL: Duration = Duration::from_secs(300);
/// How many tokens are remembered at once.
const WHOAMI_MAX: usize = 1024;
/// The most threads one page of `/threads` returns.
const THREADS_MAX: u32 = 100;
/// What a page of `/threads` returns when the client names no limit.
const THREADS_DEFAULT: u32 = 25;
/// The longest device name a page may give.
const DEVICE_NAME_MAX: usize = 64;

/// Why chat could not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    /// The box runs no chat.
    #[error("LosOS Chat is off on this box")]
    Off,
    /// The homeserver does not answer yet (starting, restarting).
    #[error("LosOS Chat is starting; try again in a moment")]
    NotUp,
    /// Who is asking cannot use chat: the spare key, the edge, or a person
    /// a rule of LosOS Login holds to their own account.
    #[error("{0}")]
    NotForYou(String),
    /// The homeserver refused, with its own status and errcode.
    #[error("the homeserver answered {status}: {errcode}")]
    Homeserver { status: u16, errcode: String },
}

impl ChatError {
    pub fn status(&self) -> u16 {
        match self {
            ChatError::Off => 404,
            ChatError::NotUp => 503,
            ChatError::NotForYou(_) => 403,
            ChatError::Homeserver { .. } => 502,
        }
    }
}

/// One answer from a server on loopback.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn errcode(&self) -> String {
        self.body
            .get("errcode")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }
}

/// The homeserver's client API, as lososd calls it.
pub trait Homeserver: Send + Sync {
    /// `path` is already encoded and may carry a query. `token` is sent as
    /// `Authorization: Bearer`.
    fn call(
        &self,
        method: &str,
        path: &str,
        token: &str,
        body: Option<&Value>,
    ) -> Result<Reply, ChatError>;
}

/// What a request to the client API carried, for LosOS Login's question.
#[derive(Debug, Clone, Default)]
pub struct Asked {
    pub cookie: Option<String>,
    pub original_url: String,
    pub original_method: String,
    pub forwarded_for: String,
}

/// LosOS Login's answer about a request: the person, with their groups, or
/// nobody.
pub trait LoginGate: Send + Sync {
    fn ask(&self, asked: &Asked) -> Result<Option<(String, Vec<String>)>, ChatError>;
}

/// How the box runs chat, from the environment `modules/chat.nix` sets.
#[derive(Debug, Clone)]
pub struct Config {
    /// `host:port` of Dendrite on loopback.
    pub homeserver: String,
    /// The part after the colon in every user ID.
    pub server_name: String,
    /// The application service token, read at each use.
    pub as_token_file: PathBuf,
    /// The socket of `losos-chat-threads`, which reads the relations table.
    pub threads_socket: PathBuf,
    /// `host:port` of LosOS Login (Authelia) on loopback.
    pub login: String,
    /// Whether other servers may reach this one.
    pub federation: bool,
}

impl Config {
    pub fn from_env() -> Option<Config> {
        let homeserver = std::env::var("LOSOS_CHAT_HOMESERVER").ok()?;
        let homeserver = homeserver.trim().to_string();
        if !homeserver.starts_with("127.") {
            tracing::warn!(%homeserver, "LOSOS_CHAT_HOMESERVER is not on loopback; chat is off");
            return None;
        }
        let server_name = std::env::var("LOSOS_CHAT_SERVER_NAME").ok()?;
        let as_token_file = PathBuf::from(std::env::var_os("LOSOS_CHAT_AS_TOKEN_FILE")?);
        let threads_socket = std::env::var_os("LOSOS_CHAT_THREADS_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/run/losos-chat/threads.sock"));
        let login = std::env::var("LOSOS_CHAT_LOGIN").unwrap_or_else(|_| "127.0.0.1:9091".into());
        let federation = std::env::var("LOSOS_CHAT_FEDERATION").is_ok_and(|v| v == "1");
        Some(Config {
            homeserver,
            server_name: server_name.trim().to_string(),
            as_token_file,
            threads_socket,
            login,
            federation,
        })
    }
}

/// A device signed in for a person.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
}

/// What `/internal/chat/authz` tells nginx.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Go on to the homeserver.
    Allow,
    /// No LosOS Login session (signed out, expired, removed): sign in.
    SignIn,
    /// Signed in, but not allowed this: a held person, or a token that is
    /// someone else's.
    Refuse,
}

/// Chat as lososd runs it. `None` inside when the box runs no chat.
pub struct Chat {
    inner: Option<Inner>,
}

struct Inner {
    config: Config,
    homeserver: Box<dyn Homeserver>,
    gate: Box<dyn LoginGate>,
    threads: Box<dyn ThreadSource>,
    /// Token hash to (user ID, when it was learned).
    owners: Mutex<HashMap<[u8; 32], (String, Instant)>>,
    /// Sign-in names whose account is made, with the display name it has.
    accounts: Mutex<HashMap<String, String>>,
    /// LosOS Login's answers being fetched now.
    asking: Flights,
}

/// LosOS Login's answer about a person, as [`LoginGate::ask`] gives it.
type Answer = Option<(String, Vec<String>)>;

/// The questions to LosOS Login in flight, by cookie and address. A page
/// sends many requests at once, each of which nginx checks; the ones that
/// arrive while LosOS Login is being asked about the same cookie wait for
/// that answer instead of asking again. Nothing is kept once it is given,
/// so a sign-out still ends chat at the next request. Chat's paths share one
/// rule in LosOS Login (`modules/identity.nix`), so the path is not part of
/// the key.
#[derive(Default)]
struct Flights(Mutex<HashMap<String, Arc<Flight>>>);

#[derive(Default)]
struct Flight {
    /// `Err(())` when the first asker's question failed.
    answer: Mutex<Option<Result<Answer, ()>>>,
    done: Condvar,
}

impl Flights {
    fn ask(&self, gate: &dyn LoginGate, asked: &Asked) -> Result<Answer, ChatError> {
        let key = format!(
            "{}\n{}",
            asked.cookie.as_deref().unwrap_or(""),
            asked.forwarded_for
        );
        let (flight, first) = {
            let mut all = lock(&self.0);
            match all.get(&key) {
                Some(f) => (f.clone(), false),
                None => {
                    let f = Arc::new(Flight::default());
                    all.insert(key.clone(), f.clone());
                    (f, true)
                }
            }
        };
        if first {
            let answer = gate.ask(asked);
            *lock(&flight.answer) = Some(answer.as_ref().map(Clone::clone).map_err(|_| ()));
            lock(&self.0).remove(&key);
            flight.done.notify_all();
            return answer;
        }
        let slot = lock(&flight.answer);
        let (slot, _) = flight
            .done
            .wait_timeout_while(slot, Duration::from_secs(30), |a| a.is_none())
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match slot.as_ref() {
            Some(Ok(answer)) => Ok(answer.clone()),
            _ => Err(ChatError::NotUp),
        }
    }
}

impl Chat {
    #[must_use]
    pub fn off() -> Chat {
        Chat { inner: None }
    }

    #[must_use]
    pub fn from_env() -> Chat {
        match Config::from_env() {
            Some(config) => {
                let homeserver = Box::new(Loopback(config.homeserver.clone()));
                let gate = Box::new(Authelia(config.login.clone()));
                let threads = Box::new(ThreadSocket(config.threads_socket.clone()));
                Chat::new(config, homeserver, gate, threads)
            }
            None => Chat::off(),
        }
    }

    #[must_use]
    pub fn new(
        config: Config,
        homeserver: Box<dyn Homeserver>,
        gate: Box<dyn LoginGate>,
        threads: Box<dyn ThreadSource>,
    ) -> Chat {
        Chat {
            inner: Some(Inner {
                config,
                homeserver,
                gate,
                threads,
                owners: Mutex::new(HashMap::new()),
                accounts: Mutex::new(HashMap::new()),
                asking: Flights::default(),
            }),
        }
    }

    /// Whether the box runs chat now: it is on and the box shares its
    /// storage (`modules/chat.nix` sets the environment only then).
    #[must_use]
    pub fn live(&self) -> bool {
        self.inner.is_some()
    }

    fn inner(&self) -> Result<&Inner, ChatError> {
        self.inner.as_ref().ok_or(ChatError::Off)
    }

    /// The Matrix user ID of a LosOS Login name on this box.
    pub fn user_id(&self, name: &str) -> Result<String, ChatError> {
        Ok(format!("@{name}:{}", self.inner()?.config.server_name))
    }

    /// `GET /api/chat`: whether the box runs chat and whether it answers.
    #[must_use]
    pub fn view(&self) -> Value {
        let Some(inner) = &self.inner else {
            return json!({ "available": false, "reason": "off" });
        };
        let up = inner
            .homeserver
            .call("GET", "/_matrix/client/versions", "", None)
            .is_ok_and(|r| r.status == 200);
        json!({
            "available": up,
            "reason": if up { "up" } else { "starting" },
            "serverName": inner.config.server_name,
            "federation": inner.config.federation,
        })
    }

    /// Make `name`'s account if it is not there yet, and keep its display
    /// name the one LosOS Login has. Remembered per name and display name,
    /// so a page load asks the homeserver nothing it already said.
    fn account(&self, token: &str, name: &str, display_name: &str) -> Result<String, ChatError> {
        let inner = self.inner()?;
        let hs = &*inner.homeserver;
        let user_id = self.user_id(name)?;
        let wanted = if display_name.trim().is_empty() {
            name
        } else {
            display_name.trim()
        };
        if lock(&inner.accounts).get(name).is_some_and(|d| d == wanted) {
            return Ok(user_id);
        }

        let made = hs.call(
            "POST",
            "/_matrix/client/v3/register",
            token,
            Some(&json!({
                "type": "m.login.application_service",
                "username": name,
                "inhibit_login": true,
            })),
        )?;
        match (made.status, made.errcode().as_str()) {
            (200, _) | (400, "M_USER_IN_USE") => {}
            (status, _) => {
                return Err(ChatError::Homeserver {
                    status,
                    errcode: made.errcode(),
                })
            }
        }

        // Set only when it differs: each change is an event in every room.
        let as_user = format!("user_id={}", encode(&user_id));
        let profile = format!(
            "/_matrix/client/v3/profile/{}/displayname",
            encode(&user_id)
        );
        let now = hs.call("GET", &format!("{profile}?{as_user}"), token, None)?;
        let current = now.body.get("displayname").and_then(Value::as_str);
        if current != Some(wanted) {
            let set = hs.call(
                "PUT",
                &format!("{profile}?{as_user}"),
                token,
                Some(&json!({ "displayname": wanted })),
            )?;
            if set.status != 200 {
                tracing::warn!(status = set.status, user = %user_id, "chat display name not set");
                return Ok(user_id);
            }
        }
        lock(&inner.accounts).insert(name.to_string(), wanted.to_string());
        Ok(user_id)
    }

    /// `GET /api/chat/people`: everyone on the box who can use chat, each
    /// with an account made, so a person can be found and invited before
    /// they ever opened chat. The homeserver's own directory lists only
    /// people who already share a room.
    pub fn people(&self, everyone: &[crate::people::Listed]) -> Result<Vec<Value>, ChatError> {
        let inner = self.inner()?;
        let token = inner.as_token()?;
        let mut out = Vec::new();
        for person in everyone {
            if person.disabled || person.groups.iter().any(|g| g == crate::people::LOCKED) {
                continue;
            }
            let user_id = self.account(&token, &person.name, &person.display_name)?;
            let shown = if person.display_name.trim().is_empty() {
                person.name.clone()
            } else {
                person.display_name.trim().to_string()
            };
            out.push(json!({ "userId": user_id, "displayName": shown }));
        }
        Ok(out)
    }

    /// `POST /api/chat/session`: make the person's account if it is not
    /// there yet, and sign a device in for them. `device` is the device the
    /// page had, to keep; `device_name` what the page calls itself.
    pub fn session(
        &self,
        person: &crate::people::Person,
        device: Option<&str>,
        device_name: Option<&str>,
    ) -> Result<Session, ChatError> {
        let inner = self.inner()?;
        if person.groups.iter().any(|g| g == crate::people::LOCKED) {
            return Err(ChatError::NotForYou(
                "finish what LosOS Login asks of your account first".to_string(),
            ));
        }
        let token = inner.as_token()?;
        let hs = &*inner.homeserver;
        let user_id = self.account(&token, &person.name, &person.display_name)?;

        let mut login = json!({
            "type": "m.login.application_service",
            "identifier": { "type": "m.id.user", "user": person.name },
            "initial_device_display_name": device_name
                .map(clean_device_name)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "LosOS Chat".to_string()),
        });
        if let Some(device) = device.filter(|d| valid_device_id(d)) {
            login["device_id"] = json!(device);
        }
        let answer = hs.call("POST", "/_matrix/client/v3/login", &token, Some(&login))?;
        if answer.status != 200 {
            return Err(ChatError::Homeserver {
                status: answer.status,
                errcode: answer.errcode(),
            });
        }
        let field = |k: &str| {
            answer
                .body
                .get(k)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or(ChatError::Homeserver {
                    status: 200,
                    errcode: format!("no {k} in the login answer"),
                })
        };
        let session = Session {
            user_id: field("user_id")?,
            access_token: field("access_token")?,
            device_id: field("device_id")?,
        };
        if session.user_id != user_id {
            return Err(ChatError::Homeserver {
                status: 200,
                errcode: "the login answer names someone else".to_string(),
            });
        }
        inner.remember(&session.access_token, &session.user_id);
        Ok(session)
    }

    /// Sign every device of a person out of the homeserver: they were
    /// removed from LosOS Login. An account that never used chat has
    /// nothing to sign out.
    pub fn close(&self, name: &str) -> Result<(), ChatError> {
        let inner = self.inner()?;
        let token = inner.as_token()?;
        let hs = &*inner.homeserver;
        let login = hs.call(
            "POST",
            "/_matrix/client/v3/login",
            &token,
            Some(&json!({
                "type": "m.login.application_service",
                "identifier": { "type": "m.id.user", "user": name },
                "initial_device_display_name": "LosOS sign-out",
            })),
        )?;
        let Some(theirs) = login
            .body
            .get("access_token")
            .and_then(Value::as_str)
            .filter(|_| login.status == 200)
        else {
            // Refused: nothing to sign out. (Dendrite signs in even a name
            // that never used chat; logout/all below then ends that device
            // too, so either way nothing is left signed in.)
            return Ok(());
        };
        let out = hs.call(
            "POST",
            "/_matrix/client/v3/logout/all",
            theirs,
            Some(&json!({})),
        )?;
        if out.status != 200 {
            return Err(ChatError::Homeserver {
                status: out.status,
                errcode: out.errcode(),
            });
        }
        lock(&inner.owners).clear();
        lock(&inner.accounts).remove(name);
        Ok(())
    }

    /// `/internal/chat/authz`: may this request reach the homeserver?
    /// `token` is the Matrix token it carries, if any.
    pub fn authorize(
        &self,
        people: &crate::people::People,
        asked: &Asked,
        token: Option<&str>,
    ) -> Result<Verdict, ChatError> {
        let inner = self.inner()?;
        let url = &asked.original_url;
        if asked.cookie.as_deref().is_none_or(str::is_empty) {
            tracing::info!(%url, "chat: no LosOS Login cookie");
            return Ok(Verdict::SignIn);
        }
        let Some((name, groups)) = inner.asking.ask(inner.gate.as_ref(), asked)? else {
            tracing::info!(%url, "chat: LosOS Login does not know the cookie");
            return Ok(Verdict::SignIn);
        };
        // Still on the box: LosOS Login reads the people file on every
        // request, and so does this.
        let person = people.get(&name).ok().flatten();
        let Some(person) = person else {
            tracing::info!(%url, %name, "chat: no such person");
            return Ok(Verdict::SignIn);
        };
        if person
            .groups
            .iter()
            .chain(groups.iter())
            .any(|g| g == crate::people::LOCKED)
        {
            return Ok(Verdict::Refuse);
        }
        let Some(token) = token.filter(|t| !t.is_empty()) else {
            return Ok(Verdict::Allow);
        };
        match inner.owner_of(token)? {
            Some(owner) if owner == self.user_id(&person.name)? => Ok(Verdict::Allow),
            Some(owner) => {
                tracing::info!(%url, %owner, name = %person.name, "chat: someone else's token");
                Ok(Verdict::Refuse)
            }
            // A token the homeserver does not know: let the homeserver say
            // so, in its own words, so the page signs the device in again.
            None => Ok(Verdict::Allow),
        }
    }

    /// `GET /_matrix/client/v1/rooms/{room}/threads`, which Dendrite lacks:
    /// the room's threads, the most recently active first, each root event
    /// with its summary under `unsigned["m.relations"]["m.thread"]`.
    pub fn threads(
        &self,
        token: &str,
        room: &str,
        query: &ThreadsQuery,
    ) -> Result<Reply, ChatError> {
        let inner = self.inner()?;
        let hs = &*inner.homeserver;
        let whoami = hs.call("GET", "/_matrix/client/v3/account/whoami", token, None)?;
        if whoami.status != 200 {
            return Ok(whoami);
        }
        let Some(me) = whoami.body.get("user_id").and_then(Value::as_str) else {
            return Err(ChatError::Homeserver {
                status: 200,
                errcode: "whoami named nobody".to_string(),
            });
        };
        let rows = inner.threads.rows(room, me, query).map_err(|e| {
            tracing::warn!(error = ?e, "could not read the thread list");
            ChatError::NotUp
        })?;
        let mut chunk = Vec::new();
        let mut last = None;
        for row in &rows {
            last = Some(row.latest);
            let get = |id: &str| {
                hs.call(
                    "GET",
                    &format!(
                        "/_matrix/client/v3/rooms/{}/event/{}",
                        encode(room),
                        encode(id)
                    ),
                    token,
                    None,
                )
            };
            let root = get(&row.root)?;
            if root.status != 200 {
                // Not the caller's to see.
                continue;
            }
            let latest = get(&row.latest_event)?;
            let mut event = root.body;
            let participated =
                row.participated || event.get("sender").and_then(Value::as_str) == Some(me);
            if query.participated && !participated {
                continue;
            }
            let summary = json!({
                "latest_event": if latest.status == 200 { latest.body } else { Value::Null },
                "count": row.count,
                "current_user_participated": participated,
            });
            if !event.get("unsigned").is_some_and(Value::is_object) {
                event["unsigned"] = json!({});
            }
            if !event["unsigned"]
                .get("m.relations")
                .is_some_and(Value::is_object)
            {
                event["unsigned"]["m.relations"] = json!({});
            }
            event["unsigned"]["m.relations"]["m.thread"] = summary;
            chunk.push(event);
        }
        let mut body = json!({ "chunk": chunk });
        if rows.len() as u32 == query.limit {
            if let Some(last) = last {
                body["next_batch"] = json!(last.to_string());
            }
        }
        Ok(Reply {
            status: 200,
            headers: Vec::new(),
            body,
        })
    }
}

impl Inner {
    fn as_token(&self) -> Result<String, ChatError> {
        match std::fs::read_to_string(&self.config.as_token_file) {
            Ok(t) if !t.trim().is_empty() => Ok(t.trim().to_string()),
            _ => Err(ChatError::NotUp),
        }
    }

    fn remember(&self, token: &str, user_id: &str) {
        let mut owners = lock(&self.owners);
        if owners.len() >= WHOAMI_MAX {
            owners.retain(|_, (_, at)| at.elapsed() < WHOAMI_TTL);
            if owners.len() >= WHOAMI_MAX {
                owners.clear();
            }
        }
        owners.insert(
            crate::boxid::sha256(token.as_bytes()),
            (user_id.to_string(), Instant::now()),
        );
    }

    /// Whose token this is, asking the homeserver when it is not known yet.
    /// `None`: the homeserver does not know the token.
    fn owner_of(&self, token: &str) -> Result<Option<String>, ChatError> {
        let key = crate::boxid::sha256(token.as_bytes());
        if let Some((owner, at)) = lock(&self.owners).get(&key) {
            if at.elapsed() < WHOAMI_TTL {
                return Ok(Some(owner.clone()));
            }
        }
        let answer =
            self.homeserver
                .call("GET", "/_matrix/client/v3/account/whoami", token, None)?;
        let owner = answer
            .body
            .get("user_id")
            .and_then(Value::as_str)
            .filter(|_| answer.status == 200)
            .map(str::to_string);
        if let Some(owner) = &owner {
            self.remember(token, owner);
        }
        Ok(owner)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Who reads the relations table for [`Chat::threads`].
pub trait ThreadSource: Send + Sync {
    fn rows(&self, room: &str, me: &str, query: &ThreadsQuery) -> anyhow::Result<Vec<ThreadRow>>;
}

/// The question `losos-chat-threads` answers, one JSON line per connection.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadsAsk {
    pub room: String,
    pub me: String,
    pub limit: u32,
    pub from: Option<i64>,
}

/// `losos-chat-threads` behind its socket.
pub struct ThreadSocket(pub PathBuf);

impl ThreadSource for ThreadSocket {
    fn rows(&self, room: &str, me: &str, query: &ThreadsQuery) -> anyhow::Result<Vec<ThreadRow>> {
        use std::os::unix::net::UnixStream;
        let mut stream = UnixStream::connect(&self.0)
            .with_context(|| format!("connecting to {}", self.0.display()))?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let ask = ThreadsAsk {
            room: room.to_string(),
            me: me.to_string(),
            limit: query.limit.min(THREADS_MAX),
            from: query.from,
        };
        let mut line = serde_json::to_vec(&ask)?;
        line.push(b'\n');
        stream.write_all(&line)?;
        stream.shutdown(std::net::Shutdown::Write)?;
        let mut raw = Vec::new();
        stream.take(4 * 1024 * 1024).read_to_end(&mut raw)?;
        let answer: Value = serde_json::from_slice(&raw).context("the thread list was not JSON")?;
        if let Some(e) = answer.get("error").and_then(Value::as_str) {
            anyhow::bail!("losos-chat-threads: {e}");
        }
        Ok(serde_json::from_value(
            answer.get("rows").cloned().unwrap_or_default(),
        )?)
    }
}

/// The query of a `/threads` request.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadsQuery {
    pub limit: u32,
    /// Threads whose latest reply is older than this position.
    pub from: Option<i64>,
    /// `include=participated`: only threads the caller wrote in.
    pub participated: bool,
}

impl ThreadsQuery {
    #[must_use]
    pub fn parse(q: &HashMap<String, String>) -> ThreadsQuery {
        let limit = q
            .get("limit")
            .and_then(|l| l.parse::<u32>().ok())
            .unwrap_or(THREADS_DEFAULT)
            .clamp(1, THREADS_MAX);
        ThreadsQuery {
            limit,
            from: q.get("from").and_then(|f| f.parse().ok()),
            participated: q.get("include").is_some_and(|i| i == "participated"),
        }
    }
}

/// One thread, as the relations table has it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThreadRow {
    pub root: String,
    pub latest: i64,
    pub latest_event: String,
    pub count: u64,
    /// Whether the caller wrote a reply in it (the root is checked apart).
    pub participated: bool,
}

/// The room's threads from Dendrite's sync database, newest activity first.
pub fn read_threads(db: &std::path::Path, ask: &ThreadsAsk) -> anyhow::Result<Vec<ThreadRow>> {
    use rusqlite::{params, OpenFlags};
    let conn = rusqlite::Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening {}", db.display()))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let mut stmt = conn.prepare(
        "SELECT r.event_id, MAX(r.id) AS latest, COUNT(*),
                (SELECT c.child_event_id FROM syncapi_relations c
                  WHERE c.room_id = r.room_id AND c.event_id = r.event_id
                    AND c.rel_type = 'm.thread' ORDER BY c.id DESC LIMIT 1),
                EXISTS (SELECT 1 FROM syncapi_relations p
                          JOIN syncapi_output_room_events e ON e.event_id = p.child_event_id
                         WHERE p.room_id = r.room_id AND p.event_id = r.event_id
                           AND p.rel_type = 'm.thread' AND e.sender = ?2)
           FROM syncapi_relations r
          WHERE r.room_id = ?1 AND r.rel_type = 'm.thread'
          GROUP BY r.event_id
         HAVING ?3 IS NULL OR latest < ?3
          ORDER BY latest DESC
          LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(
            params![ask.room, ask.me, ask.from, ask.limit.min(THREADS_MAX)],
            |row| {
                Ok(ThreadRow {
                    root: row.get(0)?,
                    latest: row.get(1)?,
                    count: row.get::<_, i64>(2)?.max(0) as u64,
                    latest_event: row.get(3)?,
                    participated: row.get(4)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A device ID a page may ask for again: what Dendrite makes (URL-safe
/// base64, `6ZsB-2Hy`), and no more.
fn valid_device_id(d: &str) -> bool {
    (1..=32).contains(&d.len())
        && d.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn clean_device_name(n: &str) -> String {
    n.chars()
        .filter(|c| !c.is_control())
        .take(DEVICE_NAME_MAX)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Percent-encode one path segment or query value.
#[must_use]
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The Matrix token of a request: the `Authorization: Bearer` header, or
/// the deprecated `access_token` query parameter of its original URL.
#[must_use]
pub fn token_of(authorization: Option<&str>, original_url: &str) -> Option<String> {
    if let Some(t) = authorization.and_then(|h| {
        h.strip_prefix("Bearer ")
            .or_else(|| h.strip_prefix("bearer "))
    }) {
        return Some(t.trim().to_string());
    }
    let query = original_url.split_once('?')?.1;
    query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == "access_token")
        .map(|(_, v)| v.to_string())
}

// ── The two servers on loopback ───────────────────────────────────────────

/// A status, its headers and the body, as one exchange returned them.
type Raw = (u16, Vec<(String, String)>, Vec<u8>);

/// One HTTP/1.0 exchange with a server on loopback.
fn exchange(addr: &str, request: &[u8]) -> Result<Raw, ChatError> {
    use std::net::{TcpStream, ToSocketAddrs};
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
    let to = addr
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .ok_or(ChatError::NotUp)?;
    let mut stream =
        TcpStream::connect_timeout(&to, Duration::from_secs(3)).map_err(|_| ChatError::NotUp)?;
    let io = |e: std::io::Error| {
        tracing::warn!(error = %e, %addr, "loopback exchange failed");
        ChatError::NotUp
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .map_err(io)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(io)?;
    stream.write_all(request).map_err(io)?;
    let mut raw = Vec::new();
    (&mut stream)
        .take(MAX_BYTES)
        .read_to_end(&mut raw)
        .map_err(io)?;
    let status = crate::signin::status_code(&raw).ok_or(ChatError::NotUp)?;
    let at = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or(ChatError::NotUp)?;
    let head = String::from_utf8_lossy(&raw[..at]);
    let headers = head
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    Ok((status, headers, raw[at + 4..].to_vec()))
}

/// A header value that cannot break the request it goes into.
fn header_safe(v: &str) -> bool {
    !v.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0)
}

/// Dendrite on loopback.
pub struct Loopback(pub String);

impl Homeserver for Loopback {
    fn call(
        &self,
        method: &str,
        path: &str,
        token: &str,
        body: Option<&Value>,
    ) -> Result<Reply, ChatError> {
        if !header_safe(path) || !header_safe(token) || path.contains(' ') {
            return Err(ChatError::NotForYou(
                "not a request lososd sends".to_string(),
            ));
        }
        let body = body.map(Value::to_string).unwrap_or_default();
        let mut req = format!(
            "{method} {path} HTTP/1.0\r\nHost: {}\r\nAccept: application/json\r\nContent-Length: {}\r\n",
            self.0,
            body.len()
        );
        if !token.is_empty() {
            req.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        if !body.is_empty() {
            req.push_str("Content-Type: application/json\r\n");
        }
        req.push_str("\r\n");
        req.push_str(&body);
        let (status, headers, raw) = exchange(&self.0, req.as_bytes())?;
        Ok(Reply {
            status,
            headers,
            body: serde_json::from_slice(&raw).unwrap_or(Value::Null),
        })
    }
}

/// LosOS Login (Authelia) on loopback, asked the question nginx asks it
/// before every `/api/` request.
pub struct Authelia(pub String);

impl LoginGate for Authelia {
    fn ask(&self, asked: &Asked) -> Result<Option<(String, Vec<String>)>, ChatError> {
        let cookie = asked.cookie.as_deref().unwrap_or("");
        for v in [
            cookie,
            &asked.original_url,
            &asked.original_method,
            &asked.forwarded_for,
        ] {
            if !header_safe(v) {
                return Ok(None);
            }
        }
        let req = format!(
            "GET /auth/api/authz/auth-request HTTP/1.0\r\nHost: {}\r\nCookie: {cookie}\r\n\
             X-Original-URL: {}\r\nX-Original-Method: {}\r\nX-Forwarded-For: {}\r\n\r\n",
            self.0, asked.original_url, asked.original_method, asked.forwarded_for
        );
        let (status, headers, _) = exchange(&self.0, req.as_bytes())?;
        let reply = Reply {
            status,
            headers,
            body: Value::Null,
        };
        if !(200..300).contains(&status) {
            return Ok(None);
        }
        let Some(name) = reply.header("remote-user").filter(|n| !n.is_empty()) else {
            return Ok(None);
        };
        let groups = reply
            .header("remote-groups")
            .unwrap_or("")
            .split(',')
            .map(|g| g.trim().to_string())
            .filter(|g| !g.is_empty())
            .collect();
        Ok(Some((name.to_string(), groups)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A homeserver in memory: accounts, devices and tokens, enough of the
    /// client API for lososd's calls.
    #[derive(Default)]
    struct Fake {
        state: Mutex<FakeState>,
    }

    #[derive(Default)]
    struct FakeState {
        accounts: HashMap<String, String>,
        /// token -> (user, device)
        tokens: HashMap<String, (String, String)>,
        events: HashMap<String, Value>,
        hidden: Vec<String>,
        calls: Vec<String>,
        next: u32,
        down: bool,
    }

    const AS: &str = "as-token";
    const SERVER: &str = "mattbox.local";

    impl Homeserver for Arc<Fake> {
        fn call(
            &self,
            method: &str,
            path: &str,
            token: &str,
            body: Option<&Value>,
        ) -> Result<Reply, ChatError> {
            let mut s = self.state.lock().unwrap();
            if s.down {
                return Err(ChatError::NotUp);
            }
            s.calls.push(format!("{method} {path}"));
            let ok = |body: Value| {
                Ok(Reply {
                    status: 200,
                    headers: vec![],
                    body,
                })
            };
            let no = |status: u16, code: &str| {
                Ok(Reply {
                    status,
                    headers: vec![],
                    body: json!({ "errcode": code }),
                })
            };
            let body = body.cloned().unwrap_or(Value::Null);
            let route = path.split('?').next().unwrap();
            match (method, route) {
                ("GET", "/_matrix/client/versions") => ok(json!({"versions": ["v1.2"]})),
                ("POST", "/_matrix/client/v3/register") => {
                    assert_eq!(token, AS);
                    let name = body["username"].as_str().unwrap().to_string();
                    if s.accounts.contains_key(&name) {
                        return no(400, "M_USER_IN_USE");
                    }
                    s.accounts.insert(name.clone(), String::new());
                    ok(json!({ "user_id": format!("@{name}:{SERVER}") }))
                }
                ("POST", "/_matrix/client/v3/login") => {
                    assert_eq!(token, AS);
                    let name = body["identifier"]["user"].as_str().unwrap().to_string();
                    if !s.accounts.contains_key(&name) {
                        return no(403, "M_FORBIDDEN");
                    }
                    s.next += 1;
                    let device = body["device_id"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("DEV{}", s.next));
                    let t = format!("tok{}", s.next);
                    let user = format!("@{name}:{SERVER}");
                    s.tokens.insert(t.clone(), (user.clone(), device.clone()));
                    ok(json!({ "user_id": user, "access_token": t, "device_id": device }))
                }
                ("POST", "/_matrix/client/v3/logout/all") => {
                    let Some((user, _)) = s.tokens.get(token).cloned() else {
                        return no(401, "M_UNKNOWN_TOKEN");
                    };
                    s.tokens.retain(|_, (u, _)| *u != user);
                    ok(json!({}))
                }
                ("GET", "/_matrix/client/v3/account/whoami") => match s.tokens.get(token) {
                    Some((user, device)) => ok(json!({ "user_id": user, "device_id": device })),
                    None => no(401, "M_UNKNOWN_TOKEN"),
                },
                (_, p) if p.contains("/displayname") => {
                    let name = p.split('/').nth(5).unwrap().to_string();
                    let name = name.replace("%40", "@").replace("%3A", ":");
                    let local = name
                        .trim_start_matches('@')
                        .split(':')
                        .next()
                        .unwrap()
                        .to_string();
                    if method == "PUT" {
                        let dn = body["displayname"].as_str().unwrap().to_string();
                        s.accounts.insert(local, dn);
                        ok(json!({}))
                    } else {
                        ok(
                            json!({ "displayname": s.accounts.get(&local).cloned().unwrap_or_default() }),
                        )
                    }
                }
                ("GET", p) if p.contains("/event/") => {
                    if !s.tokens.contains_key(token) {
                        return no(401, "M_UNKNOWN_TOKEN");
                    }
                    let id = p.rsplit('/').next().unwrap().replace("%24", "$");
                    if s.hidden.contains(&id) {
                        return no(404, "M_NOT_FOUND");
                    }
                    match s.events.get(&id) {
                        Some(e) => ok(e.clone()),
                        None => no(404, "M_NOT_FOUND"),
                    }
                }
                _ => no(404, "M_UNRECOGNIZED"),
            }
        }
    }

    struct Gate(Mutex<Option<(String, Vec<String>)>>);
    impl LoginGate for Arc<Gate> {
        fn ask(&self, _: &Asked) -> Result<Option<(String, Vec<String>)>, ChatError> {
            Ok(self.0.lock().unwrap().clone())
        }
    }

    /// A LosOS Login that takes its time and counts the questions.
    struct Slow(std::sync::atomic::AtomicUsize);
    impl LoginGate for Slow {
        fn ask(&self, _: &Asked) -> Result<Option<(String, Vec<String>)>, ChatError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(300));
            Ok(Some(("eva".into(), vec![])))
        }
    }

    #[test]
    fn requests_at_once_share_one_question_and_none_is_kept() {
        let gate = Slow(std::sync::atomic::AtomicUsize::new(0));
        let flights = Flights::default();
        let mut cookie = asked();
        cookie.cookie = Some("authelia_session=a".into());
        std::thread::scope(|s| {
            let answers: Vec<_> = (0..8)
                .map(|_| s.spawn(|| flights.ask(&gate, &cookie).unwrap()))
                .collect();
            for a in answers {
                assert_eq!(a.join().unwrap(), Some(("eva".into(), vec![])));
            }
        });
        assert_eq!(gate.0.load(std::sync::atomic::Ordering::SeqCst), 1);
        // The next request asks again: a sign-out in between counts.
        flights.ask(&gate, &cookie).unwrap();
        assert_eq!(gate.0.load(std::sync::atomic::Ordering::SeqCst), 2);
        // Another cookie is another question.
        let mut other = cookie.clone();
        other.cookie = Some("authelia_session=b".into());
        flights.ask(&gate, &other).unwrap();
        assert_eq!(gate.0.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    struct Rig {
        chat: Chat,
        hs: Arc<Fake>,
        gate: Arc<Gate>,
        _dir: assert_fs::TempDir,
        people: crate::people::People,
        db: PathBuf,
    }

    /// The helper's own function, on a file, as the socket would run it.
    struct File(PathBuf);
    impl ThreadSource for File {
        fn rows(&self, room: &str, me: &str, q: &ThreadsQuery) -> anyhow::Result<Vec<ThreadRow>> {
            read_threads(
                &self.0,
                &ThreadsAsk {
                    room: room.into(),
                    me: me.into(),
                    limit: q.limit,
                    from: q.from,
                },
            )
        }
    }

    fn rig() -> Rig {
        let dir = assert_fs::TempDir::new().unwrap();
        let as_file = dir.path().join("as_token");
        std::fs::write(&as_file, format!("{AS}\n")).unwrap();
        let db = dir.path().join("syncserver.db");
        let hs = Arc::new(Fake::default());
        let gate = Arc::new(Gate(Mutex::new(None)));
        let config = Config {
            homeserver: "127.0.0.1:8008".into(),
            server_name: SERVER.into(),
            as_token_file: as_file,
            threads_socket: dir.path().join("threads.sock"),
            login: "127.0.0.1:9091".into(),
            federation: false,
        };
        let chat = Chat::new(
            config,
            Box::new(hs.clone()),
            Box::new(gate.clone()),
            Box::new(File(db.clone())),
        );
        let people = crate::people::People::new(dir.path().join("users.yml"), None);
        Rig {
            chat,
            hs,
            gate,
            _dir: dir,
            people,
            db,
        }
    }

    fn person(name: &str, display: &str, groups: &[&str]) -> crate::people::Person {
        crate::people::Person {
            name: name.into(),
            display_name: display.into(),
            groups: groups.iter().map(|g| g.to_string()).collect(),
        }
    }

    fn add(people: &crate::people::People, name: &str) {
        let groups: &[&str] = if name == "matus" {
            &[crate::people::ADMINS]
        } else {
            &[]
        };
        // A fresh password per run: these tests never sign in with it.
        let random = crate::io_backend::random_bytes::<12>().expect("random bytes");
        let hex: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let secret = crate::setup::Secret::new(&format!("Pw-{hex}-9"));
        people
            .add(name, name, "", &secret, groups)
            .expect("person added");
    }

    fn asked() -> Asked {
        Asked {
            cookie: Some("losos_session=abc".into()),
            original_url: "https://mattbox.local/_matrix/client/v3/sync".into(),
            original_method: "GET".into(),
            forwarded_for: "192.168.1.2".into(),
        }
    }

    #[test]
    fn a_first_session_makes_the_account_names_it_and_signs_a_device_in() {
        let r = rig();
        let s = r
            .chat
            .session(
                &person("matus", "Matúš", &["losos-admins"]),
                None,
                Some("Firefox"),
            )
            .unwrap();
        assert_eq!(s.user_id, "@matus:mattbox.local");
        assert!(s.access_token.starts_with("tok"));
        let state = r.hs.state.lock().unwrap();
        assert_eq!(state.accounts["matus"], "Matúš");
        assert!(state.calls.iter().any(|c| c.starts_with(
            "PUT /_matrix/client/v3/profile/%40matus%3Amattbox.local/displayname?user_id="
        )));
    }

    #[test]
    fn a_second_session_keeps_the_account_and_the_device_it_names() {
        let r = rig();
        let p = person("matus", "Matúš", &[]);
        let first = r.chat.session(&p, None, None).unwrap();
        let again = r.chat.session(&p, Some(&first.device_id), None).unwrap();
        assert_eq!(again.device_id, first.device_id);
        assert_ne!(again.access_token, first.access_token);
        // The name had not changed, so it was not set a second time.
        let puts =
            r.hs.state
                .lock()
                .unwrap()
                .calls
                .iter()
                .filter(|c| c.starts_with("PUT "))
                .count();
        assert_eq!(puts, 1);
    }

    #[test]
    fn everyone_on_the_box_gets_an_account_once_and_the_held_are_left_out() {
        let r = rig();
        add(&r.people, "matus");
        add(&r.people, "eva");
        add(&r.people, "jan");
        let mut marks = std::collections::BTreeMap::new();
        marks.insert("jan".to_string(), vec![crate::people::LOCKED]);
        r.people.set_marks(&marks).unwrap();
        let listed = r.chat.people(&r.people.list().unwrap()).unwrap();
        let ids: Vec<&str> = listed.iter().filter_map(|p| p["userId"].as_str()).collect();
        assert_eq!(ids, ["@eva:mattbox.local", "@matus:mattbox.local"]);
        assert_eq!(r.hs.state.lock().unwrap().accounts["eva"], "eva");
        // A second look asks the homeserver nothing new.
        let before = r.hs.state.lock().unwrap().calls.len();
        r.chat.people(&r.people.list().unwrap()).unwrap();
        assert_eq!(r.hs.state.lock().unwrap().calls.len(), before);
    }

    #[test]
    fn a_device_id_that_is_not_one_is_not_passed_on() {
        let r = rig();
        let s = r
            .chat
            .session(&person("eva", "", &[]), Some("../../x\r\nEvil: 1"), None)
            .unwrap();
        assert!(s.device_id.starts_with("DEV"));
        // An empty display name falls back to the sign-in name.
        assert_eq!(r.hs.state.lock().unwrap().accounts["eva"], "eva");
    }

    #[test]
    fn a_held_person_gets_no_chat() {
        let r = rig();
        let err = r
            .chat
            .session(&person("eva", "Eva", &[crate::people::LOCKED]), None, None)
            .unwrap_err();
        assert_eq!(err.status(), 403);
        assert!(r.hs.state.lock().unwrap().calls.is_empty());
    }

    #[test]
    fn chat_off_and_chat_starting_say_so() {
        assert!(matches!(
            Chat::off().session(&person("a", "", &[]), None, None),
            Err(ChatError::Off)
        ));
        assert_eq!(Chat::off().view()["reason"], "off");
        let r = rig();
        r.hs.state.lock().unwrap().down = true;
        assert!(matches!(
            r.chat.session(&person("a", "", &[]), None, None),
            Err(ChatError::NotUp)
        ));
        assert_eq!(r.chat.view()["reason"], "starting");
    }

    #[test]
    fn closing_an_account_signs_every_device_out() {
        let r = rig();
        let p = person("eva", "Eva", &[]);
        let a = r.chat.session(&p, None, None).unwrap();
        let b = r.chat.session(&p, None, None).unwrap();
        let other = r
            .chat
            .session(&person("matus", "M", &[]), None, None)
            .unwrap();
        r.chat.close("eva").unwrap();
        let state = r.hs.state.lock().unwrap();
        assert!(!state.tokens.contains_key(&a.access_token));
        assert!(!state.tokens.contains_key(&b.access_token));
        assert!(state.tokens.contains_key(&other.access_token));
        drop(state);
        // Someone who never used chat has nothing to close.
        r.chat.close("nobody").unwrap();
    }

    #[test]
    fn authorize_wants_a_live_sign_in_of_the_token_s_own_person() {
        let r = rig();
        add(&r.people, "eva");
        add(&r.people, "matus");
        let eva = r
            .chat
            .session(&person("eva", "Eva", &[]), None, None)
            .unwrap();
        let matus = r
            .chat
            .session(&person("matus", "M", &[]), None, None)
            .unwrap();

        // No cookie, or LosOS Login does not know it: sign in.
        let mut no_cookie = asked();
        no_cookie.cookie = None;
        assert_eq!(
            r.chat
                .authorize(&r.people, &no_cookie, Some(&eva.access_token))
                .unwrap(),
            Verdict::SignIn
        );
        assert_eq!(
            r.chat
                .authorize(&r.people, &asked(), Some(&eva.access_token))
                .unwrap(),
            Verdict::SignIn
        );

        *r.gate.0.lock().unwrap() = Some(("eva".into(), vec![]));
        assert_eq!(
            r.chat
                .authorize(&r.people, &asked(), Some(&eva.access_token))
                .unwrap(),
            Verdict::Allow
        );
        // No token at all (the versions, a login flow): the sign-in is enough.
        assert_eq!(
            r.chat.authorize(&r.people, &asked(), None).unwrap(),
            Verdict::Allow
        );
        // Someone else's token in Eva's browser.
        assert_eq!(
            r.chat
                .authorize(&r.people, &asked(), Some(&matus.access_token))
                .unwrap(),
            Verdict::Refuse
        );
        // A token the homeserver forgot goes on, so it can say so itself.
        assert_eq!(
            r.chat
                .authorize(&r.people, &asked(), Some("stale"))
                .unwrap(),
            Verdict::Allow
        );

        // Held to her own account.
        *r.gate.0.lock().unwrap() = Some(("eva".into(), vec![crate::people::LOCKED.into()]));
        assert_eq!(
            r.chat
                .authorize(&r.people, &asked(), Some(&eva.access_token))
                .unwrap(),
            Verdict::Refuse
        );

        // Removed from the box while LosOS Login still had her session.
        *r.gate.0.lock().unwrap() = Some(("eva".into(), vec![]));
        r.people.remove("eva", Some("matus")).unwrap();
        assert_eq!(
            r.chat
                .authorize(&r.people, &asked(), Some(&eva.access_token))
                .unwrap(),
            Verdict::SignIn
        );
    }

    #[test]
    fn tokens_are_read_from_the_header_or_the_old_query_parameter() {
        assert_eq!(token_of(Some("Bearer abc"), "/x").as_deref(), Some("abc"));
        assert_eq!(
            token_of(
                None,
                "https://h/_matrix/media/v3/download/a/b?access_token=xyz&x=1"
            )
            .as_deref(),
            Some("xyz")
        );
        assert_eq!(token_of(Some("Basic Zm9v"), "/x"), None);
        assert_eq!(token_of(None, "/x"), None);
    }

    #[test]
    fn encoding_keeps_only_unreserved_bytes() {
        assert_eq!(encode("@matus:mattbox.local"), "%40matus%3Amattbox.local");
        assert_eq!(encode("!abc:h"), "%21abc%3Ah");
        assert_eq!(encode("$ev/../x"), "%24ev%2F..%2Fx");
    }

    fn sync_db(path: &std::path::Path) -> rusqlite::Connection {
        let c = rusqlite::Connection::open(path).unwrap();
        c.execute_batch(
            "CREATE TABLE syncapi_relations (id BIGINT PRIMARY KEY, room_id TEXT NOT NULL,
               event_id TEXT NOT NULL, child_event_id TEXT NOT NULL, child_event_type TEXT NOT NULL,
               rel_type TEXT NOT NULL, UNIQUE (room_id, event_id, child_event_id, rel_type));
             CREATE TABLE syncapi_output_room_events (id INTEGER PRIMARY KEY AUTOINCREMENT,
               event_id TEXT NOT NULL UNIQUE, room_id TEXT NOT NULL, headered_event_json TEXT NOT NULL,
               type TEXT NOT NULL, sender TEXT NOT NULL, contains_url BOOL NOT NULL,
               add_state_ids TEXT, remove_state_ids TEXT, session_id BIGINT, transaction_id TEXT,
               exclude_from_sync BOOL NOT NULL DEFAULT FALSE, history_visibility SMALLINT NOT NULL DEFAULT 2);",
        )
        .unwrap();
        c
    }

    fn event(c: &rusqlite::Connection, id: &str, sender: &str) {
        c.execute(
            "INSERT INTO syncapi_output_room_events (event_id, room_id, headered_event_json, type, sender, contains_url)
             VALUES (?1, '!r:h', '{}', 'm.room.message', ?2, 0)",
            rusqlite::params![id, sender],
        )
        .unwrap();
    }

    fn reply(c: &rusqlite::Connection, pos: i64, root: &str, child: &str, rel: &str) {
        c.execute(
            "INSERT INTO syncapi_relations VALUES (?1, '!r:h', ?2, ?3, 'm.room.message', ?4)",
            rusqlite::params![pos, root, child, rel],
        )
        .unwrap();
    }

    #[test]
    fn threads_come_newest_first_with_their_summaries() {
        let r = rig();
        add(&r.people, "eva");
        let eva = r
            .chat
            .session(&person("eva", "Eva", &[]), None, None)
            .unwrap();
        let c = sync_db(&r.db);
        for (id, sender) in [
            ("$a", "@matus:h"),
            ("$a1", "@matus:h"),
            ("$a2", "@eva:mattbox.local"),
            ("$b", "@matus:h"),
            ("$b1", "@matus:h"),
            ("$hidden", "@matus:h"),
            ("$h1", "@matus:h"),
        ] {
            event(&c, id, sender);
        }
        reply(&c, 1, "$a", "$a1", "m.thread");
        reply(&c, 2, "$b", "$b1", "m.thread");
        reply(&c, 3, "$a", "$a2", "m.thread");
        reply(&c, 4, "$hidden", "$h1", "m.thread");
        // A reaction is a relation but not a thread.
        reply(&c, 5, "$b", "$like", "m.annotation");
        {
            let mut s = r.hs.state.lock().unwrap();
            for id in ["$a", "$a1", "$a2", "$b", "$b1", "$h1", "$hidden"] {
                s.events.insert(
                    id.into(),
                    json!({ "event_id": id, "sender": if id == "$a2" { "@eva:mattbox.local" } else { "@matus:h" } }),
                );
            }
            // Eva may not see this one.
            s.hidden.push("$hidden".into());
        }

        let all = ThreadsQuery::parse(&HashMap::new());
        let page = r.chat.threads(&eva.access_token, "!r:h", &all).unwrap();
        assert_eq!(page.status, 200);
        let chunk = page.body["chunk"].as_array().unwrap();
        let ids: Vec<_> = chunk
            .iter()
            .map(|e| e["event_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["$a", "$b"]);
        let a = &chunk[0]["unsigned"]["m.relations"]["m.thread"];
        assert_eq!(a["count"], 2);
        assert_eq!(a["latest_event"]["event_id"], "$a2");
        assert_eq!(a["current_user_participated"], true);
        assert_eq!(
            chunk[1]["unsigned"]["m.relations"]["m.thread"]["current_user_participated"],
            false
        );
        assert!(page.body.get("next_batch").is_none());

        let mine = ThreadsQuery {
            participated: true,
            ..all.clone()
        };
        let page = r.chat.threads(&eva.access_token, "!r:h", &mine).unwrap();
        assert_eq!(page.body["chunk"].as_array().unwrap().len(), 1);

        // One a page, then the next.
        let one = ThreadsQuery {
            limit: 1,
            ..all.clone()
        };
        let first = r.chat.threads(&eva.access_token, "!r:h", &one).unwrap();
        assert_eq!(first.body["next_batch"], "4");
        let next = ThreadsQuery {
            from: Some(3),
            ..one
        };
        let second = r.chat.threads(&eva.access_token, "!r:h", &next).unwrap();
        assert_eq!(second.body["chunk"][0]["event_id"], "$b");

        // A token the homeserver does not know gets its own answer back.
        let page = r.chat.threads("nope", "!r:h", &all).unwrap();
        assert_eq!(page.status, 401);
    }

    #[test]
    fn the_threads_query_is_bounded() {
        let mut q = HashMap::new();
        q.insert("limit".to_string(), "100000".to_string());
        q.insert("include".to_string(), "participated".to_string());
        q.insert("from".to_string(), "17".to_string());
        let p = ThreadsQuery::parse(&q);
        assert_eq!(
            (p.limit, p.from, p.participated),
            (THREADS_MAX, Some(17), true)
        );
        q.insert("limit".to_string(), "0".to_string());
        assert_eq!(ThreadsQuery::parse(&q).limit, 1);
    }
}
