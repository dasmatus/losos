//! The log reader's HTTP side, on the Unix socket only nginx (its group)
//! and root may open.
//!
//! * `GET /api/logs` and `/api/logs/download`: the lines as JSON and as a
//!   file, for Settings › Logs (screens/settings/pane-logs.tsx). nginx asks
//!   LosOS Login first and passes the person on, as for the rest of /api/.
//! * `GET /basic/settings/logs`, `…/download`, `POST …/key`: the same pane
//!   for a browser without JavaScript, and a sign-in with the spare key of
//!   its own, because lososd draws every other such page and this one has
//!   to work while lososd does not.
//! * `GET /internal/logs/excerpt`: a few lines as text for lososd's problem
//!   reports. nginx never forwards it: every request nginx sends carries
//!   `X-Losos-Front`, and this route refuses those.
//!
//! Who may read: the spare key, or a person nginx names whom LosOS Login
//! puts in `losos-admins` (lososd's answer for `/api/me` when it gives
//! one). The headers are believed because only nginx and root can open the
//! socket, and nginx sets them on every request it sends here.

use super::catalogue::{Catalogue, Units};
use super::journal::{self, Page, Query, Span};
use super::lososd::{Answer, Lososd};
use super::redact::Redactor;
use actix_web::http::header;
use actix_web::{web, HttpRequest, HttpResponse};
use losos_page::html::{el, Html};
use losos_page::i18n::{Locale, Messages};
use losos_page::logs::{self as pane, Filter, Source};
use losos_page::nav::{self, Me};
use losos_page::shell::{self, Assets, Frame, Person, Prefs};
use losos_page::ui;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// The group LosOS Login puts administrators in (crate::people::ADMINS).
const ADMINS: &str = crate::people::ADMINS;

/// The header nginx sets on everything it sends here.
const FRONT: &str = "x-losos-front";

/// The script page's address of the pane.
const SCRIPT_PATH: &str = "/settings/logs";

/// Everything the reader knows at start.
pub struct Reader {
    pub journalctl: PathBuf,
    pub catalogue: Catalogue,
    /// The spare key's credential, read on every request.
    pub token_file: Option<PathBuf>,
    pub lososd: Option<Lososd>,
    /// The built admin page (losos-admin-ui): its catalogue and stylesheet.
    pub ui_dir: Option<PathBuf>,
    /// nginx asks LosOS Login and names the person (`modules/identity.nix`).
    pub identity: bool,
    /// The sidebar has Office (`losos.office.folder`).
    pub office: bool,
}

/// Who is asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    Spare,
    Person { name: String, groups: String },
    Nobody,
}

impl Caller {
    /// Whether LosOS Login's groups make them an administrator.
    fn admin_by_groups(&self) -> bool {
        match self {
            Caller::Spare => true,
            Caller::Person { groups, .. } => groups.split(',').any(|g| g.trim() == ADMINS),
            Caller::Nobody => false,
        }
    }
}

fn header<'a>(req: &'a HttpRequest, name: &str) -> &'a str {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map_or("", str::trim)
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Reader {
    fn token(&self) -> Option<String> {
        let path = self.token_file.as_ref()?;
        let token = std::fs::read_to_string(path).ok()?;
        let token = token.trim();
        (!token.is_empty()).then(|| token.to_string())
    }

    /// Hides secrets, the spare key among them.
    #[must_use]
    pub fn redactor(&self) -> Redactor {
        Redactor::new(self.token())
    }

    fn key_opens(&self, key: &str) -> bool {
        self.token()
            .is_some_and(|t| same(t.as_bytes(), key.trim().as_bytes()))
    }

    /// Who `req` is from.
    #[must_use]
    pub fn caller(&self, req: &HttpRequest) -> Caller {
        let auth = header(req, "authorization");
        if let Some(key) = auth
            .split_once(' ')
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, key)| key)
        {
            if self.key_opens(key) {
                return Caller::Spare;
            }
        }
        let name = header(req, "remote-user");
        if self.identity && !name.is_empty() {
            return Caller::Person {
                name: name.to_string(),
                groups: header(req, "remote-groups").to_string(),
            };
        }
        Caller::Nobody
    }

    /// The headers lososd needs to know the same person.
    fn as_caller(&self, req: &HttpRequest, caller: &Caller) -> Vec<(&'static str, String)> {
        let mut h = vec![("X-Real-IP", header(req, "x-real-ip").to_string())];
        match caller {
            Caller::Spare => {
                if let Some(t) = self.token() {
                    h.push(("Authorization", format!("Bearer {t}")));
                }
            }
            Caller::Person { name, groups } => {
                h.push(("Remote-User", name.clone()));
                h.push(("Remote-Groups", groups.clone()));
            }
            Caller::Nobody => {}
        }
        h
    }

    fn ui_file(&self, name: &str) -> Option<Vec<u8>> {
        std::fs::read(self.ui_dir.as_ref()?.join(name)).ok()
    }

    fn messages(&self, locale: Locale) -> Messages {
        for l in [locale, Locale::En] {
            if let Some(bytes) = self.ui_file(&format!("basic/messages.{}.json", l.code())) {
                return Messages::parse(l, &bytes);
            }
        }
        Messages::from_pairs(locale, &[])
    }

    fn assets(&self) -> Assets {
        self.ui_file("index.html")
            .map(|b| Assets::from_index(&String::from_utf8_lossy(&b)))
            .unwrap_or_default()
    }

    fn sources(&self, m: Option<&Messages>) -> Vec<Source> {
        self.catalogue
            .ids()
            .into_iter()
            .map(|id| Source {
                label: m.map_or_else(|| id.clone(), |m| m.t(&format!("logs.source.{id}"))),
                id,
            })
            .collect()
    }
}

