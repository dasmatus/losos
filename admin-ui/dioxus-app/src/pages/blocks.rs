//! Block components for MDX pages (Dioxus port).
//!
//! Implements blocks referenced by `blocks.json` / *.mdx pages, binding to the
//! actions/API layer while preserving the design system.

use crate::actions::{self, Action};
use crate::components::ui::{Card, Spinner};
use crate::store::BoxConfig;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn BoxName() -> Element {
    rsx! {
        div { class: "block box-name",
            h1 { class: "page-title", "LosOS" }
            p { class: "muted", "This box is managed from this frontend (Dioxus/RSX rewrite)." }
        }
    }
}

#[component]
pub fn Applying() -> Element {
    rsx! { div { class: "block applying hidden" } }
}

#[component]
pub fn Apps(config: BoxConfig) -> Element {
    let mut apps = use_signal(|| None::<Result<Value, String>>);
    let cfg = config.clone();
    use_future(move || { let cfg = cfg.clone(); async move {
        apps.set(Some(actions::dispatch(&cfg, Action::AppsList).await.map_err(|e| e.message)));
    }});
    rsx! {
        div { class: "block apps-block",
            h2 { "Apps" }
            match apps() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                Some(Ok(v)) => rsx! { AppsGrid { data: v } },
            }
        }
    }
}

#[component]
fn AppsGrid(data: Value) -> Element {
    let items = data.as_array().cloned()
        .or_else(|| data.get("apps").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();
    if items.is_empty() { return rsx! { p { class: "muted", "No apps." } }; }
    rsx! {
        div { class: "grid",
            for item in items.iter().take(12) {
                {
                    let name = item.get("name").or(item.get("id")).and_then(Value::as_str).unwrap_or("App");
                    rsx! { Card { strong { "{name}" } } }
                }
            }
        }
    }
}

#[component]
pub fn Glance(config: BoxConfig) -> Element {
    let mut ov = use_signal(|| None::<Result<Value, String>>);
    let cfg = config.clone();
    use_future(move || { let cfg = cfg.clone(); async move {
        ov.set(Some(actions::dispatch(&cfg, Action::Overview).await.map_err(|e| e.message)));
    }});
    rsx! {
        div { class: "block glance",
            h2 { "At a glance" }
            match ov() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { p { class: "muted", "Overview unavailable: {e}" } },
                Some(Ok(v)) => rsx! { Card { pre { class: "mono", "{serde_json::to_string_pretty(&v).unwrap_or_default()}" } } },
            }
        }
    }
}

#[component]
pub fn Disk(config: BoxConfig) -> Element {
    let mut st = use_signal(|| None::<Result<Value, String>>);
    let cfg = config.clone();
    use_future(move || { let cfg = cfg.clone(); async move {
        st.set(Some(actions::dispatch(&cfg, Action::StorageStatus).await.map_err(|e| e.message)));
    }});
    rsx! {
        div { class: "block disk",
            h2 { "Storage" }
            match st() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                Some(Ok(v)) => rsx! { Card { pre { class: "mono", "{serde_json::to_string_pretty(&v).unwrap_or_default()}" } } },
            }
        }
    }
}

#[component]
pub fn AboutBlock(config: BoxConfig) -> Element {
    let mut ab = use_signal(|| None::<Result<Value, String>>);
    let cfg = config.clone();
    use_future(move || { let cfg = cfg.clone(); async move {
        ab.set(Some(actions::dispatch(&cfg, Action::About).await.map_err(|e| e.message)));
    }});
    rsx! {
        div { class: "block about",
            h2 { "About this box" }
            match ab() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                Some(Ok(v)) => rsx! { Card { pre { class: "mono", "{serde_json::to_string_pretty(&v).unwrap_or_default()}" } } },
            }
        }
    }
}
