//! Signing in without a script. LosOS Login's own page is a script page,
//! so these forms ask for the same name, password and code and hand them
//! to LosOS Login's API on the person's behalf, then pass its session
//! cookie back to the browser: from then on nginx asks LosOS Login about
//! every request, as it does for the script pages. The spare admin key is
//! the other way in, kept in a cookie only nginx reads.
//!
//! App.tsx's SignInDialog is the look: the same dialog over the bare top
//! bar, its title, its fields and its link to the other way in.

use crate::host::{Forwarded, Host, LoginAnswer};
use crate::web;
use losos_page::html::{el, Html};
use losos_page::i18n::Messages;
use losos_page::shell::{self, Assets, Frame, Prefs};
use losos_page::ui::{self, Size, Variant};
use losos_plugin::{Request, Response};
use serde_json::json;

/// Which form the dialog shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Password,
    Code,
    Key,
    /// The account's second step is a passkey, which needs a script.
    Passkey,
}

/// What the dialog says under its fields.
pub struct Problem(pub String);

pub struct SignInPage<'a> {
    pub m: &'a Messages,
    pub prefs: &'a Prefs,
    pub assets: &'a Assets,
    pub mode: Mode,
    pub next: &'a str,
    /// LosOS Login is not answering yet.
    pub starting: bool,
    pub problem: Option<Problem>,
    pub name: &'a str,
}

/// The shell's facts, as App.tsx's FACTS list them.
const FACTS: usize = 17;

pub fn render(p: &SignInPage<'_>) -> Html {
    let m = p.m;
    let here = format!("/basic/sign-in?next={}", p.next);
    let frame = Frame {
        m,
        assets: p.assets,
        prefs: p.prefs,
        here: &here,
        title: &m.t("shell.signIn.title"),
        person: None,
        nav: &[],
        tabs: &[],
        rail_moved: false,
        // Until LosOS Login answers, the page asks again by itself, as the
        // script page's spinner does.
        refresh: p.starting.then_some(3),
        pins: None,
        page: None,
    };
    let (title, hint) = match p.mode {
        Mode::Password => (m.t("shell.signIn.title"), m.t("basic.signIn.hint")),
        Mode::Code => (m.t("basic.code.title"), m.t("basic.code.hint")),
        Mode::Key => (m.t("shell.signIn.title"), m.t("shell.signIn.hint")),
        Mode::Passkey => (m.t("basic.passkey.title"), m.t("basic.passkey.body")),
    };
    let error_id = "sign-in-problem";
    let invalid = p.problem.is_some();
    let problem = p
        .problem
        .as_ref()
        .map(|Problem(text)| ui::field_error(error_id, text));
    let described = invalid.then_some(error_id);

    let mut body = ui::dialog_body().class("flex flex-col gap-2");
    let action;
    let mut submit = Some(m.t("shell.signIn.go"));
    match p.mode {
        Mode::Password if p.starting => {
            action = "/basic/sign-in";
            submit = None;
            body = body.child(starting_box(m));
        }
        Mode::Password => {
            action = "/basic/sign-in";
            body = body
                .child(
                    ui::field()
                        .attr_opt("data-invalid", invalid.then_some("true"))
                        .child(ui::field_label("username", &m.t("shell.signIn.nameLabel")))
                        .child(
                            ui::input("username", "username", p.name)
                                .class("w-full")
                                .attr("autocomplete", "username")
                                .attr("autocapitalize", "none")
                                .attr("spellcheck", "false")
                                .flag("required", true)
                                .flag("autofocus", p.name.is_empty()),
                        ),
                )
                .child(
                    ui::field()
                        .attr_opt("data-invalid", invalid.then_some("true"))
                        .child(ui::field_label(
                            "password",
                            &m.t("shell.signIn.newPassword"),
                        ))
                        .child(
                            ui::input("password", "password", "")
                                .class("w-full")
                                .attr("type", "password")
                                .attr("autocomplete", "current-password")
                                .attr_opt("aria-invalid", invalid.then_some("true"))
                                .attr_opt("aria-describedby", described)
                                .flag("required", true)
                                .flag("autofocus", !p.name.is_empty()),
                        )
                        .child(problem),
                );
        }
        Mode::Code => {
            action = "/basic/sign-in/code";
            submit = Some(m.t("basic.code.go"));
            body = body.child(
                ui::field()
                    .attr_opt("data-invalid", invalid.then_some("true"))
                    .child(ui::field_label("code", &m.t("basic.code.label")))
                    .child(
                        ui::input("code", "code", "")
                            .class("w-full")
                            .class("numeric tracking-[0.2em]")
                            .attr("inputmode", "numeric")
                            .attr("autocomplete", "one-time-code")
                            .attr("pattern", "[0-9]{6}")
                            .attr("maxlength", "6")
                            .attr_opt("aria-invalid", invalid.then_some("true"))
                            .attr_opt("aria-describedby", described)
                            .flag("required", true)
                            .flag("autofocus", true),
                    )
                    .child(problem),
            );
        }
        Mode::Key => {
            action = "/basic/sign-in/key";
            submit = Some(m.t("shell.signIn.unlock"));
            body = body.child(
                ui::field()
                    .attr_opt("data-invalid", invalid.then_some("true"))
                    .child(ui::field_label("admin-key", &m.t("shell.signIn.label")))
                    .child(
                        ui::input("admin-key", "key", "")
                            .class("w-full")
                            .class("numeric tracking-[0.01em]")
                            .attr("autocomplete", "off")
                            .attr("spellcheck", "false")
                            .attr("autocapitalize", "off")
                            .attr("pattern", "[0-9a-fA-F]{64}")
                            .attr_opt("aria-invalid", invalid.then_some("true"))
                            .attr_opt("aria-describedby", described)
                            .flag("required", true)
                            .flag("autofocus", true),
                    )
                    .child(problem)
                    .child(ui::field_description(&m.t("basic.key.remembered"))),
            );
        }
        Mode::Passkey => {
            action = "/basic/sign-out";
            submit = Some(m.t("basic.passkey.back"));
        }
    }
    let other = match p.mode {
        Mode::Key => Some((
            format!("/basic/sign-in?next={}", p.next),
            m.t("shell.signIn.useSignIn"),
        )),
        Mode::Password | Mode::Passkey => Some((
            format!("/basic/sign-in?with=key&next={}", p.next),
            m.t("shell.signIn.useKey"),
        )),
        Mode::Code => None,
    };
    if let Some((to, text)) = other {
        body = body.child(
            el("a")
                .attr("href", &to)
                .class("mt-1 self-start text-[13px] text-accent underline-offset-2 hover:underline")
                .text(&text),
        );
    }

    let form = el("form")
        .attr("method", "post")
        .attr("action", action)
        .child(ui::hidden("next", p.next))
        .child(ui::dialog_header("sign-in-title", &title, &hint))
        .child(body)
        .child(submit.map(|label| {
            Html::from(
                ui::dialog_footer().child(ui::button(Variant::Primary, Size::Md).text(&label)),
            )
        }));
    let main = ui::dialog("w-[min(26rem,calc(100vw-2rem))]", "sign-in-title", form);
    shell::page(&frame, main)
}

