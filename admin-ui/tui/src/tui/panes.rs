//! The box's panes: the web UI's home screen and its settings panes, as
//! models (`model.rs`). Same panes, same order, same words: every title,
//! detail and caption below is a key of the web UI's own catalogue, apart
//! from the few `tui.*` strings this client adds.
//!
//! Left out on purpose: Access tokens (a token cannot manage tokens), the
//! Market and Machines (trading), the Lab, Look and Hardware (pictures and a
//! GPU switch nobody sets from a terminal), and the destructive two,
//! factory reset and erase, which stay in the web UI. About says so.
//!
//! The answers are read field by field out of their JSON, the way the web
//! UI's types describe them (admin-ui/app/src/lib/api.ts); a field that is
//! missing reads as absent rather than failing the pane.

use serde_json::Value;

use super::edge;
use super::form::{Form, SettingKey};
use super::model::{
    clip, pad, Chart, Confirm, ConfirmSpec, Control, EditTarget, Job, PaneModel, Prepare, Row,
    Section, Span, Tone,
};
use super::resources::{Resource, Resources};
use crate::i18n::{self, format_bytes, format_iso, format_unix, t, Locale};
use crate::theme::C;
use crate::tr;
use crate::vault::{group_fingerprint, host_name, HostRecord};

/// The crate's version, or what the build names it (`LOSOS_VERSION`).
pub const VERSION: &str = match option_env!("LOSOS_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

mod keywords {
    include!(concat!(env!("OUT_DIR"), "/pane_keywords.rs"));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    Overview,
    Storage,
    Mesh,
    Apps,
    Network,
    Security,
    Advanced,
    History,
    Backup,
    About,
    Computer,
    EdgeOverview,
    EdgeSecrets,
}

impl PaneId {
    pub const BOX: [PaneId; 11] = [
        PaneId::Overview,
        PaneId::Storage,
        PaneId::Mesh,
        PaneId::Apps,
        PaneId::Network,
        PaneId::Security,
        PaneId::Advanced,
        PaneId::History,
        PaneId::Backup,
        PaneId::About,
        PaneId::Computer,
    ];
    pub const EDGE: [PaneId; 2] = [PaneId::EdgeOverview, PaneId::EdgeSecrets];

    /// The web UI's id of a settings pane.
    pub fn settings_id(self) -> Option<&'static str> {
        Some(match self {
            PaneId::Storage => "storage",
            PaneId::Mesh => "mesh",
            PaneId::Apps => "apps",
            PaneId::Network => "network",
            PaneId::Security => "security",
            PaneId::Advanced => "advanced",
            PaneId::History => "history",
            PaneId::Backup => "backup",
            PaneId::About => "about",
            _ => return None,
        })
    }

    pub fn id(self) -> &'static str {
        match self {
            PaneId::Overview => "overview",
            PaneId::Computer => "computer",
            PaneId::EdgeOverview => "edge-overview",
            PaneId::EdgeSecrets => "edge-secrets",
            other => other.settings_id().unwrap_or(""),
        }
    }

    fn keys(self) -> (String, String, String) {
        match self {
            PaneId::Overview => (
                "shell.nav.overview".into(),
                "tui.overview.summary".into(),
                "tui.overview.keywords".into(),
            ),
            PaneId::EdgeOverview => (
                "shell.nav.overview".into(),
                "tui.edge.overviewSummary".into(),
                "tui.edge.overviewKeywords".into(),
            ),
            PaneId::Computer => (
                "tui.computer.pane".into(),
                "tui.computer.summary".into(),
                "tui.computer.keywords".into(),
            ),
            PaneId::EdgeSecrets => (
                "tui.edge.secrets".into(),
                "tui.edge.secretsSummary".into(),
                "tui.edge.secretsKeywords".into(),
            ),
            other => {
                let id = other.settings_id().unwrap_or_default();
                (
                    format!("settings.panes.{id}.label"),
                    format!("settings.panes.{id}.summary"),
                    format!("settings.panes.{id}.keywords"),
                )
            }
        }
    }

    pub fn label(self) -> String {
        t(&self.keys().0)
    }

    /// The reads this pane draws from.
    pub fn needs(self) -> &'static [&'static str] {
        match self {
            PaneId::Overview => &[
                "/api/health",
                "/api/state",
                "/api/status",
                "/api/storage",
                "/api/apps",
            ],
            PaneId::Storage => &["/api/storage"],
            PaneId::Mesh => &["/api/edge"],
            PaneId::Apps => &["/api/apps"],
            PaneId::History => &["/api/config"],
            PaneId::Backup => &["/api/backup"],
            PaneId::EdgeOverview => &["/overview"],
            PaneId::EdgeSecrets => &["/secrets"],
            _ => &[],
        }
    }

    fn english_keywords(self) -> Vec<&'static str> {
        match self {
            PaneId::Overview => vec!["home", "status", "apps", "health"],
            PaneId::EdgeOverview => vec!["tenants", "heartbeat", "boxes", "reconcile", "health"],
            PaneId::EdgeSecrets => vec!["secrets", "credentials", "stripe", "seal", "keys"],
            PaneId::Computer => vec![
                "computer", "laptop", "device", "files", "webdav", "git", "clone", "setup",
            ],
            other => keywords::PANE_KEYWORDS
                .iter()
                .find(|(id, _)| Some(*id) == other.settings_id())
                .map(|(_, words)| words.to_vec())
                .unwrap_or_default(),
        }
    }

    /// The web UI's `paneMatches`: every term must match the pane's name,
    /// summary or keywords in the language on screen or in English, accents
    /// folded.
    pub fn matches(self, search: &str) -> bool {
        let terms: Vec<String> = i18n::fold(search)
            .split_whitespace()
            .map(str::to_string)
            .collect();
        if terms.is_empty() {
            return true;
        }
        let (label, summary, words) = self.keys();
        let english = |k: &str| i18n::template(k, &[], Locale::En);
        let haystack = i18n::fold(
            &[
                t(&label),
                t(&summary),
                t(&words),
                english(&label),
                english(&summary),
                self.english_keywords().join(" "),
            ]
            .join(" "),
        );
        terms.iter().all(|term| haystack.contains(term.as_str()))
    }
}

/// A hit of the Apps pane's catalogue search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub id: String,
    pub name: String,
    pub source: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub chart: Option<Chart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SearchState {
    #[default]
    Idle,
    Searching(String),
    Results {
        query: String,
        sources: Vec<String>,
        apps: Vec<Hit>,
    },
    Unsupported,
    Failed(String),
}

/// What a pane is built from.
pub struct Ctx<'a> {
    pub host: &'a HostRecord,
    pub res: &'a Resources,
    pub form: &'a Form,
    pub search: &'a SearchState,
    /// Unix seconds, for "ago".
    pub now: f64,
    /// The computer this client runs on, for its own pane.
    pub local: &'a Local,
}

/// What the This computer pane shows: whether each of the two is set up
/// here. `available` is false when the screen was opened without the
/// computer's keychain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Local {
    pub available: bool,
    pub device: String,
    pub cloud: Option<crate::computer::Credential>,
    pub git: Option<crate::computer::Credential>,
}

