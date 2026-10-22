//! The listeners, the shared state, and what happens once at start.

use super::auth::{self, Caller, Refused};
use super::db::Store;
use super::gitcmd::Git;
use super::Config;
use actix_web::http::StatusCode;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use std::path::{Path, PathBuf};

/// The request headers nginx names the person in (`modules/app-sign-in.nix`).
pub const HEADER_USER: &str = "x-losos-cloud-user";
pub const HEADER_NAME: &str = "x-losos-cloud-name";
pub const HEADER_EMAIL: &str = "x-losos-cloud-email";
/// The administrator acting as a person (lososd, for the owner).
pub const HEADER_AS: &str = "x-losos-as";

/// What every handler shares.
pub struct State {
    pub config: Config,
    pub store: Store,
    pub git: Git,
    /// SHA-256 of the admin token.
    pub admin_hash: String,
    /// LosOS Reviewer's model and its worker's bell.
    pub reviewer: super::reviewer::worker::Handle,
}

/// Which listener a request came in on.
#[derive(Debug, Clone, Copy)]
pub struct Listener {
    /// The socket only nginx opens: the person header is believed.
    pub trusted: bool,
}

/// Where the process listens.
#[derive(Debug, Clone)]
pub struct Listen {
    /// `/run/losos-git/http.sock`, for nginx.
    pub socket: Option<PathBuf>,
    /// `127.0.0.1:3000`, tokens only.
    pub tcp: Option<String>,
    /// `/run/losos-git-peers/internal.sock`, for services on the box
    /// (`super::internal`). Its directory is the gate.
    pub internal: Option<PathBuf>,
}

fn header<'a>(req: &'a HttpRequest, name: &str) -> Option<std::borrow::Cow<'a, str>> {
    req.headers()
        .get(name)
        .map(|v| String::from_utf8_lossy(v.as_bytes()))
        .filter(|v| !v.trim().is_empty())
}

/// Why a request gets no answer it wanted: the status and one sentence,
/// as `{"message": "..."}`. A 401 carries the challenge git answers by
/// asking its credential helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fail {
    pub status: StatusCode,
    pub message: String,
}

impl Fail {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Fail {
        Fail {
            status,
            message: message.into(),
        }
    }
    pub fn not_found(what: &str) -> Fail {
        Fail::new(StatusCode::NOT_FOUND, format!("No such {what}."))
    }
    pub fn bad(message: impl Into<String>) -> Fail {
        Fail::new(StatusCode::UNPROCESSABLE_ENTITY, message)
    }
    pub fn forbidden(message: impl Into<String>) -> Fail {
        Fail::new(StatusCode::FORBIDDEN, message)
    }
    pub fn unauthorized(message: impl Into<String>) -> Fail {
        Fail::new(StatusCode::UNAUTHORIZED, message)
    }
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status.as_u16(), self.message)
    }
}

impl actix_web::ResponseError for Fail {
    fn status_code(&self) -> StatusCode {
        self.status
    }
    fn error_response(&self) -> HttpResponse {
        let mut r = HttpResponse::build(self.status);
        if self.status == StatusCode::UNAUTHORIZED {
            r.insert_header(("WWW-Authenticate", "Basic realm=\"LosOS Git\""));
        }
        r.json(message(&self.message))
    }
}

impl From<anyhow::Error> for Fail {
    fn from(e: anyhow::Error) -> Fail {
        if let Some(exists) = e.downcast_ref::<super::db::Exists>() {
            return Fail::new(
                StatusCode::CONFLICT,
                format!("There is already {}.", exists.0),
            );
        }
        tracing::error!(error = ?e, "LosOS Git could not answer");
        Fail::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "LosOS Git could not answer.",
        )
    }
}

impl From<std::io::Error> for Fail {
    fn from(e: std::io::Error) -> Fail {
        anyhow::Error::from(e).into()
    }
}

