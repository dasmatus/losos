//! The admin pages for a browser that runs no JavaScript, at `/basic/`.
//!
//! nginx sends a page there from the script page's `<noscript>`, and maps
//! `/basic/…` onto this plugin's `/api/basic/…` routes. Every page is drawn
//! here in the script page's own markup and stylesheet ([`losos_page`]),
//! reads and changes the box through lososd's routes as the person signed
//! in ([`host`]), and works by links and forms alone: menus are `<details>`,
//! dialogs open from a checkbox's label, and what moves, moves in CSS.

pub mod custom;
pub mod format;
pub mod host;
pub mod pages;
pub mod problems;
pub mod settings;
pub mod signin;
pub mod view;
pub mod web;

use host::{Host, Real};
use losos_page::html::Html;
use losos_page::i18n::{Locale, Messages, Var};
use losos_page::icons;
use losos_page::shell::{self, Prefs};
use losos_page::ui::Toast;
use losos_plugin::{Plugin, Request, Response};
use settings::{Mine, Watch};
use view::{Ctx, Me};

pub struct Basic;

impl Plugin for Basic {
    fn handle(req: Request) -> Response {
        let mut host = Real {
            caller: req.caller.clone(),
        };
        handle_with(&mut host, &req, losos_plugin::clock::now())
    }
}

losos_plugin::export!(Basic);

/// The cookies nginx hands over, one header each.
struct Cookies {
    prefs: Prefs,
    flash: Option<String>,
    job: Option<Mine>,
    session: Option<String>,
}

