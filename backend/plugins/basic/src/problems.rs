//! The Problems pane (screens/settings/pane-problems.tsx) without a script:
//! the list from `GET /api/problems`, a report opened by `?read=<id>` as
//! the script page's dialog, and the three forms (send, stop, dismiss)
//! posted to this page. While a report waits for GitHub the page looks
//! again every three seconds, so the code and then the link appear on
//! their own.

use crate::host::Host;
use crate::view::Ctx;
use crate::web;
use losos_page::html::{el, Html};
use losos_page::i18n::{Messages, Var};
use losos_page::icons;
use losos_page::ui::{self, Size, Tone, Variant};
use losos_plugin::Request;
use serde_json::Value;

/// A problem's id: the fingerprint the problems plugin keys it by.
#[must_use]
pub fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The report open on the page, as the box answered for it.
pub struct Reading {
    pub id: String,
    pub report: Option<Value>,
    pub sending: Value,
}

impl Reading {
    pub fn read(host: &mut dyn Host, id: &str) -> Reading {
        Reading {
            id: id.to_string(),
            report: host.get(&format!("/api/problems/{id}")),
            sending: host
                .get(&format!("/api/problems/{id}/send"))
                .unwrap_or_else(|| serde_json::json!({ "state": "idle" })),
        }
    }

    /// Whether the box is still waiting on GitHub, so the page looks again.
    #[must_use]
    pub fn waiting(&self) -> bool {
        matches!(self.sending["state"].as_str(), Some("starting" | "sign-in"))
    }
}

/// A form posted to the pane. Answers where to go next.
pub fn post(host: &mut dyn Host, req: &Request, here: &str) -> String {
    let form = web::form(&req.body);
    let field = |name: &str| web::field(&form, name).filter(|id| id_ok(id));
    if let Some(id) = field("dismiss") {
        host.call("POST", &format!("/api/problems/{id}/dismiss"), None, &[]);
        return here.to_string();
    }
    if let Some(id) = field("cancel") {
        host.call("POST", &format!("/api/problems/{id}/cancel"), None, &[]);
        return format!("{here}?read={id}");
    }
    if let Some(id) = field("send") {
        let note = web::field(&form, "note").unwrap_or_default();
        let body = serde_json::json!({ "note": note }).to_string();
        host.call(
            "POST",
            &format!("/api/problems/{id}/send"),
            Some("application/json"),
            body.as_bytes(),
        );
        return format!("{here}?read={id}");
    }
    here.to_string()
}

fn when(secs: u64) -> String {
    losos_report::problem::utc(secs)
}

/// problemSentence: what happened, in the reader's language.
#[must_use]
pub fn sentence(m: &Messages, p: &Value) -> String {
    let subject = p["unit"]
        .as_str()
        .or_else(|| p["process"].as_str())
        .unwrap_or("?");
    match p["kind"].as_str() {
        Some("crashed") => m.tv(
            "problems.kind.crashed",
            &[
                ("subject", Var::from(subject)),
                (
                    "signal",
                    Var::from(format!("SIG{}", p["signal"].as_str().unwrap_or("?"))),
                ),
            ],
        ),
        Some("out-of-memory") => m.tv(
            "problems.kind.outOfMemory",
            &[("subject", Var::from(subject))],
        ),
        _ => m.tv(
            "problems.kind.failed",
            &[
                ("subject", Var::from(subject)),
                (
                    "result",
                    Var::from(p["result"].as_str().unwrap_or("failed")),
                ),
            ],
        ),
    }
}