/// What a request asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Params {
    pub filter: Filter,
    pub before: Option<String>,
    pub after: Option<String>,
    pub limit: usize,
}

/// The filter in `query`; `limit` lines unless it says otherwise, at most
/// `max`.
///
/// # Errors
///
/// A value the page never sends, named.
pub fn params(query: &str, limit: usize, max: usize) -> Result<Params, String> {
    let mut p = Params {
        filter: Filter {
            unit: "lososd".to_string(),
            priority: 6,
            span: "boot".to_string(),
            text: String::new(),
            follow: false,
        },
        before: None,
        after: None,
        limit,
    };
    for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
        match &*k {
            "unit" if !v.is_empty() => p.filter.unit = v.into_owned(),
            "priority" => {
                p.filter.priority = v.parse().ok().filter(|n| *n <= 7).ok_or("priority")?;
            }
            "span" => {
                Span::parse(&v).ok_or("span")?;
                p.filter.span = v.into_owned();
            }
            "q" => {
                if v.chars().count() > 200 {
                    return Err("q".into());
                }
                p.filter.text = v.trim().to_string();
            }
            "follow" => p.filter.follow = v == "1",
            "before" if !v.is_empty() => {
                journal::cursor(&v).then_some(()).ok_or("before")?;
                p.before = Some(v.into_owned());
            }
            "after" if !v.is_empty() => {
                journal::cursor(&v).then_some(()).ok_or("after")?;
                p.after = Some(v.into_owned());
            }
            "limit" => {
                p.limit = v
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or("limit")?
                    .min(max);
            }
            _ => {}
        }
    }
    Ok(p)
}

impl Params {
    fn query(&self, catalogue: &Catalogue) -> Result<Query, String> {
        Ok(Query {
            units: catalogue
                .resolve(&self.filter.unit)
                .ok_or_else(|| "unit".to_string())?,
            priority: self.filter.priority,
            span: Span::parse(&self.filter.span),
            text: self.filter.text.clone(),
            // Following always shows the newest lines.
            before: self.before.clone().filter(|_| !self.filter.follow),
            after: self.after.clone(),
            until_usec: None,
            limit: self.limit,
        })
    }
}

async fn search(r: &web::Data<Reader>, q: Query) -> Result<Page, String> {
    let r = Arc::clone(r);
    web::block(move || journal::read(&r.journalctl, &q, &r.redactor()))
        .await
        .map_err(|e| e.to_string())?
}

fn no_store(mut b: actix_web::HttpResponseBuilder) -> actix_web::HttpResponseBuilder {
    b.insert_header((header::CACHE_CONTROL, "no-store"))
        .insert_header(("x-content-type-options", "nosniff"));
    b
}

fn json_error(status: actix_web::http::StatusCode, error: &str) -> HttpResponse {
    no_store(HttpResponse::build(status)).json(json!({ "error": error }))
}

