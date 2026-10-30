//! Settings › Logs, the journal of the box's services, as the log reader
//! (backend/src/logs/, the `losos-logs` binary) draws it for a browser
//! without JavaScript. The script page (screens/settings/pane-logs.tsx)
//! draws the same markup from the same answers, so the two look alike.
//!
//! The reader runs apart from lososd and keeps answering while lososd is
//! down, which is when an owner needs it most; this module only draws.
//! Every line it gets has been through the reader's redaction already.

use crate::html::{el, El, Html};
use crate::i18n::Messages;
use crate::icons;
use crate::ui::{self, Size, Variant};

/// The address of the page under /basic/.
pub const PATH: &str = "/basic/settings/logs";

/// The severities the filter offers, from the most to the least severe:
/// journald's priority, at most this.
pub const PRIORITIES: [u8; 5] = [3, 4, 5, 6, 7];

/// The time spans the filter offers.
pub const SPANS: [&str; 5] = ["boot", "previous", "hour", "day", "week"];

/// One line of the journal, ready to draw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    /// `2026-10-29`, in the box's own time zone.
    pub date: String,
    /// `12:00:01`.
    pub time: String,
    /// RFC 3339, for `<time datetime>`.
    pub stamp: String,
    /// The unit that wrote it, or the program for one outside any unit.
    pub unit: String,
    pub pid: Option<u32>,
    /// journald's priority, 0 (emergency) to 7 (debug).
    pub priority: u8,
    pub message: String,
}

/// What the filter shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    /// A source's id ([`Source::id`]) or a unit's name.
    pub unit: String,
    pub priority: u8,
    pub span: String,
    pub text: String,
    pub follow: bool,
}

impl Filter {
    /// The filter as a query string, without `?`.
    #[must_use]
    pub fn query(&self) -> String {
        let mut q = format!(
            "unit={}&priority={}&span={}",
            encode(&self.unit),
            self.priority,
            encode(&self.span)
        );
        if !self.text.is_empty() {
            q.push_str("&q=");
            q.push_str(&encode(&self.text));
        }
        q
    }
}

/// One choice of the Service list: a group of the box's units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub id: String,
    /// Its name on the list (`logs.source.<id>`, else its units).
    pub label: String,
}

/// Everything the pane draws.
pub struct Pane<'a> {
    pub m: &'a Messages,
    pub filter: &'a Filter,
    pub sources: &'a [Source],
    /// Oldest first, as a terminal shows them.
    pub lines: &'a [Line],
    /// The cursor of the oldest line, when there are older ones.
    pub older: Option<&'a str>,
    /// lososd does not answer: say so, and offer its own log.
    pub lososd_down: bool,
    /// The journal could not be read.
    pub error: Option<&'a str>,
}

/// The tone of a severity's badge (components/ui/badge.tsx).
#[must_use]
pub fn level_tone(priority: u8) -> ui::Tone {
    match priority {
        0..=3 => ui::Tone::Crit,
        4 => ui::Tone::Warn,
        5 => ui::Tone::Local,
        6 => ui::Tone::Neutral,
        _ => ui::Tone::Outline,
    }
}

/// The pane, under its header.
#[must_use]
pub fn pane(p: &Pane<'_>) -> Html {
    let m = p.m;
    let mut out = Html::new();
    if p.lososd_down {
        let see = Filter {
            unit: "lososd".to_string(),
            priority: 6,
            span: "boot".to_string(),
            text: String::new(),
            follow: false,
        };
        out.push(
            ui::alert(
                "crit",
                Some(icons::ALERT_02),
                Some(&m.t("logs.down.title")),
                Html::from(el("p").text(&m.t("logs.down.body"))).with(
                    el("p").child(
                        el("a")
                            .attr("href", &format!("{PATH}?{}", see.query()))
                            .class("font-medium text-accent underline-offset-4 hover:underline")
                            .text(&m.t("logs.down.see")),
                    ),
                ),
            )
            .class("mb-4"),
        );
    }
    out.push(filters(p));
    if let Some(error) = p.error {
        out.push(
            ui::alert(
                "crit",
                Some(icons::ALERT_02),
                None,
                el("p").text(&m.tv("logs.error", &[("message", error.into())])),
            )
            .class("mb-4"),
        );
    }
    out.push(lines(p));
    out
}