fn row(ctx: &Ctx, p: &Value, can_send: bool) -> Html {
    let m = &ctx.m;
    let id = p["id"].as_str().unwrap_or_default();
    let count = p["count"].as_u64().unwrap_or(1);
    let first = p["first"].as_u64().unwrap_or(0);
    let last = p["last"].as_u64().unwrap_or(0);
    let times = if count > 1 {
        m.tv(
            "problems.times",
            &[
                ("count", Var::from(count)),
                ("first", Var::from(when(first))),
                ("last", Var::from(when(last))),
            ],
        )
    } else {
        when(last)
    };
    let dismissed = p["dismissed"] == true;
    let documented = p["documented"].is_object().then_some(&p["documented"]);
    let action: Html = match documented {
        Some(d) => ui::link_button(
            &format!("/handbook/{}/", d["page"].as_str().unwrap_or_default()),
            Variant::Secondary,
            Size::Sm,
        )
        .attr("target", "_blank")
        .attr("rel", "noopener noreferrer")
        .attr("data-testid", "problem-documented")
        .text(&m.t("problems.whatToDo"))
        .into(),
        None => ui::link_button(
            &format!("{}?read={id}", ctx.here),
            Variant::Secondary,
            Size::Sm,
        )
        .attr("data-testid", "problem-read")
        .text(&m.t(if can_send && p["sent"].is_null() {
            "problems.readAndSend"
        } else {
            "problems.read"
        }))
        .into(),
    };
    let dismiss: Option<Html> = (!dismissed).then(|| {
        el("form")
            .attr("method", "post")
            .attr("action", &ctx.here)
            .child(
                ui::button(Variant::Ghost, Size::Sm)
                    .attr("type", "submit")
                    .attr("name", "dismiss")
                    .attr("value", id)
                    .text(&m.t("problems.dismiss")),
            )
            .into()
    });
    let (tone, badge) = match p["kind"].as_str() {
        Some("crashed") => (Tone::Crit, "problems.badge.crashed"),
        Some("out-of-memory") => (Tone::Crit, "problems.badge.outOfMemory"),
        _ => (Tone::Warn, "problems.badge.failed"),
    };
    let lang = m.locale.code();
    let documented_badge = documented.map(|d| {
        let title = d["title"][lang]
            .as_str()
            .or_else(|| d["title"]["en"].as_str())
            .unwrap_or_default();
        ui::badge(
            Tone::Ok,
            &m.tv("problems.badge.documented", &[("title", Var::from(title))]),
        )
    });
    let sent: Option<Html> = p["sent"]["url"].as_str().map(|url| {
        el("a")
            .attr("href", url)
            .attr("target", "_blank")
            .attr("rel", "noopener noreferrer")
            .class("text-accent hover:underline")
            .text(&m.t(if p["sent"]["existing"] == true {
                "problems.sentExisting"
            } else {
                "problems.sent"
            }))
            .into()
    });
    ui::stack_row()
        .attr("data-testid", "problem")
        .attr("data-id", id)
        .attr("data-kind", p["kind"].as_str().unwrap_or_default())
        .class("gap-1.5")
        .child(
            el("div")
                .class("flex items-start gap-3")
                .child(
                    el("div")
                        .class("min-w-0 flex-1")
                        .child(
                            el("p")
                                .class("text-sm leading-snug break-words text-ink")
                                .text(&sentence(m, p)),
                        )
                        .child(
                            el("p")
                                .class("text-[12.5px] leading-snug text-muted")
                                .text(&times),
                        ),
                )
                .child(
                    el("div")
                        .class("flex shrink-0 items-center gap-1.5")
                        .child(action)
                        .child(dismiss),
                ),
        )
        .child(
            el("div")
                .class("flex flex-wrap items-center gap-1.5 text-[12.5px] leading-snug text-muted")
                .child(ui::badge(tone, &m.t(badge)))
                .child(documented_badge)
                .child(sent),
        )
        .into()
}