/// The routes, for an `App`.
pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/api/logs", web::get().to(api_lines))
        .route("/api/logs/download", web::get().to(api_download))
        .route(pane::PATH, web::get().to(page_view))
        .route(
            &format!("{}/download", pane::PATH),
            web::get().to(page_download),
        )
        .route(&format!("{}/key", pane::PATH), web::post().to(page_key))
        .route("/internal/logs/excerpt", web::get().to(excerpt))
        .route("/health", web::get().to(|| async { "ok" }));
}

// ── The script page's API ───────────────────────────────────────────────

fn api_refusal(r: &Reader, req: &HttpRequest) -> Option<HttpResponse> {
    use actix_web::http::StatusCode;
    match r.caller(req) {
        Caller::Nobody => Some(json_error(StatusCode::UNAUTHORIZED, "signIn")),
        c if !c.admin_by_groups() => Some(json_error(StatusCode::FORBIDDEN, "adminOnly")),
        _ => None,
    }
}

async fn api_lines(r: web::Data<Reader>, req: HttpRequest) -> HttpResponse {
    if let Some(refusal) = api_refusal(&r, &req) {
        return refusal;
    }
    let bad = |what: String| {
        json_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            &format!("bad {what}"),
        )
    };
    let p = match params(req.query_string(), 200, journal::PAGE_MAX) {
        Ok(p) => p,
        Err(e) => return bad(e),
    };
    let q = match p.query(&r.catalogue) {
        Ok(q) => q,
        Err(e) => return bad(e),
    };
    let (page, error) = match search(&r, q).await {
        Ok(page) => (page, None),
        Err(e) => (Page::default(), Some(e)),
    };
    let lines: Vec<Value> = page
        .entries
        .iter()
        .map(|e| {
            let l = journal::line(e);
            json!({
                "cursor": e.cursor,
                "date": l.date,
                "time": l.time,
                "stamp": l.stamp,
                "unit": l.unit,
                "pid": l.pid,
                "priority": l.priority,
                "message": l.message,
            })
        })
        .collect();
    no_store(HttpResponse::Ok()).json(json!({
        "sources": r.catalogue.ids(),
        "unit": p.filter.unit,
        "lines": lines,
        "older": page.older,
        "newer": page.newer,
        "error": error,
    }))
}

async fn download(r: &web::Data<Reader>, req: &HttpRequest) -> HttpResponse {
    let p = match params(req.query_string(), 5000, journal::DOWNLOAD_MAX) {
        Ok(p) => p,
        Err(e) => return HttpResponse::BadRequest().body(format!("bad {e}\n")),
    };
    let q = match p.query(&r.catalogue) {
        Ok(q) => q,
        Err(e) => return HttpResponse::BadRequest().body(format!("bad {e}\n")),
    };
    match search(r, q).await {
        Ok(page) => {
            let body: String = page.entries.iter().map(journal::text).collect();
            let unit: String = p
                .filter
                .unit
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let name = format!(
                "losos-{unit}-{}.log",
                chrono::Local::now().format("%Y%m%d-%H%M")
            );
            no_store(HttpResponse::Ok())
                .content_type("text/plain; charset=utf-8")
                .insert_header((
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{name}\""),
                ))
                .body(body)
        }
        Err(e) => no_store(HttpResponse::BadGateway()).body(format!("{e}\n")),
    }
}

async fn api_download(r: web::Data<Reader>, req: HttpRequest) -> HttpResponse {
    if let Some(refusal) = api_refusal(&r, &req) {
        return refusal;
    }
    download(&r, &req).await
}

// ── The page without JavaScript ─────────────────────────────────────────

/// What lososd says about the person, or that it does not answer.
struct Asked {
    up: bool,
    me: Me,
    pins: Option<Vec<shell::NavEntry>>,
}

