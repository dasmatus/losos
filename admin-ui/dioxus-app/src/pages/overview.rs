//! Overview/home parity scaffolding: facts/context fetching and block wiring.
//! Incremental but structurally correct (mirrors React `screens/Home.tsx`).

use dioxus::prelude::*;
use serde_json::Value;

use crate::actions::{self, Action};
use crate::app::ConfigSignal;
use crate::components::ui::{Card, Spinner};
use crate::store::BoxConfig;

use super::renderer::BlockProps;
use super::RenderedPage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Health {
    #[default]
    Unknown,
    Ok,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Availability {
    #[default]
    Unknown,
    Absent,
    Present,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppAvailability {
    pub files: Availability,
    pub code: Availability,
    pub measured: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HomeStorage {
    pub used_bytes: f64,
    pub total_bytes: f64,
    pub lent_bytes: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OverviewFacts {
    pub host_name: String,
    pub health: Health,
    pub status: Option<Value>,
    pub settings: Option<Value>,
    pub box_state: Option<Value>,
    pub storage: Option<HomeStorage>,
    pub availability: AppAvailability,
    pub apps: Option<Value>,
    pub overview: Option<Value>,
    /// Open assigned issues count (best-effort).
    pub assigned_issues: Option<u64>,
}

pub type OverviewSignal = Signal<OverviewFacts>;

pub fn provide_overview_context() -> OverviewSignal {
    use_context_provider(|| Signal::new(OverviewFacts::default()))
}

pub fn use_overview_context() -> OverviewSignal {
    use_context::<OverviewSignal>()
}

fn browser_host_name() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(win) = web_sys::window() {
            if let Ok(host) = win.location().hostname() {
                if !host.is_empty() {
                    return host;
                }
            }
        }
        "This box".to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "This box".to_string()
    }
}

fn host_name_from(cfg: &BoxConfig, facts: &OverviewFacts) -> String {
    if let Some(s) = facts.settings.as_ref() {
        for key in ["hostName", "hostname", "host_name"] {
            if let Some(v) = s.get(key).and_then(Value::as_str) {
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    if !cfg.host.trim().is_empty() {
        return cfg.host.trim().to_string();
    }
    browser_host_name()
}

async fn fetch_health(cfg: &BoxConfig) -> Health {
    if cfg.demo || cfg.host.trim().is_empty() {
        return if cfg.demo { Health::Ok } else { Health::Unknown };
    }
    match actions::make_api(cfg) {
        Ok(api) => match api.get_anonymous("/api/health").await {
            Ok(_v) => Health::Ok,
            Err(_) => Health::Down,
        },
        Err(_) => Health::Down,
    }
}

fn storage_from_value(v: &Value) -> Option<HomeStorage> {
    let used = v
        .get("usedBytes")
        .or_else(|| v.get("used_bytes"))
        .and_then(Value::as_f64);
    let total = v
        .get("totalBytes")
        .or_else(|| v.get("total_bytes"))
        .and_then(Value::as_f64);
    match (used, total) {
        (Some(u), Some(t)) if t > 0.0 => Some(HomeStorage {
            used_bytes: u,
            total_bytes: t,
            lent_bytes: v
                .get("lentBytes")
                .or_else(|| v.get("lent_bytes"))
                .and_then(Value::as_f64),
        }),
        _ => None,
    }
}

/// Spawn overview facts fetching (health + authenticated box facts). Best-effort.
pub fn use_overview_facts(mut facts: OverviewSignal) {
    let config: ConfigSignal = use_context();
    use_future(move || {
        let cfg = config();
        async move {
            let health = fetch_health(&cfg).await;
            facts.with_mut(|f| {
                f.health = health;
                f.host_name = host_name_from(&cfg, f);
            });
            if cfg.demo || cfg.host.trim().is_empty() || cfg.token.trim().is_empty() {
                facts.with_mut(|f| f.availability.measured = true);
                return;
            }
            let settings = actions::dispatch(&cfg, Action::Get("/api/settings".into()))
                .await
                .ok();
            let state = actions::dispatch(&cfg, Action::Get("/api/state".into()))
                .await
                .ok();
            let status = actions::dispatch(&cfg, Action::Get("/api/status".into()))
                .await
                .ok();
            let storage = actions::dispatch(&cfg, Action::StorageStatus).await.ok();
            let overview = actions::dispatch(&cfg, Action::Overview).await.ok();
            let apps = actions::dispatch(&cfg, Action::AppsList).await.ok();
            facts.with_mut(|f| {
                f.settings = settings;
                f.box_state = state;
                f.status = status;
                f.storage = storage.as_ref().and_then(storage_from_value);
                f.overview = overview;
                f.apps = apps;
                f.host_name = host_name_from(&cfg, f);
                f.availability.measured = true;
                f.availability.files = Availability::Present;
                f.availability.code = Availability::Present;
            });
        }
    });
}

fn is_building(status: &Option<Value>) -> bool {
    status
        .as_ref()
        .and_then(|s| s.get("state").or_else(|| s.get("status")))
        .and_then(Value::as_str)
        .is_some_and(|s| s == "building" || s == "applying" || s == "running")
}

#[component]
pub fn BoxNameBlock(facts: OverviewSignal) -> Element {
    let f = facts();
    let health = match f.health {
        Health::Unknown => "Checking…",
        Health::Ok => "Answering",
        Health::Down => "Not answering",
    };
    rsx! {
        div { class: "block box-name",
            h1 { class: "page-title", "{f.host_name}" }
            p { class: "muted", "{health}" }
        }
    }
}

#[component]
pub fn ApplyingBlock(facts: OverviewSignal) -> Element {
    let f = facts();
    if !is_building(&f.status) {
        return rsx! { div { class: "block applying hidden" } };
    }
    let msg = f
        .status
        .as_ref()
        .and_then(|s| s.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("Applying a change…");
    rsx! {
        div { class: "block applying",
            Card {
                strong { "Applying" }
                p { class: "muted", "{msg}" }
            }
        }
    }
}

#[component]
pub fn AppsBlock(facts: OverviewSignal, config: BoxConfig, props: BlockProps) -> Element {
    let f = facts();
    let hide_storage = props.flag("hideStorage");
    rsx! {
        div { class: "block apps-block",
            h2 { "Apps" }
            match f.apps.as_ref() {
                None => rsx! { Spinner {} },
                Some(v) => rsx! { super::blocks::AppsGridFromValue { data: v.clone() } },
            }
            if !hide_storage {
                if let Some(st) = f.storage.as_ref() {
                    {storage_strip(st)}
                } else if f.apps.is_some() {
                    // Facts loaded but storage missing: try fetching storage via config is
                    // handled by facts; keep quiet.
                }
            }
        }
    }
}

fn storage_strip(st: &HomeStorage) -> Element {
    let pct = (st.used_bytes / st.total_bytes * 100.0).clamp(0.0, 100.0);
    let used_gb = st.used_bytes / 1_073_741_824.0;
    let total_gb = st.total_bytes / 1_073_741_824.0;
    rsx! {
        div { class: "storage-strip",
            p { class: "muted mono", "{used_gb:.1} / {total_gb:.1} GiB ({pct:.0}%)" }
        }
    }
}

#[component]
pub fn GlanceBlock(facts: OverviewSignal, config: BoxConfig, _props: BlockProps) -> Element {
    let f = facts();
    let _ = config;
    rsx! {
        div { class: "block glance",
            h2 { "At a glance" }
            match f.overview.as_ref().or(f.settings.as_ref()) {
                None => rsx! { Spinner {} },
                Some(v) => rsx! { Card { pre { class: "mono", "{serde_json::to_string_pretty(v).unwrap_or_default()}" } } },
            }
        }
    }
}

/// Render an overview page block by name, wired to overview context/props.
pub fn render_overview_block(name: &str, props: BlockProps) -> Element {
    let facts = use_overview_context();
    let config: ConfigSignal = use_context();
    match name {
        "BoxName" => rsx! { BoxNameBlock { facts } },
        "Applying" => rsx! { ApplyingBlock { facts } },
        "Apps" => rsx! { AppsBlock { facts, config: config(), props } },
        "Glance" => rsx! { GlanceBlock { facts, config: config(), _props: props } },
        _ => rsx! { div { class: "block block-unknown muted", "Unknown block: {name}" } },
    }
}

/// Render a generic/settings page block by name.
pub fn render_page_block(name: &str, props: BlockProps) -> Element {
    let config: ConfigSignal = use_context();
    match name {
        "Disk" => rsx! { super::blocks::Disk { config: config(), _props: props } },
        "About" => rsx! { super::blocks::AboutBlock { config: config(), _props: props } },
        "BoxName" => rsx! { super::blocks::BoxName {} },
        "Applying" => rsx! { super::blocks::Applying {} },
        "Apps" => rsx! { super::blocks::Apps { config: config(), props } },
        "Glance" => rsx! { super::blocks::Glance { config: config(), _props: props } },
        _ => rsx! { div { class: "block block-unknown muted", "Unknown block: {name}" } },
    }
}

/// Home route component: overview context + rendered default/custom page.
#[component]
pub fn OverviewPage() -> Element {
    let facts = provide_overview_context();
    use_overview_facts(facts);
    let store = super::use_pages_store();
    let render_block = Callback::new(move |(name, props): (String, BlockProps)| {
        render_overview_block(&name, props)
    });
    rsx! {
        RenderedPage {
            id: "overview".to_string(),
            state: store(),
            render_block,
        }
    }
}

/// Storage/settings page via blocks renderer.
#[component]
pub fn StoragePage() -> Element {
    let store = super::use_pages_store();
    let render_block = Callback::new(move |(name, props): (String, BlockProps)| {
        render_page_block(&name, props)
    });
    rsx! {
        RenderedPage {
            id: "settings.storage".to_string(),
            state: store(),
            render_block,
        }
    }
}

/// About page via blocks renderer.
#[component]
pub fn AboutPage() -> Element {
    let store = super::use_pages_store();
    let render_block = Callback::new(move |(name, props): (String, BlockProps)| {
        render_page_block(&name, props)
    });
    rsx! {
        RenderedPage {
            id: "settings.about".to_string(),
            state: store(),
            render_block,
        }
    }
}