fn sending_state(m: &Messages, sending: &Value) -> Html {
    match sending["state"].as_str() {
        Some("starting") => el("p")
            .class("flex items-center gap-2 text-sm text-muted")
            .attr("role", "status")
            .child(ui::spinner(14, "", ""))
            .text(&m.t("problems.send.starting"))
            .into(),
        Some("sign-in") => {
            let uri = sending["uri"].as_str().unwrap_or_default();
            el("div")
                .class("flex flex-col items-start gap-2 rounded-control border border-line bg-surface p-3")
                .attr("role", "status")
                .attr("data-testid", "problem-sign-in")
                .child(
                    el("p")
                        .class("text-sm text-ink")
                        .text(&m.t("problems.send.enterCode")),
                )
                .child(
                    el("p")
                        .class("font-mono text-2xl font-medium tracking-[0.2em] text-ink select-all")
                        .attr("data-testid", "problem-code")
                        .text(sending["code"].as_str().unwrap_or_default()),
                )
                .child(
                    ui::link_button(uri, Variant::Secondary, Size::Sm)
                        .attr("target", "_blank")
                        .attr("rel", "noopener noreferrer")
                        .text(&m.tv(
                            "problems.send.open",
                            &[(
                                "address",
                                Var::from(
                                    uri.trim_start_matches("https://")
                                        .trim_start_matches("http://"),
                                ),
                            )],
                        )),
                )
                .child(
                    el("p")
                        .class("flex items-center gap-2 text-[12.5px] text-muted")
                        .child(ui::spinner(12, "", ""))
                        .text(&m.t("problems.send.waiting")),
                )
                .into()
        }
        Some("sent") => {
            let url = sending["url"].as_str().unwrap_or_default();
            el("p")
                .class("text-sm text-ok")
                .attr("role", "status")
                .attr("data-testid", "problem-sent")
                .text(&m.t(if sending["existing"] == true {
                    "problems.send.addedTo"
                } else {
                    "problems.send.opened"
                }))
                .text(" ")
                .child(
                    el("a")
                        .attr("href", url)
                        .attr("target", "_blank")
                        .attr("rel", "noopener noreferrer")
                        .class("font-medium underline")
                        .text(url.trim_start_matches("https://")),
                )
                .into()
        }
        Some("cancelled") => el("p")
            .class("text-sm text-muted")
            .text(&m.t("problems.send.cancelled"))
            .into(),
        Some("failed") => el("p")
            .class("text-sm text-crit")
            .attr("role", "alert")
            .text(sending["message"].as_str().unwrap_or_default())
            .into(),
        _ => Html::new(),
    }
}

fn dialog(ctx: &Ctx, reading: &Reading, repository: Option<&str>) -> Html {
    let m = &ctx.m;
    let id = &reading.id;
    let can_send = repository.is_some();
    let state = reading.sending["state"].as_str().unwrap_or("idle");
    let busy = reading.waiting();
    let title = reading
        .report
        .as_ref()
        .and_then(|r| r["title"].as_str())
        .map_or_else(|| m.t("problems.report.title"), str::to_string);
    let description = if can_send {
        m.tv(
            "problems.report.public",
            &[("repository", Var::from(repository.unwrap_or_default()))],
        )
    } else {
        m.t("problems.report.localOnly")
    };
    let form_id = "f-problem-send";
    let mut body = ui::dialog_body().class("flex flex-col gap-3");
    match &reading.report {
        None => {
            body = body.child(
                el("p")
                    .class("py-4 text-sm text-crit")
                    .attr("role", "alert")
                    .text(&m.t("problems.report.failed")),
            );
        }
        Some(report) => {
            body = body.child(
                el("pre")
                    .class("max-h-[45vh] overflow-auto rounded-control border border-line bg-sunk p-3 font-mono text-[12px] leading-snug whitespace-pre-wrap break-words text-ink")
                    .attr("data-testid", "problem-report-body")
                    .attr("tabindex", "0")
                    .text(report["body"].as_str().unwrap_or_default()),
            );
            if can_send && state == "idle" && report["problem"]["sent"].is_null() {
                body = body.child(
                    el("label")
                        .attr("for", "f-problem-note")
                        .class("flex flex-col gap-1.5 text-[12.5px] text-muted")
                        .text(&m.t("problems.report.note"))
                        .child(
                            el("textarea")
                                .id("f-problem-note")
                                .attr("name", "note")
                                .attr("form", form_id)
                                .attr("maxlength", "4000")
                                .attr("placeholder", &m.t("problems.report.notePlaceholder"))
                                .attr("data-slot", "textarea")
                                .attr("data-testid", "problem-note")
                                .class("min-h-[4.5rem] w-full min-w-0 resize-y rounded-control border border-line bg-surface px-3 py-2 text-sm leading-snug text-ink placeholder:text-faint focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/35"),
                        ),
                );
            }
            body = body.child(sending_state(m, &reading.sending));
        }
    }
    let mut footer = ui::dialog_footer();
    footer = if busy {
        footer.child(
            el("form")
                .attr("method", "post")
                .attr("action", &ctx.here)
                .child(
                    ui::button(Variant::Ghost, Size::Md)
                        .attr("type", "submit")
                        .attr("name", "cancel")
                        .attr("value", id)
                        .text(&m.t("problems.report.stop")),
                ),
        )
    } else {
        footer.child(
            ui::link_button(&ctx.here, Variant::Ghost, Size::Md)
                .text(&m.t("problems.report.close")),
        )
    };
    if can_send && reading.report.is_some() && !busy && state != "sent" {
        footer = footer.child(
            el("form")
                .id(form_id)
                .attr("method", "post")
                .attr("action", &ctx.here)
                .child(
                    ui::button(Variant::Primary, Size::Md)
                        .attr("type", "submit")
                        .attr("name", "send")
                        .attr("value", id)
                        .attr("data-testid", "problem-send")
                        .text(&m.t(if state == "idle" {
                            "problems.report.send"
                        } else {
                            "problems.report.sendAgain"
                        })),
                ),
        );
    }
    let mut inner = ui::dialog_header("problem-title", &title, &description);
    inner.push(body);
    inner.push(footer);
    ui::dialog("w-[min(46rem,calc(100vw-2rem))]", "problem-title", inner)
}