async fn ask_lososd(
    r: &web::Data<Reader>,
    req: &HttpRequest,
    caller: &Caller,
    m: &Messages,
) -> Asked {
    let fallback = || match caller {
        Caller::Spare => Me {
            spare_key: true,
            admin: true,
            ..Me::default()
        },
        Caller::Person { name, .. } => Me {
            name: Some(name.clone()),
            admin: caller.admin_by_groups(),
            ..Me::default()
        },
        Caller::Nobody => Me::default(),
    };
    let Some(lososd) = r.lososd.clone() else {
        return Asked {
            up: false,
            me: fallback(),
            pins: None,
        };
    };
    let headers = r.as_caller(req, caller);
    let asked = web::block(move || {
        let h: Vec<(&str, String)> = headers.iter().map(|(k, v)| (*k, v.clone())).collect();
        let me = lososd.get("/api/me", &h);
        let pins = matches!(me, Answer::Json(_)).then(|| lososd.get("/api/pins", &h));
        (me, pins)
    })
    .await;
    match asked {
        Ok((Answer::Json(v), pins)) => {
            let me = Me::from_json(&v);
            let known = me.name.is_some() || me.spare_key;
            Asked {
                up: true,
                me: if known { me } else { fallback() },
                pins: match pins {
                    Some(Answer::Json(p))
                        if known && !v["locked"].as_array().is_some_and(|a| !a.is_empty()) =>
                    {
                        Some(nav::pins(&p, m))
                    }
                    _ => None,
                },
            }
        }
        Ok((Answer::Status(_), _)) => Asked {
            up: true,
            me: fallback(),
            pins: None,
        },
        _ => Asked {
            up: false,
            me: fallback(),
            pins: None,
        },
    }
}

struct PageCtx {
    m: Messages,
    prefs: Prefs,
    assets: Assets,
    here: String,
}

fn page_ctx(r: &Reader, req: &HttpRequest) -> PageCtx {
    let prefs = Prefs::parse(header(req, "x-losos-basic-prefs"));
    let locale = prefs
        .lang
        .unwrap_or_else(|| Locale::from_accept_language(header(req, "accept-language")));
    let q = req.query_string();
    PageCtx {
        m: r.messages(locale),
        prefs,
        assets: r.assets(),
        here: if q.is_empty() {
            pane::PATH.to_string()
        } else {
            format!("{}?{q}", pane::PATH)
        },
    }
}

fn html(status: u16, body: Html) -> HttpResponse {
    no_store(HttpResponse::build(
        actix_web::http::StatusCode::from_u16(status).unwrap_or(actix_web::http::StatusCode::OK),
    ))
    .content_type("text/html; charset=utf-8")
    .body(body.into_string())
}

fn sign_in_page(c: &PageCtx, login: bool, wrong: bool) -> HttpResponse {
    let title = c.m.t("logs.signIn.title");
    let frame = Frame {
        m: &c.m,
        assets: &c.assets,
        prefs: &c.prefs,
        here: &c.here,
        title: &title,
        person: None,
        nav: &[],
        tabs: nav::TABS,
        rail_moved: false,
        refresh: None,
        pins: None,
    };
    html(
        401,
        shell::page(&frame, pane::sign_in(&c.m, &c.here, wrong, login)),
    )
}

async fn page_view(r: web::Data<Reader>, req: HttpRequest) -> HttpResponse {
    let mut c = page_ctx(&r, &req);
    let caller = r.caller(&req);
    let asked = ask_lososd(&r, &req, &caller, &c.m).await;
    if caller == Caller::Nobody {
        return sign_in_page(&c, asked.up && r.identity, false);
    }
    let me = asked.me;
    if asked.up {
        c.assets
            .styles
            .push(format!("/basic/counts.css?lang={}", c.m.locale.code()));
    }
    let nav = nav::nav(&me, &c.m, SCRIPT_PATH, r.office && !me.locked);
    let person: Person = me.person(&c.m);
    let title = c.m.t("settings.panes.logs.label");
    let header = ui::pane_header(&title, &c.m.t("settings.panes.logs.summary"));

    if !me.admin {
        let frame = Frame {
            m: &c.m,
            assets: &c.assets,
            prefs: &c.prefs,
            here: &c.here,
            title: &title,
            person: Some(&person),
            nav: &nav,
            tabs: nav::TABS,
            rail_moved: false,
            refresh: None,
            pins: asked.pins.as_deref(),
        };
        return html(403, shell::page(&frame, header.with(pane::denied(&c.m))));
    }

    let (p, mut error) = match params(req.query_string(), 200, journal::PAGE_MAX) {
        Ok(p) => (p, None),
        Err(_) => (
            params("", 200, journal::PAGE_MAX).unwrap_or_else(|_| unreachable!()),
            Some(c.m.t("logs.badFilter")),
        ),
    };
    let mut lines = Vec::new();
    let mut older = None;
    match p.query(&r.catalogue) {
        Ok(q) => match search(&r, q).await {
            Ok(page) => {
                lines = page.entries.iter().map(journal::line).collect();
                older = page.older;
            }
            Err(e) => error = Some(e),
        },
        Err(_) => error = Some(c.m.t("logs.badFilter")),
    }
    let sources = r.sources(Some(&c.m));
    let body = pane::pane(&pane::Pane {
        m: &c.m,
        filter: &p.filter,
        sources: &sources,
        lines: &lines,
        older: older.as_deref(),
        lososd_down: !asked.up,
        error: error.as_deref(),
    });
    let frame = Frame {
        m: &c.m,
        assets: &c.assets,
        prefs: &c.prefs,
        here: &c.here,
        title: &title,
        person: Some(&person),
        nav: &nav,
        tabs: nav::TABS,
        rail_moved: false,
        refresh: p.filter.follow.then_some(5),
        pins: asked.pins.as_deref(),
    };
    let main = Html::from(
        el("div")
            .attr("data-pane", "logs")
            .child(header)
            .child(body),
    );
    html(200, shell::page(&frame, main))
}

