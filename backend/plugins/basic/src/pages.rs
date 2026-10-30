//! The pages a signed-in person sees, each drawn in the script page's
//! markup and classes: the overview (screens/Home.tsx), the Storage pane,
//! the panes of switches and the name (screens/settings/pane-*.tsx), the
//! search, and a page that says what is not here yet.

use crate::format;
use crate::host::Host;
use crate::settings::{self, Mine, Status, Watch};
use crate::view::{self, Ctx};
use losos_page::html::{el, Html};
use losos_page::i18n::{Messages, Var};
use losos_page::icons;
use losos_page::shell;
use losos_page::ui::{self, Size, Toast, Variant};
use serde_json::Value;

/// The page around `main`: the mesh notice above it, the toasts over it.
#[must_use]
pub fn frame(ctx: &Ctx, title: &str, notice: Html, toasts: &[Toast], main: Html) -> Html {
    let m = &ctx.m;
    let mut body = notice;
    body.push(main);
    body.push(ui::toasts(
        &m.t("ui.dismiss"),
        &m.t("ui.confirmations"),
        toasts,
    ));
    shell::page(&ctx.frame(title), body)
}

/// components/mesh-notice.tsx: shown while the box is off a mesh or not
/// sharing, unless put away.
pub fn mesh_notice(ctx: &Ctx, settings: Option<&Value>) -> Html {
    let Some(s) = settings else {
        return Html::new();
    };
    if ctx.prefs.is_off("mesh") || ctx.me.locked {
        return Html::new();
    }
    let not_joined = !s["clusterEnable"].as_bool().unwrap_or(false);
    if !not_joined && s["sharingMyStorage"].as_bool().unwrap_or(false) {
        return Html::new();
    }
    let pane = if not_joined { "mesh" } else { "market" };
    let can = ctx.me.pane_open(pane);
    let to = shell::href(&view::pane_path(pane));
    let link = (can && ctx.here != to).then_some(to.as_str());
    shell::mesh_notice(&ctx.frame(""), not_joined, link, can)
}

// ── The overview ────────────────────────────────────────────────────────

/// What the overview reads off the box.
pub struct Overview {
    pub host_name: String,
    pub settings: Option<Value>,
    pub storage: Option<Value>,
    pub status: Status,
    pub files: bool,
    pub code: bool,
}

impl Overview {
    pub fn read(host: &mut dyn Host, browser_host: &str, m: &Messages) -> Overview {
        let settings = host.get("/api/settings");
        let storage = host.get("/api/storage");
        let status = host
            .get("/api/status")
            .map(|v| Status::from_json(&v))
            .unwrap_or_default();
        let (files, code) = host.apps_running();
        let host_name = settings
            .as_ref()
            .and_then(|s| s["hostName"].as_str())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .or_else(|| {
                let name = browser_host.split(':').next().unwrap_or_default();
                (!name.is_empty()).then(|| name.to_string())
            })
            .unwrap_or_else(|| m.t("home.header.thisBox"));
        Overview {
            host_name,
            settings,
            storage,
            status,
            files,
            code,
        }
    }
}

const STAGGER: [&str; 10] = [
    "[animation-delay:0ms]",
    "[animation-delay:35ms]",
    "[animation-delay:70ms]",
    "[animation-delay:105ms]",
    "[animation-delay:140ms]",
    "[animation-delay:175ms]",
    "[animation-delay:210ms]",
    "[animation-delay:245ms]",
    "[animation-delay:280ms]",
    "[animation-delay:315ms]",
];

fn stagger(n: usize) -> &'static str {
    STAGGER[n.min(STAGGER.len() - 1)]
}

/// Bytes as numbers, when the box measured them.
fn bytes(v: &Value, key: &str) -> Option<u64> {
    v[key].as_u64()
}

