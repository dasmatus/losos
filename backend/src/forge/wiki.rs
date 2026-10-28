//! A repository's wiki: pages of Markdown in a Git repository of their
//! own, `<name>.wiki.git` beside the code, as Forgejo and GitHub keep
//! them. A box that came from Forgejo already has its wikis there.
//!
//! Whoever may read the repository reads its wiki, and whoever may push to
//! it writes the wiki, in the browser (these routes) or with git
//! (`git clone https://<box>/git/<owner>/<name>.wiki.git`, [`super::transport`]).
//! A page is one file, `Getting-started.md` for "Getting started";
//! `_Sidebar.md` and `_Footer.md` frame every page, as on Forgejo.
//!
//! A wiki is a space of LosOS Wiki: edited in the admin UI's `/wikis`, read
//! as a site of its own at `/wiki/<owner>/<name>/`. `GET /api/v1/wikis` lists
//! every wiki the caller may read, for the directory there; with `q`, the
//! pages whose title or text holds it.

use super::api::{find_repo, need_person, need_scope, seen_by_nobody, visible, Answer};
use super::db::{Permission, Repo};
use super::gitcmd::Commit;
use super::server::{self, Fail, State};
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// The branch a new wiki starts on.
pub const BRANCH: &str = "main";
/// The most of one page the API reads or takes.
const PAGE_LIMIT: usize = 1024 * 1024;
/// The most lines one search answers with.
const HIT_LIMIT: usize = 50;
/// The most changes a page's history lists.
const HISTORY_LIMIT: usize = 20;
/// The largest picture a page may show from its wiki.
const PICTURE_LIMIT: usize = 16 * 1024 * 1024;

pub fn routes(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .route("/wikis", web::get().to(directory))
        .route("/repos/{owner}/{repo}/wiki", web::get().to(overview))
        .route("/repos/{owner}/{repo}/wiki/search", web::get().to(search))
        .route(
            "/repos/{owner}/{repo}/wiki/pages/{page:.*}",
            web::get().to(page),
        )
        .route(
            "/repos/{owner}/{repo}/wiki/pages/{page:.*}",
            web::put().to(save),
        )
        .route(
            "/repos/{owner}/{repo}/wiki/pages/{page:.*}",
            web::delete().to(remove),
        )
        .route(
            "/repos/{owner}/{repo}/wiki/raw/{file:.*}",
            web::get().to(raw),
        )
}

// ── Names ───────────────────────────────────────────────────────────────────

/// A page's name as an address carries it (`Getting-started`,
/// `guides/Setup`), or why not: parts that are not empty, `.` or `..`, none
/// starting with a dot, nothing git could read as an option.
pub fn clean_name(name: &str) -> Result<String, Fail> {
    let name = name.trim_matches('/');
    let ok = !name.is_empty()
        && name.len() <= 255
        && !name.chars().any(|c| c.is_control() || c == '\\')
        && name
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.starts_with('-'));
    if ok {
        Ok(name.to_string())
    } else {
        Err(Fail::not_found("page"))
    }
}

/// The name a page gets from its title: spaces to dashes, and nothing a
/// file name or an address would trip on.
pub fn name_for(title: &str) -> String {
    let mut out = String::new();
    for c in title.trim().chars() {
        if c.is_whitespace() {
            if !out.ends_with('-') {
                out.push('-');
            }
        } else if !c.is_control() && !"/\\:*?\"<>|#%".contains(c) {
            out.push(c);
        }
    }
    let out: String = out.chars().take(120).collect();
    out.trim_matches(|c| c == '-' || c == '.').to_string()
}

/// "Getting started" from `guides/Getting-started`.
pub fn title_of(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).replace('-', " ")
}

/// A file of the wiki that is a page, and its name.
fn page_name(path: &str) -> Option<&str> {
    let lower = path.to_ascii_lowercase();
    let stem = if lower.ends_with(".md") {
        &path[..path.len() - 3]
    } else if lower.ends_with(".markdown") {
        &path[..path.len() - 9]
    } else {
        return None;
    };
    (!stem.is_empty() && !stem.split('/').any(|p| p.starts_with('.'))).then_some(stem)
}

/// Pages, not the frame: `_Sidebar` and `_Footer` (any `_` name) are drawn
/// around the pages rather than listed among them.
fn listed(name: &str) -> bool {
    !name.rsplit('/').next().unwrap_or(name).starts_with('_')
}

// ── The wiki on disk ────────────────────────────────────────────────────────

struct Wiki {
    dir: PathBuf,
    branch: String,
    /// `None` until the first page.
    commit: Option<String>,
}