pub fn build(pane: PaneId, ctx: &Ctx<'_>, cols: usize) -> PaneModel {
    match pane {
        PaneId::Overview => overview(ctx, cols),
        PaneId::Storage => storage(ctx, cols),
        PaneId::Mesh => mesh(ctx),
        PaneId::Apps => apps(ctx),
        PaneId::Network => network(ctx),
        PaneId::Security => security(ctx),
        PaneId::Advanced => advanced(ctx),
        PaneId::History => history(ctx),
        PaneId::Backup => backup(ctx),
        PaneId::About => about(ctx),
        PaneId::Computer => computer(ctx),
        PaneId::EdgeOverview => edge::overview(ctx, cols),
        PaneId::EdgeSecrets => edge::secrets(ctx),
    }
}

// ── Little helpers ──────────────────────────────────────────────────────

pub(crate) fn str_of(v: &Value) -> Option<&str> {
    v.as_str().filter(|s| !s.is_empty())
}

fn loading_section(key: &str, resource: &Resource) -> Option<Section> {
    match resource {
        Resource::Loading => Some(
            Section::new(key)
                .row(Row::new(format!("{key}-loading"), t("tui.loading")).tone(Tone::Muted)),
        ),
        Resource::Error { message, .. } => {
            Some(Section::new(key).notice(Tone::Crit, Some(t("tui.couldNotRead")), message.clone()))
        }
        Resource::Ok(_) => None,
    }
}

fn form_loading(form: &Form) -> Option<Vec<Section>> {
    if let Some(error) = &form.load_error {
        return Some(vec![Section::new("form-error").notice(
            Tone::Crit,
            Some(t("tui.couldNotRead")),
            error.clone(),
        )]);
    }
    if !form.ready() {
        return Some(vec![Section::new("form-loading")
            .row(Row::new("loading", t("tui.loading")).tone(Tone::Muted))]);
    }
    None
}

/// A meter, the web UI's capacity bar: filled is yours, hatched is lent.
pub fn meter(cols: usize, used: f64, lent: f64, total: f64) -> Vec<Span> {
    let room = cols.max(10);
    let used_cells = ((used.min(total) / total) * room as f64).round() as usize;
    let lent_cells =
        (room - used_cells.min(room)).min(((lent.max(0.0) / total) * room as f64).round() as usize);
    vec![
        Span::new("█".repeat(used_cells.min(room)), C::Accent),
        Span::new("▒".repeat(lent_cells), C::Accent),
        Span::new(
            "░".repeat(room.saturating_sub(used_cells + lent_cells)),
            C::Line,
        ),
    ]
}

fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

fn join_nonempty(parts: &[String], sep: &str) -> String {
    parts
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(sep)
}

fn phase_key(phase: &str) -> &'static str {
    match phase {
        "installing" => "install.phase.installing",
        "removing" => "install.phase.removing",
        "failed" => "install.phase.failed",
        _ => "install.phase.running",
    }
}

// ── The compute window (admin-ui/app/src/screens/settings/window.ts) ──────

fn minutes_of(hhmm: &str) -> Option<u32> {
    if !super::form::is_valid_time(hhmm) {
        return None;
    }
    let h: u32 = hhmm[..2].parse().ok()?;
    let m: u32 = hhmm[3..].parse().ok()?;
    Some(h * 60 + m)
}

fn spans_of(start: u32, end: u32) -> Vec<(u32, u32)> {
    use std::cmp::Ordering;
    match start.cmp(&end) {
        Ordering::Equal => vec![],
        Ordering::Less => vec![(start, end)],
        Ordering::Greater => vec![(start, 1440), (0, end)],
    }
}

pub fn describe_window(start_hhmm: &str, end_hhmm: &str) -> String {
    let (Some(start), Some(end)) = (minutes_of(start_hhmm), minutes_of(end_hhmm)) else {
        return t("settings.window.notSet");
    };
    if start == end {
        return t("settings.window.nothing");
    }
    let total: u32 = spans_of(start, end).iter().map(|(f, to)| to - f).sum();
    let length = if total.is_multiple_of(60) && total >= 60 {
        tr!("settings.format.hours", count = total / 60)
    } else {
        tr!("settings.format.minutes", count = total)
    };
    let key = if start > end {
        "settings.window.sharedWraps"
    } else {
        "settings.window.shared"
    };
    tr!(key, start = start_hhmm, end = end_hhmm, length = length)
}

/// The 24-hour strip, one cell per hour, lent hours in the accent.
fn hour_strip(start_hhmm: &str, end_hhmm: &str, muted: bool) -> Vec<Vec<Span>> {
    let spans = match (minutes_of(start_hhmm), minutes_of(end_hhmm)) {
        (Some(s), Some(e)) => spans_of(s, e),
        _ => vec![],
    };
    let mut cells = Vec::new();
    let mut labels = Vec::new();
    for hour in 0..24u32 {
        let (h0, h1) = (hour * 60, hour * 60 + 60);
        let covered: u32 = spans
            .iter()
            .map(|(f, to)| (*to).min(h1).saturating_sub((*f).max(h0)))
            .sum();
        let colour = if muted { C::Faint } else { C::Accent };
        cells.push(match covered {
            0 => Span::new("░░ ", C::Line),
            1..=59 => Span::new("▒▒ ", colour),
            _ => Span::new("██ ", colour),
        });
        labels.push(Span::new(
            if hour % 3 == 0 {
                format!("{hour:02} ")
            } else {
                "   ".into()
            },
            C::Faint,
        ));
    }
    vec![cells, labels]
}

// ── Overview: the web UI's home screen ───────────────────────────────────

/// The tiles of admin-ui/app/src/lib/apps.ts, in its order. Mail is left
/// out: it is drawn only once a probe of the Files app found it, and that
/// probe is not a route the envelope can reach.
pub const FILES_APPS: [&str; 2] = ["files", "photos"];

fn app_grid(names: &[String], cols: usize) -> Vec<Vec<Span>> {
    const CELL: usize = 16;
    let per_line = ((cols.saturating_sub(2)) / CELL).max(1);
    names
        .chunks(per_line)
        .map(|chunk| {
            let mut spans = vec![Span::raw("  ")];
            for name in chunk {
                spans.push(Span::new("▪ ", C::Accent));
                let cut = clip(name, CELL - 2);
                spans.push(Span::new(
                    format!("{cut}{}", " ".repeat(CELL - 2 - super::model::width(cut))),
                    C::Ink,
                ));
            }
            spans
        })
        .collect()
}

fn health_ok(res: &Resources) -> Option<bool> {
    match res.get("/api/health") {
        Resource::Loading => None,
        Resource::Ok(v) => Some(v["ok"] == Value::Bool(true)),
        Resource::Error { .. } => Some(false),
    }
}