/// One tile: its app, icon, address, name and the note under it.
type Tile<'a> = (&'a str, &'a str, &'a str, String, Option<(&'a str, String)>);

/// The Overview, or the administrator's page in its place when they changed
/// it (`custom`, crate::custom): its blocks are the header, the change being
/// applied and the card of apps. Glance, the widgets, needs a script.
pub fn overview(ctx: &Ctx, o: &Overview, custom: Option<&str>) -> Html {
    let m = &ctx.m;
    let header = el("header")
        .class("flex flex-wrap items-center gap-x-4 gap-y-2 animate-fade-in")
        .child(
            el("div")
                .class("min-w-0 flex-1")
                .child(el("h1").class("numeric truncate text-[19px] leading-tight font-semibold text-ink").text(&o.host_name))
                .child(
                    el("p").class("mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[12.5px] text-muted").child(
                        el("span")
                            .class("inline-flex items-center gap-1.5")
                            .child(ui::dot("ok"))
                            .text(&m.t("home.header.answering")),
                    ),
                ),
        );

    // The tiles (lib/apps.ts's CATALOGUE) with what the box reported.
    let usage = o.storage.as_ref().and_then(|s| {
        Some((
            bytes(s, "usedBytes")?,
            bytes(s, "totalBytes").filter(|t| *t > 0)?,
        ))
    });
    let files_note = usage.and_then(|(used, total)| {
        let ratio = used as f64 / total as f64;
        if ratio >= 0.95 {
            Some(("crit", m.t("home.attention.outOfRoom")))
        } else if ratio >= 0.9 {
            Some(("warn", m.t("home.attention.almostFull")))
        } else {
            None
        }
    });
    let settings_note = match o.status.state.as_str() {
        "building" => Some(("pending", m.t("home.applying.short"))),
        "failed" => Some(("crit", m.t("home.attention.changeFailed"))),
        _ => None,
    };
    let mut tiles: Vec<Tile<'_>> = Vec::new();
    if o.files {
        tiles.push((
            "files",
            icons::FOLDER_01,
            "/cloud/files",
            m.t("apps.files.name"),
            files_note,
        ));
        tiles.push((
            "photos",
            icons::IMAGE_02,
            "/cloud/photos",
            m.t("apps.photos.name"),
            None,
        ));
    }
    if o.code {
        tiles.push((
            "code",
            icons::GIT_BRANCH,
            "/git/",
            m.t("apps.code.name"),
            None,
        ));
        tiles.push((
            "issues",
            icons::TASK_DONE_01,
            "/git/issues",
            m.t("apps.issues.name"),
            None,
        ));
    }
    let settings_href = shell::href("/settings");
    tiles.push((
        "settings",
        icons::SETTINGS_01,
        settings_href.as_str(),
        m.t("apps.settings.name"),
        settings_note,
    ));

    let grid = el("ul")
        .attr("aria-label", &m.t("home.grid.label"))
        .class("grid list-none grid-cols-[repeat(auto-fill,minmax(78px,1fr))] m-0 gap-x-1 gap-y-4 p-0")
        .children(tiles.iter().enumerate().map(|(n, (id, glyph, href, name, note))| {
            let mut mark = el("span")
                .class("relative inline-flex")
                .child(
                    el("span")
                        .class("flex size-[50px] items-center justify-center rounded-card border border-line bg-sunk text-muted transition-[color,background-color,border-color] duration-150 group-hover:border-accent/45 group-hover:bg-accent-wash group-hover:text-accent group-focus-visible:border-accent/45 group-focus-visible:text-accent group-active:translate-y-px")
                        .child(ui::icon(glyph, 24, "")),
                );
            if let Some((level, _)) = note {
                mark = mark.child(
                    el("span")
                        .attr("aria-hidden", "true")
                        .class("inline-block shrink-0 rounded-full absolute -top-1 -right-1 size-2.5 ring-2 ring-surface")
                        .class(match *level {
                            "crit" => "bg-crit",
                            "warn" => "bg-warn",
                            _ => "bg-faint animate-breathe",
                        }),
                );
            }
            el("li")
                .attr("data-app", id)
                .class("group/tile relative animate-rise")
                .class(stagger(n))
                .child(
                    el("a")
                        .attr("href", href)
                        .attr("title", &m.t(&format!("apps.{id}.note")))
                        .class("group flex flex-col items-center gap-2 rounded-card px-1 py-2 no-underline outline-offset-4")
                        .child(mark)
                        .child(el("span").class("block w-full truncate text-center text-[12.5px] leading-tight font-medium text-ink").text(name))
                        .child(note.as_ref().map(|(level, text)| {
                            Html::from(
                                el("span")
                                    .class("block w-full truncate text-center text-[11px] leading-tight")
                                    .class(match *level {
                                        "crit" => "text-crit",
                                        "warn" => "text-warn",
                                        _ => "text-muted",
                                    })
                                    .text(text),
                            )
                        })),
                )
        }));

    let mut apps = el("div").class("flex flex-col gap-4").child(grid);
    if !o.files {
        apps = apps.child(setup_prompt(m));
    }

    let sharing = o
        .settings
        .as_ref()
        .is_some_and(|s| s["sharingMyStorage"].as_bool() == Some(true));
    let joined = o
        .settings
        .as_ref()
        .is_some_and(|s| s["clusterEnable"].as_bool() == Some(true));
    let lending = match (sharing, joined) {
        (false, _) => "off",
        (true, false) => "waiting",
        (true, true) => "lent",
    };
    let apps = Html::from(apps);
    let card = |hide_storage: bool| -> Html {
        let card = ui::card()
            .class("overflow-hidden")
            .child(ui::card_content().class("pt-5").child(apps.clone()));
        if hide_storage {
            return card.into();
        }
        card.child(ui::card_footer().class("bg-sunk/40").child(storage_strip(
            m,
            lending,
            o.storage.as_ref(),
        )))
        .into()
    };
    let header = Html::from(header);
    let applying = if o.status.building() {
        applying(m, &o.status)
    } else {
        Html::new()
    };

    let body = match custom {
        None => header.with(applying).with(card(false)),
        Some(html) => crate::custom::splice(html, |name, props| match name {
            "BoxName" => header.clone(),
            "Applying" => applying.clone(),
            "Apps" => card(props.get("hideStorage") == Some(&Value::Bool(true))),
            _ => Html::new(),
        }),
    };
    el("div").class("flex flex-col gap-4").child(body).into()
}

/// AppGrid.tsx's SetupPrompt. The first-run wizard is a script page, so
/// this one says so on the page it opens.
fn setup_prompt(m: &Messages) -> Html {
    el("div")
        .class("flex flex-col items-start gap-3 rounded-card border border-dashed border-line bg-sunk/60 px-4 py-4 sm:flex-row sm:items-center sm:gap-4 animate-rise [animation-delay:70ms]")
        .child(
            el("span")
                .class("flex size-10 shrink-0 items-center justify-center rounded-card bg-accent-wash text-accent")
                .child(ui::icon(icons::ROCKET_01, 21, "")),
        )
        .child(
            el("div")
                .class("min-w-0 flex-1")
                .child(el("p").class("text-[13.5px] font-medium text-ink").text(&m.t("home.setup.title")))
                .child(el("p").class("text-[12.5px] text-muted").text(&m.t("home.setup.body"))),
        )
        .child(
            ui::link_button(&shell::href("/setup"), Variant::Primary, Size::Sm)
                .class("shrink-0 no-underline")
                .text(&m.t("home.setup.action")),
        )
        .into()
}

/// Home.tsx's ApplyingChange.
fn applying(m: &Messages, status: &Status) -> Html {
    el("div")
        .attr("data-slot", "alert")
        .attr("data-variant", "default")
        .attr("role", "status")
        .class("relative grid w-full grid-cols-[0_1fr] items-start gap-y-0.5 rounded-control border px-3.5 py-3 text-[13px] leading-snug text-ink animate-rise border-line bg-surface shadow-card")
        .child(
            el("p")
                .attr("data-slot", "alert-title")
                .class("col-start-2 min-h-4 font-medium tracking-tight text-ink flex items-center gap-2")
                .child(ui::dot("pending"))
                .child(el("span").class("flex-1").text(&m.t("home.applying.title"))),
        )
        .child(
            el("div")
                .attr("data-slot", "alert-description")
                .class("col-start-2 grid min-w-0 gap-1 text-[13px] text-muted [&_p]:leading-snug [&>*]:min-w-0")
                .when(!status.message.is_empty(), |d| d.child(el("p").class("w-full truncate text-[12px] text-muted").text(&status.message)))
                .child(ui::progress_sweep(&m.t("home.applying.short"), "mt-1.5")),
        )
        .into()
}

/// Home.tsx's StorageStrip. The widths are classes (`w-[N%]`): a page
/// with no script has no other way to set them under the policy.
fn storage_strip(m: &Messages, lending: &str, storage: Option<&Value>) -> Html {
    let measured = storage.and_then(|s| {
        let total = bytes(s, "totalBytes").filter(|t| *t > 0)?;
        let used = bytes(s, "usedBytes")?;
        let lent = if lending == "lent" {
            bytes(s, "lentBytes")
        } else {
            None
        };
        Some((used, lent, total))
    });
    let mut track = el("div")
        .attr("aria-hidden", "true")
        .class("flex h-2.5 w-full overflow-hidden rounded-full bg-sunk");
    let mut legend = el("div").class("flex flex-wrap items-center gap-x-4 gap-y-1 text-[12px]");
    let key = |swatch: &str, name: String, value: Option<String>| -> Html {
        el("span")
            .class("inline-flex items-center gap-1.5 text-muted")
            .child(
                el("span")
                    .attr("aria-hidden", "true")
                    .class("inline-block size-2.5 shrink-0 rounded-[3px]")
                    .class(swatch),
            )
            .text(&name)
            .child(value.map(|v| Html::from(el("span").class("numeric text-faint").text(&v))))
            .into()
    };
    match measured {
        Some((used, lent, total)) => {
            let used_pct = format::percent(used, total);
            let lent_pct = lent.map_or(0, |l| format::percent(l, total).min(100 - used_pct));
            track = track.child(
                el("div")
                    .attr("data-width", "used")
                    .class("h-full bg-accent")
                    .class(&format!("w-[{used_pct}%]")),
            );
            if lending == "lent" {
                track = track.child(
                    el("div")
                        .class("hatched-solid h-full")
                        .class(&format!("w-[{lent_pct}%]")),
                );
            }
            legend = legend.child(key(
                "bg-accent",
                m.t("home.storage.onThisBox"),
                Some(format::bytes_1000(m, used)),
            ));
            if lending == "lent" {
                legend = legend.child(key(
                    "hatched-solid",
                    m.t("home.storage.lent"),
                    lent.map(|l| format::bytes_1000(m, l)),
                ));
            }
            let free = total.saturating_sub(used).saturating_sub(lent.unwrap_or(0));
            if free > 0 {
                legend = legend.child(key(
                    "bg-sunk ring-1 ring-line ring-inset",
                    m.t("home.storage.roomLeft"),
                    Some(format::bytes_1000(m, free)),
                ));
            }
        }
        None => {
            track = track.child(el("div").class("h-full bg-accent flex-1"));
            if lending == "lent" {
                track = track.child(el("div").class("hatched-solid h-full w-14 shrink-0"));
            }
            legend = legend.child(key("bg-accent", m.t("home.storage.onThisBox"), None));
            if lending == "lent" {
                legend = legend.child(key("hatched-solid", m.t("home.storage.lent"), None));
            }
        }
    }
    let line = match lending {
        "lent" => "home.storage.sharing",
        "waiting" => "home.storage.waiting",
        _ => "home.storage.local",
    };
    el("div")
        .class("flex w-full min-w-0 flex-col gap-2")
        .child(track)
        .child(legend)
        .child(el("p").class("text-[12px] text-faint").text(&m.t(line)))
        .into()
}

// ── Settings panes ──────────────────────────────────────────────────────

/// Where the pane's rebuild stands, for its bar and its toast.
pub struct Rebuild {
    pub watch: Watch,
}

impl Rebuild {
    #[must_use]
    pub fn busy(&self) -> bool {
        matches!(self.watch, Watch::Building { .. } | Watch::Starting)
    }

    /// The toast of a rebuild of ours that ended.
    #[must_use]
    pub fn toast(&self, m: &Messages) -> Option<Toast> {
        match &self.watch {
            Watch::Ended {
                done: true,
                message,
            } => Some(Toast {
                tone: "success",
                title: m.t("settings.form.doneTitle"),
                body: if message.is_empty() {
                    m.t("settings.form.doneMessage")
                } else {
                    message.clone()
                },
            }),
            Watch::Ended {
                done: false,
                message,
            } => Some(Toast {
                tone: "error",
                title: m.t("settings.form.failedTitle"),
                body: if message.is_empty() {
                    m.t("settings.form.failedMessage")
                } else {
                    message.clone()
                },
            }),
            _ => None,
        }
    }
}

const BAR: &str = "sticky bottom-[calc(1rem+var(--bottom-nav))] z-30 mt-5 flex rounded-card border border-line bg-surface/95 p-3.5 shadow-pop backdrop-blur-sm";

/// apply-bar.tsx while a rebuild runs: the spinner, the sweeping band and
/// lososd's last line. The page asks again every few seconds.
fn rebuild_bar(m: &Messages, watch: &Watch) -> Html {
    let (title, message) = match watch {
        Watch::Building {
            ours: true,
            message,
        } => (m.t("settings.form.applyingYours"), message.clone()),
        Watch::Building {
            ours: false,
            message,
        } => (m.t("settings.form.joinedTitle"), message.clone()),
        _ => (
            m.t("settings.form.applyingYours"),
            m.t("settings.form.starting"),
        ),
    };
    el("div")
        .attr("data-basic-rebuild", "")
        .class(BAR)
        .class("animate-rise flex-col items-stretch gap-2.5")
        .child(
            el("div")
                .class("flex items-center gap-2.5")
                .child(ui::spinner(
                    16,
                    &m.t("settings.applyBar.spinner"),
                    "text-accent",
                ))
                .child(
                    el("p")
                        .class("flex-1 text-[13.5px] font-medium")
                        .text(&title),
                ),
        )
        .child(ui::progress_sweep(&m.t("settings.form.applyingYours"), ""))
        .child((!message.is_empty()).then(|| ui::log_view(&message)))
        .into()
}

/// apply-bar.tsx with changes waiting. It is in the page from the start
/// and shows once a field differs from what the box has; the count is a
/// CSS counter over those fields (basic.css, and counts.css for the words).
fn apply_bar(m: &Messages) -> Html {
    el("div")
        .attr("data-basic-apply", "")
        .class(BAR)
        .class("animate-rise items-center gap-3")
        .child(
            el("div")
                .class("min-w-0 flex-1")
                .child(
                    el("p")
                        .attr("data-basic-count", "")
                        .attr("aria-live", "polite")
                        .class("text-[13.5px] leading-snug font-medium"),
                )
                .child(
                    el("p")
                        .class("mt-0.5 text-[12.5px] leading-snug text-muted")
                        .text(&m.t("settings.applyBar.valid")),
                ),
        )
        .child(
            ui::button(Variant::Ghost, Size::Sm)
                .attr("type", "reset")
                .text(&m.t("settings.applyBar.discard")),
        )
        .child(
            ui::button(Variant::Primary, Size::Sm)
                .attr("name", "apply")
                .attr("value", "1")
                .text(&m.t("settings.applyBar.apply")),
        )
        .into()
}

/// Settings.tsx's column: the pane's header, its body, the Apply bar.
fn pane_column(ctx: &Ctx, pane: &str, body: Html, bar: Html) -> Html {
    let m = &ctx.m;
    el("div")
        .class("flex gap-4 max-md:flex-col md:gap-5")
        .child(
            el("div")
                .class("min-w-0 flex-1")
                .child(ui::pane_header(
                    &m.t(&format!("settings.panes.{pane}.label")),
                    &m.t(&format!("settings.panes.{pane}.summary")),
                ))
                .child(el("div").class("animate-fade-in").child(body))
                .child(bar),
        )
        .into()
}

/// A pane whose fields are settings: one form, posted to this page, with
/// the Apply bar at its foot, and `after` it what has forms of its own.
fn settings_form(ctx: &Ctx, pane: &str, rebuild: &Rebuild, fields: Html, after: Html) -> Html {
    let m = &ctx.m;
    if rebuild.busy() {
        let mut body = fields;
        body.push(after);
        return pane_column(ctx, pane, body, rebuild_bar(m, &rebuild.watch));
    }
    let form = el("form")
        .attr("method", "post")
        .attr("action", &ctx.here)
        .attr("data-basic-form", "")
        .attr("novalidate", "")
        .child(fields)
        .child(apply_bar(m));
    let mut body = Html::from(form);
    body.push(after);
    pane_column(ctx, pane, body, Html::new())
}

/// A switch row and the value it showed, so only a change is sent.
fn switch(id: &'static str, settings: &Value, title: &str, detail: &str, disabled: bool) -> Html {
    let on = settings[id].as_bool().unwrap_or(false);
    let mut h = ui::hidden(&format!("was.{id}"), if on { "on" } else { "off" });
    h.push(ui::switch_row(
        &format!("f-{id}"),
        id,
        title,
        (!detail.is_empty()).then_some(detail),
        on,
        disabled,
    ));
    h
}

pub fn hardware(ctx: &Ctx, settings: &Value, rebuild: &Rebuild) -> Html {
    let m = &ctx.m;
    let off = rebuild.busy();
    let fields = ui::pane_section()
        .child(ui::group_title(&m.t("panes.hardware.graphics")))
        .child(ui::group().child(switch(
            "gpuEnable",
            settings,
            &m.t("panes.hardware.gpu.title"),
            &m.t("panes.hardware.gpu.detail"),
            off,
        )))
        .child(ui::group_caption(Html::text(
            &m.t("panes.hardware.caption"),
        )));
    settings_form(ctx, "hardware", rebuild, fields.into(), Html::new())
}

pub const HARDWARE: &[&str] = &["gpuEnable"];
pub const SECURITY: &[&str] = &[
    "hardeningUsbguard",
    "hardeningMalloc",
    "hardeningApparmor",
    "hardeningNosmt",
];
pub const NETWORK_SWITCHES: &[&str] = &["proxyEnable"];
pub const NETWORK_TEXTS: &[&str] = &["hostName"];

pub fn security(ctx: &Ctx, settings: &Value, options: Option<&Value>, rebuild: &Rebuild) -> Html {
    let m = &ctx.m;
    let off = rebuild.busy();
    let mut group = ui::group();
    for (key, word) in [
        ("hardeningUsbguard", "usbguard"),
        ("hardeningMalloc", "malloc"),
        ("hardeningApparmor", "apparmor"),
        ("hardeningNosmt", "nosmt"),
    ] {
        group = group.child(switch(
            key,
            settings,
            &m.t(&format!("panes.security.{word}.title")),
            &m.t(&format!("panes.security.{word}.detail")),
            off,
        ));
    }
    let mut section = ui::pane_section();
    if runs_without_tpm(options) {
        section = section.child(
            ui::alert(
                "warn",
                Some(icons::SECURITY_LOCK),
                Some(&m.t("ui.noTpm.title")),
                Html::from(el("p").text(&m.t("ui.noTpm.body"))),
            )
            .class("mb-5"),
        );
    }
    let fields = section
        .child(ui::group_title(&m.t("panes.security.group")))
        .child(group)
        .child(ui::group_caption(Html::text(
            &m.t("panes.security.caption"),
        )));
    settings_form(ctx, "security", rebuild, fields.into(), Html::new())
}

/// components/no-tpm-notice.tsx's runsWithoutTpm: the options document
/// says the box was installed with its key on the boot partition.
fn runs_without_tpm(options: Option<&Value>) -> bool {
    options
        .and_then(|o| o["options"].as_array())
        .and_then(|a| a.iter().find(|o| o["name"] == "tpm.enable"))
        .is_some_and(|o| o["current"] == Value::Bool(false))
}

/// pane-network.tsx's name and reach. The name's field is "changed" by CSS
/// when it no longer matches its old value: a `pattern` that refuses the
/// old value, read as `:valid` (basic.css), on a form that does not let the
/// browser hold it back (`novalidate`); lososd checks the shape.
pub fn network(
    ctx: &Ctx,
    settings: &Value,
    rebuild: &Rebuild,
    problem: Option<&str>,
    domains: Html,
) -> Html {
    let m = &ctx.m;
    let off = rebuild.busy();
    let name = settings["hostName"].as_str().unwrap_or_default();
    let error_id = "f-hostName-problem";
    let name_row = ui::row()
        .child(ui::row_text(
            &m.t("panes.network.called"),
            None,
            Some("f-hostName"),
        ))
        .child(ui::hidden("was.hostName", name))
        .child(
            ui::input("f-hostName", "hostName", name)
                .attr("maxlength", "63")
                .attr("autocomplete", "off")
                .attr("spellcheck", "false")
                .attr("autocapitalize", "off")
                .attr("autocorrect", "off")
                .attr("data-was-text", "")
                .attr("pattern", &not_this(name))
                .attr_opt("aria-invalid", problem.map(|_| "true"))
                .attr_opt("aria-describedby", problem.map(|_| error_id))
                .flag("disabled", off)
                .class("w-[13rem]"),
        );
    let reached = if name.is_empty() {
        "—".to_string()
    } else {
        format!("{name}.local")
    };
    let names = ui::pane_section()
        .child(ui::group_title(&m.t("panes.network.name")))
        .child(
            ui::group().child(name_row).child(
                ui::row()
                    .child(ui::row_text(&m.t("panes.network.reachedAt"), None, None))
                    .child(ui::row_value(&reached)),
            ),
        )
        .child(problem.map(|p| {
            Html::from(
                el("div")
                    .class("px-1.5 pt-2")
                    .child(ui::field_error(error_id, p)),
            )
        }))
        .child(ui::group_caption(Html::text(
            &m.t("panes.network.nameCaption"),
        )));
    let reach = ui::pane_section()
        .child(ui::group_title(&m.t("panes.network.reaching")))
        .child(ui::group().child(switch(
            "proxyEnable",
            settings,
            &m.t("panes.network.outside"),
            "",
            off,
        )))
        .child(ui::group_caption(Html::text(
            &m.t("panes.network.reachCaption"),
        )));
    let mut fields = Html::from(names);
    fields.push(reach);
    settings_form(ctx, "network", rebuild, fields, domains)
}

/// A `pattern` every value but `old` matches. The browser compiles it with
/// the `v` flag, so every character that is not a letter or a digit is
/// written as `\u{…}`, which that flag always takes.
#[must_use]
pub fn not_this(old: &str) -> String {
    let mut out = String::from("(?!");
    for c in old.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push_str(&format!("\\u{{{:x}}}", c as u32));
        }
    }
    out.push_str("$).*");
    out
}

