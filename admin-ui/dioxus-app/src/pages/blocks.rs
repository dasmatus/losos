//! Block components for MDX pages (Dioxus port).
//! Presentational blocks preserving design-system CSS token classes.
//! Overview-context wiring lives in `overview.rs`; settings/page blocks here.

use crate::actions::{self, Action};
use crate::components::ui::{Card, Spinner};
use crate::store::BoxConfig;
use dioxus::prelude::*;
use serde_json::Value;

use super::renderer::BlockProps;

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
pub fn Apps(config: BoxConfig, #[props(default)] props: BlockProps) -> Element {
    let mut apps = use_signal(|| None::<Result<Value, String>>);
    let mut storage = use_signal(|| None::<Result<Value, String>>);
    let cfg = config.clone();
    let cfg2 = config.clone();
    let hide_storage = props.flag("hideStorage");
    use_future(move || {
        let cfg = cfg.clone();
        async move {
            apps.set(Some(
                actions::dispatch(&cfg, Action::AppsList)
                    .await
                    .map_err(|e| e.message),
            ));
        }
    });
    use_future(move || {
        let cfg = cfg2.clone();
        let hide = hide_storage;
        async move {
            if hide {
                return;
            }
            storage.set(Some(
                actions::dispatch(&cfg, Action::StorageStatus)
                    .await
                    .map_err(|e| e.message),
            ));
        }
    });
    rsx! {
        div { class: "block apps-block",
            h2 { "Apps" }
            match apps() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                Some(Ok(v)) => rsx! { AppsGrid { data: v } },
            }
            if !hide_storage {
                match storage() {
                    None => rsx! {},
                    Some(Err(_)) => rsx! {},
                    Some(Ok(v)) => rsx! { StorageStrip { data: v } },
                }
            }
        }
    }
}

#[component]
pub fn AppsGridFromValue(data: Value) -> Element {
    rsx! { AppsGrid { data } }
}

#[component]
fn AppsGrid(data: Value) -> Element {
    let items = data
        .as_array()
        .cloned()
        .or_else(|| data.get("apps").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();
    if items.is_empty() {
        return rsx! { p { class: "muted", "No apps." } };
    }
    rsx! {
        div { class: "grid",
            for item in items.iter().take(24) {
                {
                    let name = item
                        .get("name")
                        .or(item.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or("App");
                    let desc = item
                        .get("description")
                        .or(item.get("summary"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    rsx! {
                        Card {
                            strong { "{name}" }
                            if !desc.is_empty() { p { class: "muted", "{desc}" } }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn StorageStrip(data: Value) -> Element {
    let used = data
        .get("usedBytes")
        .or_else(|| data.get("used_bytes"))
        .and_then(Value::as_f64);
    let total = data
        .get("totalBytes")
        .or_else(|| data.get("total_bytes"))
        .and_then(Value::as_f64);
    rsx! {
        div { class: "storage-strip",
            if let (Some(u), Some(t)) = (used, total) {
                if t > 0.0 {
                    {
                        let pct = (u / t * 100.0).clamp(0.0, 100.0);
                        let used_gb = u / 1_073_741_824.0;
                        let total_gb = t / 1_073_741_824.0;
                        rsx! {
                            p { class: "muted mono", "{used_gb:.1} / {total_gb:.1} GiB ({pct:.0}%)" }
                        }
                    }
                } else {
                    p { class: "muted", "Storage" }
                }
            } else {
                pre { class: "mono muted", "{serde_json::to_string_pretty(&data).unwrap_or_default()}" }
            }
        }
    }
}

#[component]
pub fn Glance(config: BoxConfig, #[props(default)] _props: BlockProps) -> Element {
    let mut ov = use_signal(|| None::<Result<Value, String>>);
    let cfg = config.clone();
    use_future(move || {
        let cfg = cfg.clone();
        async move {
            ov.set(Some(
                actions::dispatch(&cfg, Action::Overview)
                    .await
                    .map_err(|e| e.message),
            ));
        }
    });
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
pub fn Disk(config: BoxConfig, #[props(default)] _props: BlockProps) -> Element {
    let mut st = use_signal(|| None::<Result<Value, String>>);
    let cfg = config.clone();
    use_future(move || {
        let cfg = cfg.clone();
        async move {
            st.set(Some(
                actions::dispatch(&cfg, Action::StorageStatus)
                    .await
                    .map_err(|e| e.message),
            ));
        }
    });
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
pub fn AboutBlock(config: BoxConfig, #[props(default)] _props: BlockProps) -> Element {
    let mut ab = use_signal(|| None::<Result<Value, String>>);
    let cfg = config.clone();
    use_future(move || {
        let cfg = cfg.clone();
        async move {
            ab.set(Some(
                actions::dispatch(&cfg, Action::About)
                    .await
                    .map_err(|e| e.message),
            ));
        }
    });
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