fn open(state: &State, repo: &Repo) -> Result<Wiki, Fail> {
    let dir = state.config.wiki_path(&repo.owner, &repo.name);
    if !dir.join("HEAD").exists() {
        return Ok(Wiki {
            dir,
            branch: BRANCH.to_string(),
            commit: None,
        });
    }
    let head = state
        .git
        .run(&dir, &["symbolic-ref", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| BRANCH.to_string());
    let mut branch = head.clone();
    let mut commit = state.git.commit_of(&dir, &head)?;
    // A wiki whose HEAD names a branch it lacks (an old Forgejo wiki on
    // `master` with HEAD at `main`): its one branch.
    if commit.is_none() {
        if let Some(r) = state
            .git
            .refs(&dir)?
            .into_iter()
            .find(|r| r.name.starts_with("refs/heads/"))
        {
            branch = r.name.trim_start_matches("refs/heads/").to_string();
            commit = Some(r.sha);
        }
    }
    Ok(Wiki {
        dir,
        branch,
        commit,
    })
}

/// Every page of a wiki: `(name, path, blob, size)`, by name.
fn pages(state: &State, wiki: &Wiki) -> Result<Vec<(String, String, String, i64)>, Fail> {
    let Some(commit) = &wiki.commit else {
        return Ok(Vec::new());
    };
    let mut out: Vec<_> = state
        .git
        .files(&wiki.dir, commit)?
        .into_iter()
        .filter_map(|(path, oid, size)| {
            page_name(&path).map(|n| (n.to_string(), path.clone(), oid, size))
        })
        .collect();
    out.sort_by_key(|p| p.0.to_lowercase());
    Ok(out)
}

fn commit_json(c: &Commit) -> Value {
    json!({ "sha": c.sha, "date": c.date, "author": c.author_name, "subject": c.subject })
}

fn wiki_clone_url(req: &HttpRequest, state: &State, repo: &Repo) -> String {
    let code = super::api::clone_url(req, state, repo);
    match code.strip_suffix(".git") {
        Some(base) => format!("{base}.wiki.git"),
        None => code,
    }
}

/// The wiki's published site, beside LosOS Git's pages: `/wiki/<owner>/<name>`
/// (the admin UI's wiki/ page, served by nginx to whoever may read it).
fn html_url(_state: &State, repo: &Repo) -> String {
    format!("/wiki/{}/{}", repo.owner, repo.name)
}

/// The page called "Home", else the first one.
fn home(list: &[(String, String, String, i64)]) -> Option<String> {
    list.iter()
        .find(|p| p.0.eq_ignore_ascii_case("home"))
        .or_else(|| list.iter().find(|p| listed(&p.0)))
        .map(|p| p.0.clone())
}

fn readable(
    req: &HttpRequest,
    state: &State,
    owner: &str,
    name: &str,
) -> Result<(Repo, Permission), Fail> {
    let c = server::caller(req, state)?;
    seen_by_nobody(state, &c)?;
    need_scope(&c, "repository", false)?;
    find_repo(state, &c, owner, name)
}

// ── Reading ─────────────────────────────────────────────────────────────────

async fn overview(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    state: web::Data<State>,
) -> Answer {
    let (repo, p) = readable(&req, &state, &path.0, &path.1)?;
    let wiki = open(&state, &repo)?;
    let list = pages(&state, &wiki)?;
    let last = match &wiki.commit {
        Some(c) => state.git.log(&wiki.dir, None, c, 1)?.into_iter().next(),
        None => None,
    };
    Ok(HttpResponse::Ok().json(json!({
        "exists": wiki.commit.is_some(),
        "writable": p >= Permission::Write,
        "clone_url": wiki_clone_url(&req, &state, &repo),
        "branch": wiki.branch,
        "home": home(&list),
        "updated": last.as_ref().map(commit_json),
        "pages": list
            .iter()
            .filter(|p| listed(&p.0))
            .map(|(name, path, _, size)| json!({
                "name": name,
                "title": title_of(name),
                "path": path,
                "size": size,
            }))
            .collect::<Vec<_>>(),
    })))
}

/// The text of the file `path` names in the wiki, if it is one.
fn text_of(state: &State, wiki: &Wiki, oid: &str) -> Result<String, Fail> {
    let (bytes, size) = state.git.blob(&wiki.dir, oid, PAGE_LIMIT)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if size > bytes.len() {
        text.push_str("\n\n…");
    }
    Ok(text)
}

/// The nearest `_<frame>` page, beside the page, else at the top: its name
/// (its links start from its own folder) and its text.
fn frame(
    state: &State,
    wiki: &Wiki,
    list: &[(String, String, String, i64)],
    name: &str,
    frame: &str,
) -> Result<Option<(String, String)>, Fail> {
    let folder = name.rsplit_once('/').map(|(f, _)| f);
    let want = |f: Option<&str>| match f {
        Some(f) => format!("{f}/{frame}"),
        None => frame.to_string(),
    };
    for candidate in [folder.map(|f| want(Some(f))), Some(want(None))]
        .into_iter()
        .flatten()
    {
        if let Some(p) = list.iter().find(|p| p.0.eq_ignore_ascii_case(&candidate)) {
            return Ok(Some((p.0.clone(), text_of(state, wiki, &p.2)?)));
        }
    }
    Ok(None)
}

async fn page(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    state: web::Data<State>,
) -> Answer {
    let (repo, _) = readable(&req, &state, &path.0, &path.1)?;
    let name = clean_name(&path.2)?;
    page_json(&state, &repo, &name)
}

fn page_json(state: &State, repo: &Repo, name: &str) -> Answer {
    let wiki = open(state, repo)?;
    let list = pages(state, &wiki)?;
    let found = list
        .iter()
        .find(|p| p.0 == name)
        .or_else(|| list.iter().find(|p| p.0.eq_ignore_ascii_case(name)))
        .ok_or_else(|| Fail::not_found("page"))?;
    let commit = wiki.commit.clone().unwrap_or_default();
    let history = state
        .git
        .changes(&wiki.dir, &commit, &found.1, HISTORY_LIMIT)?;
    let sidebar = frame(state, &wiki, &list, &found.0, "_Sidebar")?;
    let footer = frame(state, &wiki, &list, &found.0, "_Footer")?;
    Ok(HttpResponse::Ok().json(json!({
        "name": found.0,
        "title": title_of(&found.0),
        "path": found.1,
        "sha": found.2,
        "commit": commit,
        "text": text_of(state, &wiki, &found.2)?,
        "updated": history.first().map(commit_json),
        "history": history.iter().map(commit_json).collect::<Vec<_>>(),
        "sidebar": sidebar.as_ref().map(|f| &f.1),
        "sidebar_page": sidebar.as_ref().map(|f| &f.0),
        "footer": footer.as_ref().map(|f| &f.1),
        "footer_page": footer.as_ref().map(|f| &f.0),
    })))
}

/// A picture kept in the wiki, for the pages that show it: raster images
/// only, so nothing served here runs on the box's origin.
fn picture_type(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        _ => return None,
    })
}