// ── Storage ─────────────────────────────────────────────────────────────

/// pane-storage.tsx: the meter, the reserve and the dialog that asks
/// before the reserve is used, opened by a checkbox's label.
/// The Storage pane, or the administrator's page in its place (`custom`,
/// crate::custom), whose one block is the disk.
pub fn storage(
    ctx: &Ctx,
    storage: Option<&Value>,
    rebuild: &Rebuild,
    custom: Option<&str>,
) -> Html {
    let m = &ctx.m;
    let facts = storage.cloned().unwrap_or(Value::Null);
    let reserve = bytes(&facts, "reserveBytes");
    let has_reserve = reserve.is_some_and(|r| r > 0);
    let spent = reserve == Some(0);
    let size = |b: Option<u64>| format::bytes_1024(m.locale, b);

    let action = if !has_reserve || rebuild.busy() {
        ui::button(Variant::Secondary, Size::Sm)
            .attr("type", "button")
            .flag("disabled", true)
            .text(&m.t(if spent {
                "panes.storage.reserveUsed"
            } else {
                "panes.storage.useReserve"
            }))
            .into()
    } else {
        Html::from(
            el("label")
                .attr("for", "basic-grow")
                .attr("role", "button")
                .class(&ui::button_classes(Variant::Secondary, Size::Sm))
                .class("cursor-pointer")
                .text(&m.t("panes.storage.useReserve")),
        )
    };

    let group = ui::group()
        .child(ui::stack_row().child(capacity_meter(m, storage)))
        .child(
            ui::row()
                .child(ui::row_text(
                    &m.t("panes.storage.held.title"),
                    Some(&m.t(if spent {
                        "panes.storage.held.spent"
                    } else {
                        "panes.storage.held.detail"
                    })),
                    None,
                ))
                .child(
                    el("div")
                        .class("flex items-center gap-2.5")
                        .child(ui::row_value(&match reserve {
                            None => m.t("panes.storage.notReported"),
                            Some(_) => size(reserve),
                        }))
                        .child(action),
                ),
        );
    let disk = Html::from(
        ui::pane_section()
            .child(ui::group_title(&m.t("panes.storage.disk")))
            .child(group)
            .child(ui::group_caption(Html::text(
                &m.t("panes.storage.diskCaption"),
            ))),
    );
    let mut body = match custom {
        None => disk,
        Some(html) => crate::custom::splice(html, |name, _| match name {
            "Disk" => disk.clone(),
            _ => Html::new(),
        }),
    };

    if has_reserve && !rebuild.busy() {
        let text = format!(
            "{} {}",
            m.tv(
                "panes.storage.dialogAdds",
                &[("size", Var::Text(size(reserve)))]
            ),
            m.t("panes.storage.dialogTail")
        );
        let dialog = el("form")
            .attr("method", "post")
            .attr("action", &ctx.here)
            .child(ui::hidden("grow", "1"))
            .child(ui::dialog_header(
                "basic-grow-title",
                &m.t("panes.storage.dialogTitle"),
                &text,
            ))
            .child(
                ui::dialog_footer()
                    .child(
                        el("label")
                            .attr("for", "basic-grow")
                            .attr("role", "button")
                            .class(&ui::button_classes(Variant::Ghost, Size::Md))
                            .class("cursor-pointer")
                            .text(&m.t("panes.storage.cancel")),
                    )
                    .child(
                        ui::button(Variant::Primary, Size::Md).text(&m.t("panes.storage.useIt")),
                    ),
            );
        body.push(
            el("input")
                .attr("type", "checkbox")
                .id("basic-grow")
                .attr("data-basic-toggle", "")
                .attr("aria-hidden", "true")
                .attr("tabindex", "-1")
                .class("sr-only"),
        );
        body.push(ui::dialog(
            "w-[min(28rem,calc(100vw-2rem))]",
            "basic-grow-title",
            dialog,
        ));
    }
    let bar = if rebuild.busy() {
        rebuild_bar(m, &rebuild.watch)
    } else {
        Html::new()
    };
    pane_column(ctx, "storage", body, bar)
}