async fn page_download(r: web::Data<Reader>, req: HttpRequest) -> HttpResponse {
    let caller = r.caller(&req);
    if caller == Caller::Nobody {
        return no_store(HttpResponse::SeeOther())
            .insert_header((header::LOCATION, pane::PATH))
            .finish();
    }
    // lososd's word, when it gives one, on whether they are an administrator.
    let m = Messages::from_pairs(Locale::En, &[]);
    let asked = ask_lososd(&r, &req, &caller, &m).await;
    if !asked.me.admin {
        return no_store(HttpResponse::Forbidden()).body("administrators only\n");
    }
    download(&r, &req).await
}

/// Where a sign-in may go on to: this page, never another one.
fn back(next: &str) -> String {
    if next == pane::PATH
        || (next.starts_with(&format!("{}?", pane::PATH)) && !next.contains(['\r', '\n']))
    {
        next.to_string()
    } else {
        pane::PATH.to_string()
    }
}

async fn page_key(r: web::Data<Reader>, req: HttpRequest, body: web::Bytes) -> HttpResponse {
    // A form on another site cannot sign anyone in here.
    if !matches!(header(&req, "sec-fetch-site"), "" | "same-origin" | "none") {
        return no_store(HttpResponse::Forbidden()).body("cross-site\n");
    }
    let mut next = String::new();
    let mut key = String::new();
    for (k, v) in url::form_urlencoded::parse(&body) {
        match &*k {
            "next" => next = v.into_owned(),
            "key" => key = v.into_owned(),
            _ => {}
        }
    }
    let next = back(&next);
    if !key.is_empty() && r.key_opens(&key) {
        // The cookie the other pages without JavaScript read
        // (backend/plugins/basic/src/web.rs, `spare_cookie`).
        return no_store(HttpResponse::SeeOther())
            .insert_header((header::LOCATION, next))
            .insert_header((
                header::SET_COOKIE,
                format!(
                    "losos_spare={}; Path=/basic; Secure; HttpOnly; SameSite=Strict",
                    key.trim()
                ),
            ))
            .finish();
    }
    actix_web::rt::time::sleep(Duration::from_secs(1)).await;
    let mut c = page_ctx(&r, &req);
    c.here = next;
    let up = match &r.lososd {
        Some(l) => {
            let l = l.clone();
            web::block(move || l.get("/api/health", &[]) != Answer::Down)
                .await
                .unwrap_or(false)
        }
        None => false,
    };
    sign_in_page(&c, up && r.identity, true)
}

// ── For lososd's problem reports ────────────────────────────────────────

