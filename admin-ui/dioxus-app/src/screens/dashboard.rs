use crate::api::{BoxApi, Trust};
use crate::screens::panes;
use crate::store::BoxConfig;
use dioxus::prelude::*;
use serde_json::Value;

const TABS: &[&str] = &[
    "Dashboard",
    "Storage",
    "Mesh",
    "Apps",
    "Network",
    "Security",
    "Advanced",
    "History",
    "Backup",
    "Cloud",
    "Settings",
];

#[component]
pub fn DashboardScreen(config: BoxConfig, on_change: EventHandler<BoxConfig>) -> Element {
    let mut current = use_signal(|| 0usize);
    let mut refresh = use_signal(|| 0u64);
    let cfg = config.clone();

    rsx! {
        div { class: "content",
            div { class: "row-between", style: "margin: 4px 0 8px;",
                div { class: "row",
                    h1 { style: "margin:0; font-size: var(--ls-fs-2xl);", "LosOS" }
                    if config.demo {
                        span { class: "chip demo", "demo" }
                    } else {
                        span { class: "chip neutral", "{config.host}" }
                    }
                }
                button { class: "btn", onclick: move |_| refresh += 1, "Refresh" }
            }

            match current() {
                0 => rsx! { panes::OverviewPane { config: cfg.clone(), on_change, refresh: refresh() } },
                1 => rsx! { panes::StoragePane { config: cfg.clone(), on_change, refresh: refresh() } },
                2 => rsx! { panes::MeshPane { config: cfg.clone(), on_change, refresh: refresh() } },
                3 => rsx! { panes::AppsPane { config: cfg.clone(), on_change, refresh: refresh() } },
                4 => rsx! { panes::RoutePane { title: "Network", path: "/api/settings", config: cfg.clone(), on_change, refresh: refresh(), hint: "Network settings (read-only port)." } },
                5 => rsx! { panes::RoutePane { title: "Security", path: "/api/settings", config: cfg.clone(), on_change, refresh: refresh(), hint: "Security settings (read-only port)." } },
                6 => rsx! { panes::RoutePane { title: "Advanced", path: "/api/options", config: cfg.clone(), on_change, refresh: refresh(), hint: "Advanced options (read-only port)." } },
                7 => rsx! { panes::HistoryPane { config: cfg.clone(), on_change, refresh: refresh() } },
                8 => rsx! { panes::BackupPane { config: cfg.clone(), on_change, refresh: refresh() } },
                9 => rsx! { panes::CloudPane { config: cfg.clone(), on_change, refresh: refresh() } },
                10 => rsx! { panes::SettingsPane { config: cfg.clone(), on_change } },
                _ => rsx! { div { "Unknown tab" } },
            }
        }

        nav { class: "tab-bar",
            for (i, name) in TABS.iter().enumerate() {
                button {
                    key: "{i}",
                    class: if current() == i { "tab-item selected" } else { "tab-item" },
                    onclick: move |_| current.set(i),
                    "{name}"
                }
            }
        }
    }
}

pub fn make_api(config: &BoxConfig) -> Result<BoxApi, String> {
    let trust = if config.tls_pin.as_deref().unwrap_or("").is_empty() {
        Trust::Permissive
    } else {
        Trust::Pinned(config.tls_pin.clone().unwrap_or_default())
    };
    BoxApi::new(&config.host, &config.token, trust).map_err(|e| e.message)
}

pub async fn fetch_json(
    config: &BoxConfig,
    path: &str,
    anonymous: bool,
) -> Result<Value, (u16, String)> {
    if config.demo {
        return Ok(demo_value(path));
    }
    let api = make_api(config).map_err(|m| (0u16, m))?;
    let res = if anonymous {
        api.get_anonymous(path).await
    } else {
        api.get(path).await
    };
    res.map_err(|e| (e.status, e.message))
}

fn demo_value(path: &str) -> Value {
    match path {
        "/api/health" => serde_json::json!({ "status": "ok", "demo": true }),
        "/api/status" => {
            serde_json::json!({ "host": "losos", "release": "demo", "uptimeSeconds": 3661, "generation": "dioxus-rewrite" })
        }
        "/api/storage" => {
            serde_json::json!({ "usedBytes": 197568495616_i64, "totalBytes": 549755813888_i64, "reserveBytes": 10737418240_i64, "label": "Demo /persist" })
        }
        "/api/apps" => {
            serde_json::json!({ "apps": [{"name":"LosOS Cloud","state":"running"}, {"name":"LosOS Git","state":"running"}] })
        }
        "/api/edge" => serde_json::json!({ "reachable": false, "lanSearched": true, "edges": [] }),
        "/api/settings" => serde_json::json!({ "hostname": "losos", "demo": true }),
        "/api/options" => serde_json::json!({ "options": [] }),
        "/api/config" => serde_json::json!({ "generations": [] }),
        "/api/backup" => serde_json::json!({ "lastRun": null, "snapshots": [] }),
        "/api/cloud/files?path=%2F" => {
            serde_json::json!({ "path": "/", "entries": [{"name":"Documents","type":"directory"}, {"name":"Photos","type":"directory"}] })
        }
        _ => serde_json::json!({ "demo": true, "path": path }),
    }
}
