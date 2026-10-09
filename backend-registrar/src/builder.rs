//! The widget builder: a Claude agent writes a widget for a box's owner, and
//! the owner pays for the tokens it used plus the operator's markup.
//!
//! A hand-written widget (`backend/src/look.rs`) is HTML, style and script
//! that the admin page draws in a sandboxed frame. Writing one takes some
//! knowledge of that frame, so the builder lets the owner describe the widget
//! in a sentence and has an agent write it. The result is source text and
//! nothing more: it lands in the same editor and the same frame as a widget
//! typed by hand, with no privilege a typed one lacks.
//!
//! # Where things run
//!
//! The agent is a Claude Managed Agent. Anthropic runs its loop and gives each
//! build a disposable container to write and check the file in; this edge
//! starts one session per build, watches it, reads back the file the agent
//! wrote to `/mnt/session/outputs/`, and deletes the session. The agent and
//! its environment are created once by `losos-registrar builder-setup`, and
//! their ids are configuration (`losos.edge.builder.{agentId,environmentId}`).
//!
//! The Anthropic API key lives on the edge only, sealed with `systemd-creds`
//! and read per request from the registrar's credential directory. A box never
//! holds it: lososd relays the owner's request with the box's proxy token like
//! the market, and the browser talks to lososd only.
//!
//! # What the owner pays
//!
//! A prepaid balance, in the market's currency, topped up through the market's
//! Stripe account with a fixed pack (`--builder-packs`), never an amount the
//! request names. A build is charged what its session actually used, token by
//! token, at the model's list price plus the markup (20% by default): see
//! [`Pricing::charge`]. Before a build starts, the balance is turned into a
//! **session budget**, the platform's hard spend cap, so a build can never run
//! up more than the owner has. The cap is enforced before each model request,
//! so the last request may cross it; the balance can then dip a few cents
//! below zero and the next top-up covers it.
//!
//! Container time ($0.08 an hour of activity at list price) is not passed on:
//! a build runs for minutes, and the markup covers it. Neither is the
//! currency spread: `--builder-usd-rate` converts list prices, which are in US
//! dollars, into the market currency, and its default of 1.0 overstates the
//! price in euros slightly rather than understating it.
//!
//! # Shape
//!
//! Like `market.rs`: the pricing, the state machine and the webhook handling
//! are pure functions over [`BuilderState`], unit-tested below; [`Builder`]
//! adds the persisted store, the Anthropic client and the watchers. The
//! integration tests drive the real routes against a stub of the Anthropic API.

use std::collections::{BTreeMap, HashSet};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::action::Action;
use crate::fsutil::atomic_write;
use crate::market::{now_secs, MarketError};
use crate::stripe_gate::{CreditRequest, GateClient};

/// The markup on the model's list price, in basis points: 20%.
pub const DEFAULT_MARKUP_BPS: u32 = 2_000;
/// The most an operator may configure. A typo (`--builder-markup-bps 20000`)
/// would otherwise triple every owner's bill.
pub const MAX_MARKUP_BPS: u32 = 10_000;
/// The model the agent runs on.
pub const MODEL: &str = "claude-opus-5-5";
/// List prices of [`MODEL`], in millionths of a US dollar per million tokens:
/// $4 in, $20 out, $0.20 for a cache read and $5 for a cache write.
pub const MODEL_PRICES: Prices = Prices {
    input: 4_000_000,
    output: 20_000_000,
    cache_read: 200_000,
    cache_write: 5_000_000,
};
/// Where the Anthropic API is; overridden only by tests.
pub const DEFAULT_API: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";
const BETA: &str = "managed-agents-2026-04-01";
/// One US dollar in the market currency, in millionths: 1.0.
pub const DEFAULT_USD_RATE_PPM: u64 = 1_000_000;
/// The most one build may spend, in US cents of list price, whatever the
/// balance. Writing one widget takes a few dozen cents; this bounds a run that
/// goes round in circles.
pub const DEFAULT_MAX_BUILD_CENTS: u64 = 300;
/// The smallest budget a build is started with. Below it the agent would stop
/// before writing anything, and the owner would pay for nothing.
pub const MIN_BUILD_CENTS: u64 = 20;
/// Credit packs offered when the operator names none, in minor units.
pub const DEFAULT_PACKS: [u64; 3] = [500, 1_000, 2_000];

/// Longest description an owner may give, in characters.
pub const MAX_PROMPT_CHARS: usize = 2_000;
/// Largest widget, in bytes: the box's own limit (`look.rs`).
pub const MAX_SOURCE_BYTES: usize = 64 * 1024;
/// Longest summary kept from the agent, in characters.
const MAX_SUMMARY_CHARS: usize = 600;
/// Largest summary file read back.
const MAX_SUMMARY_BYTES: u64 = 8 * 1024;
/// Builds running across the whole edge at once.
const MAX_RUNNING: usize = 8;
/// Builds kept per box, newest first; older ones are only history.
const KEEP_BUILDS: usize = 20;
/// How long a finished build keeps its source. The owner takes it into the
/// editor right away; after that the box holds the widget, not the edge.
const SOURCE_RETENTION_SECS: u64 = 7 * 24 * 3600;
/// A build that has run this long is interrupted.
const BUILD_DEADLINE_SECS: u64 = 15 * 60;
/// A build whose session cannot be read back for this long is given up: the
/// owner is not charged, and the operator finds it in the Console.
const SETTLE_GIVE_UP_SECS: u64 = 24 * 3600;
/// A credit Checkout lasts 31 minutes (Stripe's minimum is 30, plus transit);
/// an order that hears nothing for three days more is dropped.
const CHECKOUT_TTL_SECS: u64 = 31 * 60;
const PENDING_HOLD_SECS: u64 = 3 * 24 * 3600;
/// Budget for one Anthropic request.
const CLAUDE_TIMEOUT: Duration = Duration::from_secs(20);
const STORE_FILE_MODE: u32 = 0o600;

/// What the agent is told, once, when `builder-setup` makes it. The
/// per-build request goes in the session's first message.
pub const SYSTEM_PROMPT: &str = r#"You write widgets for the LosOS admin page. A LosOS box is a small home server; its owner sees a board of tiles on the admin page, and a widget is one tile.

What you deliver
- Write the widget to /mnt/session/outputs/widget.html. It is a fragment, not a document: optional <style>, the markup, optional <script>. No <html>, <head> or <body>.
- Keep it under 60 KiB. Everything inline: no external scripts, stylesheets or fonts.
- Write one or two plain sentences to /mnt/session/outputs/summary.txt saying what the widget shows and anything the owner should know. Write them in the language the request names.
- When you change an existing widget, keep what the owner did not ask to change.
- Work in /mnt/session/outputs only. You have no network. If node is installed you may syntax-check your script with it; nothing else needs checking.

Where it runs
- The admin page loads the fragment into a sandboxed iframe (sandbox="allow-scripts", no allow-same-origin). It is an opaque origin: no cookies, no localStorage or sessionStorage (they throw), no alert/confirm/prompt, no form submission, no popups.
- The tile is as wide as half or all of the board row (roughly 280 to 900 px) and grows to the height of what you draw. Do not set a fixed page height; do not make the body scroll.
- fetch() to public HTTPS APIs works when they answer CORS requests from any origin (Access-Control-Allow-Origin: *). Requests carry Origin: null. Prefer APIs that need no key; never ask the owner for a key in the widget.
- The box's colours are CSS variables on <html> that follow its light and dark themes: --ground, --surface, --sunk, --ink, --muted, --faint, --line, --hair, --accent, --accent-wash, --ok, --warn, --crit, --font-ui, --font-code. Use them instead of fixed colours, so the tile matches the page in both themes.

The losos object
- losos.metric(name) returns a promise of one of the box's readings. Names and shapes:
  - "storage.bytes": { usedBytes, totalBytes, reserveBytes, freeBytes, usedRatio (0-1), source }, numbers may be null
  - "uptime.days": { days: [{ date, value }], upRatio (0-1), outages, longestOutageSeconds, observedDays, source }
  - "mesh.compute": { joined, sharingStorage, sharingCompute, windowStart, windowEnd ("HH:MM"), windowHours, givenSeconds, takenSeconds, source }
  - "apps.list": { apps: [{ id, name, path, reachable, onMesh }], hostName }
  - "rebuilds.recent": { entries: [{ job, state, message, startedAt, seenAt }], current, busy }
  - "box.settings": { hostName, https, mode ("local" or "mesh"), gpu, proxy, clusterEnable, shareCompute, computeWindowStart, computeWindowEnd }
  - "box.status": { state ("idle", "building", "done", "failed"), progress, message, job }
  A reading the box does not report rejects the promise; show a dash, not an error.