/// The filter: four fields and the actions, in one form that asks again.
fn filters(p: &Pane<'_>) -> Html {
    let m = p.m;
    let f = p.filter;
    let mut sources: Vec<(String, String)> = p
        .sources
        .iter()
        .map(|s| (s.id.clone(), s.label.clone()))
        .collect();
    // A unit asked for by name (a problem report's link) is on the list too.
    if !sources.iter().any(|(id, _)| *id == f.unit) {
        sources.push((f.unit.clone(), f.unit.clone()));
    }
    let priorities: Vec<(String, String)> = PRIORITIES
        .iter()
        .map(|n| (n.to_string(), m.t(&format!("logs.priority.{n}"))))
        .collect();
    let spans: Vec<(String, String)> = SPANS
        .iter()
        .map(|s| ((*s).to_string(), m.t(&format!("logs.span.{s}"))))
        .collect();
    let field = |id: &str, label: &str, control: Html| -> Html {
        ui::field()
            .class("min-w-0")
            .child(ui::field_label(id, label))
            .child(control)
            .into()
    };
    let download = format!("{PATH}/download?{}", f.query());
    let follow = if f.follow {
        ui::link_button(
            &format!("{PATH}?{}", f.query()),
            Variant::Secondary,
            Size::Md,
        )
        .attr("aria-pressed", "true")
        .child(ui::icon(icons::PAUSE, 16, ""))
        .child(el("span").text(&m.t("logs.stop")))
    } else {
        ui::button(Variant::Secondary, Size::Md)
            .attr("name", "follow")
            .attr("value", "1")
            .attr("aria-pressed", "false")
            .child(ui::icon(icons::PLAY, 16, ""))
            .child(el("span").text(&m.t("logs.follow")))
    };
    el("form")
        .attr("method", "get")
        .attr("action", PATH)
        .attr("role", "search")
        .attr("data-slot", "log-filters")
        .class("mb-4 grid gap-3 rounded-card border border-line bg-surface p-3.5 shadow-card sm:grid-cols-2 lg:grid-cols-[minmax(0,1.4fr)_minmax(0,1.25fr)_minmax(0,1fr)_minmax(0,0.85fr)]")
        .child(field(
            "logs-unit",
            &m.t("logs.filter.source"),
            ui::native_select("logs-unit", "unit", &sources, &f.unit),
        ))
        .child(field(
            "logs-priority",
            &m.t("logs.filter.priority"),
            ui::native_select("logs-priority", "priority", &priorities, &f.priority.to_string()),
        ))
        .child(field(
            "logs-span",
            &m.t("logs.filter.span"),
            ui::native_select("logs-span", "span", &spans, &f.span),
        ))
        .child(field(
            "logs-q",
            &m.t("logs.filter.text"),
            ui::input("logs-q", "q", &f.text)
                .attr("type", "search")
                .attr("placeholder", &m.t("logs.filter.textPlaceholder"))
                .attr("autocomplete", "off")
                .class("w-full")
                .into(),
        ))
        .child(
            el("div")
                .class("flex flex-wrap items-center gap-2 sm:col-span-full")
                .child(
                    ui::button(Variant::Primary, Size::Md)
                        .child(ui::icon(icons::SEARCH_01, 16, ""))
                        .child(el("span").text(&m.t("logs.filter.show"))),
                )
                .child(follow)
                .child(el("div").class("flex-1"))
                .child(
                    ui::link_button(&download, Variant::Ghost, Size::Md)
                        .attr("download", "")
                        .child(ui::icon(icons::DOWNLOAD_04, 16, ""))
                        .child(el("span").text(&m.t("logs.download"))),
                ),
        )
        .into()
}