/// Who sent `req`, or why they are turned away.
pub fn caller(req: &HttpRequest, state: &State) -> Result<Caller, Fail> {
    let trusted = req
        .app_data::<web::Data<Listener>>()
        .map(|l| l.trusted)
        .unwrap_or(false);
    let user = header(req, HEADER_USER);
    let name = header(req, HEADER_NAME);
    let email = header(req, HEADER_EMAIL);
    let person = user.as_deref().map(|u| {
        (
            u.trim(),
            name.as_deref().unwrap_or("").trim(),
            email.as_deref().unwrap_or("").trim(),
        )
    });
    let authorization = header(req, "authorization");
    let act_as = header(req, HEADER_AS);
    // The pages act for the person nginx names; a page of another site
    // must not, whatever cookie the browser sends along.
    if trusted && person.is_some() && authorization.is_none() && cross_site(req) {
        return Err(Fail::forbidden(
            "A page of another site cannot act for you here.",
        ));
    }
    match auth::resolve(
        &state.store,
        &state.admin_hash,
        trusted,
        person,
        authorization.as_deref(),
        act_as.as_deref(),
    ) {
        Ok(Ok(c)) => Ok(c),
        Ok(Err(Refused::BadCredentials)) => {
            Err(Fail::unauthorized("The token is not one this box made."))
        }
        Ok(Err(Refused::BadName(why))) => Err(Fail::forbidden(why)),
        Err(e) => Err(e.into()),
    }
}

/// A browser request from another site: an unsafe method that the browser
/// says came from elsewhere (`Sec-Fetch-Site`), or, from a browser too old
/// to say, whose `Origin` is not this host. git and scripts send neither.
pub fn cross_site(req: &HttpRequest) -> bool {
    if matches!(req.method().as_str(), "GET" | "HEAD" | "OPTIONS") {
        return false;
    }
    if let Some(site) = header(req, "sec-fetch-site") {
        return !matches!(site.as_ref(), "same-origin" | "none");
    }
    let Some(origin) = header(req, "origin") else {
        return false;
    };
    let host = header(req, "host").unwrap_or_default();
    let origin: &str = &origin;
    let origin_host = origin
        .split_once("://")
        .map(|(_, h)| h)
        .unwrap_or(origin)
        .trim_end_matches('/');
    origin == "null" || !origin_host.eq_ignore_ascii_case(&host)
}

pub fn message(text: &str) -> serde_json::Value {
    serde_json::json!({ "message": text })
}

/// The admin token: read it, or make it on the first start. 0600, in the
/// state directory, which only this service and root can enter.
pub fn ensure_admin_token(path: &Path) -> anyhow::Result<String> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        let t = existing.trim();
        if t.len() >= 32 {
            return Ok(t.to_string());
        }
    }
    let token = super::db::new_token()?;
    let tmp = path.with_extension("new");
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(token.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(token)
}

/// Take in every repository on disk that has no record: one moved over
/// from Forgejo's tree, or one restored from a backup. Each becomes a
/// private repository of the account its folder names, made if missing.
pub fn adopt_repositories(state: &State) -> anyhow::Result<usize> {
    let root = state.config.repositories();
    let Ok(owners) = std::fs::read_dir(&root) else {
        return Ok(0);
    };
    let mut adopted = 0;
    for owner_dir in owners.flatten() {
        let owner = owner_dir.file_name().to_string_lossy().into_owned();
        if super::names::owner(&owner).is_err() || !owner_dir.path().is_dir() {
            continue;
        }
        let Ok(repos) = std::fs::read_dir(owner_dir.path()) else {
            continue;
        };
        for repo_dir in repos.flatten() {
            let file = repo_dir.file_name().to_string_lossy().into_owned();
            let Some(name) = file.strip_suffix(".git") else {
                continue;
            };
            // A wiki (Forgejo left one per repository that had one) is its
            // repository's, not a record of its own; it only needs
            // Forgejo's hooks out before anyone pushes to it.
            if super::names::wiki_of(name).is_some() {
                if repo_dir.path().join("HEAD").exists() {
                    if let Err(e) = clear_hooks(&repo_dir.path()) {
                        tracing::warn!(wiki = %repo_dir.path().display(), error = ?e, "could not clear hooks");
                    }
                }
                continue;
            }
            if super::names::repo(name).is_err() || !repo_dir.path().join("HEAD").exists() {
                continue;
            }
            if state.store.repo(&owner, name)?.is_some() {
                continue;
            }
            let user = state.store.ensure_user(&owner, "", "")?;
            let branch = state
                .git
                .run(&repo_dir.path(), &["symbolic-ref", "--short", "HEAD"])
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "main".to_string());
            state.store.create_repo(
                user.id,
                name,
                "",
                super::db::Visibility::Private,
                &branch,
                None,
            )?;
            if let Err(e) = clear_hooks(&repo_dir.path()) {
                tracing::warn!(repo = %repo_dir.path().display(), error = ?e, "could not clear hooks");
            }
            if let Err(e) = state.git.configure(&repo_dir.path()) {
                tracing::warn!(repo = %repo_dir.path().display(), error = ?e, "could not configure");
            }
            tracing::info!(
                owner = owner.as_str(),
                repo = name,
                "took in a repository found on disk"
            );
            adopted += 1;
        }
    }
    Ok(adopted)
}