- losos.theme is "light" or "dark"; losos.onTheme(fn) calls fn when it changes.
- losos.lang is "en", "sk" or "de", the language of the admin page. Show text in that language when the widget has words.
- losos.resize() asks the board to re-measure after a change it cannot see.
- There is nothing else: a widget can read, never change, the box.

Style
- Calm and small: the board already has a heading per tile. Use --font-ui, 13 to 14 px body text, tabular numbers for figures.
- Handle loading and failure in the tile itself."#;

// ── pricing ──────────────────────────────────────────────────────────────

/// List prices, in millionths of a US dollar per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Prices {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

/// What a session used, as the session object reports it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

impl Usage {
    /// The `usage` of a session object. A field it does not carry counts as
    /// zero, so a reading never fails on a shape it half recognises.
    #[must_use]
    pub fn of_session(session: &Value) -> Self {
        let u = &session["usage"];
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        Self {
            input_tokens: n("input_tokens"),
            output_tokens: n("output_tokens"),
            cache_creation_input_tokens: n("cache_creation_input_tokens"),
            cache_read_input_tokens: n("cache_read_input_tokens"),
        }
    }
}

/// How list prices become the owner's price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pricing {
    pub prices: Prices,
    pub markup_bps: u32,
    /// One US dollar in the market currency, in millionths.
    pub usd_rate_ppm: u64,
}

/// 10^20: millionths of a dollar per million tokens (10^12) to cents (10^-2),
/// times the basis-point (10^4) and parts-per-million (10^6) scales, all
/// cancelled at once so nothing is rounded before the end.
const CHARGE_SCALE: u128 = 100_000_000_000_000_000_000;

impl Pricing {
    fn factor(&self) -> u128 {
        u128::from(10_000 + self.markup_bps) * u128::from(self.usd_rate_ppm)
    }

    /// What `usage` costs the owner, in minor units of the market currency:
    /// every token at its list price plus the markup, rounded up to the cent.
    #[must_use]
    pub fn charge(&self, usage: &Usage) -> u64 {
        let p = &self.prices;
        let list = u128::from(usage.input_tokens) * u128::from(p.input)
            + u128::from(usage.output_tokens) * u128::from(p.output)
            + u128::from(usage.cache_read_input_tokens) * u128::from(p.cache_read)
            + u128::from(usage.cache_creation_input_tokens) * u128::from(p.cache_write);
        let charge = (list * self.factor()).div_ceil(CHARGE_SCALE);
        u64::try_from(charge).unwrap_or(u64::MAX)
    }

    /// The owner's price for a million tokens at list price `per_mtok`, in
    /// minor units, rounded half up. For display.
    #[must_use]
    pub fn per_million(&self, per_mtok: u64) -> u64 {
        // millionths of a dollar to cents is 10^4; markup 10^4; rate 10^6.
        let scale: u128 = 100_000_000_000_000;
        let v = (u128::from(per_mtok) * self.factor() + scale / 2) / scale;
        u64::try_from(v).unwrap_or(u64::MAX)
    }

    /// What a cap of `cents` of list-price spend costs the owner at most, in
    /// minor units, rounded up. For display.
    #[must_use]
    pub fn cap_price(&self, cents: u64) -> u64 {
        let v = (u128::from(cents) * self.factor()).div_ceil(10_000 * 1_000_000);
        u64::try_from(v).unwrap_or(u64::MAX)
    }

    /// How much list-price spend, in US cents, a balance of `minor` units
    /// pays for. Rounded down; zero for a balance at or below zero.
    #[must_use]
    pub fn covers_cents(&self, minor: i64) -> u64 {
        let Ok(minor) = u128::try_from(minor) else {
            return 0;
        };
        // minor / ((1 + markup) * rate), with both scales.
        let v = minor * 10_000 * 1_000_000 / self.factor().max(1);
        u64::try_from(v).unwrap_or(u64::MAX)
    }
}