/// The lines, oldest at the top, with a day's date where it changes.
fn lines(p: &Pane<'_>) -> Html {
    let m = p.m;
    if p.lines.is_empty() && p.error.is_none() {
        return ui::empty(
            icons::COMPUTER_TERMINAL_01,
            &m.t("logs.empty.title"),
            &m.t("logs.empty.body"),
        )
        .into();
    }
    let mut list = el("ol")
        .attr("data-slot", "log-lines")
        .attr("aria-label", &m.t("settings.panes.logs.label"))
        .class("font-mono text-[12.5px] leading-relaxed");
    let mut day = "";
    for line in p.lines {
        if line.date != day {
            day = &line.date;
            list = list.child(
                el("li")
                    .attr("data-slot", "log-day")
                    .class("sticky top-0 z-[1] border-b border-hair bg-sunk/90 px-3.5 py-1 font-sans text-[12px] font-medium text-muted backdrop-blur-sm")
                    .text(day),
            );
        }
        list = list.child(line_row(m, line));
    }
    let older = p.older.map(|cursor| {
        let mut f = p.filter.clone();
        f.follow = false;
        Html::from(
            ui::link_button(
                &format!("{PATH}?{}&before={}", f.query(), encode(cursor)),
                Variant::Ghost,
                Size::Sm,
            )
            .child(ui::icon(icons::ARROW_UP_01, 16, ""))
            .child(el("span").text(&m.t("logs.older"))),
        )
    });
    el("div")
        .attr("data-slot", "log-view")
        .class(
            "overflow-hidden rounded-card border border-line bg-surface shadow-card animate-rise",
        )
        .child(older.map(|b| {
            Html::from(
                el("div")
                    .class("flex justify-center border-b border-hair px-3.5 py-1.5")
                    .child(b),
            )
        }))
        .child(list)
        .when(p.filter.follow, |v| v.child(following(m)))
        .into()
}

fn line_row(m: &Messages, line: &Line) -> El {
    let who = match line.pid {
        Some(pid) => format!("{}[{pid}]", line.unit),
        None => line.unit.clone(),
    };
    let short = line.unit.strip_suffix(".service").unwrap_or(&line.unit);
    el("li")
        .attr("data-slot", "log-line")
        .attr("data-priority", &line.priority.to_string())
        .class("grid grid-cols-[auto_auto_minmax(0,1fr)] items-baseline gap-x-3 gap-y-0.5 border-b border-hair px-3.5 py-1.5 last:border-b-0 sm:grid-cols-[4.5rem_5.5rem_minmax(0,10rem)_minmax(0,1fr)]")
        .child(
            el("time")
                .attr("datetime", &line.stamp)
                .attr("title", &format!("{} {}", line.date, line.time))
                .class("text-faint tabular-nums")
                .text(&line.time),
        )
        .child(
            el("span").class("leading-none").child(ui::badge(
                level_tone(line.priority),
                &m.t(&format!("logs.level.{}", line.priority.min(7))),
            )),
        )
        .child(
            el("span")
                .attr("title", &who)
                .class("truncate text-muted")
                .text(short),
        )
        .child(
            el("span")
                .class("col-span-full min-w-0 break-words whitespace-pre-wrap sm:col-span-1")
                .class(if line.priority >= 7 {
                    "text-muted"
                } else {
                    "text-ink"
                })
                .text(&line.message),
        )
}

fn following(m: &Messages) -> Html {
    el("p")
        .attr("role", "status")
        .attr("data-slot", "log-following")
        .class("flex items-center gap-2 border-t border-hair px-3.5 py-2 text-[12.5px] text-muted")
        .child(ui::dot("pending"))
        .child(el("span").text(&m.t("logs.following")))
        .into()
}