async fn raw(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    state: web::Data<State>,
) -> Answer {
    let (repo, _) = readable(&req, &state, &path.0, &path.1)?;
    let file = path.2.trim_matches('/');
    let mime = picture_type(file).ok_or_else(|| Fail::not_found("picture"))?;
    let wiki = open(&state, &repo)?;
    let Some(commit) = &wiki.commit else {
        return Err(Fail::not_found("picture"));
    };
    let (_, oid, _) = state
        .git
        .files(&wiki.dir, commit)?
        .into_iter()
        .find(|(p, _, _)| p == file)
        .ok_or_else(|| Fail::not_found("picture"))?;
    let (bytes, size) = state.git.blob(&wiki.dir, &oid, PICTURE_LIMIT)?;
    if size > bytes.len() {
        return Err(Fail::not_found("picture"));
    }
    Ok(HttpResponse::Ok()
        .content_type(mime)
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Cache-Control", "private, no-cache"))
        .insert_header(("ETag", format!("\"{oid}\"")))
        .body(bytes))
}

// ── Writing ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Save {
    text: String,
    /// The page as it was read (its blob), or none for a new page: a page
    /// someone else saved in between is not written over.
    sha: Option<String>,
    message: Option<String>,
}

fn writable(
    req: &HttpRequest,
    state: &State,
    owner: &str,
    name: &str,
) -> Result<(Repo, super::db::User), Fail> {
    let c = server::caller(req, state)?;
    let user = need_person(&c)?.clone();
    need_scope(&c, "repository", true)?;
    let (repo, p) = find_repo(state, &c, owner, name)?;
    if p < Permission::Write {
        return Err(Fail::forbidden("You may read this wiki but not change it."));
    }
    Ok((repo, user))
}

fn changed_meanwhile() -> Fail {
    Fail::new(
        StatusCode::CONFLICT,
        "Someone else saved this page since you opened it. Open it again to see their change.",
    )
}