fn overview(ctx: &Ctx<'_>, cols: usize) -> PaneModel {
    let saved = ctx.form.saved.as_ref();
    let health = health_ok(ctx.res);
    let (health_word, tone) = match health {
        Some(true) => (t("home.header.answering"), Tone::Ok),
        None => (t("home.header.checking"), Tone::Muted),
        Some(false) => (t("home.header.notAnswering"), Tone::Crit),
    };
    let mut sections = vec![Section::new("box").row(
        Row::new(
            "name",
            saved.map_or_else(|| t("home.header.thisBox"), |s| s.host_name.clone()),
        )
        .detail(ctx.host.url.clone())
        .value(format!("● {health_word}"))
        .tone(tone),
    )];

    if let Some(status) = ctx.res.get("/api/status").data() {
        let message = status["message"].as_str().unwrap_or_default().to_string();
        match status["state"].as_str() {
            Some("building") => sections.push(Section::new("applying").notice(
                Tone::Accent,
                Some(t("home.applying.title")),
                message,
            )),
            Some("failed") => sections.push(Section::new("failed").notice(
                Tone::Crit,
                Some(t("home.attention.changeFailed")),
                message,
            )),
            _ => {}
        }
    }

    // The apps, as the home screen's grid has them.
    let mut names: Vec<String> = FILES_APPS
        .iter()
        .map(|id| t(&format!("apps.{id}.name")))
        .collect();
    names.push(t("apps.code.name"));
    names.push(t("apps.settings.name"));
    let mut installed = Vec::new();
    if let Some(apps) = ctx.res.get("/api/apps").data() {
        if apps["available"] == Value::Bool(true) {
            for app in apps["apps"].as_array().into_iter().flatten() {
                let phase = app["phase"].as_str().unwrap_or("running");
                let release = app["release"].as_str().unwrap_or("-");
                installed.push(
                    Row::new(format!("installed-{release}"), release)
                        .detail(tr!(
                            "install.runsAs",
                            chart = app["chart"]["name"].as_str().unwrap_or("-"),
                            version = app["chart"]["version"].as_str().unwrap_or("-"),
                            user = app["runAs"].as_str().unwrap_or("-")
                        ))
                        .value(t(phase_key(phase)))
                        .tone(match phase {
                            "running" => Tone::Ok,
                            "failed" => Tone::Crit,
                            _ => Tone::Muted,
                        }),
                );
            }
        }
    }
    sections.push(
        Section::new("apps")
            .title(t("shell.nav.apps"))
            .lines(app_grid(&names, cols.saturating_sub(2)))
            .rows(installed),
    );

    // The storage strip.
    let sharing = match ctx.res.get("/api/state").data() {
        Some(state) => state["sharing"] == Value::Bool(true),
        None => saved.is_some_and(|s| s.sharing_my_storage),
    };
    let lending = if !sharing {
        "off"
    } else if saved.is_some_and(|s| s.cluster_enable) {
        "lent"
    } else {
        "waiting"
    };
    let disk = ctx.res.get("/api/storage").data();
    let measured = disk
        .and_then(|d| Some((d["usedBytes"].as_f64()?, d["totalBytes"].as_f64()?)))
        .filter(|(_, total)| *total > 0.0);
    let mut legend = vec![
        Span::raw("  "),
        Span::new("█ ", C::Accent),
        Span::new(t("home.storage.onThisBox"), C::Muted),
    ];
    if let Some((used, _)) = measured {
        legend.push(Span::new(
            format!(" {}", format_bytes(Some(used))),
            C::Faint,
        ));
    }
    if lending == "lent" {
        legend.push(Span::new("   ▒ ", C::Accent));
        legend.push(Span::new(t("home.storage.lent"), C::Muted));
    }
    if let Some((used, total)) = measured {
        legend.push(Span::new("   ░ ", C::Line));
        legend.push(Span::new(t("home.storage.roomLeft"), C::Muted));
        legend.push(Span::new(
            format!(" {}", format_bytes(Some((total - used).max(0.0)))),
            C::Faint,
        ));
    }
    let bar_cols = cols.saturating_sub(6);
    let mut bar = vec![Span::raw("  ")];
    match measured {
        Some((used, total)) => bar.extend(meter(bar_cols, used, 0.0, total)),
        None => bar.push(Span::new("░".repeat(bar_cols), C::Line)),
    }
    sections.push(
        Section::new("storage")
            .title(t("shell.nav.storage"))
            .lines(vec![bar, legend])
            .caption(t(match lending {
                "lent" => "home.storage.sharing",
                "waiting" => "home.storage.waiting",
                _ => "home.storage.local",
            })),
    );

    PaneModel::new(t("shell.nav.overview"), "").with(sections)
}

// ── Storage ──────────────────────────────────────────────────────────────

fn settings_base(pane: PaneId) -> PaneModel {
    let id = pane.settings_id().unwrap_or_default();
    PaneModel::new(
        t(&format!("settings.panes.{id}.label")),
        t(&format!("settings.panes.{id}.summary")),
    )
}

fn storage(ctx: &Ctx<'_>, cols: usize) -> PaneModel {
    let base = settings_base(PaneId::Storage);
    let resource = ctx.res.get("/api/storage");
    if let Some(waiting) = loading_section("storage", resource) {
        return base.with(vec![waiting]);
    }
    let facts = resource.data().cloned().unwrap_or_default();
    let total = facts["totalBytes"].as_f64();
    let measured = total.filter(|t| *t > 0.0);
    let used = facts["usedBytes"].as_f64().unwrap_or(0.0);
    let reserve = facts["reserveBytes"].as_f64();
    let has_reserve = reserve.is_some_and(|r| r > 0.0);
    let spent = reserve == Some(0.0);

    let meter_lines = match measured {
        Some(total) => vec![
            meter(cols.saturating_sub(8), used, 0.0, total),
            vec![
                Span::new("█ ", C::Accent),
                Span::new(format!("{} ", t("settings.capacity.yours")), C::Muted),
                Span::new(format_bytes(Some(used)), C::Ink),
                Span::new("   ░ ", C::Line),
                Span::new(format!("{} ", t("settings.capacity.free")), C::Muted),
                Span::new(format_bytes(Some((total - used).max(0.0))), C::Ink),
                Span::new(
                    format!(
                        "   {}",
                        tr!("settings.capacity.total", total = format_bytes(Some(total)))
                    ),
                    C::Muted,
                ),
            ],
        ],
        None => vec![vec![Span::new(t("settings.capacity.noSize"), C::Muted)]],
    };
    let dialog = match reserve {
        None => t("panes.storage.dialogUnknown"),
        Some(r) => tr!("panes.storage.dialogAdds", size = format_bytes(Some(r))),
    };
    base.with(vec![Section::new("disk")
        .title(t("panes.storage.disk"))
        .rows(vec![
            Row::new(
                "meter",
                tr!(
                    "settings.capacity.label",
                    used = format_bytes(Some(used)),
                    lent = format_bytes(Some(0.0)),
                    free = format_bytes(measured.map(|t| (t - used).max(0.0))),
                    total = format_bytes(total)
                ),
            )
            .extra(meter_lines),
            Row::new("reserve", t("panes.storage.held.title"))
                .detail(t(if spent {
                    "panes.storage.held.spent"
                } else {
                    "panes.storage.held.detail"
                }))
                .value(match reserve {
                    None => t("panes.storage.notReported"),
                    Some(r) => format_bytes(Some(r)),
                })
                .control(Control::Run {
                    label: t(if spent {
                        "panes.storage.reserveUsed"
                    } else {
                        "panes.storage.useReserve"
                    }),
                    disabled: !has_reserve,
                    confirm: Some(Confirm::Ready(ConfirmSpec {
                        title: t("panes.storage.dialogTitle"),
                        body: vec![format!("{dialog} {}", t("panes.storage.dialogTail"))],
                        confirm: t("panes.storage.useIt"),
                        cancel: t("panes.storage.cancel"),
                    })),
                    job: Some(Job::Grow),
                }),
        ])
        .caption(t("panes.storage.diskCaption"))])
}