/// The pane: `list` is `GET /api/problems`, `reading` the report open.
#[must_use]
pub fn pane(ctx: &Ctx, list: Option<&Value>, reading: Option<&Reading>) -> Html {
    let m = &ctx.m;
    let header = ui::pane_header(
        &m.t("settings.panes.problems.label"),
        &m.t("settings.panes.problems.summary"),
    );
    let Some(list) = list else {
        return el("div")
            .child(header)
            .child(
                ui::pane_section().child(ui::group().child(ui::row().child(ui::row_text(
                    &m.t("problems.unavailable"),
                    None,
                    None,
                )))),
            )
            .into();
    };
    let repository = list["reporting"]["repository"].as_str();
    let can_send = repository.is_some();
    let problems: &[Value] = list["problems"].as_array().map_or(&[], Vec::as_slice);
    let open: Vec<&Value> = problems.iter().filter(|p| p["dismissed"] != true).collect();
    let gone: Vec<&Value> = problems.iter().filter(|p| p["dismissed"] == true).collect();

    let reporting = ui::pane_section()
        .child(ui::group_title(&m.t("problems.reports.title")))
        .child(
            ui::group().child(
                ui::row()
                    .child(ui::row_text(
                        &match repository {
                            Some(r) => m.tv("problems.reports.on", &[("repository", Var::from(r))]),
                            None => m.t("problems.reports.off"),
                        },
                        Some(&m.t(if can_send {
                            "problems.reports.onDetail"
                        } else {
                            "problems.reports.offDetail"
                        })),
                        None,
                    ))
                    .child(ui::badge(
                        Tone::Outline,
                        &m.t(if can_send {
                            "problems.badge.askFirst"
                        } else {
                            "problems.badge.localOnly"
                        }),
                    )),
            ),
        )
        .child(ui::group_caption(
            Html::text(&m.t("problems.reports.caption")).with(" ").with(
                el("a")
                    .attr("href", "/handbook/manual/problem-reports/")
                    .attr("target", "_blank")
                    .attr("rel", "noopener noreferrer")
                    .class("text-accent hover:underline")
                    .text(&m.t("problems.reports.more")),
            ),
        ));

    let mut group = ui::group();
    if open.is_empty() {
        group = group.child(ui::row().child(ui::row_text(
            &m.t("problems.none"),
            Some(&m.t("problems.noneDetail")),
            None,
        )));
    }
    for p in &open {
        group = group.child(row(ctx, p, can_send));
    }
    let listed = ui::pane_section()
        .child(ui::group_title(&m.t("problems.list.title")))
        .child(group);

    let dismissed: Option<Html> = (!gone.is_empty()).then(|| {
        let mut g = ui::group();
        for p in &gone {
            g = g.child(row(ctx, p, can_send));
        }
        ui::pane_section()
            .child(ui::group_title(&m.t("problems.dismissed.title")))
            .child(g)
            .child(ui::group_caption(Html::text(
                &m.t("problems.dismissed.caption"),
            )))
            .into()
    });

    el("div")
        .attr("data-basic-problems", "")
        .child(header)
        .child(
            el("div")
                .class("animate-fade-in")
                .child(reporting)
                .child(listed)
                .child(dismissed),
        )
        .child(reading.map(|r| dialog(ctx, r, repository)))
        .into()
}

#[must_use]
pub fn glyph() -> &'static str {
    icons::ALERT_02
}