async fn excerpt(r: web::Data<Reader>, req: HttpRequest) -> HttpResponse {
    if req.headers().contains_key(FRONT) {
        return HttpResponse::NotFound().finish();
    }
    let mut unit = None;
    let mut until = None;
    let mut lines = 50usize;
    let mut priority = 6u8;
    for (k, v) in url::form_urlencoded::parse(req.query_string().as_bytes()) {
        match &*k {
            "unit" => unit = r.catalogue.resolve(&v),
            "until" => until = v.parse::<u64>().ok(),
            "lines" => lines = v.parse::<usize>().unwrap_or(50).clamp(1, 200),
            "priority" => priority = v.parse::<u8>().unwrap_or(6).min(7),
            _ => {}
        }
    }
    let Some(units) = unit.filter(|u| *u != Units::All) else {
        return HttpResponse::BadRequest().body("bad unit\n");
    };
    let q = Query {
        units,
        priority,
        span: None,
        text: String::new(),
        before: None,
        after: None,
        until_usec: until,
        limit: lines,
    };
    match search(&r, q).await {
        Ok(page) => no_store(HttpResponse::Ok())
            .content_type("text/plain; charset=utf-8")
            .body(page.entries.iter().map(journal::text).collect::<String>()),
        Err(e) => no_store(HttpResponse::BadGateway()).body(format!("{e}\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use std::os::unix::fs::PermissionsExt;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn reader(dir: &std::path::Path) -> web::Data<Reader> {
        let token = dir.join("admin-token");
        std::fs::write(&token, format!("{KEY}\n")).unwrap();
        let bin = dir.join("journalctl");
        std::fs::write(
            &bin,
            concat!(
                "#!/bin/sh\n",
                "echo \"$@\" > \"$(dirname \"$0\")/args\"\n",
                r#"echo '{"__CURSOR":"s=1;i=2","__REALTIME_TIMESTAMP":"1761739201000000","MESSAGE":"token=s3cr3t up","PRIORITY":"6","_SYSTEMD_UNIT":"lososd.service","_PID":"42"}'"#,
                "\n",
                r#"echo '{"__CURSOR":"s=1;i=1","__REALTIME_TIMESTAMP":"1761739200000000","MESSAGE":"starting","PRIORITY":"5","_SYSTEMD_UNIT":"lososd.service","_PID":"42"}'"#,
                "\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        web::Data::new(Reader {
            journalctl: bin,
            catalogue: Catalogue::default(),
            token_file: Some(token),
            lososd: None,
            ui_dir: None,
            identity: true,
            office: false,
        })
    }

    macro_rules! app {
        ($r:expr) => {
            test::init_service(App::new().app_data($r.clone()).configure(routes)).await
        };
    }

    #[actix_web::test]
    async fn the_api_wants_an_administrator() {
        let dir = tempfile::tempdir().unwrap();
        let r = reader(dir.path());
        let app = app!(r);
        let res =
            test::call_service(&app, test::TestRequest::get().uri("/api/logs").to_request()).await;
        assert_eq!(res.status(), 401);
        let res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/logs")
                .insert_header(("Remote-User", "guest"))
                .insert_header(("Remote-Groups", "losos-viewers"))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 403);
        let res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/logs")
                .insert_header(("Authorization", "Bearer wrong"))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 401);
    }

    #[actix_web::test]
    async fn lines_come_oldest_first_and_redacted() {
        let dir = tempfile::tempdir().unwrap();
        let r = reader(dir.path());
        let app = app!(r);
        let v: Value = test::call_and_read_body_json(
            &app,
            test::TestRequest::get()
                .uri("/api/logs?unit=lososd&priority=5&span=hour")
                .insert_header(("Remote-User", "matus"))
                .insert_header(("Remote-Groups", "losos-owner,losos-admins"))
                .to_request(),
        )
        .await;
        assert_eq!(v["lines"][0]["message"], "starting");
        assert_eq!(v["lines"][1]["message"], "token=[hidden] up");
        assert_eq!(v["lines"][1]["pid"], 42);
        assert_eq!(v["newer"], "s=1;i=2");
        assert_eq!(v["sources"], json!(["lososd", "losos", "everything"]));
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.contains("--priority=5"), "{args}");
        assert!(args.contains("--since=-1h"), "{args}");
        assert!(args.contains("--unit=lososd.service"), "{args}");
    }

    #[actix_web::test]
    async fn bad_filters_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let r = reader(dir.path());
        let app = app!(r);
        for uri in [
            "/api/logs?unit=*",
            "/api/logs?priority=9",
            "/api/logs?span=year",
            "/api/logs?before=--merge",
            "/api/logs?limit=0",
        ] {
            let res = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(uri)
                    .insert_header(("Authorization", format!("Bearer {KEY}")))
                    .to_request(),
            )
            .await;
            assert_eq!(res.status(), 400, "{uri}");
        }
    }

    #[actix_web::test]
    async fn a_download_is_a_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let r = reader(dir.path());
        let app = app!(r);
        let res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/logs/download?unit=lososd")
                .insert_header(("Authorization", format!("bearer {KEY}")))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 200);
        let disposition = res
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            disposition.starts_with("attachment; filename=\"losos-lososd-"),
            "{disposition}"
        );
        let body = String::from_utf8(test::read_body(res).await.to_vec()).unwrap();
        assert!(
            body.contains(" lososd.service[42] notice: starting\n"),
            "{body}"
        );
        assert!(body.contains("token=[hidden] up"), "{body}");
    }

    #[actix_web::test]
    async fn the_page_works_without_lososd() {
        let dir = tempfile::tempdir().unwrap();
        let r = reader(dir.path());
        let app = app!(r);
        // Nobody: the spare-key form, and no LosOS Login while lososd is away.
        let res =
            test::call_service(&app, test::TestRequest::get().uri(pane::PATH).to_request()).await;
        assert_eq!(res.status(), 401);
        let body = String::from_utf8(test::read_body(res).await.to_vec()).unwrap();
        assert!(body.contains("action=\"/basic/settings/logs/key\""));
        assert!(!body.contains("/basic/sign-in?next="));

        // The spare key.
        let res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(pane::PATH)
                .insert_header(("Authorization", format!("Bearer {KEY}")))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 200);
        let body = String::from_utf8(test::read_body(res).await.to_vec()).unwrap();
        assert!(body.contains("data-slot=\"log-line\""));
        assert!(body.contains("token=[hidden] up"));
        assert!(!body.contains("s3cr3t"));
        assert!(!body.contains(KEY));
        // lososd is down, and the page says so.
        assert!(body.contains("unit=lososd&amp;priority=6&amp;span=boot"));
        assert!(body.contains("role=\"alert\""));

        // Following asks again by itself.
        let res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("{}?unit=lososd&follow=1", pane::PATH))
                .insert_header(("Authorization", format!("Bearer {KEY}")))
                .to_request(),
        )
        .await;
        let body = String::from_utf8(test::read_body(res).await.to_vec()).unwrap();
        assert!(body.contains("http-equiv=\"refresh\""), "{body}");
        assert!(body.contains("data-slot=\"log-following\""));
    }

    #[actix_web::test]
    async fn a_person_who_is_not_an_administrator_is_told_so() {
        let dir = tempfile::tempdir().unwrap();
        let r = reader(dir.path());
        let app = app!(r);
        let res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(pane::PATH)
                .insert_header(("Remote-User", "guest"))
                .insert_header(("Remote-Groups", "losos-viewers"))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 403);
        let body = String::from_utf8(test::read_body(res).await.to_vec()).unwrap();
        assert!(!body.contains("data-slot=\"log-line\""));
    }

    #[actix_web::test]
    async fn the_spare_key_signs_in() {
        let dir = tempfile::tempdir().unwrap();
        let r = reader(dir.path());
        let app = app!(r);
        let res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/basic/settings/logs/key")
                .insert_header(("content-type", "application/x-www-form-urlencoded"))
                .set_payload(format!(
                    "next=%2Fbasic%2Fsettings%2Flogs%3Funit%3Dlososd&key={KEY}"
                ))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 303);
        assert_eq!(
            res.headers().get(header::LOCATION).unwrap(),
            "/basic/settings/logs?unit=lososd"
        );
        let cookie = res
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.starts_with(&format!("losos_spare={KEY}; Path=/basic;")));

        // Somewhere else is never where it goes.
        let res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/basic/settings/logs/key")
                .set_payload(format!("next=https%3A%2F%2Fevil.example%2F&key={KEY}"))
                .to_request(),
        )
        .await;
        assert_eq!(res.headers().get(header::LOCATION).unwrap(), pane::PATH);

        let res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/basic/settings/logs/key")
                .set_payload("next=&key=nope")
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 401);
        assert!(res.headers().get(header::SET_COOKIE).is_none());

        let res = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/basic/settings/logs/key")
                .insert_header(("Sec-Fetch-Site", "cross-site"))
                .set_payload(format!("key={KEY}"))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 403);
    }

    #[actix_web::test]
    async fn excerpts_are_for_the_box_alone() {
        let dir = tempfile::tempdir().unwrap();
        let r = reader(dir.path());
        let app = app!(r);
        let res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/internal/logs/excerpt?unit=lososd")
                .insert_header((FRONT, "1"))
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), 404);
        let body = test::call_and_read_body(
            &app,
            test::TestRequest::get()
                .uri("/internal/logs/excerpt?unit=lososd&until=1761739201000000&lines=10")
                .to_request(),
        )
        .await;
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            body.contains("lososd.service[42] info: token=[hidden] up"),
            "{body}"
        );
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        assert!(args.contains("--until=@1761739201"), "{args}");
        assert!(!args.contains("--boot"), "{args}");
    }
}