// ── Mesh ─────────────────────────────────────────────────────────────────

fn mesh(ctx: &Ctx<'_>) -> PaneModel {
    let base = settings_base(PaneId::Mesh);
    let edge = ctx.res.get("/api/edge");
    let mut sections = Vec::new();
    let reachable = edge.data().map(|e| e["reachable"] == Value::Bool(true));

    if let (Some(true), Some(e)) = (reachable, edge.data()) {
        let rows = e["edges"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|found| {
                let official = found["official"] == Value::Bool(true);
                let url = found["url"].as_str().unwrap_or("-");
                Row::new(
                    format!("edge-{url}"),
                    format!(
                        "{} {}",
                        tr!(
                            "panes.mesh.edge.found",
                            name = found["name"].as_str().unwrap_or(url)
                        ),
                        if official { "✓" } else { "!" }
                    ),
                )
                .detail(format!(
                    "{url}\n{}",
                    t(if official {
                        "panes.mesh.edge.officialTip"
                    } else {
                        "panes.mesh.edge.companyTip"
                    })
                ))
                .value(t(if found["source"] == "configured" {
                    "panes.mesh.edge.viaInternet"
                } else {
                    "panes.mesh.edge.viaLan"
                }))
                .tone(Tone::Ok)
            })
            .collect();
        sections.push(
            Section::new("edge")
                .title(t("panes.mesh.edge.title"))
                .rows(rows)
                .caption(t(if e["official"] == Value::Bool(true) {
                    "panes.mesh.edge.caption"
                } else {
                    "panes.mesh.edge.captionCompany"
                })),
        );
    } else {
        let mut row = Row::new("edge-none", t("panes.mesh.edge.looking"));
        match edge {
            Resource::Error { message, .. } => {
                row.title = t("panes.mesh.edge.unknown");
                row = row.detail(message.clone());
            }
            Resource::Ok(e) => {
                row.title = t("panes.mesh.edge.none");
                let detail = if e["lanSearched"] == Value::Bool(true) {
                    match str_of(&e["configuredUrl"]) {
                        None => t("panes.mesh.edge.noneDetail"),
                        Some(url) => tr!("panes.mesh.edge.noneTried", url = url),
                    }
                } else {
                    t("panes.mesh.edge.lanUnsearched")
                };
                row = row
                    .detail(detail)
                    .value(t("panes.mesh.edge.sharingOff"))
                    .tone(Tone::Crit);
            }
            Resource::Loading => {}
        }
        sections.push(
            Section::new("edge")
                .title(t("panes.mesh.edge.title"))
                .row(row)
                .caption(t("panes.mesh.edge.caption")),
        );
    }

    if let Some(waiting) = form_loading(ctx.form) {
        sections.extend(waiting);
        return base.with(sections);
    }
    let draft = ctx.form.draft.as_ref().expect("form is ready");
    let blocked = reachable == Some(false);
    let join_refused = blocked && !ctx.form.saved.as_ref().is_some_and(|s| s.cluster_enable);
    let joined = draft.cluster_enable;

    let mut join = Row::new("join", t("panes.mesh.join")).control(Control::Toggle {
        on: joined,
        disabled: join_refused,
        key: SettingKey::ClusterEnable,
    });
    if join_refused {
        join = join.detail(t("panes.mesh.edge.needed"));
    }
    sections.push(
        Section::new("join")
            .title(t("panes.mesh.otherBoxes"))
            .row(join)
            .caption(t("panes.mesh.joinCaption")),
    );

    let window_off = !joined;
    let mut lend = Row::new("lend", t("panes.mesh.lend"))
        .control(Control::Toggle {
            on: draft.share_compute,
            disabled: window_off,
            key: SettingKey::ShareCompute,
        })
        .extra({
            let mut extra = hour_strip(
                &draft.compute_window_start,
                &draft.compute_window_end,
                !draft.share_compute || window_off,
            );
            extra.push(vec![Span::new(
                describe_window(&draft.compute_window_start, &draft.compute_window_end),
                C::Muted,
            )]);
            extra
        });
    if window_off {
        lend = lend.detail(t("panes.mesh.joinFirst"));
    }
    let problem = ctx.form.problems().compute_window;
    let mut until = Row::new("until", t("panes.mesh.lendUntil")).control(Control::Edit {
        value: draft.compute_window_end.clone(),
        prompt: t("panes.mesh.lendUntil"),
        masked: false,
        disabled: window_off,
        target: EditTarget::Setting(SettingKey::ComputeWindowEnd),
    });
    if let Some(p) = problem {
        until = until.detail(p).tone(Tone::Crit);
    }
    sections.push(
        Section::new("spare")
            .title(t("panes.mesh.spareTime"))
            .rows(vec![
                lend,
                Row::new("from", t("panes.mesh.lendFrom")).control(Control::Edit {
                    value: draft.compute_window_start.clone(),
                    prompt: t("panes.mesh.lendFrom"),
                    masked: false,
                    disabled: window_off,
                    target: EditTarget::Setting(SettingKey::ComputeWindowStart),
                }),
                until,
            ])
            .caption(tr!("panes.mesh.windowCaption", your = t("panes.mesh.your"))),
    );
    base.with(sections)
}

// ── Apps ─────────────────────────────────────────────────────────────────

/// GET /api/apps/search's answer, as far as it can be read.
pub fn parse_hits(body: &Value) -> (Vec<String>, Vec<Hit>) {
    let mut apps = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    for entry in body["results"].as_array().into_iter().flatten() {
        let trimmed = |v: &Value| {
            v.as_str()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_string)
        };
        let (Some(id), Some(name), Some(source)) = (
            trimmed(&entry["id"]),
            trimmed(&entry["name"]),
            trimmed(&entry["source"]),
        ) else {
            continue;
        };
        let c = &entry["chart"];
        let chart = match (
            trimmed(&c["repo"]),
            trimmed(&c["name"]),
            trimmed(&c["version"]),
        ) {
            (Some(repo), Some(name), Some(version)) => Some(Chart {
                repo,
                name,
                version,
            }),
            _ => None,
        };
        apps.push(Hit {
            id,
            name,
            source,
            summary: trimmed(&entry["summary"]),
            version: trimmed(&entry["version"]),
            chart,
        });
    }
    for s in body["sources"].as_array().into_iter().flatten() {
        if let Some(name) = s.as_str().filter(|s| !s.trim().is_empty()) {
            sources.push(name.to_string());
        }
    }
    if sources.is_empty() {
        for app in &apps {
            if !sources.contains(&app.source) {
                sources.push(app.source.clone());
            }
        }
    }
    (sources, apps)
}