fn message_or(m: Option<&str>, default: String) -> String {
    match m.map(str::trim) {
        Some(m) if !m.is_empty() => m.chars().take(500).collect(),
        _ => default,
    }
}

async fn save(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    body: web::Json<Save>,
    state: web::Data<State>,
) -> Answer {
    let (repo, user) = writable(&req, &state, &path.0, &path.1)?;
    let name = clean_name(&path.2)?;
    if body.text.len() > PAGE_LIMIT {
        return Err(Fail::bad("The page is longer than a megabyte."));
    }
    let mut wiki = open(&state, &repo)?;
    if !wiki.dir.join("HEAD").exists() {
        state.git.init_bare(&wiki.dir, BRANCH)?;
        wiki = open(&state, &repo)?;
    }
    let list = pages(&state, &wiki)?;
    let existing = list.iter().find(|p| p.0.eq_ignore_ascii_case(&name));
    let (file, message) = match (existing, body.sha.as_deref()) {
        (Some(p), Some(sha)) if p.2 == sha => (
            p.1.clone(),
            message_or(
                body.message.as_deref(),
                format!("Update {}", title_of(&p.0)),
            ),
        ),
        (Some(_), _) => return Err(changed_meanwhile()),
        (None, Some(_)) => return Err(changed_meanwhile()),
        (None, None) => (
            format!("{name}.md"),
            message_or(body.message.as_deref(), format!("Add {}", title_of(&name))),
        ),
    };
    let mut text = body.text.replace("\r\n", "\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let who = super::pulls::signature(&user);
    state
        .git
        .write_file(
            &wiki.dir,
            &wiki.branch,
            wiki.commit.as_deref(),
            &file,
            Some(text.as_bytes()),
            &message,
            &who,
        )
        .map_err(|_| changed_meanwhile())?;
    tracing::info!(
        wiki = format!("{}/{}", repo.owner, repo.name),
        page = file.as_str(),
        "wiki page saved"
    );
    page_json(&state, &repo, page_name(&file).unwrap_or(&name))
}

#[derive(Deserialize)]
struct RemoveQuery {
    sha: Option<String>,
}

async fn remove(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
    query: web::Query<RemoveQuery>,
    state: web::Data<State>,
) -> Answer {
    let (repo, user) = writable(&req, &state, &path.0, &path.1)?;
    let name = clean_name(&path.2)?;
    let wiki = open(&state, &repo)?;
    let list = pages(&state, &wiki)?;
    let found = list
        .iter()
        .find(|p| p.0.eq_ignore_ascii_case(&name))
        .ok_or_else(|| Fail::not_found("page"))?;
    if query.sha.as_deref().is_some_and(|s| s != found.2) {
        return Err(changed_meanwhile());
    }
    state
        .git
        .write_file(
            &wiki.dir,
            &wiki.branch,
            wiki.commit.as_deref(),
            &found.1,
            None,
            &format!("Delete {}", title_of(&found.0)),
            &super::pulls::signature(&user),
        )
        .map_err(|_| changed_meanwhile())?;
    Ok(HttpResponse::NoContent().finish())
}

// ── The directory ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DirectoryQuery {
    #[serde(default)]
    q: String,
}

/// Lines of a page around a hit, cut to something a list shows.
fn snippet(line: &str) -> String {
    let line = line.trim();
    let mut out: String = line.chars().take(200).collect();
    if out.len() < line.len() {
        out.push('…');
    }
    out
}

async fn directory(
    req: HttpRequest,
    query: web::Query<DirectoryQuery>,
    state: web::Data<State>,
) -> Answer {
    let c = server::caller(&req, &state)?;
    seen_by_nobody(&state, &c)?;
    need_scope(&c, "repository", false)?;
    let q = query.q.trim();
    let mut wikis = Vec::new();
    let mut hits = Vec::new();
    let mut repos = visible(&state, &c, state.store.all_repos()?)?;
    repos.sort_by(|a, b| {
        (a.0.owner.to_lowercase(), a.0.name.to_lowercase())
            .cmp(&(b.0.owner.to_lowercase(), b.0.name.to_lowercase()))
    });
    for (repo, p) in repos {
        let wiki = open(&state, &repo)?;
        let Some(commit) = wiki.commit.clone() else {
            continue;
        };
        let list = pages(&state, &wiki)?;
        let shown: Vec<_> = list.iter().filter(|p| listed(&p.0)).collect();
        if shown.is_empty() {
            continue;
        }
        let last = state
            .git
            .log(&wiki.dir, None, &commit, 1)?
            .into_iter()
            .next();
        wikis.push(json!({
            "owner": repo.owner,
            "repo": repo.name,
            "full_name": format!("{}/{}", repo.owner, repo.name),
            "description": repo.description,
            "visibility": repo.visibility,
            "writable": p >= Permission::Write,
            "pages": shown.len(),
            "home": home(&list),
            "updated": last.as_ref().map(commit_json),
            "html_url": html_url(&state, &repo),
        }));
        if hits.len() < HIT_LIMIT {
            hits.extend(find(&state, &repo, &wiki, &commit, &list, q)?);
        }
    }
    hits.truncate(HIT_LIMIT);
    Ok(HttpResponse::Ok().json(json!({ "wikis": wikis, "hits": hits })))
}