/// capacity-meter.tsx.
fn capacity_meter(m: &Messages, storage: Option<&Value>) -> Html {
    let facts = storage.cloned().unwrap_or(Value::Null);
    let total = bytes(&facts, "totalBytes").filter(|t| *t > 0);
    let size = |b: Option<u64>| format::bytes_1024(m.locale, b);
    let Some(total) = total else {
        let why = match storage {
            None => "settings.capacity.error",
            Some(_) => "settings.capacity.noSize",
        };
        return el("div")
            .class("flex flex-col gap-2.5")
            .child(
                el("div")
                    .attr("role", "img")
                    .attr("aria-label", &m.t("settings.capacity.unmeasured"))
                    .class("flex h-3 w-full overflow-hidden rounded-full border border-line bg-sunk opacity-60"),
            )
            .child(el("p").class("text-[12.5px] leading-snug text-muted").text(&m.t(why)))
            .into();
    };
    let used = bytes(&facts, "usedBytes").unwrap_or(0);
    let lent = bytes(&facts, "lentBytes").unwrap_or(0);
    let free = total.saturating_sub(used).saturating_sub(lent);
    let label = m.tv(
        "settings.capacity.label",
        &[
            ("used", Var::Text(size(Some(used)))),
            ("lent", Var::Text(size(Some(lent)))),
            ("free", Var::Text(size(Some(free)))),
            ("total", Var::Text(size(Some(total)))),
        ],
    );
    let key = |swatch: &str, name: String, value: String| -> Html {
        el("li")
            .class("flex items-center gap-2 text-[12.5px] text-muted")
            .child(
                el("span")
                    .attr("aria-hidden", "true")
                    .class("size-2.5 shrink-0 rounded-[3px]")
                    .class(swatch),
            )
            .text(&name)
            .child(el("span").class("numeric text-ink").text(&value))
            .into()
    };
    let used_pct = format::percent(used, total);
    let lent_pct = format::percent(lent, total).min(100 - used_pct);
    el("div")
        .class("flex flex-col gap-2.5")
        .child(
            el("div")
                .attr("role", "img")
                .attr("aria-label", &label)
                .class("flex h-3 w-full overflow-hidden rounded-full border border-line bg-sunk")
                .child(
                    el("div")
                        .class("h-full bg-accent")
                        .class(&format!("w-[{used_pct}%]")),
                )
                .child(
                    el("div")
                        .class("hatched-solid h-full")
                        .class(&format!("w-[{lent_pct}%]")),
                ),
        )
        .child(
            el("ul")
                .class("flex flex-wrap gap-x-5 gap-y-1.5")
                .child(key(
                    "bg-accent",
                    m.t("settings.capacity.yours"),
                    size(Some(used)),
                ))
                .child(key(
                    "hatched-solid",
                    m.t("settings.capacity.lent"),
                    size(Some(lent)),
                ))
                .child(key(
                    "bg-sunk border border-line",
                    m.t("settings.capacity.free"),
                    size(Some(free)),
                ))
                .child(total_line(m, &size(Some(total)))),
        )
        .into()
}

