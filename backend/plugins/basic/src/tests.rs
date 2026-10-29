//! The routes end to end against a box made of fixtures.

use super::*;
use crate::host::{Answer, Forwarded, LoginAnswer};
use losos_plugin::{Caller, Principal};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Default)]
struct Fake {
    answers: HashMap<String, Answer>,
    calls: Vec<(String, String, Vec<u8>)>,
    login: HashMap<String, LoginAnswer>,
    logins: Vec<(String, Option<String>, Option<Value>)>,
    key: Option<String>,
    files: bool,
}

impl Fake {
    fn owner() -> Fake {
        let mut f = Fake {
            files: true,
            ..Fake::default()
        };
        f.set("/api/me", json!({"name": "matus", "displayName": "Matus", "admin": true, "standalone": true, "sections": []}));
        f.set(
            "/api/settings",
            json!({
                "sharingMyStorage": false, "hostName": "losos", "gpuEnable": false,
                "proxyEnable": true, "clusterEnable": false, "shareCompute": false,
                "computeWindowStart": "23:00", "computeWindowEnd": "07:00",
                "hardeningApparmor": false, "hardeningMalloc": false,
                "hardeningNosmt": false, "hardeningUsbguard": false
            }),
        );
        f.set(
            "/api/options",
            json!({"options": [{"name": "cloud.trashDays", "set": "30"}], "stray": []}),
        );
        f.set(
            "/api/status",
            json!({"state": "idle", "progress": 0, "message": ""}),
        );
        f.set("/api/storage", json!({"totalBytes": 100_000_000_000u64, "usedBytes": 25_000_000_000u64, "reserveBytes": 8_000_000_000u64}));
        f.set("/api/apply", json!({"job": "j1"}));
        f
    }

    fn set(&mut self, path: &str, json: Value) {
        self.answers
            .insert(path.to_string(), Answer { status: 200, json });
    }

    fn posted(&self, path: &str) -> Option<String> {
        self.calls
            .iter()
            .find(|(m, p, _)| m == "POST" && p == path)
            .map(|(_, _, b)| String::from_utf8_lossy(b).into_owned())
    }
}

impl Host for Fake {
    fn call(&mut self, method: &str, path: &str, _ct: Option<&str>, body: &[u8]) -> Answer {
        self.calls
            .push((method.to_string(), path.to_string(), body.to_vec()));
        self.answers.get(path).cloned().unwrap_or(Answer {
            status: 404,
            json: Value::Null,
        })
    }

    fn login(
        &mut self,
        method: &str,
        path: &str,
        _f: &Forwarded,
        session: Option<&str>,
        body: Option<&Value>,
    ) -> Option<LoginAnswer> {
        self.logins.push((
            format!("{method} {path}"),
            session.map(str::to_string),
            body.cloned(),
        ));
        self.login.get(path).cloned()
    }

    fn key_opens(&mut self, key: &str) -> bool {
        self.key.as_deref() == Some(key)
    }

    fn ui_file(&mut self, path: &str) -> Option<Vec<u8>> {
        match path {
            "index.html" => Some(br#"<link rel="stylesheet" href="/assets/index-AB.css"><link rel="icon" href="/assets/losos-1.png">"#.to_vec()),
            "basic/messages.en.json" => Some(
                serde_json::to_vec(&json!({
                    "settings.applyBar.pending": {"one": "{count} change not applied yet", "other": "{count} changes not applied yet"},
                    "home.header.answering": "Answering",
                    "settings.panes.hardware.label": "Hardware",
                }))
                .unwrap(),
            ),
            _ => None,
        }
    }

    fn apps_running(&mut self) -> (bool, bool) {
        (self.files, false)
    }
}

fn person() -> Caller {
    Caller {
        principal: Principal::Person(losos_plugin::Person {
            name: "matus".into(),
            display_name: "Matus".into(),
            groups: vec!["losos-admins".into()],
            admin: true,
        }),
        remote: "192.168.1.20".into(),
        section: "admin".into(),
    }
}

fn request(
    method: &str,
    route: &str,
    path: &str,
    query: &str,
    body: &str,
    headers: &[(&str, &str)],
) -> Request {
    let mut h: Vec<(String, String)> = vec![
        ("x-forwarded-host".into(), "losos.local".into()),
        ("x-forwarded-proto".into(), "https".into()),
    ];
    h.extend(
        headers
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string())),
    );
    Request {
        method: method.into(),
        path: path.into(),
        route: route.into(),
        params: Vec::new(),
        query: query.into(),
        headers: h,
        body: body.as_bytes().to_vec(),
        caller: person(),
    }
}