/// Take out every hook a repository came with but git's own `*.sample`
/// files. Forgejo's call a `forgejo` binary that is not here, and would
/// refuse every push.
pub fn clear_hooks(repo: &std::path::Path) -> std::io::Result<()> {
    let Ok(entries) = std::fs::read_dir(repo.join("hooks")) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "sample") {
            continue;
        }
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Every route, for one listener.
pub fn routes(cfg: &mut web::ServiceConfig) {
    super::api::routes(cfg);
    super::transport::routes(cfg);
}

/// Open the state, then serve until stopped.
pub fn run(config: Config, listen: Listen) -> anyhow::Result<()> {
    std::fs::create_dir_all(config.repositories())?;
    let store = Store::open(&config.database())?;
    let admin = ensure_admin_token(&config.admin_token_file())?;
    let git = Git::new(&config.git, &config.state_dir);
    let state = web::Data::new(State {
        admin_hash: super::db::token_hash(&admin),
        config,
        store,
        git,
        reviewer: super::reviewer::worker::Handle::from_env(),
    });
    super::reviewer::worker::start(state.clone())?;
    let adopted = adopt_repositories(&state)?;
    if adopted > 0 {
        tracing::info!(adopted, "repositories taken in from disk");
    }
    // Repository issues from before the tracker, or from a Forgejo import.
    super::tracker::adopt::adopt_repo_issues(&state.store)?;
    super::tracker::hooks::start(state.clone())?;

    actix_web::rt::System::new().block_on(async move {
        let mut servers = Vec::new();
        if let Some(path) = listen.socket {
            let _ = std::fs::remove_file(&path);
            let data = state.clone();
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(data.clone())
                    .app_data(web::Data::new(Listener { trusted: true }))
                    .configure(routes)
            })
            .bind_uds(&path)?;
            // nginx is in this service's group (modules/git.nix); nobody
            // else may open the socket that believes the person header.
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660))?;
            tracing::info!(socket = %path.display(), "listening for nginx");
            servers.push(server.run());
        }
        if let Some(addr) = listen.tcp {
            let data = state.clone();
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(data.clone())
                    .app_data(web::Data::new(Listener { trusted: false }))
                    .configure(routes)
            })
            .bind(&addr)?;
            tracing::info!(address = addr.as_str(), "listening for tokens");
            servers.push(server.run());
        }
        if let Some(path) = listen.internal {
            let _ = std::fs::remove_file(&path);
            let data = state.clone();
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(data.clone())
                    .app_data(web::JsonConfig::default().limit(64 * 1024))
                    .configure(super::internal::routes)
            })
            .bind_uds(&path)?;
            // Anyone who can enter the directory may connect: modules/git.nix
            // makes it 0750 for losos-git and the losos-git-peers group.
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666))?;
            tracing::info!(socket = %path.display(), "listening for services on the box");
            servers.push(server.run());
        }
        if servers.is_empty() {
            anyhow::bail!("nothing to listen on: give --socket or --listen");
        }
        for result in futures_util::future::join_all(servers).await {
            result?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn writes_from_another_site_are_told_apart() {
        let post = || TestRequest::post().insert_header(("host", "box.local"));
        let r = post()
            .insert_header(("sec-fetch-site", "cross-site"))
            .to_http_request();
        assert!(cross_site(&r));
        let r = post()
            .insert_header(("sec-fetch-site", "same-origin"))
            .insert_header(("origin", "https://box.local"))
            .to_http_request();
        assert!(!cross_site(&r));
        let r = post()
            .insert_header(("origin", "https://evil.example"))
            .to_http_request();
        assert!(cross_site(&r), "an old browser's Origin");
        let r = post()
            .insert_header(("origin", "https://box.local"))
            .to_http_request();
        assert!(!cross_site(&r));
        assert!(!cross_site(&post().to_http_request()), "git sends neither");
        let r = TestRequest::get()
            .insert_header(("sec-fetch-site", "cross-site"))
            .to_http_request();
        assert!(!cross_site(&r), "a GET changes nothing");
    }
}