/// "{total} in all", with the number in the figure face.
fn total_line(m: &Messages, total: &str) -> Html {
    let text = m.t("settings.capacity.total");
    let (before, after) = text.split_once("{total}").unwrap_or((text.as_str(), ""));
    el("li")
        .class("ml-auto text-[12.5px] text-muted")
        .text(before)
        .child(el("span").class("numeric").text(total))
        .text(after)
        .into()
}

/// use-storage.ts's outcome of a grow, as a toast.
#[must_use]
pub fn grow_toast(m: &Messages, word: &str) -> Option<Toast> {
    let mut parts = word.split('.');
    let kind = parts.next()?;
    let mut num = || parts.next().and_then(|p| p.parse::<u64>().ok());
    let size = |b: u64| format::bytes_1024(m.locale, Some(b));
    match kind {
        "grew" => {
            let (before, after) = (num()?, num()?);
            Some(Toast {
                tone: "success",
                title: m.t("panes.storage.grew.title"),
                body: m.tv(
                    "panes.storage.grew.detail",
                    &[
                        ("before", Var::Text(size(before))),
                        ("after", Var::Text(size(after))),
                    ],
                ),
            })
        }
        "nogrow" => {
            let claimed = num().unwrap_or(0);
            Some(Toast {
                tone: "done",
                title: m.t("panes.storage.nothing.title"),
                body: if claimed > 0 {
                    m.tv(
                        "panes.storage.nothing.claimed",
                        &[("size", Var::Text(size(claimed)))],
                    )
                } else {
                    m.t("panes.storage.nothing.none")
                },
            })
        }
        "growfail" => Some(Toast {
            tone: "error",
            title: m.t("panes.storage.failed.title"),
            body: m.t("settings.form.noAnswer"),
        }),
        _ => None,
    }
}