/// The pages of one wiki whose title holds `q` (any case), then the lines
/// of the others that do; nothing for fewer than two characters.
fn find(
    state: &State,
    repo: &Repo,
    wiki: &Wiki,
    commit: &str,
    list: &[(String, String, String, i64)],
    q: &str,
) -> Result<Vec<Value>, Fail> {
    let mut hits = Vec::new();
    if q.chars().count() < 2 {
        return Ok(hits);
    }
    let lower = q.to_lowercase();
    let mut seen = std::collections::BTreeSet::new();
    for (name, _, _, _) in list.iter().filter(|p| listed(&p.0)) {
        if title_of(name).to_lowercase().contains(&lower) {
            seen.insert(name.clone());
            hits.push(hit(state, repo, name, None));
        }
    }
    for (path, line, text) in state.git.grep(&wiki.dir, commit, q, "*.md", 3)? {
        let Some(name) = page_name(&path) else {
            continue;
        };
        // One hit per page: its first matching line.
        if !listed(name) || !seen.insert(name.to_string()) {
            continue;
        }
        hits.push(hit(state, repo, name, Some((line, snippet(&text)))));
        if hits.len() >= HIT_LIMIT {
            break;
        }
    }
    Ok(hits)
}

/// `GET …/wiki/search?q=`: one wiki's hits, for its published site.
async fn search(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    query: web::Query<DirectoryQuery>,
    state: web::Data<State>,
) -> Answer {
    let (repo, _) = readable(&req, &state, &path.0, &path.1)?;
    let wiki = open(&state, &repo)?;
    let hits = match &wiki.commit {
        Some(commit) => find(
            &state,
            &repo,
            &wiki,
            commit,
            &pages(&state, &wiki)?,
            query.q.trim(),
        )?,
        None => Vec::new(),
    };
    Ok(HttpResponse::Ok().json(json!({ "hits": hits })))
}

fn hit(state: &State, repo: &Repo, name: &str, line: Option<(usize, String)>) -> Value {
    json!({
        "owner": repo.owner,
        "repo": repo.name,
        "page": name,
        "title": title_of(name),
        "line": line.as_ref().map(|l| l.0),
        "text": line.map(|l| l.1),
        "html_url": format!("{}/{}", html_url(state, repo), name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_titles_and_files() {
        assert_eq!(name_for("Getting started"), "Getting-started");
        assert_eq!(name_for("  What's  new?  "), "What's-new");
        assert_eq!(name_for("a/b\\c:d"), "abcd");
        assert_eq!(name_for("..."), "");
        assert_eq!(title_of("guides/Getting-started"), "Getting started");
        assert_eq!(page_name("Home.md"), Some("Home"));
        assert_eq!(page_name("guides/Setup.MD"), Some("guides/Setup"));
        assert_eq!(page_name("Notes.markdown"), Some("Notes"));
        assert_eq!(page_name("logo.png"), None);
        assert_eq!(page_name(".gitea/x.md"), None);
        assert!(listed("Home") && !listed("_Sidebar") && !listed("guides/_Footer"));
        assert_eq!(clean_name("/guides/Setup/").unwrap(), "guides/Setup");
        for bad in [
            "", "../x", "a/../b", ".hidden", "a//b", "-flag", "a\\b", "a\0b",
        ] {
            assert!(clean_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_raster_pictures_are_served() {
        assert_eq!(picture_type("beds.png"), Some("image/png"));
        assert_eq!(picture_type("Guides/photo.JPG"), Some("image/jpeg"));
        assert_eq!(picture_type("a.webp"), Some("image/webp"));
        for other in [
            "drawing.svg",
            "page.html",
            "Home.md",
            "noextension",
            "x.png.exe",
        ] {
            assert_eq!(picture_type(other), None, "{other}");
        }
    }
}