/// The page a visitor who is not an administrator gets: sign in with
/// LosOS Login (lososd's page, when it answers) or the spare key, which
/// this page checks itself, so it works while lososd is down.
#[must_use]
pub fn sign_in(m: &Messages, next: &str, wrong: bool, login: bool) -> Html {
    let card = ui::card().class("mx-auto mt-6 w-full max-w-md").child(
        el("div")
            .class("flex flex-col gap-1.5 px-5 pt-5 pb-3")
            .child(
                el("h1")
                    .class("text-[17px] leading-tight font-semibold tracking-tight")
                    .text(&m.t("logs.signIn.title")),
            )
            .child(
                el("p")
                    .class("text-[13px] leading-snug text-muted")
                    .text(&m.t("logs.signIn.body")),
            ),
    );
    let login_link = login.then(|| {
        Html::from(
            el("div").class("px-5 pb-3").child(
                ui::link_button(
                    &format!("/basic/sign-in?next={}", encode(next)),
                    Variant::Primary,
                    Size::Md,
                )
                .class("w-full")
                .text(&m.t("logs.signIn.login")),
            ),
        )
    });
    let key = el("form")
        .attr("method", "post")
        .attr("action", &format!("{PATH}/key"))
        .class("flex flex-col gap-3 border-t border-hair px-5 py-4")
        .child(ui::hidden("next", next))
        .child(
            ui::field()
                .child(ui::field_label("logs-key", &m.t("logs.signIn.key")))
                .child(
                    ui::input("logs-key", "key", "")
                        .attr("type", "password")
                        .attr("autocomplete", "off")
                        .attr("spellcheck", "false")
                        .flag("required", true)
                        .attr("aria-invalid", if wrong { "true" } else { "false" })
                        .class("w-full font-mono"),
                )
                .child(wrong.then(|| ui::field_error("logs-key-error", &m.t("logs.signIn.wrong")))),
        )
        .child(
            ui::button(
                if login {
                    Variant::Secondary
                } else {
                    Variant::Primary
                },
                Size::Md,
            )
            .text(&m.t("logs.signIn.keySubmit")),
        );
    card.child(login_link).child(key).into()
}

/// The page a signed-in person who is not an administrator gets.
#[must_use]
pub fn denied(m: &Messages) -> Html {
    ui::empty(
        icons::SQUARE_LOCK_01,
        &m.t("logs.denied.title"),
        &m.t("logs.denied.body"),
    )
    .into()
}

/// Letters, digits and `-._~` as they are, everything else as `%XX`.
#[must_use]
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Locale;

    fn filter() -> Filter {
        Filter {
            unit: "lososd".to_string(),
            priority: 6,
            span: "boot".to_string(),
            text: "a b&c".to_string(),
            follow: false,
        }
    }

    #[test]
    fn the_filter_is_a_query_string() {
        assert_eq!(
            filter().query(),
            "unit=lososd&priority=6&span=boot&q=a%20b%26c"
        );
    }

    #[test]
    fn lines_are_drawn_with_their_day_and_their_text_escaped() {
        let m = Messages::from_pairs(Locale::En, &[("logs.level.3", "error")]);
        let lines = [
            Line {
                date: "2026-10-28".into(),
                time: "23:59:59".into(),
                stamp: "2026-10-28T23:59:59+01:00".into(),
                unit: "lososd.service".into(),
                pid: Some(7),
                priority: 3,
                message: "<script>x</script>".into(),
            },
            Line {
                date: "2026-10-29".into(),
                time: "00:00:01".into(),
                unit: "nginx.service".into(),
                priority: 6,
                ..Line::default()
            },
        ];
        let f = filter();
        let html = pane(&Pane {
            m: &m,
            filter: &f,
            sources: &[],
            lines: &lines,
            older: Some("s=1;i=2"),
            lososd_down: true,
            error: None,
        })
        .into_string();
        assert!(html.contains("&lt;script&gt;x&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
        assert_eq!(html.matches("data-slot=\"log-day\"").count(), 2);
        assert!(html.contains("lososd.service[7]"));
        assert!(html.contains(">lososd<"));
        assert!(html.contains("before=s%3D1%3Bi%3D2"));
        assert!(html.contains("unit=lososd&amp;priority=6&amp;span=boot"));
        assert!(html.contains("role=\"alert\""));
    }

    #[test]
    fn a_unit_asked_for_by_name_is_on_the_list() {
        let m = Messages::from_pairs(Locale::En, &[]);
        let mut f = filter();
        f.unit = "systemd-networkd.service".into();
        let html = pane(&Pane {
            m: &m,
            filter: &f,
            sources: &[Source {
                id: "lososd".into(),
                label: "lososd".into(),
            }],
            lines: &[],
            older: None,
            lososd_down: false,
            error: None,
        })
        .into_string();
        assert!(html.contains("value=\"systemd-networkd.service\" selected>"));
        assert!(html.contains("data-slot=\"empty\""));
    }
}