fn get(f: &mut Fake, route: &str, path: &str) -> Response {
    handle_with(f, &request("GET", route, path, "", "", &[]), 1000)
}

fn html(r: &Response) -> String {
    String::from_utf8_lossy(&r.body).into_owned()
}

fn cookies(r: &Response) -> Vec<&str> {
    r.headers
        .iter()
        .filter(|(k, _)| k == "set-cookie")
        .map(|(_, v)| v.as_str())
        .collect()
}

fn location(r: &Response) -> &str {
    r.headers
        .iter()
        .find(|(k, _)| k == "location")
        .map_or("", |(_, v)| v.as_str())
}

fn no_script(page: &str) {
    assert!(!page.contains("<script"), "a script on the page");
    assert!(!page.contains(" style="), "a style attribute on the page");
    assert!(
        !page.contains(" on"),
        "an event handler on the page: {}",
        &page[page.find(" on").unwrap()..][..40]
    );
}

#[test]
fn nobody_signed_in_goes_to_sign_in() {
    let mut f = Fake::owner();
    f.answers.remove("/api/me");
    let r = get(&mut f, "/api/basic/{page}", "/api/basic/storage");
    assert_eq!(r.status, 303);
    assert_eq!(location(&r), "/basic/sign-in?next=/basic/storage");
}

#[test]
fn the_overview_draws_without_a_script() {
    let mut f = Fake::owner();
    let r = get(&mut f, "/api/basic/", "/api/basic/");
    assert_eq!(r.status, 200);
    let page = html(&r);
    no_script(&page);
    assert!(page.contains(">losos</h1>"));
    assert!(page.contains("data-app=\"files\""));
    assert!(!page.contains("data-app=\"code\""));
    assert!(page.contains("w-[25%]"));
    assert!(page.contains("href=\"/assets/index-AB.css\""));
    assert!(page.contains("href=\"/basic/counts.css?lang=en\""));
    assert!(page.contains("data-notice=\"mesh\""));
}