// ── state ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    #[error("the widget builder is not offered on this edge")]
    Unconfigured,
    #[error("{0}")]
    Invalid(&'static str),
    #[error("no such build")]
    NotFound,
    #[error("{0}")]
    Conflict(&'static str),
    /// The Anthropic API failed or refused. The text is for the log.
    #[error("anthropic: {0}")]
    Upstream(String),
    /// Starting a top-up failed; see [`MarketError`].
    #[error(transparent)]
    Market(#[from] MarketError),
    #[error("builder store: {0}")]
    Store(String),
}

impl From<io::Error> for BuilderError {
    fn from(e: io::Error) -> Self {
        BuilderError::Store(e.to_string())
    }
}

impl From<serde_json::Error> for BuilderError {
    fn from(e: serde_json::Error) -> Self {
        BuilderError::Store(e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildStatus {
    Running,
    Done,
    Failed,
}

/// Why a build produced no widget. A fixed set, so the admin page can say it
/// in the owner's language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildFault {
    /// The agent finished without writing `widget.html`.
    NoWidget,
    /// It wrote one larger than the box keeps, or not UTF-8.
    Unusable,
    /// The session could not be read back; nothing was charged.
    Upstream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Build {
    pub id: String,
    pub tenant: String,
    pub status: BuildStatus,
    pub prompt: String,
    /// Whether the owner asked to change a widget rather than start one.
    #[serde(default)]
    pub revision: bool,
    pub created_at: u64,
    #[serde(default)]
    pub finished_at: Option<u64>,
    #[serde(default)]
    pub session_id: Option<String>,
    /// The session's spend cap, in US cents of list price.
    pub budget_cents: u64,
    #[serde(default)]
    pub usage: Usage,
    /// What the owner was charged, in minor units.
    #[serde(default)]
    pub charged: u64,
    /// Whether the build used (nearly) all of its cap, so the agent may have
    /// been stopped before it was done.
    #[serde(default)]
    pub at_limit: bool,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub fault: Option<BuildFault>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CreditStatus {
    Pending,
    Paid,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditOrder {
    pub id: String,
    pub tenant: String,
    pub amount: u64,
    pub currency: String,
    pub status: CreditStatus,
    #[serde(default)]
    pub session_id: Option<String>,
    pub created_at: u64,
    #[serde(default)]
    pub paid_at: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderState {
    /// Each box's balance, in minor units. May dip below zero by the last
    /// model request of a build; see the module docs.
    #[serde(default)]
    pub balances: BTreeMap<String, i64>,
    #[serde(default)]
    pub credits: BTreeMap<String, CreditOrder>,
    #[serde(default)]
    pub builds: BTreeMap<String, Build>,
    /// Sessions and files to delete on Anthropic's side, retried each tick
    /// until the API accepts.
    #[serde(default)]
    pub cleanup: Vec<Cleanup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Cleanup {
    Session { id: String },
    File { id: String },
}

/// A checked description: trimmed, within [`MAX_PROMPT_CHARS`], no control
/// characters but newlines and tabs.
///
/// # Errors
/// A sentence for a 400.
pub fn check_prompt(prompt: &str) -> Result<String, BuilderError> {
    let p = prompt.trim();
    if p.is_empty() {
        return Err(BuilderError::Invalid("describe the widget you want"));
    }
    if p.chars().count() > MAX_PROMPT_CHARS {
        return Err(BuilderError::Invalid(
            "the description is longer than 2000 characters",
        ));
    }
    if p.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Err(BuilderError::Invalid(
            "the description has a control character in it",
        ));
    }
    Ok(p.to_string())
}

/// The language a build's words are written in, from the admin page's.
#[must_use]
pub fn language(lang: Option<&str>) -> &'static str {
    match lang {
        Some("sk") => "Slovak",
        Some("de") => "German",
        _ => "English",
    }
}

/// The session's first message: the owner's request, and the widget to
/// change when there is one. The request is quoted, not obeyed as the edge's
/// own words; it is still the owner's intent, and the container it runs in
/// has nothing in it to protect.
#[must_use]
pub fn kickoff(prompt: &str, base: Option<&str>, lang: &str) -> String {
    let mut text = format!(
        "The owner of a LosOS box asked for this widget. Write the summary in {lang}.\n\n<request>\n{prompt}\n</request>\n"
    );
    if let Some(base) = base {
        text.push_str(
            "\nChange this widget, the one the owner has now, rather than starting over:\n\n<widget>\n",
        );
        text.push_str(base);
        text.push_str("\n</widget>\n");
    }
    text
}

/// Whether `tenant` may start a build now, and with what cap.
///
/// # Errors
/// [`BuilderError::Conflict`] while the box has a build running, the edge has
/// as many as it runs at once, or the balance covers less than a build needs.
pub fn admit(
    state: &BuilderState,
    tenant: &str,
    pricing: &Pricing,
    max_build_cents: u64,
) -> Result<u64, BuilderError> {
    let running = state
        .builds
        .values()
        .filter(|b| b.status == BuildStatus::Running);
    let mut total = 0;
    for b in running {
        if b.tenant == tenant {
            return Err(BuilderError::Conflict(
                "a widget is already being built for this box",
            ));
        }
        total += 1;
    }
    if total >= MAX_RUNNING {
        return Err(BuilderError::Conflict(
            "the builder is busy; try again in a few minutes",
        ));
    }
    let balance = state.balances.get(tenant).copied().unwrap_or(0);
    let budget = pricing.covers_cents(balance).min(max_build_cents);
    if budget < MIN_BUILD_CENTS {
        return Err(BuilderError::Conflict(
            "top up your balance before building a widget",
        ));
    }
    Ok(budget)
}

/// Keep the summary short and printable.
#[must_use]
pub fn clean_summary(raw: &str) -> Option<String> {
    let text: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= MAX_SUMMARY_CHARS {
        return Some(text);
    }
    let cut: String = text.chars().take(MAX_SUMMARY_CHARS - 1).collect();
    Some(format!("{}…", cut.trim_end()))
}

/// What a finished session left behind.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub usage: Usage,
    /// `usage.list_cost` in US cents, when the session reported it.
    pub list_cents: Option<u64>,
    pub widget: Option<Vec<u8>>,
    pub summary: Option<String>,
}

/// Close a running build: charge it and record what it made.
pub fn settle(state: &mut BuilderState, id: &str, outcome: Outcome, pricing: &Pricing, now: u64) {
    let Some(build) = state.builds.get_mut(id) else {
        return;
    };
    if build.status != BuildStatus::Running {
        return;
    }
    let charge = pricing.charge(&outcome.usage);
    build.usage = outcome.usage;
    build.charged = charge;
    build.finished_at = Some(now);
    // A cent of rounding either way: see `usage.list_cost` in the API docs.
    build.at_limit = outcome
        .list_cents
        .is_some_and(|c| c + 1 >= build.budget_cents);
    match outcome.widget {
        None => {
            build.status = BuildStatus::Failed;
            build.fault = Some(BuildFault::NoWidget);
        }
        Some(bytes) => match String::from_utf8(bytes) {
            Ok(source) if !source.trim().is_empty() && source.len() <= MAX_SOURCE_BYTES => {
                build.status = BuildStatus::Done;
                build.source = Some(source);
                build.summary = outcome.summary.as_deref().and_then(clean_summary);
            }
            Ok(source) if source.trim().is_empty() => {
                build.status = BuildStatus::Failed;
                build.fault = Some(BuildFault::NoWidget);
            }
            _ => {
                build.status = BuildStatus::Failed;
                build.fault = Some(BuildFault::Unusable);
            }
        },
    }
    let tenant = build.tenant.clone();
    let balance = state.balances.entry(tenant).or_insert(0);
    *balance = balance.saturating_sub(i64::try_from(charge).unwrap_or(i64::MAX));
}

/// Give up on a build whose session cannot be read: no charge.
pub fn abandon(state: &mut BuilderState, id: &str, now: u64) {
    if let Some(build) = state.builds.get_mut(id) {
        if build.status == BuildStatus::Running {
            build.status = BuildStatus::Failed;
            build.fault = Some(BuildFault::Upstream);
            build.finished_at = Some(now);
        }
    }
}

/// Apply one verified Stripe event. Returns whether anything changed.
///
/// Only a completed, paid session that matches a recorded credit order in
/// id, amount and currency adds to a balance, and only once. Every other
/// event, including every market event, is left alone: the market's own
/// handler sees the same delivery.
pub fn apply_event(state: &mut BuilderState, event: &Value, now: u64) -> bool {
    let kind = event["type"].as_str().unwrap_or("");
    let object = &event["data"]["object"];
    let Some(order_id) = object["client_reference_id"].as_str() else {
        return false;
    };
    let Some(order) = state.credits.get(order_id) else {
        return false;
    };
    let session = object["id"].as_str();
    match kind {
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
            if object["payment_status"].as_str() != Some("paid") {
                return false;
            }
            let matches = session.is_some_and(|s| s.starts_with("cs_"))
                && order
                    .session_id
                    .as_deref()
                    .is_none_or(|s| Some(s) == session)
                && object["amount_total"].as_u64() == Some(order.amount)
                && object["currency"]
                    .as_str()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&order.currency));
            if !matches {
                tracing::error!(
                    target: Action::Builder.target(),
                    "paid session for credit {order_id} does not match the recorded order; not crediting",
                );
                return false;
            }
            if order.status == CreditStatus::Paid {
                return false;
            }
            let (tenant, amount) = (order.tenant.clone(), order.amount);
            if let Some(order) = state.credits.get_mut(order_id) {
                order.status = CreditStatus::Paid;
                order.paid_at = Some(now);
                order.session_id = session.map(str::to_string);
            }
            let balance = state.balances.entry(tenant).or_insert(0);
            *balance = balance.saturating_add(i64::try_from(amount).unwrap_or(i64::MAX));
            true
        }
        "checkout.session.expired" => {
            let current = order.session_id.is_some() && order.session_id.as_deref() == session;
            if order.status == CreditStatus::Pending && current {
                if let Some(order) = state.credits.get_mut(order_id) {
                    order.status = CreditStatus::Expired;
                }
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Drop what is only history: credit orders that never paid, builds past the
/// newest [`KEEP_BUILDS`] of their box, and the source of builds older than
/// [`SOURCE_RETENTION_SECS`]. Running builds and paid orders stay. Returns
/// whether anything changed.
pub fn prune(state: &mut BuilderState, now: u64) -> bool {
    let before = state.clone();
    state.credits.retain(|_, o| match o.status {
        CreditStatus::Paid => true,
        CreditStatus::Pending => now < o.created_at + CHECKOUT_TTL_SECS + PENDING_HOLD_SECS,
        CreditStatus::Expired => now < o.created_at + PENDING_HOLD_SECS,
    });
    let mut per_tenant: BTreeMap<&str, Vec<(u64, &str)>> = BTreeMap::new();
    for b in state.builds.values() {
        if b.status != BuildStatus::Running {
            per_tenant
                .entry(&b.tenant)
                .or_default()
                .push((b.created_at, &b.id));
        }
    }
    let mut drop = HashSet::new();
    for list in per_tenant.values_mut() {
        list.sort_unstable_by(|a, b| b.cmp(a));
        for (_, id) in list.iter().skip(KEEP_BUILDS) {
            drop.insert((*id).to_string());
        }
    }
    state.builds.retain(|id, _| !drop.contains(id));
    for b in state.builds.values_mut() {
        if b.status != BuildStatus::Running
            && b.finished_at
                .is_some_and(|t| now >= t + SOURCE_RETENTION_SECS)
        {
            b.source = None;
        }
    }
    *state != before
}

// ── views ────────────────────────────────────────────────────────────────

/// The owner's price, as the admin page shows it.
#[derive(Debug, Serialize)]
pub struct PriceView {
    pub model: &'static str,
    /// Per million input / output tokens, in minor units, markup included.
    pub input_per_million: u64,
    pub output_per_million: u64,
    pub markup_percent: f64,
    /// The most one build may cost, in minor units.
    pub max_build: u64,
}

/// A build without its source, for the history.
#[derive(Debug, Serialize)]
pub struct BuildSummary {
    pub id: String,
    pub status: BuildStatus,
    pub prompt: String,
    pub revision: bool,
    pub created_at: u64,
    pub finished_at: Option<u64>,
    pub charged: u64,
    pub fault: Option<BuildFault>,
    pub has_source: bool,
}

impl From<&Build> for BuildSummary {
    fn from(b: &Build) -> Self {
        Self {
            id: b.id.clone(),
            status: b.status,
            prompt: b.prompt.clone(),
            revision: b.revision,
            created_at: b.created_at,
            finished_at: b.finished_at,
            charged: b.charged,
            fault: b.fault,
            has_source: b.source.is_some(),
        }
    }
}

/// A build as its box sees it, source included.
#[derive(Debug, Serialize)]
pub struct BuildView {
    #[serde(flatten)]
    pub summary: BuildSummary,
    pub usage: Usage,
    pub at_limit: bool,
    pub source: Option<String>,
    pub notes: Option<String>,
}

impl From<&Build> for BuildView {
    fn from(b: &Build) -> Self {
        Self {
            summary: b.into(),
            usage: b.usage,
            at_limit: b.at_limit,
            source: b.source.clone(),
            notes: b.summary.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AccountView {
    pub currency: String,
    pub balance: i64,
    pub packs: Vec<u64>,
    pub price: PriceView,
    /// Whether the balance covers a build now.
    pub can_build: bool,
    pub builds: Vec<BuildSummary>,
}

#[derive(Debug, Serialize)]
pub struct CreditView {
    pub order_id: String,
    pub checkout_url: String,
    pub amount: u64,
    pub currency: String,
}

// ── the Anthropic API ────────────────────────────────────────────────────

/// The few Managed Agents and Files calls a build makes, over plain REST: the
/// crate already speaks HTTP through reqwest, and there is no Rust SDK.
///
/// Like the Stripe client, it holds the key's path, never the key: each call
/// reads it and puts it in that request's header only.
#[derive(Clone)]
pub struct Claude {
    http: reqwest::Client,
    api: reqwest::Url,
    key_file: String,
}

/// A session, as far as a build cares.
#[derive(Debug, Clone)]
pub struct SessionState {
    pub status: String,
    pub usage: Usage,
    pub list_cents: Option<u64>,
}

/// A file a session wrote.
#[derive(Debug, Clone)]
pub struct OutputFile {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub created_at: String,
}

impl Claude {
    /// # Errors
    /// When `api` is not a base URL, or is plain http to anything but this
    /// machine (the test stub): the key travels in every request.
    pub fn new(api: &str, key_file: &str) -> Result<Self, BuilderError> {
        let api = reqwest::Url::parse(api)
            .ok()
            .filter(|u| !u.cannot_be_a_base())
            .ok_or(BuilderError::Invalid("the Anthropic API is not a base URL"))?;
        let loopback = api.scheme() == "http"
            && matches!(api.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if api.scheme() != "https" && !loopback {
            return Err(BuilderError::Invalid(
                "the Anthropic API must use https:// to protect the key in transit",
            ));
        }
        let http = reqwest::Client::builder()
            .timeout(CLAUDE_TIMEOUT)
            .user_agent(concat!("losos-registrar/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| BuilderError::Upstream(format!("build client: {e}")))?;
        Ok(Self {
            http,
            api,
            key_file: key_file.to_string(),
        })
    }

    /// The key, read now. A missing file means the builder is not set up on
    /// this edge (503), not that Anthropic failed.
    async fn key(&self) -> Result<String, BuilderError> {
        let raw = match tokio::fs::read_to_string(&self.key_file).await {
            Ok(raw) => raw,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                tracing::error!(target: Action::Builder.target(), "key file {} is absent", self.key_file);
                return Err(BuilderError::Unconfigured);
            }
            Err(e) => return Err(BuilderError::Store(format!("read key: {e}"))),
        };
        let key = raw.trim();
        if !key.starts_with("sk-ant-") || key.chars().any(|c| c.is_whitespace() || c.is_control()) {
            tracing::error!(
                target: Action::Builder.target(),
                "key file {} does not hold an Anthropic API key",
                self.key_file,
            );
            return Err(BuilderError::Unconfigured);
        }
        Ok(key.to_string())
    }

    /// Whether the key file holds a usable key.
    ///
    /// # Errors
    /// [`BuilderError::Unconfigured`] when it does not.
    pub async fn check_key(&self) -> Result<(), BuilderError> {
        self.key().await.map(|_| ())
    }

    fn url(&self, segments: &[&str]) -> reqwest::Url {
        let mut url = self.api.clone();
        if let Ok(mut path) = url.path_segments_mut() {
            path.pop_if_empty().extend(segments);
        }
        url
    }

    async fn send(
        &self,
        method: reqwest::Method,
        url: reqwest::Url,
        body: Option<&Value>,
        beta: bool,
    ) -> Result<reqwest::Response, BuilderError> {
        let key = self.key().await?;
        let mut req = self
            .http
            .request(method, url)
            .header("x-api-key", key)
            .header("anthropic-version", API_VERSION);
        if beta {
            req = req.header("anthropic-beta", BETA);
        }
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| BuilderError::Upstream(format!("request: {e}")))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let text = resp.text().await.unwrap_or_default();
        let text: String = text.chars().take(300).collect();
        Err(BuilderError::Upstream(format!("{status}: {text}")))
    }

    async fn json(
        &self,
        method: reqwest::Method,
        segments: &[&str],
        body: Option<&Value>,
    ) -> Result<Value, BuilderError> {
        let resp = self.send(method, self.url(segments), body, true).await?;
        if resp.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        resp.json()
            .await
            .map_err(|e| BuilderError::Upstream(format!("answer is not JSON: {e}")))
    }

    /// Create a session that starts working at once, under a hard cap.
    ///
    /// # Errors
    /// The API's refusal or failure.
    pub async fn create_session(
        &self,
        agent_id: &str,
        environment_id: &str,
        build_id: &str,
        tenant: &str,
        text: &str,
        budget_cents: u64,
    ) -> Result<String, BuilderError> {
        let body = json!({
            "agent": agent_id,
            "environment_id": environment_id,
            "title": format!("Widget {build_id}"),
            "metadata": { "losos_build": build_id, "losos_box": tenant },
            "budget": {
                "type": "limit",
                "max_list_cost": { "amount": budget_cents.to_string(), "currency": "USD" },
            },
            "initial_events": [{
                "type": "user.message",
                "content": [{ "type": "text", "text": text }],
            }],
        });
        let session = self
            .json(reqwest::Method::POST, &["v1", "sessions"], Some(&body))
            .await?;
        session["id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| BuilderError::Upstream("session had no id".to_string()))
    }

    /// # Errors
    /// The API's refusal or failure.
    pub async fn session(&self, id: &str) -> Result<SessionState, BuilderError> {
        let s = self
            .json(reqwest::Method::GET, &["v1", "sessions", id], None)
            .await?;
        Ok(SessionState {
            status: s["status"].as_str().unwrap_or("").to_string(),
            usage: Usage::of_session(&s),
            list_cents: s["usage"]["list_cost"]["amount"]
                .as_str()
                .and_then(|a| a.parse().ok()),
        })
    }

    /// Stop the agent where it is.
    ///
    /// # Errors
    /// The API's refusal or failure.
    pub async fn interrupt(&self, id: &str) -> Result<(), BuilderError> {
        let body = json!({ "events": [{ "type": "user.interrupt" }] });
        self.json(
            reqwest::Method::POST,
            &["v1", "sessions", id, "events"],
            Some(&body),
        )
        .await
        .map(|_| ())
    }

    /// The files the session wrote to `/mnt/session/outputs/`.
    ///
    /// # Errors
    /// The API's refusal or failure.
    pub async fn outputs(&self, session_id: &str) -> Result<Vec<OutputFile>, BuilderError> {
        let mut url = self.url(&["v1", "files"]);
        url.query_pairs_mut()
            .append_pair("scope_id", session_id)
            .append_pair("limit", "100");
        let list: Value = self
            .send(reqwest::Method::GET, url, None, true)
            .await?
            .json()
            .await
            .map_err(|e| BuilderError::Upstream(format!("file list is not JSON: {e}")))?;
        Ok(list["data"]
            .as_array()
            .map(|files| {
                files
                    .iter()
                    .filter_map(|f| {
                        Some(OutputFile {
                            id: f["id"].as_str()?.to_string(),
                            filename: f["filename"].as_str()?.to_string(),
                            size: f["size_bytes"].as_u64().unwrap_or(0),
                            created_at: f["created_at"].as_str().unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// A file's bytes, refused past `max` before or after the download.
    ///
    /// # Errors
    /// The API's refusal or failure, or a file larger than `max`.
    pub async fn download(&self, file: &OutputFile, max: u64) -> Result<Vec<u8>, BuilderError> {
        if file.size > max {
            return Err(BuilderError::Invalid("file too large"));
        }
        let bytes = self
            .send(
                reqwest::Method::GET,
                self.url(&["v1", "files", &file.id, "content"]),
                None,
                false,
            )
            .await?
            .bytes()
            .await
            .map_err(|e| BuilderError::Upstream(format!("download: {e}")))?;
        if bytes.len() as u64 > max {
            return Err(BuilderError::Invalid("file too large"));
        }
        Ok(bytes.to_vec())
    }

    /// # Errors
    /// The API's refusal or failure.
    pub async fn delete(&self, what: &Cleanup) -> Result<(), BuilderError> {
        let (segments, beta) = match what {
            Cleanup::Session { id } => (vec!["v1", "sessions", id.as_str()], true),
            Cleanup::File { id } => (vec!["v1", "files", id.as_str()], false),
        };
        match self
            .send(reqwest::Method::DELETE, self.url(&segments), None, beta)
            .await
        {
            Ok(_) => Ok(()),
            // Already gone is done.
            Err(BuilderError::Upstream(e)) if e.starts_with("404") => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Create the agent and its environment, or update an agent that exists.
    /// What `builder-setup` runs.
    ///
    /// # Errors
    /// The API's refusal or failure.
    pub async fn setup(
        &self,
        agent_id: Option<&str>,
        environment_id: Option<&str>,
    ) -> Result<(String, String), BuilderError> {
        let environment = match environment_id {
            Some(id) => id.to_string(),
            None => {
                let body = environment_spec();
                let env = self
                    .json(reqwest::Method::POST, &["v1", "environments"], Some(&body))
                    .await?;
                env["id"]
                    .as_str()
                    .ok_or_else(|| BuilderError::Upstream("environment had no id".to_string()))?
                    .to_string()
            }
        };
        let body = agent_spec();
        let agent = match agent_id {
            Some(id) => {
                self.json(reqwest::Method::POST, &["v1", "agents", id], Some(&body))
                    .await?
            }
            None => {
                self.json(reqwest::Method::POST, &["v1", "agents"], Some(&body))
                    .await?
            }
        };
        let agent = agent["id"]
            .as_str()
            .ok_or_else(|| BuilderError::Upstream("agent had no id".to_string()))?
            .to_string();
        Ok((agent, environment))
    }
}

/// The agent `builder-setup` creates: the model, the system prompt, and the
/// built-in tools to write and check files, with the web tools off. Every
/// tool call runs without asking: nobody watches a build, and the container
/// holds nothing but the widget.
#[must_use]
pub fn agent_spec() -> Value {
    json!({
        "name": "LosOS widget builder",
        "description": "Writes a widget for the LosOS admin page from the owner's description.",
        "model": { "id": MODEL, "effort": "medium" },
        "system": SYSTEM_PROMPT,
        "tools": [{
            "type": "agent_toolset_20260401",
            "default_config": { "permission_policy": { "type": "always_allow" } },
            "configs": [
                { "name": "web_fetch", "enabled": false },
                { "name": "web_search", "enabled": false },
            ],
        }],
    })
}

/// The environment: a cloud container with no network at all.
#[must_use]
pub fn environment_spec() -> Value {
    json!({
        "name": "losos-widget-builder",
        "config": {
            "type": "cloud",
            "networking": {
                "type": "limited",
                "allow_package_managers": false,
                "allow_mcp_servers": false,
            },
        },
    })
}

// ── the builder ──────────────────────────────────────────────────────────

/// `--builder-*` settings. Present only when the operator enabled it.
#[derive(Debug, Clone)]
pub struct BuilderOpts {
    /// `builder.json`: balances, credit orders and builds. 0600.
    pub state_file: String,
    /// The Anthropic API key, read per request.
    pub key_file: String,
    pub api: String,
    pub agent_id: String,
    pub environment_id: String,
    pub markup_bps: u32,
    pub usd_rate_ppm: u64,
    pub packs: Vec<u64>,
    pub max_build_cents: u64,
    /// How often a running session is looked at.
    pub poll: Duration,
}

pub struct Builder {
    opts: BuilderOpts,
    claude: Claude,
    /// The market's gate, for top-ups, and its settings.
    gate: GateClient,
    currency: String,
    return_url: String,
    path: PathBuf,
    state: Mutex<BuilderState>,
    /// Builds a watcher task is following right now.
    watching: std::sync::Mutex<HashSet<String>>,
}

fn random_id(prefix: &str) -> Result<String, BuilderError> {
    let mut bytes = [0u8; 12];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| BuilderError::Store("no randomness".to_string()))?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{prefix}_{hex}"))
}

impl Builder {
    /// Open the store. A missing file is an empty one; an unparseable one is
    /// an error, because starting empty would forget what owners have paid.
    ///
    /// # Errors
    /// An unreadable store or a bad API URL.
    pub async fn open(
        opts: BuilderOpts,
        gate_socket: &str,
        currency: &str,
        return_url: &str,
    ) -> Result<Self, BuilderError> {
        let path = PathBuf::from(&opts.state_file);
        let state = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => BuilderState::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            claude: Claude::new(&opts.api, &opts.key_file)?,
            gate: GateClient::new(gate_socket),
            currency: currency.to_string(),
            return_url: return_url.to_string(),
            opts,
            path,
            state: Mutex::new(state),
            watching: std::sync::Mutex::new(HashSet::new()),
        })
    }

    #[must_use]
    pub fn pricing(&self) -> Pricing {
        Pricing {
            prices: MODEL_PRICES,
            markup_bps: self.opts.markup_bps,
            usd_rate_ppm: self.opts.usd_rate_ppm,
        }
    }

    /// # Errors
    /// [`BuilderError::Unconfigured`] when the key file is missing or wrong.
    pub async fn check_key(&self) -> Result<(), BuilderError> {
        self.claude.check_key().await
    }

    async fn commit(
        &self,
        live: &mut BuilderState,
        next: BuilderState,
    ) -> Result<(), BuilderError> {
        let bytes = serde_json::to_vec_pretty(&next)?;
        atomic_write(&self.path, &bytes, STORE_FILE_MODE).await?;
        *live = next;
        Ok(())
    }

    /// The box's balance, prices and recent builds.
    pub async fn account(&self, tenant: &str) -> AccountView {
        let pricing = self.pricing();
        let state = self.state.lock().await;
        let mut builds: Vec<&Build> = state
            .builds
            .values()
            .filter(|b| b.tenant == tenant)
            .collect();
        builds.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        AccountView {
            currency: self.currency.clone(),
            balance: state.balances.get(tenant).copied().unwrap_or(0),
            packs: self.opts.packs.clone(),
            price: PriceView {
                model: MODEL,
                input_per_million: pricing.per_million(MODEL_PRICES.input),
                output_per_million: pricing.per_million(MODEL_PRICES.output),
                markup_percent: f64::from(self.opts.markup_bps) / 100.0,
                max_build: pricing.cap_price(self.opts.max_build_cents),
            },
            can_build: admit(&state, tenant, &pricing, self.opts.max_build_cents).is_ok(),
            builds: builds.into_iter().map(BuildSummary::from).collect(),
        }
    }

    /// One build, source included, if it is this box's.
    ///
    /// # Errors
    /// [`BuilderError::NotFound`] for an unknown id or another box's build.
    pub async fn build(&self, tenant: &str, id: &str) -> Result<BuildView, BuilderError> {
        let state = self.state.lock().await;
        state
            .builds
            .get(id)
            .filter(|b| b.tenant == tenant)
            .map(BuildView::from)
            .ok_or(BuilderError::NotFound)
    }

    /// Start a Checkout for one of the operator's credit packs.
    ///
    /// # Errors
    /// [`BuilderError::Invalid`] for an amount that is not a pack; the gate's
    /// and Stripe's faults otherwise.
    pub async fn top_up(&self, tenant: &str, amount: u64) -> Result<CreditView, BuilderError> {
        if !self.opts.packs.contains(&amount) {
            return Err(BuilderError::Invalid("that is not one of the credit packs"));
        }
        let order_id = random_id("cr")?;
        let now = now_secs();
        {
            let mut state = self.state.lock().await;
            let pending = state
                .credits
                .values()
                .filter(|o| o.tenant == tenant && o.status == CreditStatus::Pending)
                .filter(|o| now < o.created_at + CHECKOUT_TTL_SECS)
                .count();
            if pending >= 3 {
                return Err(BuilderError::Conflict(
                    "finish or close the top-ups already open first",
                ));
            }
            let mut next = state.clone();
            next.credits.insert(
                order_id.clone(),
                CreditOrder {
                    id: order_id.clone(),
                    tenant: tenant.to_string(),
                    amount,
                    currency: self.currency.clone(),
                    status: CreditStatus::Pending,
                    session_id: None,
                    created_at: now,
                    paid_at: None,
                },
            );
            self.commit(&mut state, next).await?;
        }
        let made = self
            .gate
            .credit_checkout(CreditRequest {
                order_id: order_id.clone(),
                appliance_id: tenant.to_string(),
                amount,
                return_url: self.return_url.clone(),
                expires_at: now + CHECKOUT_TTL_SECS,
            })
            .await;
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        match made {
            Ok((session_id, checkout_url)) => {
                if let Some(o) = next.credits.get_mut(&order_id) {
                    o.session_id = Some(session_id);
                }
                self.commit(&mut state, next).await?;
                tracing::info!(target: Action::Builder.target(), "top-up {order_id} for {tenant}: {amount} {}", self.currency);
                Ok(CreditView {
                    order_id,
                    checkout_url,
                    amount,
                    currency: self.currency.clone(),
                })
            }
            Err(e) => {
                next.credits.remove(&order_id);
                // The order is useless without a session; a failed write
                // here leaves a pending row that `prune` drops.
                let _ = self.commit(&mut state, next).await;
                Err(e.into())
            }
        }
    }

    /// Start a build: check the balance, open a session capped at what it
    /// covers, record it, and follow it.
    ///
    /// # Errors
    /// A refusal from [`check_prompt`] or [`admit`], or the API's failure.
    pub async fn start(
        self: &Arc<Self>,
        tenant: &str,
        prompt: &str,
        base: Option<&str>,
        lang: Option<&str>,
    ) -> Result<BuildView, BuilderError> {
        let prompt = check_prompt(prompt)?;
        let base = match base.map(str::trim).filter(|b| !b.is_empty()) {
            Some(b) if b.len() > MAX_SOURCE_BYTES => {
                return Err(BuilderError::Invalid(
                    "the widget to change is larger than a box keeps",
                ))
            }
            other => other,
        };
        let pricing = self.pricing();
        let id = random_id("bld")?;
        let now = now_secs();
        // Admitted and recorded under one lock, so two requests from one box
        // cannot both start a build on the same balance.
        let budget = {
            let mut state = self.state.lock().await;
            let budget = admit(&state, tenant, &pricing, self.opts.max_build_cents)?;
            let mut next = state.clone();
            next.builds.insert(
                id.clone(),
                Build {
                    id: id.clone(),
                    tenant: tenant.to_string(),
                    status: BuildStatus::Running,
                    prompt: prompt.clone(),
                    revision: base.is_some(),
                    created_at: now,
                    finished_at: None,
                    session_id: None,
                    budget_cents: budget,
                    usage: Usage::default(),
                    charged: 0,
                    at_limit: false,
                    source: None,
                    summary: None,
                    fault: None,
                },
            );
            self.commit(&mut state, next).await?;
            budget
        };
        let text = kickoff(&prompt, base, language(lang));
        let made = self
            .claude
            .create_session(
                &self.opts.agent_id,
                &self.opts.environment_id,
                &id,
                tenant,
                &text,
                budget,
            )
            .await;
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        match made {
            Ok(session_id) => {
                if let Some(b) = next.builds.get_mut(&id) {
                    b.session_id = Some(session_id);
                }
                self.commit(&mut state, next).await?;
                tracing::info!(target: Action::Builder.target(), "build {id} for {tenant} started with a {budget} cent cap");
            }
            Err(e) => {
                // No session, nothing used, nothing to charge.
                next.builds.remove(&id);
                let _ = self.commit(&mut state, next).await;
                return Err(e);
            }
        }
        let view = state.builds.get(&id).map(BuildView::from);
        drop(state);
        self.follow(&id);
        view.ok_or(BuilderError::NotFound)
    }

    /// Follow every running build no task follows yet: after a restart, and
    /// after a watcher gave up for the moment. Called each reconcile tick.
    pub async fn resume(self: &Arc<Self>) {
        let ids: Vec<String> = {
            let state = self.state.lock().await;
            state
                .builds
                .values()
                .filter(|b| b.status == BuildStatus::Running)
                .map(|b| b.id.clone())
                .collect()
        };
        for id in ids {
            self.follow(&id);
        }
    }

    fn follow(self: &Arc<Self>, id: &str) {
        {
            let Ok(mut watching) = self.watching.lock() else {
                return;
            };
            if !watching.insert(id.to_string()) {
                return;
            }
        }
        let me = Arc::clone(self);
        let id = id.to_string();
        tokio::spawn(async move {
            if let Err(e) = me.watch(&id).await {
                tracing::warn!(target: Action::Builder.target(), "build {id}: {e}; picked up again on the next tick");
            }
            if let Ok(mut watching) = me.watching.lock() {
                watching.remove(&id);
            }
        });
    }

    /// Wait for a build's session to settle, then read it back and charge it.
    async fn watch(&self, id: &str) -> Result<(), BuilderError> {
        let (session_id, created_at) = {
            let state = self.state.lock().await;
            match state.builds.get(id) {
                Some(b) if b.status == BuildStatus::Running => (b.session_id.clone(), b.created_at),
                _ => return Ok(()),
            }
        };
        let Some(session_id) = session_id else {
            // The edge stopped between recording the build and opening its
            // session. A session may exist that this edge never learned the
            // id of; its cap bounds it, and the operator finds it by its
            // `losos_build` metadata. The owner is not charged.
            self.give_up(id).await?;
            return Ok(());
        };
        let mut interrupted = false;
        let mut last: Option<SessionState> = None;
        loop {
            tokio::time::sleep(self.opts.poll).await;
            let now = now_secs();
            let s = match self.claude.session(&session_id).await {
                Ok(s) => s,
                Err(e) if now >= created_at + SETTLE_GIVE_UP_SECS => {
                    tracing::error!(target: Action::Builder.target(), "build {id}: session {session_id} unreadable for a day ({e}); not charging");
                    self.give_up(id).await?;
                    return Ok(());
                }
                Err(e) => return Err(e),
            };
            if s.status == "terminated" {
                break;
            }
            // The session goes idle for a moment between tool calls too, so
            // a build is over once two readings a poll apart agree: idle,
            // and nothing more used.
            let settled = s.status == "idle"
                && last
                    .as_ref()
                    .is_some_and(|l| l.status == "idle" && l.usage == s.usage);
            if settled {
                break;
            }
            if !interrupted && now >= created_at + BUILD_DEADLINE_SECS {
                tracing::warn!(target: Action::Builder.target(), "build {id} ran past its deadline; interrupting");
                self.claude.interrupt(&session_id).await?;
                interrupted = true;
            }
            last = Some(s);
        }
        self.collect(id, &session_id).await
    }

    /// Read a settled session back, charge the build, and queue the session
    /// and its files for deletion.
    async fn collect(&self, id: &str, session_id: &str) -> Result<(), BuilderError> {
        let s = self.claude.session(session_id).await?;
        let files = self.claude.outputs(session_id).await?;
        let newest = |name: &str| {
            files
                .iter()
                .filter(|f| f.filename == name || f.filename.ends_with(&format!("/{name}")))
                .max_by(|a, b| a.created_at.cmp(&b.created_at))
        };
        let widget = match newest("widget.html") {
            None => None,
            // One byte over the box's limit is enough to call it unusable.
            Some(f) => match self.claude.download(f, MAX_SOURCE_BYTES as u64 + 1).await {
                Ok(bytes) => Some(bytes),
                Err(BuilderError::Invalid(_)) => Some(vec![0; MAX_SOURCE_BYTES + 1]),
                Err(e) => return Err(e),
            },
        };
        let summary = match newest("summary.txt") {
            None => None,
            Some(f) => match self.claude.download(f, MAX_SUMMARY_BYTES).await {
                Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
                Err(BuilderError::Invalid(_)) => None,
                Err(e) => return Err(e),
            },
        };
        let outcome = Outcome {
            usage: s.usage,
            list_cents: s.list_cents,
            widget,
            summary,
        };
        let pricing = self.pricing();
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        settle(&mut next, id, outcome, &pricing, now_secs());
        next.cleanup.push(Cleanup::Session {
            id: session_id.to_string(),
        });
        next.cleanup
            .extend(files.iter().map(|f| Cleanup::File { id: f.id.clone() }));
        self.commit(&mut state, next).await?;
        if let Some(b) = state.builds.get(id) {
            tracing::info!(
                target: Action::Builder.target(),
                "build {id} for {} is {:?}: {} in, {} out, charged {} {}",
                b.tenant,
                b.status,
                b.usage.input_tokens + b.usage.cache_read_input_tokens + b.usage.cache_creation_input_tokens,
                b.usage.output_tokens,
                b.charged,
                self.currency,
            );
        }
        Ok(())
    }

    async fn give_up(&self, id: &str) -> Result<(), BuilderError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        abandon(&mut next, id, now_secs());
        self.commit(&mut state, next).await
    }

    /// Apply a verified Stripe event to the balances.
    ///
    /// # Errors
    /// A failed write; Stripe retries the delivery.
    pub async fn apply_event(&self, event: &Value) -> Result<(), BuilderError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        if apply_event(&mut next, event, now_secs()) {
            self.commit(&mut state, next).await?;
        }
        Ok(())
    }

    /// The tick's housekeeping: follow orphaned builds, delete finished
    /// sessions and files, drop old history.
    pub async fn tick(self: &Arc<Self>) {
        self.resume().await;
        let pending: Vec<Cleanup> = self.state.lock().await.cleanup.clone();
        let mut done = Vec::new();
        for item in pending {
            match self.claude.delete(&item).await {
                Ok(()) => done.push(item),
                Err(e) => {
                    tracing::warn!(target: Action::Builder.target(), "cleanup {item:?}: {e}");
                    // A session that is still settling refuses; try again
                    // next tick, and stop here so a dead API costs one call.
                    break;
                }
            }
        }
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        next.cleanup.retain(|c| !done.contains(c));
        let pruned = prune(&mut next, now_secs());
        if pruned || !done.is_empty() {
            if let Err(e) = self.commit(&mut state, next).await {
                tracing::error!(target: Action::Builder.target(), "builder store: {e}");
            }
        }
    }
}

/// `losos-registrar builder-setup`: make the agent and its environment, or
/// update the agent to this binary's prompt, and print the ids the edge's
/// configuration needs. Run once by the operator, with the same key file.
///
/// # Errors
/// A bad key file or the API's refusal.
pub async fn setup(opts: crate::opts::BuilderSetupOpts) -> miette::Result<()> {
    let claude = Claude::new(&opts.api, &opts.key_file).map_err(|e| miette::miette!("{e}"))?;
    let (agent, environment) = claude
        .setup(opts.agent_id.as_deref(), opts.environment_id.as_deref())
        .await
        .map_err(|e| miette::miette!("{e}"))?;
    println!("losos.edge.builder.agentId = \"{agent}\";");
    println!("losos.edge.builder.environmentId = \"{environment}\";");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pricing() -> Pricing {
        Pricing {
            prices: MODEL_PRICES,
            markup_bps: DEFAULT_MARKUP_BPS,
            usd_rate_ppm: DEFAULT_USD_RATE_PPM,
        }
    }

    fn running(id: &str, tenant: &str, budget: u64) -> Build {
        Build {
            id: id.to_string(),
            tenant: tenant.to_string(),
            status: BuildStatus::Running,
            prompt: "a clock".to_string(),
            revision: false,
            created_at: 100,
            finished_at: None,
            session_id: Some(format!("sesn_{id}")),
            budget_cents: budget,
            usage: Usage::default(),
            charged: 0,
            at_limit: false,
            source: None,
            summary: None,
            fault: None,
        }
    }

    #[test]
    fn a_million_tokens_cost_opus_list_price_plus_twenty_percent() {
        let p = pricing();
        let out = Usage {
            output_tokens: 1_000_000,
            ..Usage::default()
        };
        let input = Usage {
            input_tokens: 1_000_000,
            ..Usage::default()
        };
        // $20 and $4 list, plus 20%.
        assert_eq!(p.charge(&out), 2_400);
        assert_eq!(p.charge(&input), 480);
        assert_eq!(p.per_million(MODEL_PRICES.output), 2_400);
        assert_eq!(p.per_million(MODEL_PRICES.input), 480);
        // Cache reads and writes at their own list prices, marked up alike.
        let cache = Usage {
            cache_read_input_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
            ..Usage::default()
        };
        assert_eq!(p.charge(&cache), 24 + 600);
    }

    #[test]
    fn a_charge_rounds_up_to_the_cent_and_nothing_costs_nothing() {
        let p = pricing();
        assert_eq!(p.charge(&Usage::default()), 0);
        let one = Usage {
            output_tokens: 1,
            ..Usage::default()
        };
        assert_eq!(p.charge(&one), 1);
        // A typical build: 60k fresh in, 400k cached reads, 30k written to
        // cache, 25k out. List $0.24 + $0.08 + $0.15 + $0.50 = $0.97.
        let typical = Usage {
            input_tokens: 60_000,
            cache_read_input_tokens: 400_000,
            cache_creation_input_tokens: 30_000,
            output_tokens: 25_000,
        };
        assert_eq!(p.charge(&typical), 117); // 97 * 1.2 = 116.4, rounded up
    }

    #[test]
    fn the_rate_and_markup_scale_the_price() {
        let p = Pricing {
            markup_bps: 0,
            usd_rate_ppm: 920_000,
            ..pricing()
        };
        let out = Usage {
            output_tokens: 1_000_000,
            ..Usage::default()
        };
        assert_eq!(p.charge(&out), 1_840);
        assert_eq!(p.covers_cents(1_840), 2_000);
    }

    #[test]
    fn a_balance_becomes_a_cap_in_list_price_cents() {
        let p = pricing();
        assert_eq!(p.covers_cents(1_200), 1_000);
        assert_eq!(p.covers_cents(0), 0);
        assert_eq!(p.covers_cents(-50), 0);
        // What the cap allows never costs more than the balance.
        for balance in [21_i64, 99, 500, 1_234] {
            let cents = p.covers_cents(balance);
            let list_out = Usage {
                // $20 per million out: one cent is 500 tokens.
                output_tokens: cents * 500,
                ..Usage::default()
            };
            assert!(
                p.charge(&list_out) <= u64::try_from(balance).unwrap(),
                "{balance}"
            );
        }
    }

    #[test]
    fn a_build_is_admitted_only_with_balance_and_one_at_a_time() {
        let p = pricing();
        let mut s = BuilderState::default();
        assert!(matches!(
            admit(&s, "box", &p, DEFAULT_MAX_BUILD_CENTS),
            Err(BuilderError::Conflict(_))
        ));
        s.balances.insert("box".into(), 1_200);
        // The per-build cap wins over a larger balance.
        assert_eq!(admit(&s, "box", &p, DEFAULT_MAX_BUILD_CENTS).unwrap(), 300);
        s.balances.insert("box".into(), 120);
        assert_eq!(admit(&s, "box", &p, DEFAULT_MAX_BUILD_CENTS).unwrap(), 100);
        s.balances.insert("box".into(), 20);
        assert!(admit(&s, "box", &p, DEFAULT_MAX_BUILD_CENTS).is_err());
        s.balances.insert("box".into(), 500);
        s.builds.insert("b1".into(), running("b1", "box", 300));
        assert!(admit(&s, "box", &p, DEFAULT_MAX_BUILD_CENTS).is_err());
        s.balances.insert("other".into(), 500);
        assert!(admit(&s, "other", &p, DEFAULT_MAX_BUILD_CENTS).is_ok());
        for i in 0..MAX_RUNNING {
            s.builds.insert(
                format!("x{i}"),
                running(&format!("x{i}"), &format!("t{i}"), 1),
            );
        }
        assert!(admit(&s, "other", &p, DEFAULT_MAX_BUILD_CENTS).is_err());
    }

    #[test]
    fn settling_charges_the_balance_and_keeps_the_widget() {
        let p = pricing();
        let mut s = BuilderState::default();
        s.balances.insert("box".into(), 500);
        s.builds.insert("b1".into(), running("b1", "box", 300));
        let usage = Usage {
            output_tokens: 100_000,
            ..Usage::default()
        };
        settle(
            &mut s,
            "b1",
            Outcome {
                usage,
                list_cents: Some(200),
                widget: Some(b"<div>12:00</div>".to_vec()),
                summary: Some("A clock.\n\nIt ticks.".to_string()),
            },
            &p,
            200,
        );
        let b = &s.builds["b1"];
        assert_eq!(b.status, BuildStatus::Done);
        assert_eq!(b.charged, 240);
        assert_eq!(s.balances["box"], 260);
        assert_eq!(b.source.as_deref(), Some("<div>12:00</div>"));
        assert_eq!(b.summary.as_deref(), Some("A clock. It ticks."));
        assert!(!b.at_limit);
        // Settling twice charges once.
        settle(&mut s, "b1", Outcome::default(), &p, 300);
        assert_eq!(s.balances["box"], 260);
    }

    #[test]
    fn a_build_without_a_usable_widget_still_pays_for_what_it_used() {
        let p = pricing();
        for (widget, fault) in [
            (None, BuildFault::NoWidget),
            (Some(b"   ".to_vec()), BuildFault::NoWidget),
            (Some(vec![b'a'; MAX_SOURCE_BYTES + 1]), BuildFault::Unusable),
            (Some(vec![0xff, 0xfe]), BuildFault::Unusable),
        ] {
            let mut s = BuilderState::default();
            s.balances.insert("box".into(), 100);
            s.builds.insert("b".into(), running("b", "box", 80));
            settle(
                &mut s,
                "b",
                Outcome {
                    usage: Usage {
                        output_tokens: 25_000,
                        ..Usage::default()
                    },
                    list_cents: Some(80),
                    widget,
                    summary: None,
                },
                &p,
                1,
            );
            let b = &s.builds["b"];
            assert_eq!((b.status, b.fault), (BuildStatus::Failed, Some(fault)));
            assert!(b.at_limit);
            assert_eq!(s.balances["box"], 40);
        }
    }

    #[test]
    fn an_abandoned_build_costs_nothing() {
        let mut s = BuilderState::default();
        s.balances.insert("box".into(), 100);
        s.builds.insert("b".into(), running("b", "box", 80));
        abandon(&mut s, "b", 5);
        assert_eq!(s.builds["b"].fault, Some(BuildFault::Upstream));
        assert_eq!(s.balances["box"], 100);
    }

    fn credit(id: &str, amount: u64) -> CreditOrder {
        CreditOrder {
            id: id.to_string(),
            tenant: "box".to_string(),
            amount,
            currency: "eur".to_string(),
            status: CreditStatus::Pending,
            session_id: Some(format!("cs_{id}")),
            created_at: 0,
            paid_at: None,
        }
    }

    fn completed(order: &str, session: &str, amount: u64) -> Value {
        json!({ "type": "checkout.session.completed", "data": { "object": {
            "id": session, "client_reference_id": order, "payment_status": "paid",
            "amount_total": amount, "currency": "EUR",
        }}})
    }

    #[test]
    fn a_paid_top_up_credits_the_balance_once() {
        let mut s = BuilderState::default();
        s.credits.insert("cr_1".into(), credit("cr_1", 500));
        assert!(apply_event(&mut s, &completed("cr_1", "cs_cr_1", 500), 10));
        assert_eq!(s.balances["box"], 500);
        assert!(!apply_event(&mut s, &completed("cr_1", "cs_cr_1", 500), 11));
        assert_eq!(s.balances["box"], 500);
        assert_eq!(s.credits["cr_1"].status, CreditStatus::Paid);
    }

    #[test]
    fn a_top_up_that_does_not_match_its_order_credits_nothing() {
        let mut s = BuilderState::default();
        s.credits.insert("cr_1".into(), credit("cr_1", 500));
        for event in [
            completed("cr_1", "cs_cr_1", 5_000),
            completed("cr_1", "cs_other", 500),
            completed("cr_2", "cs_cr_1", 500),
            completed("ord_1", "cs_cr_1", 500),
            json!({ "type": "checkout.session.completed", "data": { "object": {
                "id": "cs_cr_1", "client_reference_id": "cr_1", "payment_status": "unpaid",
                "amount_total": 500, "currency": "eur" }}}),
            json!({ "type": "checkout.session.completed", "data": { "object": {
                "id": "cs_cr_1", "client_reference_id": "cr_1", "payment_status": "paid",
                "amount_total": 500, "currency": "usd" }}}),
        ] {
            assert!(!apply_event(&mut s, &event, 1), "{event}");
        }
        assert!(s.balances.is_empty());
        let mut expired = completed("cr_1", "cs_cr_1", 500);
        expired["type"] = "checkout.session.expired".into();
        assert!(apply_event(&mut s, &expired, 2));
        assert_eq!(s.credits["cr_1"].status, CreditStatus::Expired);
    }

    #[test]
    fn prune_keeps_running_builds_paid_orders_and_the_newest_history() {
        let mut s = BuilderState::default();
        for i in 0..(KEEP_BUILDS as u64 + 5) {
            let mut b = running(&format!("b{i:02}"), "box", 10);
            b.status = BuildStatus::Done;
            b.created_at = i;
            b.finished_at = Some(i);
            b.source = Some("x".into());
            s.builds.insert(b.id.clone(), b);
        }
        s.builds.insert("live".into(), running("live", "box", 10));
        let mut paid = credit("cr_p", 500);
        paid.status = CreditStatus::Paid;
        s.credits.insert("cr_p".into(), paid);
        s.credits.insert("cr_old".into(), credit("cr_old", 500));
        // A day on: the history is cut to the newest, the sources are kept,
        // and the unpaid top-up still waits for a late event.
        assert!(prune(&mut s, 24 * 3600));
        assert_eq!(s.builds.len(), KEEP_BUILDS + 1);
        assert!(s.builds.contains_key("live"));
        assert!(!s.builds.contains_key("b00"));
        assert!(s.builds["b24"].source.is_some());
        assert!(s.credits.contains_key("cr_old"));
        // A week on: sources are gone, the unpaid order is dropped, the paid
        // one stays.
        assert!(prune(&mut s, SOURCE_RETENTION_SECS + 100));
        assert!(s.builds.values().all(|b| b.source.is_none()));
        assert!(s.credits.contains_key("cr_p"));
        assert!(!s.credits.contains_key("cr_old"));
        assert!(!prune(&mut s, SOURCE_RETENTION_SECS + 200));
    }

    #[test]
    fn descriptions_are_checked_and_the_kickoff_quotes_them() {
        assert!(check_prompt("  ").is_err());
        assert!(check_prompt(&"a".repeat(MAX_PROMPT_CHARS + 1)).is_err());
        assert!(check_prompt("bad\u{7}bell").is_err());
        assert_eq!(
            check_prompt(" A clock\nwith seconds ").unwrap(),
            "A clock\nwith seconds"
        );
        let k = kickoff("A clock", None, language(Some("sk")));
        assert!(k.contains("<request>\nA clock\n</request>"));
        assert!(k.contains("Slovak"));
        assert!(!k.contains("<widget>"));
        let k = kickoff("Bigger", Some("<div>1</div>"), language(None));
        assert!(k.contains("<widget>\n<div>1</div>\n</widget>"));
        assert!(k.contains("English"));
    }

    #[test]
    fn summaries_are_flattened_and_cut() {
        assert_eq!(clean_summary(" \n "), None);
        assert_eq!(
            clean_summary("a\tb\u{1b}[31mc").as_deref(),
            Some("a b [31mc")
        );
        let long = clean_summary(&"word ".repeat(400)).unwrap();
        assert_eq!(long.chars().count(), MAX_SUMMARY_CHARS);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn the_session_usage_is_read_tolerantly() {
        let s = json!({ "usage": { "input_tokens": 5, "output_tokens": 7,
            "cache_read_input_tokens": 11, "list_cost": { "amount": "3", "currency": "USD" } } });
        assert_eq!(
            Usage::of_session(&s),
            Usage {
                input_tokens: 5,
                output_tokens: 7,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 11,
            }
        );
        assert_eq!(Usage::of_session(&json!({})), Usage::default());
    }

    #[test]
    fn the_agent_cannot_reach_the_web_and_runs_on_opus() {
        let a = agent_spec();
        assert_eq!(a["model"]["id"], MODEL);
        let configs = a["tools"][0]["configs"].as_array().unwrap();
        for tool in ["web_fetch", "web_search"] {
            assert!(configs
                .iter()
                .any(|c| c["name"] == tool && c["enabled"] == false));
        }
        let e = environment_spec();
        assert_eq!(e["config"]["networking"]["type"], "limited");
        assert_eq!(e["config"]["networking"]["allow_package_managers"], false);
        assert!(e["config"]["networking"].get("allowed_hosts").is_none());
    }

    #[test]
    fn the_api_must_be_https_except_on_loopback() {
        assert!(Claude::new("https://api.anthropic.com", "/k").is_ok());
        assert!(Claude::new("http://127.0.0.1:9/", "/k").is_ok());
        assert!(Claude::new("http://api.anthropic.com", "/k").is_err());
        assert!(Claude::new("not a url", "/k").is_err());
    }
}