pub fn search_path(query: &str) -> String {
    let q: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    format!("/api/apps/search?q={q}")
}

pub fn chart_path(chart: &Chart) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("repo", &chart.repo)
        .append_pair("name", &chart.name)
        .append_pair("version", &chart.version)
        .finish();
    format!("/api/apps/chart?{query}")
}

fn apps(ctx: &Ctx<'_>) -> PaneModel {
    let base = settings_base(PaneId::Apps);
    let mut sections = Vec::new();
    let apps = ctx.res.get("/api/apps");
    let available = apps
        .data()
        .is_some_and(|a| a["available"] == Value::Bool(true));
    let records: Vec<&Value> = if available {
        apps.data()
            .and_then(|a| a["apps"].as_array())
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    if let Resource::Error { message, .. } = apps {
        sections.push(
            Section::new("installed")
                .title(t("install.installed"))
                .notice(
                    Tone::Crit,
                    None,
                    tr!("install.listFailed", message = message.clone()),
                ),
        );
    } else if !records.is_empty() {
        let rows = records
            .iter()
            .map(|record| {
                let release = record["release"].as_str().unwrap_or("-");
                let phase = record["phase"].as_str().unwrap_or("running");
                let run_as = record["runAs"].as_str().unwrap_or("notshared");
                let mut detail = vec![tr!(
                    "install.runsAs",
                    chart = record["chart"]["name"].as_str().unwrap_or("-"),
                    version = record["chart"]["version"].as_str().unwrap_or("-"),
                    user = run_as
                )];
                if phase == "failed" {
                    detail.push(record["message"].as_str().unwrap_or_default().to_string());
                }
                if phase == "running" && !record["appPort"].is_null() {
                    let port = match &record["frontPort"] {
                        Value::Number(n) => n.to_string(),
                        other => other.as_str().unwrap_or("-").to_string(),
                    };
                    detail.push(format!("{}:{port}", host_name(&ctx.host.url)));
                }
                let mut row = Row::new(
                    format!("app-{release}"),
                    format!("{release}  ·  {}", t(phase_key(phase))),
                )
                .detail(join_nonempty(&detail, "\n"))
                .control(Control::Run {
                    label: t("install.remove"),
                    disabled: phase == "installing" || phase == "removing",
                    confirm: Some(Confirm::Ready(ConfirmSpec {
                        title: tr!("install.removeTitle", name = release),
                        body: vec![tr!(
                            "install.removeBody",
                            path = format!("/home/{run_as}/data/apps/{release}")
                        )],
                        confirm: t("install.removeConfirm"),
                        cancel: t("install.cancel"),
                    })),
                    job: Some(Job::RemoveApp(release.to_string())),
                });
                if phase == "failed" {
                    row = row.tone(Tone::Crit);
                }
                row
            })
            .collect();
        sections.push(
            Section::new("installed")
                .title(t("install.installed"))
                .rows(rows)
                .caption(t("install.installedCaption")),
        );
    }

    let search = ctx.search;
    let query = match search {
        SearchState::Results { query, .. } | SearchState::Searching(query) => query.clone(),
        _ => String::new(),
    };
    let mut rows = vec![
        Row::new("search", t("panes.apps.searchLabel")).control(Control::Edit {
            value: query,
            prompt: t("panes.apps.searchPlaceholder"),
            masked: false,
            disabled: *search == SearchState::Unsupported,
            target: EditTarget::AppSearch,
        }),
    ];
    let mut find = Section::new("find").title(t("panes.apps.findMore"));
    match search {
        SearchState::Searching(_) => {
            rows.push(Row::new("searching", t("panes.apps.looking")).tone(Tone::Muted))
        }
        SearchState::Unsupported => {
            find = find.notice(Tone::Muted, None, t("panes.apps.unsupported"))
        }
        SearchState::Failed(message) => {
            find = find.notice(
                Tone::Crit,
                None,
                tr!("panes.apps.failed", message = message.clone()),
            )
        }
        SearchState::Results { apps: hits, .. } if hits.is_empty() => {
            rows.push(Row::new("empty", t("panes.apps.empty")).tone(Tone::Muted))
        }
        SearchState::Results { apps: hits, .. } => {
            for hit in hits {
                let existing = hit.chart.as_ref().and_then(|chart| {
                    records.iter().find(|r| {
                        r["chart"]["name"] == chart.name.as_str()
                            && r["chart"]["repo"] == chart.repo.as_str()
                    })
                });
                let mut row = Row::new(
                    format!("hit-{}-{}", hit.source, hit.id),
                    match &hit.version {
                        Some(v) => format!("{}  {v}", hit.name),
                        None => hit.name.clone(),
                    },
                )
                .detail(join_nonempty(
                    &[
                        hit.summary.clone().unwrap_or_default(),
                        format!("{}: {}", t("panes.apps.col.source"), hit.source),
                    ],
                    "\n",
                ));
                match (&hit.chart, existing) {
                    (Some(chart), None) if available => {
                        row = row.control(Control::Run {
                            label: t("install.button"),
                            disabled: false,
                            confirm: Some(Confirm::Prepare(Prepare::Install(chart.clone()))),
                            job: None,
                        });
                    }
                    (_, Some(existing)) => {
                        row = row.value(t(phase_key(existing["phase"].as_str().unwrap_or(""))));
                    }
                    _ => {}
                }
                rows.push(row);
            }
        }
        SearchState::Idle => {}
    }
    let footer = match search {
        SearchState::Results { sources, .. } if !sources.is_empty() => tr!(
            "panes.apps.footerSearched",
            count = sources.len(),
            sources = sources.join(", "),
            footer = t("panes.apps.footer")
        ),
        _ => t("panes.apps.footer"),
    };
    let no_cluster = apps
        .data()
        .is_some_and(|a| a["available"] == Value::Bool(false))
        && matches!(search, SearchState::Results { .. });
    sections.push(find.rows(rows).caption(if no_cluster {
        format!("{footer} {}", t("install.noCluster"))
    } else {
        footer
    }));
    base.with(sections)
}

/// The confirmation an install asks, once the box has named the release.
pub fn install_confirm(release: &str, shared_available: bool) -> ConfirmSpec {
    let mut body = vec![tr!(
        "install.pointUser",
        user = "notshared",
        path = format!("/home/notshared/data/apps/{release}")
    )];
    if shared_available {
        body.push(t("tui.apps.sharedInWeb"));
    }
    ConfirmSpec {
        title: tr!("install.confirmTitle", name = release),
        body,
        confirm: t("install.confirm"),
        cancel: t("install.cancel"),
    }
}

// ── Network ──────────────────────────────────────────────────────────────

fn network(ctx: &Ctx<'_>) -> PaneModel {
    let base = settings_base(PaneId::Network);
    if let Some(waiting) = form_loading(ctx.form) {
        return base.with(waiting);
    }
    let draft = ctx.form.draft.as_ref().expect("form is ready");
    let problems = ctx.form.problems();
    let mut called = Row::new("called", t("panes.network.called")).control(Control::Edit {
        value: draft.host_name.clone(),
        prompt: t("panes.network.called"),
        masked: false,
        disabled: false,
        target: EditTarget::Setting(SettingKey::HostName),
    });
    if let Some(p) = problems.host_name {
        called = called.detail(p).tone(Tone::Crit);
    }
    base.with(vec![
        Section::new("name")
            .title(t("panes.network.name"))
            .rows(vec![
                called,
                Row::new("reached", t("panes.network.reachedAt")).value(
                    if draft.host_name.is_empty() {
                        "-".to_string()
                    } else {
                        format!("{}.local", draft.host_name)
                    },
                ),
            ])
            .caption(t("panes.network.nameCaption")),
        Section::new("reach")
            .title(t("panes.network.reaching"))
            .rows(vec![Row::new("outside", t("panes.network.outside"))
                .control(Control::Toggle {
                    on: draft.proxy_enable,
                    disabled: false,
                    key: SettingKey::ProxyEnable,
                })])
            .caption(t("panes.network.reachCaption")),
    ])
}

// ── Security ─────────────────────────────────────────────────────────────

fn security(ctx: &Ctx<'_>) -> PaneModel {
    let base = settings_base(PaneId::Security);
    if let Some(waiting) = form_loading(ctx.form) {
        return base.with(waiting);
    }
    let draft = ctx.form.draft.as_ref().expect("form is ready");
    let switch = |key: SettingKey, name: &str| {
        Row::new(name, t(&format!("panes.security.{name}.title")))
            .detail(t(&format!("panes.security.{name}.detail")))
            .control(Control::Toggle {
                on: draft.get_bool(key),
                disabled: false,
                key,
            })
    };
    let mut section = Section::new("protections")
        .title(t("panes.security.group"))
        .rows(vec![
            switch(SettingKey::HardeningUsbguard, "usbguard"),
            switch(SettingKey::HardeningMalloc, "malloc"),
            switch(SettingKey::HardeningApparmor, "apparmor"),
            switch(SettingKey::HardeningNosmt, "nosmt"),
        ])
        .caption(t("panes.security.caption"));
    if ctx.form.no_tpm() {
        section = section.notice(Tone::Warn, Some(t("ui.noTpm.title")), t("ui.noTpm.body"));
    }
    base.with(vec![section])
}

// ── Advanced: every option, read only ────────────────────────────────────

fn describe_value(editor: &Value, value: &Value) -> String {
    match value {
        Value::Null => t("advanced.none"),
        Value::Bool(b) => t(if *b { "advanced.on" } else { "advanced.off" }),
        Value::Number(n) => {
            if editor["kind"] == "float" && n.is_i64() {
                format!("{n}.0")
            } else {
                n.to_string()
            }
        }
        Value::String(s) if s.is_empty() => "\"\"".to_string(),
        Value::String(s) => s.clone(),
        Value::Array(items) if items.is_empty() => t("advanced.none"),
        Value::Array(items) => items
            .iter()
            .map(|v| describe_value(editor, v))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(o) => o
            .get("package")
            .or_else(|| o.get("nix"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

fn advanced(ctx: &Ctx<'_>) -> PaneModel {
    let base = PaneModel::new(
        t("settings.panes.advanced.label"),
        t("tui.advanced.summary"),
    );
    if let Some(waiting) = form_loading(ctx.form) {
        return base.with(waiting);
    }
    let options = ctx
        .form
        .options
        .as_ref()
        .filter(|o| o["available"] == Value::Bool(true));
    let Some(options) = options else {
        return base.with(vec![Section::new("none").notice(
            Tone::Muted,
            None,
            t("advanced.unavailable"),
        )]);
    };
    let mut groups: Vec<(String, Vec<&Value>)> = Vec::new();
    for option in options["options"].as_array().into_iter().flatten() {
        if option["hidden"] == Value::Bool(true) {
            continue;
        }
        let group = option["group"].as_str().unwrap_or("general").to_string();
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, list)) => list.push(option),
            None => groups.push((group, vec![option])),
        }
    }
    groups.sort_by(|(a, _), (b, _)| match (a.as_str(), b.as_str()) {
        ("general", "general") => std::cmp::Ordering::Equal,
        ("general", _) => std::cmp::Ordering::Less,
        (_, "general") => std::cmp::Ordering::Greater,
        _ => a.cmp(b),
    });
    let mut sections: Vec<Section> = groups
        .into_iter()
        .map(|(group, list)| {
            let rows = list
                .into_iter()
                .map(|option| {
                    let name = option["name"].as_str().unwrap_or("-");
                    let mut badges = Vec::new();
                    if option["danger"] == Value::Bool(true) {
                        badges.push(t("advanced.badge.danger"));
                    }
                    match option["fixed"].as_str() {
                        Some("installer") => badges.push(t("advanced.badge.installer")),
                        Some(owner) => badges.push(tr!("advanced.badge.fixed", owner = owner)),
                        None => {}
                    }
                    if !option["set"].is_null() {
                        badges.push(t("advanced.badge.set"));
                    }
                    let title = if badges.is_empty() {
                        format!("losos.{name}")
                    } else {
                        format!("losos.{name}  [{}]", badges.join(", "))
                    };
                    let default = match option["defaultText"].as_str() {
                        Some(text) => tr!("advanced.row.default", text = text),
                        None => t("advanced.row.noDefault"),
                    };
                    Row::new(format!("option-{name}"), title)
                        .detail(join_nonempty(
                            &[
                                option["description"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                default,
                            ],
                            "\n",
                        ))
                        .value(describe_value(&option["editor"], &option["current"]))
                        .tone(Tone::Ink)
                })
                .collect();
            Section::new(format!("group-{group}"))
                .title(if group == "general" {
                    t("advanced.group.general")
                } else {
                    format!("losos.{group}")
                })
                .rows(rows)
        })
        .collect();
    let stray: Vec<Row> = options["stray"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            let key = s["key"].as_str().unwrap_or("-");
            Row::new(format!("stray-{key}"), format!("losos.{key}"))
                .value(s["value"].as_str().unwrap_or_default())
                .tone(Tone::Crit)
        })
        .collect();
    if !stray.is_empty() {
        sections.push(
            Section::new("stray")
                .title(t("advanced.stray.title"))
                .rows(stray)
                .caption(t("advanced.stray.caption")),
        );
    }
    sections.push(Section::new("caption").caption(t("advanced.caption")));
    base.with(sections)
}

// ── History ──────────────────────────────────────────────────────────────

fn history(ctx: &Ctx<'_>) -> PaneModel {
    let base = settings_base(PaneId::History);
    let config = ctx.res.get("/api/config");
    if let Some(waiting) = loading_section("config", config) {
        return base.with(vec![waiting]);
    }
    let c = config.data().cloned().unwrap_or_default();
    let repo = &c["repository"];
    let head = &c["head"];
    let url = &ctx.host.url;
    let mut sections = Vec::new();
    let mut repo_rows = Vec::new();
    if repo.is_object() {
        repo_rows.push(
            Row::new("git", t("history.repo.onGit"))
                .detail(format!(
                    "{}/{}",
                    repo["owner"].as_str().unwrap_or("-"),
                    repo["name"].as_str().unwrap_or("-")
                ))
                .value(format!("{url}{}", repo["url"].as_str().unwrap_or_default())),
        );
    }
    if head.is_object() {
        repo_rows.push(
            Row::new("branch", t("history.repo.branch"))
                .value(head["branch"].as_str().unwrap_or("-"))
                .tone(Tone::Ink),
        );
    }
    repo_rows.push(
        Row::new("head", t("history.repo.head"))
            .value(head["sha"].as_str().map_or("-".to_string(), short))
            .tone(Tone::Ink),
    );
    if repo.is_object() {
        repo_rows.push(Row::new("clone", t("history.repo.clone")).detail(format!(
            "{url}{}.git",
            repo["url"].as_str().unwrap_or_default()
        )));
    }
    sections.push(
        Section::new("repo")
            .title(t("history.repo.title"))
            .rows(repo_rows)
            .caption(t(if repo.is_object() {
                "history.repo.caption"
            } else {
                "history.repo.off"
            })),
    );

    let state_line = |prefix: &str, report: &Value| {
        join_nonempty(
            &[
                t(&format!(
                    "{prefix}.{}",
                    report["state"].as_str().unwrap_or("idle")
                )),
                report["syncedAt"]
                    .as_str()
                    .map(|at| tr!("history.sync.lastAt", when = format_iso(at)))
                    .unwrap_or_default(),
            ],
            "  ·  ",
        )
    };

    if repo.is_object() {
        let sync = &c["sync"];
        sections.push(
            Section::new("sync").title(t("history.sync.title")).row(
                Row::new("sync", state_line("history.sync.state", sync))
                    .detail(join_nonempty(
                        &[
                            sync["detail"].as_str().unwrap_or_default().to_string(),
                            sync["remoteHead"]
                                .as_str()
                                .map(|h| tr!("history.sync.remote", sha = short(h)))
                                .unwrap_or_default(),
                        ],
                        "\n",
                    ))
                    .control(Control::Run {
                        label: t("history.sync.now"),
                        disabled: false,
                        confirm: None,
                        job: Some(Job::SyncConfig),
                    }),
            ),
        );
    }

    let upstream = &c["upstream"];
    let report = &upstream["report"];
    let mut upstream_rows = Vec::new();
    if let Some(source) = upstream["url"].as_str() {
        upstream_rows.push(
            Row::new("source", t("history.update.source"))
                .detail(source)
                .value(upstream["branch"].as_str().unwrap_or("-"))
                .tone(Tone::Ink),
        );
    }
    upstream_rows.push(
        Row::new("report", state_line("history.update.state", report)).detail(join_nonempty(
            &[
                report["detail"].as_str().unwrap_or_default().to_string(),
                report["remoteHead"]
                    .as_str()
                    .map(|h| tr!("history.update.remote", sha = short(h)))
                    .unwrap_or_default(),
            ],
            "\n",
        )),
    );
    sections.push(
        Section::new("update")
            .title(t("history.update.title"))
            .rows(upstream_rows)
            .caption(t(if upstream["url"].is_string() {
                "history.update.caption"
            } else {
                "history.update.off"
            })),
    );

    let log = c["log"].as_array().cloned().unwrap_or_default();
    let commits = if log.is_empty() {
        vec![Row::new("empty", t("history.commits.empty")).tone(Tone::Muted)]
    } else {
        log.iter()
            .map(|entry| {
                let sha = entry["sha"].as_str().unwrap_or_default();
                Row::new(
                    format!("commit-{sha}"),
                    entry["subject"].as_str().unwrap_or_default(),
                )
                .detail(entry["when"].as_str().map(format_iso).unwrap_or_default())
                .value(short(sha))
            })
            .collect()
    };
    sections.push(
        Section::new("commits")
            .title(t("history.commits.title"))
            .rows(commits)
            .caption(t("history.commits.caption")),
    );
    base.with(sections)
}

// ── Backup ───────────────────────────────────────────────────────────────

fn backup(ctx: &Ctx<'_>) -> PaneModel {
    let base = settings_base(PaneId::Backup);
    let view = match ctx.res.get("/api/backup") {
        Resource::Loading => {
            return base.with(vec![
                Section::new("l").row(Row::new("l", t("panes.backup.loading")).tone(Tone::Muted))
            ])
        }
        Resource::Error { message, .. } => {
            return base.with(vec![Section::new("e").row(
                Row::new("e", t("panes.backup.unreachable"))
                    .detail(message.clone())
                    .tone(Tone::Crit),
            )])
        }
        Resource::Ok(v) => v,
    };
    let target = &view["target"];
    let has_target = target.is_object();
    let mut sections = Vec::new();
    let target_rows = if has_target {
        let bucket = target["bucket"].as_str().unwrap_or("-");
        let prefix = target["prefix"].as_str().unwrap_or_default();
        let class = target["storageClass"].as_str().unwrap_or("standard");
        vec![
            Row::new("endpoint", t("panes.backup.target.endpoint"))
                .value(target["endpoint"].as_str().unwrap_or("-"))
                .tone(Tone::Ink),
            Row::new("bucket", t("panes.backup.target.bucket"))
                .value(if prefix.is_empty() {
                    bucket.to_string()
                } else {
                    format!("{bucket}/{prefix}")
                })
                .tone(Tone::Ink),
            Row::new("class", t("panes.backup.target.class"))
                .value(if class == "standard" {
                    t("panes.backup.target.classStandard")
                } else {
                    class.to_string()
                })
                .tone(Tone::Ink),
            Row::new("key", t("panes.backup.target.key"))
                .value(target["accessKeyId"].as_str().unwrap_or("-"))
                .tone(Tone::Ink),
        ]
    } else {
        vec![Row::new("none", t("tui.backup.noTarget")).tone(Tone::Muted)]
    };
    sections.push(
        Section::new("target")
            .title(t("panes.backup.target.title"))
            .rows(target_rows)
            .caption(t("panes.backup.target.caption")),
    );
    let job = Some(&view["job"]).filter(|j| j["kind"] == "backup");
    let job_state = job.and_then(|j| j["state"].as_str());
    let last = &view["last"];
    let mut last_row = Row::new("last", t("panes.backup.runs.last")).detail(if last.is_object() {
        tr!(
            "panes.backup.runs.lastDetail",
            size = format_bytes(last["bytes"].as_f64()),
            count = last["files"].as_f64().unwrap_or(0.0)
        )
    } else {
        t("panes.backup.runs.never")
    });
    if let Some(time) = last["time"].as_f64() {
        last_row = last_row.value(format_unix(time));
    }
    let mut runs = vec![last_row];
    if job_state == Some("running") {
        runs.push(
            Row::new("running", t("panes.backup.runs.running"))
                .detail(t("panes.backup.runs.runningDetail"))
                .tone(Tone::Accent),
        );
    }
    if job_state == Some("failed") {
        runs.push(
            Row::new("failed", t("panes.backup.runs.failed"))
                .detail(
                    job.and_then(|j| j["message"].as_str())
                        .unwrap_or_default()
                        .to_string(),
                )
                .tone(Tone::Crit),
        );
    }
    let mut now = Row::new("now", t("panes.backup.runs.now")).control(Control::Run {
        label: t("panes.backup.runs.button"),
        disabled: !has_target || job_state == Some("running"),
        confirm: None,
        job: Some(Job::RunBackup),
    });
    if !has_target {
        now = now.detail(t("panes.backup.runs.needsTarget"));
    }
    runs.push(now);
    sections.push(
        Section::new("runs")
            .title(t("panes.backup.runs.title"))
            .rows(runs)
            .caption(t("panes.backup.runs.caption")),
    );
    sections.push(Section::new("web").caption(t("tui.backup.inWeb")));
    base.with(sections)
}

// ── About ────────────────────────────────────────────────────────────────

fn about(ctx: &Ctx<'_>) -> PaneModel {
    let base = settings_base(PaneId::About);
    if let Some(waiting) = form_loading(ctx.form) {
        return base.with(waiting);
    }
    let saved = ctx.form.saved.as_ref().expect("form is ready");
    let url = &ctx.host.url;
    // The scheme this client reached the box over, as the web page takes
    // its own.
    let by_name = format!(
        "{}://{}.local",
        if url.starts_with("https://") {
            "https"
        } else {
            "http"
        },
        saved.host_name
    );
    let storage_key = if !saved.sharing_my_storage {
        "panes.about.storageLocal"
    } else if saved.cluster_enable {
        "panes.about.storageShared"
    } else {
        "panes.about.storageWaiting"
    };
    let lent = saved.cluster_enable && saved.share_compute;
    let yes_no = |b: bool| {
        t(if b {
            "panes.about.yes"
        } else {
            "panes.about.no"
        })
    };
    let mut box_rows = vec![
        Row::new("called", t("panes.about.called"))
            .value(saved.host_name.clone())
            .tone(Tone::Ink),
        Row::new("reached", t("panes.about.reachedAt"))
            .value(url.clone())
            .tone(Tone::Ink),
    ];
    if by_name != *url {
        box_rows.push(
            Row::new("byname", t("panes.about.byName"))
                .value(by_name)
                .tone(Tone::Ink),
        );
    }
    box_rows.push(
        Row::new("storage", t("panes.about.storage"))
            .value(t(storage_key))
            .tone(Tone::Accent),
    );
    box_rows.push(
        Row::new("outside", t("panes.about.reachableOutside")).value(yes_no(saved.proxy_enable)),
    );
    base.with(vec![
        Section::new("box")
            .title(t("panes.about.thisBox"))
            .rows(box_rows)
            .caption(tr!(
                "panes.about.addressCaption",
                name = format!("{}.local", saved.host_name)
            )),
        Section::new("handbook")
            .title(t("panes.about.handbook"))
            .row(
                Row::new("handbook", t("panes.about.handbook"))
                    .value(format!("{url}/handbook/"))
                    .tone(Tone::Accent),
            )
            .caption(t("panes.about.handbookCaption")),
        Section::new("spare")
            .title(t("panes.about.spareTime"))
            .rows(vec![
                Row::new("mesh", t("panes.about.onMesh")).value(t(if saved.cluster_enable {
                    "panes.about.joined"
                } else {
                    "panes.about.notJoined"
                })),
                Row::new("lent", t("panes.about.lentOut")).value(if lent {
                    format!(
                        "{}-{}",
                        saved.compute_window_start, saved.compute_window_end
                    )
                } else {
                    t("panes.about.never")
                }),
            ])
            .caption(if lent {
                describe_window(&saved.compute_window_start, &saved.compute_window_end)
            } else {
                t("panes.about.idle")
            }),
        Section::new("current")
            .title(t("panes.about.current"))
            .rows(vec![
                Row::new("updates", t("panes.about.updates"))
                    .value(tr!("panes.about.everyNightAt", time = "03:00")),
                Row::new("restarts", t("panes.about.restarts"))
                    .value(tr!("panes.about.everyNightAt", time = "00:07")),
            ])
            .caption(t("panes.about.caption")),
        terminal_section(ctx.host),
    ])
}

/// The client's own facts, the last section of About.
pub fn terminal_section(host: &HostRecord) -> Section {
    Section::new("terminal")
        .title(t("tui.about.title"))
        .rows(vec![
            Row::new("client", t("tui.about.client"))
                .value(format!("losos {VERSION}"))
                .tone(Tone::Ink),
            Row::new("hostkey", t("tui.about.hostKey"))
                .detail(group_fingerprint(&host.fingerprint)),
            Row::new("userkey", t("tui.about.userKey"))
                .detail(group_fingerprint(&host.key_fingerprint)),
        ])
        .caption(t("tui.about.webOnly"))
}

/// Pad a cell of a table, for the edge's tenant list.
pub(crate) fn cell(text: &str, n: usize) -> String {
    pad(text, n)
}

// ── This computer ──────────────────────────────────────────────────────────

fn computer(ctx: &Ctx<'_>) -> PaneModel {
    use crate::computer::Service;
    let mut model = PaneModel::new(t("tui.computer.pane"), t("tui.computer.summary"));
    let local = ctx.local;
    if !local.available {
        model.sections.push(Section::new("computer").notice(
            Tone::Warn,
            None,
            t("tui.computer.noKeychain"),
        ));
        return model;
    }
    let row = |key: &str, service: Service, done: &Option<crate::computer::Credential>| {
        let (title, detail) = match service {
            Service::Cloud => ("tui.computer.cloud", "tui.computer.cloudDetail"),
            Service::Git => ("tui.computer.git", "tui.computer.gitDetail"),
        };
        let (detail, label) = match done {
            Some(c) => (
                tr!("tui.computer.setUpAt", location = &c.location),
                t("tui.computer.setUpAgain"),
            ),
            None => (
                format!("{} {}", t("tui.computer.notSetUp"), t(detail)),
                t("tui.computer.setUp"),
            ),
        };
        Row::new(key, t(title))
            .detail(detail)
            .control(Control::Ask {
                label,
                prompt: t("tui.computer.passwordFor"),
                disabled: false,
                target: EditTarget::Computer(service),
            })
    };
    model.sections.push(
        Section::new("computer")
            .rows(vec![
                row("cloud", Service::Cloud, &local.cloud),
                row("git", Service::Git, &local.git),
            ])
            .caption(tr!("tui.computer.caption", device = &local.device)),
    );
    model
}