#[test]
fn a_switch_applies_only_what_changed() {
    let mut f = Fake::owner();
    let body = "was.gpuEnable=off&gpuEnable=on&apply=1";
    let req = request(
        "POST",
        "/api/basic/settings/{pane}",
        "/api/basic/settings/hardware",
        "",
        body,
        &[("sec-fetch-site", "same-origin")],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert_eq!(r.status, 303);
    assert_eq!(location(&r), "/basic/settings/hardware");
    assert!(cookies(&r)
        .iter()
        .any(|c| c.starts_with("losos_basic_job=j1.1000;")));
    let nix = f.posted("/api/apply").unwrap();
    assert!(nix.contains("  losos.gpu.enable = true;\n"));
    assert!(nix.contains("  losos.proxy.enable = true;\n"));
    assert!(nix.contains("  losos.cloud.trashDays = 30;\n"));
}

#[test]
fn nothing_changed_starts_nothing() {
    let mut f = Fake::owner();
    let req = request(
        "POST",
        "/api/basic/settings/{pane}",
        "/api/basic/settings/security",
        "",
        "was.hardeningNosmt=off",
        &[],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert_eq!(r.status, 303);
    assert!(f.posted("/api/apply").is_none());
}

#[test]
fn a_form_from_another_site_is_refused() {
    let mut f = Fake::owner();
    let req = request(
        "POST",
        "/api/basic/settings/{pane}",
        "/api/basic/settings/hardware",
        "",
        "was.gpuEnable=off&gpuEnable=on",
        &[("sec-fetch-site", "cross-site")],
    );
    assert_eq!(handle_with(&mut f, &req, 1000).status, 403);
    assert!(f.posted("/api/apply").is_none());
}

#[test]
fn a_bad_name_is_said_and_not_sent() {
    let mut f = Fake::owner();
    let req = request(
        "POST",
        "/api/basic/settings/{pane}",
        "/api/basic/settings/network",
        "",
        "was.hostName=losos&hostName=-bad&was.proxyEnable=on&proxyEnable=on",
        &[],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert_eq!(r.status, 400);
    assert!(html(&r).contains("value=\"-bad\""));
    assert!(html(&r).contains("data-slot=\"field-error\""));
    assert!(f.posted("/api/apply").is_none());
}

#[test]
fn a_rebuild_shows_its_bar_and_asks_again() {
    let mut f = Fake::owner();
    f.set(
        "/api/status",
        json!({"state": "building", "message": "building the system", "job": "j1"}),
    );
    let req = request(
        "GET",
        "/api/basic/settings/{pane}",
        "/api/basic/settings/hardware",
        "",
        "",
        &[("x-losos-basic-job", "j1.990")],
    );
    let r = handle_with(&mut f, &req, 1000);
    let page = html(&r);
    no_script(&page);
    assert!(page.contains("http-equiv=\"refresh\" content=\"4\""));
    assert!(page.contains("data-basic-rebuild"));
    assert!(page.contains("building the system"));
    assert!(!page.contains("data-basic-form"));
    assert!(page.contains("disabled"));
}

#[test]
fn the_end_of_our_rebuild_is_a_toast_once() {
    let mut f = Fake::owner();
    f.set(
        "/api/status",
        json!({"state": "failed", "message": "error: no space", "job": "j1"}),
    );
    let req = request(
        "GET",
        "/api/basic/settings/{pane}",
        "/api/basic/settings/hardware",
        "",
        "",
        &[("x-losos-basic-job", "j1.990")],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert!(html(&r).contains("data-basic-toast"));
    assert!(html(&r).contains("error: no space"));
    assert!(cookies(&r)
        .iter()
        .any(|c| c.starts_with("losos_basic_job=;")));
    let again = get(
        &mut f,
        "/api/basic/settings/{pane}",
        "/api/basic/settings/hardware",
    );
    assert!(!html(&again).contains("data-basic-toast"));
}

/// What the pages plugin answers GET /api/pages with for `pages` (id to
/// source): each page as losos-mdx draws it against the admin page's
/// blocks.json.
fn changed_pages(pages: &[(&str, &str)], style: Option<&str>) -> Value {
    let registry = losos_mdx::page::Registry::from_json(include_str!(
        "../../../../admin-ui/app/src/pages/blocks.json"
    ))
    .unwrap();
    let mut out = serde_json::Map::new();
    for (id, source) in pages {
        let rendered = losos_mdx::page::render_page(source, registry.page(id).unwrap()).unwrap();
        assert!(rendered.problems.is_empty(), "{:?}", rendered.problems);
        out.insert(
            (*id).to_string(),
            json!({ "html": rendered.html, "at": "2026-10-29T10:00:00Z", "by": "matus" }),
        );
    }
    json!({ "pages": out, "style": style })
}

#[test]
fn a_changed_overview_draws_its_text_button_and_kept_blocks() {
    let mut f = Fake::owner();
    f.set(
        "/api/pages",
        changed_pages(
            &[(
                "overview",
                "# Office box\n\nAsk first.\n\n<Button href=\"/apps\" variant=\"secondary\">Apps</Button>\n\n<Apps hideStorage />\n",
            )],
            Some("4"),
        ),
    );
    let page = html(&get(&mut f, "/api/basic/", "/api/basic/"));
    no_script(&page);
    assert!(page.contains("Office box</h1>"), "{page}");
    assert!(page.contains("mdx-page-text"));
    assert!(!page.contains(">losos</h1>"), "BoxName was left out");
    assert!(page.contains("data-app=\"files\""), "the apps are kept");
    assert!(!page.contains("w-[25%]"), "the storage strip was hidden");
    assert!(
        !page.contains("losos-block"),
        "a placeholder reached the page"
    );
    assert!(page.contains(&losos_page::ui::button_classes(
        losos_page::ui::Variant::Secondary,
        losos_page::ui::Size::Md
    )));
    // The stylesheet comes last, after the bundle's.
    let bundle = page.find("href=\"/assets/index-AB.css\"").unwrap();
    let theirs = page.find("href=\"/api/pages/style.css?v=4\"").unwrap();
    assert!(bundle < theirs);
}

#[test]
fn a_page_nobody_changed_draws_as_before() {
    let mut f = Fake::owner();
    let before = html(&get(&mut f, "/api/basic/{page}", "/api/basic/storage"));
    f.set(
        "/api/pages",
        changed_pages(&[("overview", "<BoxName />\n")], None),
    );
    let after = html(&get(&mut f, "/api/basic/{page}", "/api/basic/storage"));
    assert_eq!(before, after);
    f.set(
        "/api/pages",
        changed_pages(
            &[("settings.storage", "Ask before you grow it.\n\n<Disk />\n")],
            None,
        ),
    );
    let changed = html(&get(&mut f, "/api/basic/{page}", "/api/basic/storage"));
    assert!(changed.contains("Ask before you grow it."));
    assert!(
        changed.contains("for=\"basic-grow\""),
        "the disk block is kept"
    );
}

#[test]
fn the_reserve_is_used_after_asking() {
    let mut f = Fake::owner();
    let page = html(&get(&mut f, "/api/basic/{page}", "/api/basic/storage"));
    no_script(&page);
    assert!(page.contains("for=\"basic-grow\""));
    assert!(page.contains("data-basic-toggle"));
    f.set(
        "/api/grow",
        json!({"grew": true, "beforeBytes": 10, "afterBytes": 20, "claimedBytes": 10}),
    );
    let r = handle_with(
        &mut f,
        &request(
            "POST",
            "/api/basic/{page}",
            "/api/basic/storage",
            "",
            "grow=1",
            &[],
        ),
        1000,
    );
    assert_eq!(r.status, 303);
    assert!(cookies(&r)
        .iter()
        .any(|c| c.starts_with("losos_basic_flash=grew.10.20;")));
    let after = handle_with(
        &mut f,
        &request(
            "GET",
            "/api/basic/{page}",
            "/api/basic/storage",
            "",
            "",
            &[("x-losos-basic-flash", "grew.10.20")],
        ),
        1000,
    );
    assert!(html(&after).contains("data-tone=\"success\""));
    assert!(cookies(&after)
        .iter()
        .any(|c| c.starts_with("losos_basic_flash=;")));
}

#[test]
fn panes_not_here_yet_say_so_and_others_are_not_found() {
    let mut f = Fake::owner();
    let r = get(
        &mut f,
        "/api/basic/settings/{pane}",
        "/api/basic/settings/backup",
    );
    assert_eq!(r.status, 200);
    assert!(html(&r).contains("data-basic-later"));
    let r = get(&mut f, "/api/basic/{page}", "/api/basic/nowhere");
    assert_eq!(r.status, 404);
    let mut viewer = Fake::owner();
    viewer.set(
        "/api/me",
        json!({"name": "ann", "admin": false, "standalone": true, "sections": ["storage"]}),
    );
    assert_eq!(
        get(
            &mut viewer,
            "/api/basic/settings/{pane}",
            "/api/basic/settings/hardware"
        )
        .status,
        404
    );
}

#[test]
fn prefs_are_a_cookie_and_the_rail_moves() {
    let req = request(
        "POST",
        "/api/basic/prefs",
        "/api/basic/prefs",
        "",
        "back=%2Fbasic%2Fstorage&rail=1",
        &[("x-losos-basic-prefs", "theme=dark")],
    );
    let r = handle_with(&mut Fake::owner(), &req, 1000);
    assert_eq!(location(&r), "/basic/storage");
    let c = cookies(&r);
    assert!(c
        .iter()
        .any(|c| c.starts_with("losos_basic=theme=dark&rail=1;")));
    assert!(c.iter().any(|c| c.starts_with("losos_basic_flash=rail;")));
    let evil = request(
        "POST",
        "/api/basic/prefs",
        "/api/basic/prefs",
        "",
        "back=https%3A%2F%2Fevil%2F&theme=light",
        &[],
    );
    assert_eq!(
        location(&handle_with(&mut Fake::owner(), &evil, 1000)),
        "/basic/"
    );
}

#[test]
fn the_counts_are_the_catalogues_plurals() {
    let r = handle_with(
        &mut Fake::owner(),
        &request(
            "GET",
            "/api/basic/counts.css",
            "/api/basic/counts.css",
            "lang=en",
            "",
            &[],
        ),
        1000,
    );
    let css = html(&r);
    assert!(css.starts_with("@counter-style basic-pending { system: fixed; symbols: \"1 change not applied yet\" \"2 changes not applied yet\""));
}

#[test]
fn a_password_signs_in_through_losos_login() {
    let mut f = Fake::owner();
    f.login.insert(
        "firstfactor".into(),
        LoginAnswer {
            status: 200,
            json: json!({"status": "OK"}),
            cookies: vec!["losos_session=s1; Path=/; Secure; HttpOnly".into()],
        },
    );
    f.login.insert(
        "authz/auth-request".into(),
        LoginAnswer {
            status: 200,
            ..LoginAnswer::default()
        },
    );
    let req = request(
        "POST",
        "/api/basic/sign-in",
        "/api/basic/sign-in",
        "",
        "username=Matus&password=pw&next=%2Fbasic%2Fstorage",
        &[],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert_eq!(r.status, 303);
    assert_eq!(location(&r), "/basic/storage");
    assert!(cookies(&r).contains(&"losos_session=s1; Path=/; Secure; HttpOnly"));
    let (_, _, body) = &f.logins[0];
    assert_eq!(body.as_ref().unwrap()["username"], "matus");
    assert_eq!(f.logins[1].1.as_deref(), Some("s1"));
}

#[test]
fn a_second_step_asks_for_the_code() {
    let mut f = Fake::owner();
    f.login.insert(
        "firstfactor".into(),
        LoginAnswer {
            status: 200,
            cookies: vec!["losos_session=s1; Path=/".into()],
            ..LoginAnswer::default()
        },
    );
    f.login.insert(
        "authz/auth-request".into(),
        LoginAnswer {
            status: 401,
            ..LoginAnswer::default()
        },
    );
    f.login.insert(
        "user/info".into(),
        LoginAnswer {
            status: 200,
            json: json!({"data": {"has_totp": true}}),
            cookies: vec![],
        },
    );
    let req = request(
        "POST",
        "/api/basic/sign-in",
        "/api/basic/sign-in",
        "",
        "username=matus&password=pw&next=%2Fbasic%2F",
        &[],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert_eq!(location(&r), "/basic/sign-in?step=code&next=/basic/");
    let page = html(&get(&mut f, "/api/basic/sign-in", "/api/basic/sign-in"));
    no_script(&page);
}

#[test]
fn a_wrong_password_is_said_on_the_dialog() {
    let mut f = Fake::owner();
    f.login.insert(
        "firstfactor".into(),
        LoginAnswer {
            status: 401,
            ..LoginAnswer::default()
        },
    );
    let req = request(
        "POST",
        "/api/basic/sign-in",
        "/api/basic/sign-in",
        "",
        "username=matus&password=no",
        &[],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert_eq!(r.status, 401);
    let page = html(&r);
    no_script(&page);
    assert!(page.contains("value=\"matus\""));
    assert!(page.contains("data-slot=\"field-error\""));
}

#[test]
fn the_spare_key_is_kept_for_the_session() {
    let mut f = Fake::owner();
    let key = "ab".repeat(32);
    f.key = Some(key.clone());
    let req = request(
        "POST",
        "/api/basic/sign-in/key",
        "/api/basic/sign-in/key",
        "",
        &format!("key={key}"),
        &[],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert_eq!(r.status, 303);
    assert!(cookies(&r)
        .iter()
        .any(|c| c.starts_with(&format!("losos_spare={key};")) && c.contains("SameSite=Strict")));
    let wrong = request(
        "POST",
        "/api/basic/sign-in/key",
        "/api/basic/sign-in/key",
        "",
        &format!("key={}", "cd".repeat(32)),
        &[],
    );
    assert_eq!(handle_with(&mut f, &wrong, 1000).status, 401);
}

#[test]
fn signing_out_forgets_both() {
    let mut f = Fake::owner();
    f.login.insert(
        "logout".into(),
        LoginAnswer {
            status: 200,
            cookies: vec!["losos_session=; Max-Age=0".into()],
            ..LoginAnswer::default()
        },
    );
    let req = request(
        "POST",
        "/api/basic/sign-out",
        "/api/basic/sign-out",
        "",
        "",
        &[("x-losos-login-session", "s1")],
    );
    let r = handle_with(&mut f, &req, 1000);
    assert_eq!(location(&r), "/basic/sign-in");
    let c = cookies(&r);
    assert!(c.iter().any(|c| c.starts_with("losos_spare=;")));
    assert!(c.contains(&"losos_session=; Max-Age=0"));
}

#[test]
fn login_starting_asks_again() {
    let mut f = Fake::owner();
    let r = get(&mut f, "/api/basic/sign-in", "/api/basic/sign-in");
    assert_eq!(r.status, 503);
    assert!(html(&r).contains("http-equiv=\"refresh\" content=\"3\""));
    assert!(html(&r).contains("data-basic-facts"));
}

#[test]
fn a_domain_is_added_and_removed_by_forms_of_its_own() {
    let mut f = Fake::owner();
    let pane = "/api/basic/settings/network";
    let route = "/api/basic/settings/{pane}";
    // No edge that hands out names: the section says so.
    f.set("/api/domains", json!({"available": false}));
    let page = html(&get(&mut f, route, pane));
    no_script(&page);
    assert!(page.contains("data-testid=\"custom-domains\""));
    assert!(!page.contains("name=\"domain-add\""));

    let view = json!({
        "available": true, "eligible": true, "reason": null,
        "target": "b1.edge.example", "addresses": ["203.0.113.7"], "max_domains": 3,
        "domains": [{
            "domain": "cloud.example.org", "status": "waiting",
            "txt_name": "_losos-challenge.cloud.example.org", "txt_value": "abc",
            "txt_found": true, "points_here": false, "problem": "notPointing",
            "checked_at": null, "verified_at": null
        }]
    });
    f.set("/api/domains", view.clone());
    let page = html(&get(&mut f, route, pane));
    no_script(&page);
    assert!(page.contains("name=\"domain-add\""));
    assert!(page.contains("name=\"domain-remove\" value=\"cloud.example.org\""));
    assert!(page.contains("b1.edge.example."));
    // A domain still waiting for its records: the page looks again.
    assert!(page.contains("http-equiv=\"refresh\" content=\"30\""));

    let r = handle_with(
        &mut f,
        &request("POST", route, pane, "", "domain-add=Shop.Example.org.", &[]),
        1000,
    );
    assert_eq!(r.status, 303);
    assert_eq!(
        f.posted("/api/domains").as_deref(),
        Some(r#"{"domain":"shop.example.org"}"#)
    );
    assert!(cookies(&r)
        .iter()
        .any(|c| c.starts_with("losos_basic_flash=domadd;")));
    // Nothing went to the settings.
    assert!(f.posted("/api/apply").is_none());

    f.answers.insert(
        "/api/domains/remove".into(),
        Answer {
            status: 409,
            json: json!({"error": "That domain is not yours"}),
        },
    );
    let r = handle_with(
        &mut f,
        &request(
            "POST",
            route,
            pane,
            "",
            "domain-remove=cloud.example.org",
            &[],
        ),
        1000,
    );
    let word = cookies(&r)
        .iter()
        .find_map(|c| c.strip_prefix("losos_basic_flash="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert!(word.starts_with("domfail."), "{word}");
    let after = handle_with(
        &mut f,
        &request(
            "GET",
            route,
            pane,
            "",
            "",
            &[("x-losos-basic-flash", &word)],
        ),
        1000,
    );
    assert!(html(&after).contains("That domain is not yours"));
}

/// Every string in the page crates' sources, one per line: what the admin
/// bundle's Tailwind reads (admin-ui/app/src/styles/index.css's `@source`)
/// so the classes only these pages use are in the stylesheet too. The
/// bundle is built from admin-ui/app alone, so it cannot read the Rust
/// files themselves. `UPDATE_BASIC_CLASSES=1 cargo test` writes the file.
#[test]
fn the_bundle_knows_every_class_these_pages_use() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut strings = std::collections::BTreeSet::new();
    for dir in ["page/src", "basic/src"] {
        let mut files: Vec<_> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        files.sort();
        for file in files {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            // The icons are paths, and the tests' strings are no page's.
            if name == "icons.rs" || name == "tests.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap();
            for literal in string_literals(&text) {
                for token in literal.split_whitespace() {
                    if token.contains('-') || token.contains(':') || token.contains('[') {
                        strings.insert(token.to_string());
                    }
                }
            }
        }
    }
    let want: String = strings.into_iter().map(|s| s + "\n").collect();
    let path = root.join("../../admin-ui/app/src/styles/basic-classes.txt");
    if std::env::var_os("UPDATE_BASIC_CLASSES").is_some() {
        std::fs::write(&path, &want).unwrap();
    }
    let have = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        have == want,
        "{} is out of date: run UPDATE_BASIC_CLASSES=1 cargo test -p losos-plugin-basic",
        path.display()
    );
}

/// The contents of every `"…"` and `r#"…"#` in `src`, escapes left as
/// they are.
fn string_literals(src: &str) -> Vec<&str> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'\'' => {
                // A char literal ('"', '\'') or a lifetime.
                if b.get(i + 2) == Some(&b'\'') {
                    i += 3;
                } else if b.get(i + 1) == Some(&b'\\') && b.get(i + 3) == Some(&b'\'') {
                    i += 4;
                } else {
                    i += 1;
                }
            }
            b'r' if b.get(i + 1) == Some(&b'#') && b.get(i + 2) == Some(&b'"') => {
                let start = i + 3;
                let end = src[start..].find("\"#").map_or(b.len(), |e| start + e);
                out.push(&src[start..end]);
                i = end + 2;
            }
            b'"' => {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                out.push(&src[start..j.min(b.len())]);
                i = j + 1;
            }
            _ => i += 1,
        }
    }
    out
}