impl Cookies {
    fn of(req: &Request) -> Cookies {
        let h = |n: &str| {
            req.header(n)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        Cookies {
            prefs: Prefs::parse(&h("x-losos-basic-prefs").unwrap_or_default()),
            flash: h("x-losos-basic-flash").filter(|f| f.len() <= 512),
            job: h("x-losos-basic-job").and_then(|j| Mine::parse(&j)),
            session: h("x-losos-login-session"),
        }
    }
}

/// The page's own address (`/basic/storage`) and the script page's
/// (`/storage`), from the route's (`/api/basic/storage`).
fn addresses(req: &Request) -> (String, String) {
    let rest = req.path.strip_prefix("/api/basic").unwrap_or("/");
    let rest = if rest.is_empty() { "/" } else { rest };
    let path = if rest == "/" {
        "/".to_string()
    } else {
        rest.trim_end_matches('/').to_string()
    };
    (shell::href(&path), path)
}

/// Every route of the manifest. `now` is the clock, in seconds.
pub fn handle_with(host: &mut dyn Host, req: &Request, now: u64) -> Response {
    let cookies = Cookies::of(req);
    if req.method == "POST" && !web::same_site(req) {
        return Response::bytes(
            403,
            "text/plain; charset=utf-8",
            b"This form was sent from another site.".to_vec(),
        );
    }
    let mut resp = match (req.method.as_str(), req.route.as_str()) {
        ("GET", "/api/basic/counts.css") => counts_css(host, req),
        ("POST", "/api/basic/prefs") => prefs(req, &cookies),
        ("GET", "/api/basic/sign-in") => sign_in_page(host, req, &cookies),
        ("POST", "/api/basic/sign-in" | "/api/basic/sign-in/code" | "/api/basic/sign-in/key") => {
            sign_in(host, req, &cookies)
        }
        ("POST", "/api/basic/sign-out") => {
            signin::sign_out(host, &web::forwarded(req), cookies.session.as_deref())
        }
        (_, "/api/basic/" | "/api/basic/{page}" | "/api/basic/settings/{pane}") => {
            page(host, req, &cookies, now)
        }
        _ => Response::error(404, "not found"),
    };
    // A word for one page only: gone once that page has been drawn.
    let sets_flash = resp
        .headers
        .iter()
        .any(|(k, v)| k == "set-cookie" && v.starts_with("losos_basic_flash="));
    if cookies.flash.is_some() && !sets_flash && req.method == "GET" && resp.status == 200 {
        resp = resp.header("set-cookie", &web::flash_spent());
    }
    resp
}

fn locale_of(req: &Request, prefs: &Prefs) -> Locale {
    view::locale(prefs, req.header("accept-language").unwrap_or_default())
}

/// The plural of "N changes not applied yet" for every N a pane can have,
/// as a counter style the Apply bar's CSS counter is written in.
fn counts_css(host: &mut dyn Host, req: &Request) -> Response {
    let locale = web::query(req, "lang")
        .and_then(|l| Locale::from_code(&l))
        .unwrap_or(Locale::En);
    let m = view::messages(host, locale);
    let symbols: Vec<String> = (1..=24)
        .map(|n| {
            let text = m.tv("settings.applyBar.pending", &[("count", Var::Number(n))]);
            let quoted: String = text
                .chars()
                .map(|c| match c {
                    '"' | '\\' => format!("\\{c}"),
                    c if c.is_control() => " ".to_string(),
                    c => c.to_string(),
                })
                .collect();
            format!("\"{quoted}\"")
        })
        .collect();
    let css = format!(
        "@counter-style basic-pending {{ system: fixed; symbols: {}; fallback: decimal; }}\n",
        symbols.join(" ")
    );
    Response::bytes(200, "text/css; charset=utf-8", css.into_bytes())
        .header("cache-control", "public, max-age=300")
        .header("x-content-type-options", "nosniff")
}

/// `POST /basic/prefs`: the theme, the language, the rail, a notice put
/// away. Back to the page it came from.
fn prefs(req: &Request, cookies: &Cookies) -> Response {
    let form = web::form(&req.body);
    let mut prefs = cookies.prefs.clone();
    let mut moved = false;
    if let Some(theme) = web::field(&form, "theme").and_then(shell::Theme::from_code) {
        prefs.theme = theme;
    }
    if let Some(lang) = web::field(&form, "lang") {
        prefs.lang = Locale::from_code(lang);
    }
    if let Some(rail) = web::field(&form, "rail") {
        let to = rail == "1";
        moved = to != prefs.rail;
        prefs.rail = to;
    }
    if let Some(off) = web::field(&form, "off")
        .filter(|o| !o.is_empty() && o.bytes().all(|b| b.is_ascii_lowercase()))
    {
        if !prefs.is_off(off) {
            prefs.off.push(off.to_string());
        }
    }
    let mut resp = web::see_other(&web::safe_back(web::field(&form, "back")))
        .header("set-cookie", &web::prefs_cookie(&prefs.encode()));
    if moved {
        resp = resp.header("set-cookie", &web::flash_cookie("rail"));
    }
    resp
}

fn sign_in_parts(
    host: &mut dyn Host,
    req: &Request,
    cookies: &Cookies,
) -> (Messages, shell::Assets) {
    let locale = locale_of(req, &cookies.prefs);
    let m = view::messages(host, locale);
    let mut assets = view::assets(host);
    assets
        .styles
        .push(format!("/basic/counts.css?lang={}", locale.code()));
    (m, assets)
}

fn sign_in_page(host: &mut dyn Host, req: &Request, cookies: &Cookies) -> Response {
    let next = web::safe_back(web::query(req, "next").as_deref());
    let mode = signin::mode_of(req);
    let fwd = web::forwarded(req);
    // Already in: straight on.
    if mode == signin::Mode::Password {
        if let Some(session) = cookies.session.as_deref() {
            if let Some(a) = host.login("GET", "authz/auth-request", &fwd, Some(session), None) {
                if a.status == 200 {
                    return web::see_other(&next);
                }
            }
        }
    }
    let starting = mode == signin::Mode::Password && !signin::login_up(host, &fwd);
    let (m, assets) = sign_in_parts(host, req, cookies);
    let html = signin::render(&signin::SignInPage {
        m: &m,
        prefs: &cookies.prefs,
        assets: &assets,
        mode,
        next: &next,
        starting,
        problem: None,
        name: "",
    });
    web::page(if starting { 503 } else { 200 }, html)
}

fn sign_in(host: &mut dyn Host, req: &Request, cookies: &Cookies) -> Response {
    let form = web::form(&req.body);
    let fwd = web::forwarded(req);
    let (m, assets) = sign_in_parts(host, req, cookies);
    let outcome = match req.route.as_str() {
        "/api/basic/sign-in/code" => {
            signin::code(host, &fwd, &m, cookies.session.as_deref(), &form)
        }
        "/api/basic/sign-in/key" => signin::key(host, &m, &form),
        _ => signin::password(host, &fwd, &m, &form),
    };
    let next = signin::next_of(&form);
    let name = web::field(&form, "username")
        .unwrap_or_default()
        .to_string();
    let (mode, problem, status, starting) = match outcome {
        signin::Outcome::Go(resp) => return resp,
        signin::Outcome::Again(mode, problem, status) => (mode, Some(problem), status, false),
        signin::Outcome::Down => (signin::Mode::Password, None, 503, true),
    };
    let html = signin::render(&signin::SignInPage {
        m: &m,
        prefs: &cookies.prefs,
        assets: &assets,
        mode,
        next: &next,
        starting,
        problem,
        name: &name,
    });
    web::page(status, html)
}

/// A signed-in page: who is asking, then the page.
fn page(host: &mut dyn Host, req: &Request, cookies: &Cookies, now: u64) -> Response {
    let (here, path) = addresses(req);
    let Some(me_json) = host.get("/api/me") else {
        return web::see_other(&format!("/basic/sign-in?next={here}"));
    };
    let me = Me::from_json(&me_json);
    if me.name.is_none() && !me.spare_key {
        return web::see_other(&format!("/basic/sign-in?next={here}"));
    }
    let locale = locale_of(req, &cookies.prefs);
    let m = view::messages(host, locale);
    let mut assets = view::assets(host);
    assets
        .styles
        .push(format!("/basic/counts.css?lang={}", locale.code()));
    // The pages an administrator changed, and their stylesheet, last so an
    // equal rule of theirs wins (crate::custom).
    let changed = if me.locked {
        None
    } else {
        host.get("/api/pages")
    };
    if let Some(href) = custom::style_href(changed.as_ref()) {
        assets.styles.push(href);
    }
    let office = !me.locked && host.office_folder();
    let nav = view::nav(&me, &m, &path, office);
    let person = me.person(&m);
    let pins = if me.locked {
        None
    } else {
        host.get("/api/pins").map(|v| view::pins(&v, &m))
    };
    let mut ctx = Ctx {
        m,
        prefs: cookies.prefs.clone(),
        assets,
        me,
        person,
        nav,
        path: path.clone(),
        here: here.clone(),
        rail_moved: cookies.flash.as_deref() == Some("rail"),
        refresh: None,
        pins,
        page: None,
    };

    let mut toasts: Vec<Toast> = Vec::new();
    if let Some(flash) = cookies.flash.as_deref() {
        if let Some(t) = pages::grow_toast(&ctx.m, flash) {
            toasts.push(t);
        }
        if let Some(t) = pages::domain_toast(&ctx.m, flash) {
            toasts.push(t);
        }
        if let Some(why) = flash.strip_prefix("nostart") {
            let why = why
                .strip_prefix('.')
                .map(web::decode)
                .filter(|w| !w.is_empty());
            toasts.push(pages::not_started(&ctx.m, why.as_deref()));
        }
    }

    // The settings ride along for the mesh notice and every settings pane.
    let settings = if ctx.me.pane_open("network") || ctx.me.admin {
        host.get("/api/settings")
    } else {
        None
    };
    let mut extra_cookies: Vec<String> = Vec::new();
    if ctx.prefs.is_off("mesh")
        && settings
            .as_ref()
            .is_some_and(|s| s["clusterEnable"] == true && s["sharingMyStorage"] == true)
    {
        // On a mesh again: the notice may come back the next time.
        ctx.prefs.off.retain(|o| o != "mesh");
        extra_cookies.push(web::prefs_cookie(&ctx.prefs.encode()));
    }

    let pane = view::pane_at(&path);
    let form = web::form(&req.body);
    let post = req.method == "POST";

    // Settings panes and the overview watch this browser's rebuild.
    let watches =
        path == "/" || matches!(pane, Some("network" | "hardware" | "security" | "storage"));
    let rebuild = if watches {
        Some(pages::rebuild(host, cookies.job.as_ref(), now))
    } else {
        None
    };
    if let Some(r) = &rebuild {
        if let Some(t) = r.toast(&ctx.m) {
            toasts.push(t);
        }
        if matches!(r.watch, Watch::Ended { .. })
            || (cookies.job.is_some() && r.watch == Watch::Idle)
        {
            extra_cookies.push(Mine::gone());
        }
        if r.busy() {
            ctx.refresh = Some(4);
        }
    }

    let notice = |ctx: &Ctx| pages::mesh_notice(ctx, settings.as_ref());
    let finish = |status: u16, html: Html, extra: &[String]| {
        let mut resp = web::page(status, html);
        for c in extra {
            resp = resp.header("set-cookie", c);
        }
        resp
    };

    // ── The changes ─────────────────────────────────────────────────────
    if post {
        let busy = rebuild.as_ref().is_some_and(pages::Rebuild::busy);
        match (path.as_str(), pane) {
            ("/storage", _)
                if ctx.me.pane_open("storage")
                    && web::field(&form, "grow") == Some("1")
                    && !busy =>
            {
                let answer = host.call("POST", "/api/grow", None, &[]);
                return web::see_other(&here)
                    .header("set-cookie", &web::flash_cookie(&pages::grow_word(&answer)));
            }
            (_, Some("network"))
                if ctx.me.pane_open("network")
                    && (web::field(&form, "domain-add").is_some()
                        || web::field(&form, "domain-remove").is_some()) =>
            {
                let removed = web::field(&form, "domain-remove");
                let domain = removed
                    .or_else(|| web::field(&form, "domain-add"))
                    .unwrap_or_default()
                    .trim()
                    .trim_end_matches('.')
                    .to_lowercase();
                if domain.is_empty() {
                    return web::see_other(&here);
                }
                let route = if removed.is_some() {
                    "/api/domains/remove"
                } else {
                    "/api/domains"
                };
                let body = serde_json::json!({ "domain": domain }).to_string();
                let answer = host.call("POST", route, Some("application/json"), body.as_bytes());
                let word = pages::domain_word(&answer, removed.map(|_| domain.as_str()));
                return web::see_other(&here).header("set-cookie", &web::flash_cookie(&word));
            }
            (_, Some("problems")) if ctx.me.pane_open("problems") => {
                return web::see_other(&problems::post(host, req, &here));
            }
            (_, Some(p @ ("network" | "hardware" | "security")))
                if ctx.me.pane_open(p) && !busy =>
            {
                let (switches, texts): (&[&'static str], &[&'static str]) = match p {
                    "network" => (pages::NETWORK_SWITCHES, pages::NETWORK_TEXTS),
                    "hardware" => (pages::HARDWARE, &[]),
                    _ => (pages::SECURITY, &[]),
                };
                let changes = settings::changes(&form, switches, texts);
                if let Some(c) = changes.iter().find(|c| c.key == "hostName") {
                    let name = c.value.as_str().unwrap_or_default();
                    if !settings::host_name_ok(name) {
                        let key = if name.is_empty() {
                            "settings.form.hostEmpty"
                        } else if name.len() > 63 {
                            "settings.form.hostTooLong"
                        } else {
                            "settings.form.hostChars"
                        };
                        let mut shown = settings.clone().unwrap_or_default();
                        shown["hostName"] = c.value.clone();
                        let problem = ctx.m.t(key);
                        let r = rebuild.unwrap_or(pages::Rebuild { watch: Watch::Idle });
                        let title = ctx.m.t("settings.panes.network.label");
                        let domains = pages::domains(&ctx, host.get("/api/domains").as_ref(), true);
                        let main = pages::network(&ctx, &shown, &r, Some(&problem), domains);
                        return finish(
                            400,
                            pages::frame(&ctx, &title, notice(&ctx), &toasts, main),
                            &extra_cookies,
                        );
                    }
                }
                if changes.is_empty() {
                    return web::see_other(&here);
                }
                return match settings::apply(host, &changes) {
                    Ok(job) => {
                        let mine = Mine {
                            job: if job.is_empty() {
                                "started".to_string()
                            } else {
                                job
                            },
                            since: now,
                        };
                        web::see_other(&here).header("set-cookie", &mine.cookie())
                    }
                    Err(settings::NotStarted(why)) => {
                        let word = format!("nostart.{}", encode(&why.unwrap_or_default()));
                        web::see_other(&here).header("set-cookie", &web::flash_cookie(&word))
                    }
                };
            }
            _ => return web::see_other(&here),
        }
    }

    // ── The pages ───────────────────────────────────────────────────────
    let idle = pages::Rebuild { watch: Watch::Idle };
    let r = rebuild.as_ref().unwrap_or(&idle);
    let empty = serde_json::Value::Null;
    let s = settings.as_ref().unwrap_or(&empty);
    let (status, title, main) = match (path.as_str(), pane) {
        ("/", _) => {
            let browser = web::forwarded(req).host;
            let o = pages::Overview::read(host, &browser, &ctx.m);
            if o.status.building() {
                ctx.refresh = Some(5);
            }
            ctx.page = Some("overview");
            (
                200,
                ctx.m.t("shell.nav.overview"),
                pages::overview(
                    &ctx,
                    &o,
                    custom::changed(changed.as_ref(), "overview").as_deref(),
                ),
            )
        }
        ("/search", _) => {
            let q = web::query(req, "q").unwrap_or_default();
            (200, ctx.m.t("palette.title"), pages::search(&ctx, &q))
        }
        ("/setup", _) => (
            200,
            ctx.m.t("home.setup.title"),
            pages::later(&ctx, &ctx.m.t("home.setup.title"), icons::ROCKET_01),
        ),
        ("/pins", _) => (
            200,
            ctx.m.t("pins.edit"),
            pages::later(&ctx, &ctx.m.t("pins.edit"), icons::PIN),
        ),
        (_, Some(p)) if !ctx.me.pane_open(p) => {
            (404, ctx.m.t("basic.notFound.title"), pages::not_found(&ctx))
        }
        (_, Some("storage")) => {
            let st = host.get("/api/storage");
            ctx.page = Some("settings.storage");
            (
                200,
                ctx.m.t("settings.panes.storage.label"),
                pages::storage(
                    &ctx,
                    st.as_ref(),
                    r,
                    custom::changed(changed.as_ref(), "settings.storage").as_deref(),
                ),
            )
        }
        (_, Some("network")) => {
            let answer = host.get("/api/domains");
            if pages::domains_waiting(answer.as_ref()) && ctx.refresh.is_none() {
                ctx.refresh = Some(30);
            }
            let domains = pages::domains(&ctx, answer.as_ref(), false);
            (
                200,
                ctx.m.t("settings.panes.network.label"),
                pages::network(&ctx, s, r, None, domains),
            )
        }
        (_, Some("hardware")) => (
            200,
            ctx.m.t("settings.panes.hardware.label"),
            pages::hardware(&ctx, s, r),
        ),
        (_, Some("security")) => {
            let options = host.get("/api/options");
            (
                200,
                ctx.m.t("settings.panes.security.label"),
                pages::security(&ctx, s, options.as_ref(), r),
            )
        }
        (_, Some("problems")) => {
            let list = host.get("/api/problems");
            let reading = web::query(req, "read")
                .filter(|id| problems::id_ok(id))
                .map(|id| problems::Reading::read(host, &id));
            if reading.as_ref().is_some_and(problems::Reading::waiting) {
                ctx.refresh = Some(3);
            }
            (
                200,
                ctx.m.t("settings.panes.problems.label"),
                problems::pane(&ctx, list.as_ref(), reading.as_ref()),
            )
        }
        (_, Some(p)) => {
            let title = ctx.m.t(&format!("settings.panes.{p}.label"));
            (
                200,
                title.clone(),
                pages::later(&ctx, &title, view::pane_glyph(p)),
            )
        }
        _ => match pages::label_of(&ctx, &path) {
            Some((label, glyph)) => (200, label.clone(), pages::later(&ctx, &label, glyph)),
            None => (404, ctx.m.t("basic.notFound.title"), pages::not_found(&ctx)),
        },
    };
    let html = pages::frame(&ctx, &title, notice(&ctx), &toasts, main);
    finish(status, html, &extra_cookies)
}

/// A value for a cookie: letters and digits as they are, the rest as
/// `%XX`, at most a few hundred bytes.
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if out.len() > 300 {
            break;
        }
        if b.is_ascii_alphanumeric() {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests;