/// The flash word for a grow's answer (`POST /api/grow`).
#[must_use]
pub fn grow_word(answer: &crate::host::Answer) -> String {
    if !answer.ok() {
        return "growfail".to_string();
    }
    let n = |k: &str| answer.json[k].as_u64().unwrap_or(0);
    if answer.json["grew"].as_bool() == Some(true) {
        format!("grew.{}.{}", n("beforeBytes"), n("afterBytes"))
    } else {
        format!("nogrow.{}", n("claimedBytes"))
    }
}

/// The toast an apply that did not start leaves.
#[must_use]
pub fn not_started(m: &Messages, why: Option<&str>) -> Toast {
    Toast {
        tone: "error",
        title: m.t("settings.form.didNotStart"),
        body: why.map_or_else(|| m.t("settings.form.noAnswer"), str::to_string),
    }
}

/// The rebuild's state for this browser.
pub fn rebuild(host: &mut dyn Host, mine: Option<&Mine>, now: u64) -> Rebuild {
    let status = host
        .get("/api/status")
        .map(|v| Status::from_json(&v))
        .unwrap_or_default();
    Rebuild {
        watch: settings::watch(&status, mine, now),
    }
}

// ── Search, later, not found ────────────────────────────────────────────

/// The palette's pages, as a form: every page this person may open whose
/// name holds the words.
pub fn search(ctx: &Ctx, q: &str) -> Html {
    let m = &ctx.m;
    let mut pages: Vec<(String, String, &'static str)> = Vec::new();
    fn walk(out: &mut Vec<(String, String, &'static str)>, entries: &[shell::NavEntry]) {
        for e in entries {
            if let Some(h) = &e.href {
                out.push((e.label.clone(), h.clone(), e.glyph));
            }
            walk(out, &e.children);
        }
    }
    walk(&mut pages, &ctx.nav);
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    let hits: Vec<&(String, String, &'static str)> = pages
        .iter()
        .filter(|(label, _, _)| {
            let l = label.to_lowercase();
            words.iter().all(|w| l.contains(w.as_str()))
        })
        .collect();
    let form = el("form")
        .attr("method", "get")
        .attr("action", "/basic/search")
        .attr("role", "search")
        .class("mb-4 flex gap-2")
        .child(
            ui::input("basic-q", "q", q)
                .class("w-full")
                .attr("type", "search")
                .attr("aria-label", &m.t("palette.title"))
                .attr("placeholder", &m.t("palette.placeholder"))
                .attr("autocomplete", "off")
                .flag("autofocus", true),
        )
        .child(ui::button(Variant::Secondary, Size::Md).text(&m.t("palette.title")));
    let list = if hits.is_empty() {
        Html::from(ui::empty(
            icons::SEARCH_01,
            &m.tv(
                "palette.noMatch",
                &[("query", Var::Text(q.trim().to_string()))],
            ),
            &m.t("basic.search.noneBody"),
        ))
    } else {
        Html::from(ui::group().children(hits.into_iter().enumerate().map(|(n, (label, href, glyph))| {
            el("a")
                .attr("href", href)
                .class("relative flex min-h-[42px] items-center gap-3 px-3.5 py-2 text-sm text-ink no-underline transition-colors duration-100 hover:bg-accent-wash focus-visible:bg-accent-wash focus-visible:outline-none animate-rise")
                .class(stagger(n))
                .class("after:pointer-events-none after:absolute after:right-0 after:bottom-0 after:left-3.5 after:h-px after:bg-hair after:content-[''] last:after:hidden")
                .child(el("span").class("text-muted").child(ui::icon(glyph, 18, "")))
                .child(el("span").class("flex-1").text(label))
                .child(el("span").class("text-faint").child(ui::icon(icons::ARROW_RIGHT_01, 16, "")))
        })))
    };
    el("div")
        .child(ui::pane_header(
            &m.t("palette.title"),
            &m.t("basic.search.summary"),
        ))
        .child(form)
        .child(list)
        .into()
}

/// A page the script UI has and these pages do not have yet.
pub fn later(ctx: &Ctx, title: &str, glyph: &'static str) -> Html {
    let m = &ctx.m;
    el("div")
        .attr("data-basic-later", "")
        .child(ui::pane_header(title, &m.t("basic.later.summary")))
        .child(ui::empty(
            glyph,
            &m.t("basic.later.title"),
            &m.t("basic.later.body"),
        ))
        .into()
}

pub fn not_found(ctx: &Ctx) -> Html {
    let m = &ctx.m;
    ui::empty(
        icons::COMPASS_01,
        &m.t("basic.notFound.title"),
        &m.t("basic.notFound.body"),
    )
    .child(
        ui::link_button(&shell::href("/"), Variant::Secondary, Size::Sm)
            .text(&m.t("shell.nav.overview")),
    )
    .into()
}

/// The label of a page, for its title.
#[must_use]
pub fn label_of(ctx: &Ctx, path: &str) -> Option<(String, &'static str)> {
    fn find(entries: &[shell::NavEntry], to: &str) -> Option<(String, &'static str)> {
        for e in entries {
            if e.href.as_deref() == Some(to) {
                return Some((e.label.clone(), e.glyph));
            }
            if let Some(hit) = find(&e.children, to) {
                return Some(hit);
            }
        }
        None
    }
    find(&ctx.nav, &shell::href(path))
}

// ── Your own domain ─────────────────────────────────────────────────────

/// domains-section.tsx, from `GET /api/domains`. It sits under the
/// network pane's form, not in it: adding and removing a domain are forms
/// of their own that go to the edge at once, with no rebuild.
pub fn domains(ctx: &Ctx, answer: Option<&Value>, off: bool) -> Html {
    let m = &ctx.m;
    let one = |row: Html| Html::from(ui::group().child(ui::row().child(row)));
    let body = match answer {
        None => one(ui::row_text(
            &m.t("panes.network.domains.unreachable"),
            None,
            None,
        )),
        Some(v) if v["available"] != Value::Bool(true) => one(ui::row_text(
            &m.t("panes.network.domains.unavailable"),
            Some(&m.t(if v["reason"] == "noOfficialEdge" {
                "panes.network.domains.noOfficialEdge"
            } else {
                "panes.network.domains.notOffered"
            })),
            None,
        )),
        Some(v) if v["eligible"] != Value::Bool(true) => one(ui::row_text(
            &m.t("panes.network.domains.needsStripe"),
            Some(&m.t("panes.network.domains.needsStripeDetail")),
            None,
        )),
        Some(v) => domain_list(ctx, v, off),
    };
    ui::pane_section()
        .attr("data-testid", "custom-domains")
        .child(ui::group_title(&m.t("panes.network.domains.title")))
        .child(body)
        .child(ui::group_caption(Html::text(
            &m.t("panes.network.domains.caption"),
        )))
        .into()
}

/// Whether a domain still waits for its records, so the page looks again.
#[must_use]
pub fn domains_waiting(answer: Option<&Value>) -> bool {
    answer
        .and_then(|v| v["domains"].as_array())
        .is_some_and(|list| list.iter().any(|d| d["status"] != "live"))
}

fn domain_list(ctx: &Ctx, v: &Value, off: bool) -> Html {
    let m = &ctx.m;
    let target = v["target"].as_str().unwrap_or_default();
    let list: &[Value] = v["domains"].as_array().map_or(&[], Vec::as_slice);
    let max = v["max_domains"].as_u64().unwrap_or(0);
    let full = list.len() as u64 >= max;
    let addresses: Vec<&str> = v["addresses"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut group = ui::group().child(
        ui::row()
            .child(ui::row_text(
                &m.t("panes.network.domains.target"),
                Some(&m.t("panes.network.domains.targetDetail")),
                None,
            ))
            .child(ui::row_value(target)),
    );
    for d in list {
        group = group.child(domain_row(ctx, d, target, &addresses, off));
    }
    let add_id = "f-domain";
    let full_note = full.then(|| m.tv("panes.network.domains.full", &[("count", Var::from(max))]));
    group = group.child(
        ui::row()
            .child(ui::row_text(
                &m.t("panes.network.domains.add"),
                full_note.as_deref(),
                Some(add_id),
            ))
            .child(
                el("form")
                    .attr("method", "post")
                    .attr("action", &ctx.here)
                    .class("flex items-center gap-2")
                    .child(
                        ui::input(add_id, "domain-add", "")
                            .attr("placeholder", "cloud.example.org")
                            .attr("maxlength", "253")
                            .attr("required", "")
                            .attr("autocomplete", "off")
                            .attr("spellcheck", "false")
                            .attr("autocapitalize", "off")
                            .attr("autocorrect", "off")
                            .flag("disabled", off || full)
                            .class("w-[13rem]"),
                    )
                    .child(
                        ui::button(Variant::Primary, Size::Sm)
                            .attr("type", "submit")
                            .flag("disabled", off || full)
                            .text(&m.t("panes.network.domains.addButton")),
                    ),
            ),
    );
    group.into()
}

fn domain_row(ctx: &Ctx, d: &Value, target: &str, addresses: &[&str], off: bool) -> Html {
    let m = &ctx.m;
    let name = d["domain"].as_str().unwrap_or_default();
    let live = d["status"] == "live";
    let record = |mark: bool, name: &str, kind: &str, value: &str| {
        Html::from_iter([
            el("dt").class("text-muted").text(kind),
            el("dd").class("break-all text-ink select-all").text(name),
            el("dd")
                .class("break-all text-ink select-all")
                .text(value)
                .text(" ")
                .child(
                    el("span")
                        .class(if mark { "text-ok" } else { "text-muted" })
                        .text(&m.t(if mark {
                            "panes.network.domains.seen"
                        } else {
                            "panes.network.domains.notSeen"
                        })),
                ),
        ])
    };
    let problem = d["problem"].as_str().map(|p| {
        Html::from(
            el("p")
                .class("text-[12.5px] leading-snug text-muted")
                .text(&m.t(&format!("panes.network.domains.problem.{p}"))),
        )
    });
    let records = (!live).then(|| {
        let txt = d["txt_value"]
            .as_str()
            .map_or_else(|| "—".to_string(), |t| format!("\"{t}\""));
        Html::from(
            el("dl")
                .class("numeric grid grid-cols-[auto_auto_1fr] gap-x-3 gap-y-1 text-[12.5px] leading-snug")
                .child(record(
                    d["points_here"] == Value::Bool(true),
                    name,
                    "CNAME",
                    &format!("{target}."),
                ))
                .child(record(
                    d["txt_found"] == Value::Bool(true),
                    d["txt_name"].as_str().unwrap_or_default(),
                    "TXT",
                    &txt,
                )),
        )
    });
    let apex = (!live && !addresses.is_empty()).then(|| {
        Html::from(
            el("p")
                .class("text-[12.5px] leading-snug text-muted")
                .text(&m.tv(
                    "panes.network.domains.apex",
                    &[("addresses", Var::from(addresses.join(", ").as_str()))],
                )),
        )
    });
    ui::stack_row()
        .attr("data-testid", "custom-domain")
        .child(
            el("form")
                .attr("method", "post")
                .attr("action", &ctx.here)
                .class("flex items-center gap-3")
                .child(
                    el("p")
                        .class("min-w-0 flex-1 truncate text-sm text-ink")
                        .text(name),
                )
                .child(ui::badge(
                    if live { ui::Tone::Ok } else { ui::Tone::Warn },
                    &m.t(if live {
                        "panes.network.domains.live"
                    } else {
                        "panes.network.domains.waiting"
                    }),
                ))
                .child(
                    ui::button(Variant::Secondary, Size::Sm)
                        .attr("type", "submit")
                        .attr("name", "domain-remove")
                        .attr("value", name)
                        .flag("disabled", off)
                        .text(&m.t("panes.network.domains.remove")),
                ),
        )
        .child(problem)
        .child(records)
        .child(apex)
        .into()
}

/// What adding or removing a domain leaves for the next page: `domadd`,
/// `domgone.<domain>` or `domfail.<the edge's sentence>`.
#[must_use]
pub fn domain_word(answer: &crate::host::Answer, removed: Option<&str>) -> String {
    if !answer.ok() {
        let said = answer.json["error"]
            .as_str()
            .or_else(|| answer.json["message"].as_str())
            .unwrap_or_default();
        return format!("domfail.{}", crate::encode(said));
    }
    match removed {
        Some(d) => format!("domgone.{}", crate::encode(d)),
        None => "domadd".to_string(),
    }
}

/// The toast a domain change left.
#[must_use]
pub fn domain_toast(m: &Messages, word: &str) -> Option<Toast> {
    let (kind, rest) = word.split_once('.').unwrap_or((word, ""));
    let rest = crate::web::decode(rest);
    match kind {
        "domadd" => Some(Toast {
            tone: "success",
            title: m.t("panes.network.domains.addedTitle"),
            body: m.t("panes.network.domains.addedBody"),
        }),
        "domgone" => Some(Toast {
            tone: "done",
            title: m.tv(
                "panes.network.domains.removedTitle",
                &[("domain", Var::from(rest.as_str()))],
            ),
            body: String::new(),
        }),
        "domfail" => Some(Toast {
            tone: "error",
            title: m.t("panes.network.domains.failedTitle"),
            body: if rest.is_empty() {
                m.t("panes.network.domains.failed")
            } else {
                rest
            },
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_value_alone_fails_the_pattern() {
        assert_eq!(not_this("losos-2"), "(?!losos\\u{2d}2$).*");
        assert_eq!(not_this(""), "(?!$).*");
    }
}