/// App.tsx's "LosOS Login is starting" box, its facts taking turns by
/// CSS (basic.css's `[data-basic-facts]`) where the script page uses a
/// timer.
fn starting_box(m: &Messages) -> Html {
    el("div")
        .class("flex flex-col gap-3 rounded-card border border-line bg-sunk p-3")
        .child(
            el("div")
                .class("flex items-center gap-2 text-[13px] text-muted")
                .child(ui::spinner(18, &m.t("ui.working"), ""))
                .child(
                    el("p")
                        .attr("aria-live", "polite")
                        .class("flex-1")
                        .text(&m.t("shell.signIn.starting")),
                ),
        )
        .child(
            el("div")
                .attr("data-basic-facts", "")
                .class("grid text-[13px] text-ink")
                .children((1..=FACTS).map(|n| {
                    el("p")
                        .attr("data-fact", &n.to_string())
                        .child(
                            el("span")
                                .class("font-medium")
                                .text(&format!("{}. ", m.t("shell.facts.title"))),
                        )
                        .text(&m.t(&format!("shell.facts.{n}")))
                })),
        )
        .into()
}

// ── The forms ───────────────────────────────────────────────────────────

/// The cookie's value in a `Set-Cookie` from LosOS Login.
fn session_of(cookies: &[String]) -> Option<String> {
    cookies.iter().find_map(|c| {
        let first = c.split(';').next()?.trim();
        let value = first.strip_prefix("losos_session=")?;
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// LosOS Login's cookies, passed on as they came: their domain, path and
/// flags are its own.
fn relay(mut resp: Response, answer: &LoginAnswer) -> Response {
    for c in &answer.cookies {
        resp = resp.header("set-cookie", c);
    }
    resp
}

/// Where to go once in: the page asked for, checked.
#[must_use]
pub fn next_of(form: &[(String, String)]) -> String {
    web::safe_back(web::field(form, "next"))
}

/// Whether this session may open the pages yet: the question nginx asks.
fn session_opens(host: &mut dyn Host, fwd: &Forwarded, session: &str) -> Option<bool> {
    let a = host.login("GET", "authz/auth-request", fwd, Some(session), None)?;
    Some(a.status == 200)
}

pub enum Outcome {
    /// Go here, with these headers.
    Go(Response),
    /// Draw the dialog again in this mode with this problem.
    Again(Mode, Problem, u16),
    /// LosOS Login is not answering.
    Down,
}

/// `POST /basic/sign-in`: name and password.
pub fn password(
    host: &mut dyn Host,
    fwd: &Forwarded,
    m: &Messages,
    form: &[(String, String)],
) -> Outcome {
    let next = next_of(form);
    let name = web::field(form, "username")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let password = web::field(form, "password").unwrap_or_default();
    if name.is_empty() || password.is_empty() {
        return Outcome::Again(Mode::Password, Problem(m.t("basic.signIn.wrong")), 400);
    }
    let body = json!({
        "username": name,
        "password": password,
        "keepMeLoggedIn": false,
        "targetURL": format!("{}{next}", fwd.origin()),
        "requestMethod": "GET",
    });
    let Some(answer) = host.login("POST", "firstfactor", fwd, None, Some(&body)) else {
        return Outcome::Down;
    };
    if answer.status != 200 {
        return Outcome::Again(Mode::Password, Problem(m.t("basic.signIn.wrong")), 401);
    }
    let Some(session) = session_of(&answer.cookies) else {
        return Outcome::Again(Mode::Password, Problem(m.t("shell.signIn.noAnswer")), 502);
    };
    match session_opens(host, fwd, &session) {
        Some(true) => Outcome::Go(relay(web::see_other(&next), &answer)),
        Some(false) => {
            // A second step: a code this form can take, or a passkey it cannot.
            let info = host.login("GET", "user/info", fwd, Some(&session), None);
            let totp = info
                .as_ref()
                .is_some_and(|i| i.json["data"]["has_totp"] == json!(true));
            let to = if totp {
                format!("/basic/sign-in?step=code&next={next}")
            } else {
                format!("/basic/sign-in?step=passkey&next={next}")
            };
            Outcome::Go(relay(web::see_other(&to), &answer))
        }
        None => Outcome::Down,
    }
}

/// `POST /basic/sign-in/code`: the six digits of the second step.
pub fn code(
    host: &mut dyn Host,
    fwd: &Forwarded,
    m: &Messages,
    session: Option<&str>,
    form: &[(String, String)],
) -> Outcome {
    let next = next_of(form);
    let Some(session) = session else {
        return Outcome::Go(web::see_other(&format!("/basic/sign-in?next={next}")));
    };
    let token: String = web::field(form, "code")
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    if token.len() != 6 {
        return Outcome::Again(Mode::Code, Problem(m.t("basic.code.wrong")), 400);
    }
    let body = json!({ "token": token, "targetURL": format!("{}{next}", fwd.origin()) });
    match host.login("POST", "secondfactor/totp", fwd, Some(session), Some(&body)) {
        None => Outcome::Down,
        Some(a) if a.status == 200 => Outcome::Go(relay(web::see_other(&next), &a)),
        Some(_) => Outcome::Again(Mode::Code, Problem(m.t("basic.code.wrong")), 401),
    }
}

/// An admin key is 64 hex characters (lib/api.ts's TOKEN_PATTERN).
fn well_formed(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `POST /basic/sign-in/key`: the spare admin key.
pub fn key(host: &mut dyn Host, m: &Messages, form: &[(String, String)]) -> Outcome {
    let next = next_of(form);
    let key = web::field(form, "key")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !well_formed(&key) {
        return Outcome::Again(Mode::Key, Problem(m.t("shell.signIn.badShape")), 400);
    }
    if !host.key_opens(&key) {
        return Outcome::Again(Mode::Key, Problem(m.t("shell.signIn.rejected")), 401);
    }
    Outcome::Go(web::see_other(&next).header("set-cookie", &web::spare_cookie(&key)))
}

/// `POST /basic/sign-out`: LosOS Login forgets the session, the browser
/// forgets the key, and the sign-in page comes back.
pub fn sign_out(host: &mut dyn Host, fwd: &Forwarded, session: Option<&str>) -> Response {
    let mut resp = web::see_other("/basic/sign-in")
        .header("set-cookie", &web::spare_gone())
        .header("set-cookie", &web::flash_cookie("signed-out"));
    if let Some(session) = session {
        if let Some(a) = host.login("POST", "logout", fwd, Some(session), Some(&json!({}))) {
            resp = relay(resp, &a);
        }
    }
    resp
}

/// Whether LosOS Login answers at all.
pub fn login_up(host: &mut dyn Host, fwd: &Forwarded) -> bool {
    host.login("GET", "health", fwd, None, None)
        .is_some_and(|a| a.status == 200)
}

/// The page's mode from its address.
#[must_use]
pub fn mode_of(req: &Request) -> Mode {
    match (
        web::query(req, "with").as_deref(),
        web::query(req, "step").as_deref(),
    ) {
        (Some("key"), _) => Mode::Key,
        (_, Some("code")) => Mode::Code,
        (_, Some("passkey")) => Mode::Passkey,
        _ => Mode::Password,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_is_read_off_the_cookie() {
        let cookies = vec![
            "other=1; Path=/".to_string(),
            "losos_session=abc123; Domain=box.local; Path=/; Secure; HttpOnly; SameSite=Lax"
                .to_string(),
        ];
        assert_eq!(session_of(&cookies).as_deref(), Some("abc123"));
        assert_eq!(session_of(&["losos_session=; Max-Age=0".to_string()]), None);
    }

    #[test]
    fn keys_have_one_shape() {
        assert!(well_formed(&"a".repeat(64)));
        assert!(!well_formed(&"g".repeat(64)));
        assert!(!well_formed("abc"));
    }
}
