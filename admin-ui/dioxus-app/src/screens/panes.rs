use crate::api::{self, format_bytes};
use crate::screens::dashboard::{fetch_json, make_api};
use crate::store::BoxConfig;
use dioxus::prelude::*;
use serde_json::{json, Value};

fn handle_401(status: u16, config: &BoxConfig, on_change: &EventHandler<BoxConfig>) -> bool {
    if status == 401 {
        let mut next = config.clone();
        next.token.clear();
        next.demo = false;
        on_change.call(next);
        true
    } else {
        false
    }
}

#[component]
pub fn OverviewPane(
    config: BoxConfig,
    on_change: EventHandler<BoxConfig>,
    refresh: u64,
) -> Element {
    let cfg1 = config.clone();
    let cfg2 = config.clone();
    let cfg3 = config.clone();
    let cfg4 = config.clone();
    let health = use_resource(move || {
        let cfg = cfg1.clone();
        let _ = refresh;
        async move { fetch_json(&cfg, "/api/health", true).await }
    });
    let status = use_resource(move || {
        let cfg = cfg2.clone();
        let _ = refresh;
        async move { fetch_json(&cfg, "/api/status", false).await }
    });
    let storage = use_resource(move || {
        let cfg = cfg3.clone();
        let _ = refresh;
        async move { fetch_json(&cfg, "/api/storage", false).await }
    });
    let apps = use_resource(move || {
        let cfg = cfg4.clone();
        let _ = refresh;
        async move { fetch_json(&cfg, "/api/apps", false).await }
    });

    for res in [
        status.read().as_ref(),
        storage.read().as_ref(),
        apps.read().as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Err((401, _)) = res {
            handle_401(401, &config, &on_change);
        }
    }

    rsx! {
        div { class: "card",
            h2 { class: "card-title", "Box status" }
            match &*health.read() {
                Some(Ok(v)) => rsx! {
                    div { class: "row",
                        span { class: "chip ok", "reachable" }
                        span { class: "muted", "{api::opt_str(v, \"status\").unwrap_or_else(|| \"ok\".into())}" }
                    }
                },
                Some(Err((_, m))) => rsx! { div { class: "banner error", "Health: {m}" } },
                None => rsx! { span { class: "muted", "Checking…" } },
            }
            match &*status.read() {
                Some(Ok(v)) => rsx! { StatusDetails { value: v.clone() } },
                Some(Err((_, m))) if !config.demo => rsx! { div { class: "banner error", "Status: {m}" } },
                _ => rsx! {},
            }
        }
        div { class: "card",
            h2 { class: "card-title", "Storage" }
            match &*storage.read() {
                Some(Ok(v)) => rsx! { StorageDetails { value: v.clone() } },
                Some(Err((_, m))) if !config.demo => rsx! { div { class: "banner error", "{m}" } },
                _ if !config.demo => rsx! { span { class: "muted", "Loading…" } },
                _ => rsx! {},
            }
        }
        div { class: "card",
            h2 { class: "card-title", "Apps" }
            match &*apps.read() {
                Some(Ok(v)) => rsx! { AppsSummary { value: v.clone() } },
                Some(Err((_, m))) if !config.demo => rsx! { div { class: "banner error", "{m}" } },
                _ if !config.demo => rsx! { span { class: "muted", "Loading…" } },
                _ => rsx! {},
            }
        }
        div { class: "card",
            h2 { class: "card-title", "Cross-platform rewrite (Dioxus)" }
            p { class: "muted",
                "Replaces the Kotlin companion (`admin-ui/companion`) with one Rust UI targeting web, desktop, and mobile. "
                "Reuses TUI patterns (`admin-ui/tui`) and shared crates (`losos-access`)."
            }
        }
    }
}

#[component]
fn StatusDetails(value: Value) -> Element {
    let host = api::opt_str(&value, "host")
        .or_else(|| api::opt_str(&value, "hostname"))
        .unwrap_or_else(|| "–".into());
    let release = api::opt_str(&value, "release")
        .or_else(|| api::opt_str(&value, "generation"))
        .unwrap_or_else(|| "–".into());
    let uptime = api::opt_i64(&value, "uptimeSeconds")
        .map(|s| format!("{s} s"))
        .unwrap_or_else(|| "–".into());
    rsx! { div { class: "kv", style: "margin-top:10px;",
        div { class: "k", "Host" } div { "{host}" }
        div { class: "k", "Release" } div { "{release}" }
        div { class: "k", "Uptime" } div { "{uptime}" }
    } }
}

#[component]
fn StorageDetails(value: Value) -> Element {
    let used = api::opt_f64(&value, "usedBytes")
        .or_else(|| api::opt_f64(&value, "used").map(|g| g * 1_073_741_824.0));
    let total = api::opt_f64(&value, "totalBytes")
        .or_else(|| api::opt_f64(&value, "total").map(|g| g * 1_073_741_824.0));
    let reserve = api::opt_f64(&value, "reserveBytes");
    let frac = match (used, total) {
        (Some(u), Some(t)) if t > 0.0 => (u / t).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let pct = (frac * 100.0).round() as i64;
    let label = api::opt_str(&value, "label").unwrap_or_default();
    rsx! {
        div { class: "stack",
            div { class: "row-between",
                span { "{format_bytes(used)} used of {format_bytes(total)}" }
                span { class: "muted", "{pct}%" }
            }
            div { class: "progress", span { style: "width: {pct}%;" } }
            if let Some(r) = reserve {
                div { class: "muted", "Reserve: {format_bytes(Some(r))}" }
            }
            if !label.is_empty() { div { class: "muted", "{label}" } }
        }
    }
}

#[component]
fn AppsSummary(value: Value) -> Element {
    let items = value
        .get("apps")
        .and_then(Value::as_array)
        .or_else(|| value.get("releases").and_then(Value::as_array))
        .or_else(|| value.as_array());
    match items {
        Some(arr) if !arr.is_empty() => rsx! {
            div { class: "stack",
                for (i, app) in arr.iter().take(12).enumerate() {
                    {
                        let name = api::opt_str(app, "name").or_else(|| api::opt_str(app, "release")).or_else(|| app.as_str().map(str::to_string)).unwrap_or_else(|| "app".into());
                        let state = api::opt_str(app, "state").or_else(|| api::opt_str(app, "status")).unwrap_or_else(|| "installed".into());
                        rsx! { div { key: "{i}", class: "row-between",
                            span { "{name}" }
                            span { class: "chip neutral", "{state}" }
                        } }
                    }
                }
                if arr.len() > 12 { div { class: "muted", "…and {arr.len() - 12} more" } }
            }
        },
        _ => rsx! { span { class: "muted", "No apps reported." } },
    }
}

/// Storage pane with grow action (port of companion StoragePane/ReserveCard).
#[component]
pub fn StoragePane(config: BoxConfig, on_change: EventHandler<BoxConfig>, refresh: u64) -> Element {
    let cfg = config.clone();
    let mut nonce = use_signal(|| 0u64);
    let data = use_resource(move || {
        let cfg = cfg.clone();
        let _ = refresh;
        let _ = nonce();
        async move { fetch_json(&cfg, "/api/storage", false).await }
    });
    let mut busy = use_signal(|| false);
    let mut msg = use_signal(String::new);
    let mut err = use_signal(String::new);
    let mut confirm = use_signal(|| false);

    if let Some(Err((401, _))) = data.read().as_ref() {
        handle_401(401, &config, &on_change);
    }

    let do_grow = {
        let config = config.clone();
        let on_change = on_change;
        move |_| {
            let config = config.clone();
            let on_change = on_change;
            spawn(async move {
                busy.set(true);
                err.set(String::new());
                msg.set(String::new());
                match make_api(&config) {
                    Err(m) => err.set(m),
                    Ok(api) => match api.post("/api/grow").await {
                        Ok(_) => {
                            msg.set("Reserve claimed (grow requested).".into());
                            nonce += 1;
                        }
                        Err(e) => {
                            if !handle_401(e.status, &config, &on_change) {
                                err.set(e.message);
                            }
                        }
                    },
                }
                busy.set(false);
                confirm.set(false);
            });
        }
    };

    rsx! {
        div { class: "card",
            h2 { class: "card-title", "Storage" }
            match &*data.read() {
                Some(Ok(v)) => {
                    let reserve = api::opt_f64(v, "reserveBytes").unwrap_or(0.0);
                    rsx! {
                        StorageDetails { value: v.clone() }
                        div { style: "margin-top:12px;", class: "row",
                            if reserve > 0.0 && !confirm() {
                                button { class: "btn", disabled: busy(), onclick: move |_| confirm.set(true), "Use reserve" }
                            }
                            if confirm() {
                                span { class: "muted", "Claim {format_bytes(Some(reserve))}?" }
                                button { class: "btn primary", disabled: busy(), onclick: do_grow, if busy() { "Claiming…" } else { "Confirm" } }
                                button { class: "btn", disabled: busy(), onclick: move |_| confirm.set(false), "Cancel" }
                            }
                        }
                    }
                }
                Some(Err((_, m))) => rsx! { div { class: "banner error", "{m}" } },
                None => rsx! { span { class: "muted", "Loading…" } },
            }
            if !msg.read().is_empty() { div { class: "banner info", style: "margin-top:8px;", "{msg}" } }
            if !err.read().is_empty() { div { class: "banner error", style: "margin-top:8px;", "{err}" } }
        }
    }
}

/// Apps pane with search, guarded install/remove (port of companion AppsPane).
#[component]
pub fn AppsPane(config: BoxConfig, on_change: EventHandler<BoxConfig>, refresh: u64) -> Element {
    let cfg = config.clone();
    let mut nonce = use_signal(|| 0u64);
    let installed = use_resource(move || {
        let cfg = cfg.clone();
        let _ = refresh;
        let _ = nonce();
        async move { fetch_json(&cfg, "/api/apps", false).await }
    });
    let mut query = use_signal(String::new);
    let mut results: Signal<Option<Result<Value, (u16, String)>>> = use_signal(|| None);
    let mut busy = use_signal(String::new);
    let mut msg = use_signal(String::new);
    let mut err = use_signal(String::new);
    let mut pending_remove: Signal<Option<String>> = use_signal(|| None);
    let mut pending_install: Signal<Option<Value>> = use_signal(|| None);

    if let Some(Err((401, _))) = installed.read().as_ref() {
        handle_401(401, &config, &on_change);
    }

    let do_search = {
        let config = config.clone();
        let on_change = on_change;
        move |_| {
            let term = query.read().trim().to_string();
            if term.is_empty() {
                return;
            }
            let config = config.clone();
            let on_change = on_change;
            spawn(async move {
                let path = format!("/api/apps/search?q={}", api::encode_query(&term));
                let res = fetch_json(&config, &path, false).await;
                if let Err((401, _)) = &res {
                    handle_401(401, &config, &on_change);
                }
                results.set(Some(res));
            });
        }
    };

    let confirm_remove = {
        let config = config.clone();
        let on_change = on_change;
        move |_| {
            let Some(name) = pending_remove.read().clone() else {
                return;
            };
            let config = config.clone();
            let on_change = on_change;
            spawn(async move {
                busy.set(format!("remove:{name}"));
                err.set(String::new());
                msg.set(String::new());
                match make_api(&config) {
                    Err(m) => err.set(m),
                    Ok(api) => match api
                        .post_json("/api/apps/remove", json!({"release": name}))
                        .await
                    {
                        Ok(_) => {
                            msg.set(format!("Removing {name}…"));
                            nonce += 1;
                        }
                        Err(e) => {
                            if !handle_401(e.status, &config, &on_change) {
                                err.set(e.message);
                            }
                        }
                    },
                }
                busy.set(String::new());
                pending_remove.set(None);
            });
        }
    };

    let confirm_install = {
        let config = config.clone();
        let on_change = on_change;
        move |_| {
            let Some(item) = pending_install.read().clone() else {
                return;
            };
            let config = config.clone();
            let on_change = on_change;
            spawn(async move {
                let chart = item.get("chart").cloned().unwrap_or(Value::Null);
                let repo = api::opt_str(&chart, "repo").unwrap_or_default();
                let cname = api::opt_str(&chart, "name").unwrap_or_default();
                let version = api::opt_str(&chart, "version").unwrap_or_default();
                if repo.is_empty() || cname.is_empty() || version.is_empty() {
                    err.set("This result does not include an installable chart.".into());
                    pending_install.set(None);
                    return;
                }
                let key = format!("{repo}/{cname}");
                busy.set(format!("install:{key}"));
                err.set(String::new());
                msg.set(String::new());
                match make_api(&config) {
                    Err(m) => err.set(m),
                    Ok(api) => {
                        let q = format!(
                            "/api/apps/chart?repo={}&name={}&version={}",
                            api::encode_query(&repo),
                            api::encode_query(&cname),
                            api::encode_query(&version)
                        );
                        match api.get(&q).await {
                            Err(e) => {
                                if !handle_401(e.status, &config, &on_change) {
                                    err.set(e.message);
                                }
                            }
                            Ok(prepared) => {
                                let release = api::opt_str(&prepared, "release")
                                    .unwrap_or_else(|| cname.clone());
                                let chart_ref = prepared.get("chart").cloned().unwrap_or(chart);
                                let body = json!({"release": release, "chart": chart_ref, "runAs": "notshared", "values": {}, "valuesYaml": Value::Null});
                                match api.post_json("/api/apps/install", body).await {
                                    Ok(_) => {
                                        msg.set(format!("Installing {release}…"));
                                        nonce += 1;
                                    }
                                    Err(e) => {
                                        if !handle_401(e.status, &config, &on_change) {
                                            err.set(e.message);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                busy.set(String::new());
                pending_install.set(None);
            });
        }
    };

    rsx! {
        div { class: "card",
            h2 { class: "card-title", "Installed apps" }
            match &*installed.read() {
                Some(Ok(v)) => {
                    let items = v.get("apps").and_then(Value::as_array).or_else(|| v.get("releases").and_then(Value::as_array)).cloned().unwrap_or_default();
                    if items.is_empty() {
                        rsx! { span { class: "muted", "Nothing installed yet." } }
                    } else {
                        rsx! { div { class: "stack",
                            for (i, app) in items.iter().enumerate() {
                                {
                                    let name = api::opt_str(app, "name").or_else(|| api::opt_str(app, "release")).unwrap_or_else(|| "app".into());
                                    let state = api::opt_str(app, "state").or_else(|| api::opt_str(app, "status")).unwrap_or_default();
                                    let n2 = name.clone();
                                    let removing = busy.read().as_str() == format!("remove:{name}");
                                    rsx! { div { key: "{i}", class: "row-between",
                                        div { span { "{name}" } if !state.is_empty() { span { class: "muted", " · {state}" } } }
                                        button { class: "btn danger", disabled: !busy.read().is_empty(), onclick: move |_| pending_remove.set(Some(n2.clone())), if removing { "Removing…" } else { "Remove" } }
                                    } }
                                }
                            }
                        } }
                    }
                }
                Some(Err((_, m))) => rsx! { div { class: "banner error", "{m}" } },
                None => rsx! { span { class: "muted", "Loading…" } },
            }
        }
        if let Some(name) = pending_remove.read().clone() {
            div { class: "card", style: "border-color: var(--ls-danger);",
                p { "Remove {name}? The box keeps its data files as it normally does." }
                div { class: "row",
                    button { class: "btn danger", disabled: !busy.read().is_empty(), onclick: confirm_remove, "Confirm remove" }
                    button { class: "btn", disabled: !busy.read().is_empty(), onclick: move |_| pending_remove.set(None), "Cancel" }
                }
            }
        }

        div { class: "card",
            h2 { class: "card-title", "Find apps" }
            div { class: "row",
                input { r#type: "text", placeholder: "Search charts…", value: "{query}", oninput: move |e| query.set(e.value()) }
                button { class: "btn", onclick: do_search, "Search" }
            }
            if let Some(res) = results.read().as_ref() {
                match res {
                    Ok(v) => {
                        let items = v.get("results").and_then(Value::as_array).or_else(|| v.get("items").and_then(Value::as_array)).or_else(|| v.as_array()).cloned().unwrap_or_default();
                        if items.is_empty() {
                            rsx! { p { class: "muted", "No results." } }
                        } else {
                            rsx! { div { class: "stack", style: "margin-top:10px;",
                                for (i, item) in items.iter().take(20).enumerate() {
                                    {
                                        let name = api::opt_str(item, "name").or_else(|| item.get("chart").and_then(|c| api::opt_str(c, "name"))).unwrap_or_else(|| "app".into());
                                        let desc = api::opt_str(item, "description").unwrap_or_default();
                                        let it2 = item.clone();
                                        rsx! { div { key: "{i}", class: "row-between",
                                            div { div { "{name}" } if !desc.is_empty() { div { class: "muted", "{desc}" } } }
                                            button { class: "btn primary", disabled: !busy.read().is_empty(), onclick: move |_| pending_install.set(Some(it2.clone())), "Install" }
                                        } }
                                    }
                                }
                            } }
                        }
                    }
                    Err((_, m)) => rsx! { div { class: "banner error", "{m}" } },
                }
            }
        }
        if let Some(item) = pending_install.read().as_ref() {
            {
                let name = api::opt_str(item, "name").or_else(|| item.get("chart").and_then(|c| api::opt_str(c, "name"))).unwrap_or_else(|| "app".into());
                rsx! { div { class: "card", style: "border-color: var(--ls-accent);",
                    p { "Install {name}? The box will fetch chart details and install as `notshared` with default values." }
                    div { class: "row",
                        button { class: "btn primary", disabled: !busy.read().is_empty(), onclick: confirm_install, "Confirm install" }
                        button { class: "btn", disabled: !busy.read().is_empty(), onclick: move |_| pending_install.set(None), "Cancel" }
                    }
                } }
            }
        }

        if !msg.read().is_empty() { div { class: "banner info", "{msg}" } }
        if !err.read().is_empty() { div { class: "banner error", "{err}" } }
    }
}

/// Mesh pane (edge + lending summary, read-only port).
#[component]
pub fn MeshPane(config: BoxConfig, on_change: EventHandler<BoxConfig>, refresh: u64) -> Element {
    rsx! {
        RoutePane { title: "Mesh (edge)", path: "/api/edge", config: config.clone(), on_change, refresh, hint: "Edges/pool status (read-only in this pass)." }
        RoutePane { title: "Lending settings", path: "/api/settings", config, on_change, refresh, hint: "Sharing toggles are a follow-up (POST /api/apply)." }
    }
}

#[component]
pub fn RoutePane(
    title: &'static str,
    path: &'static str,
    config: BoxConfig,
    on_change: EventHandler<BoxConfig>,
    refresh: u64,
    hint: &'static str,
) -> Element {
    let cfg = config.clone();
    let data = use_resource(move || {
        let cfg = cfg.clone();
        let _ = refresh;
        async move { fetch_json(&cfg, path, false).await }
    });
    if let Some(Err((401, _))) = data.read().as_ref() {
        handle_401(401, &config, &on_change);
    }
    rsx! { div { class: "card",
        h2 { class: "card-title", "{title}" }
        p { class: "muted", "{hint}" }
        match &*data.read() {
            Some(Ok(v)) => rsx! { JsonSummary { value: v.clone(), path } },
            Some(Err((_, m))) => rsx! { div { class: "banner error", "{m}" } },
            None => rsx! { span { class: "muted", "Loading {path}…" } },
        }
    } }
}

#[component]
fn JsonSummary(value: Value, path: &'static str) -> Element {
    let summary = summarize(&value);
    let raw = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    let preview: String = raw.chars().take(2400).collect();
    rsx! {
        if !summary.is_empty() {
            div { class: "kv", style: "margin-bottom:10px;",
                for (k, v) in summary { div { class: "k", "{k}" } div { "{v}" } }
            }
        }
        details { summary { class: "muted", "Raw response ({path})" }
            pre { style: "margin:8px 0 0; white-space: pre-wrap; font-family: var(--ls-font-mono); font-size: var(--ls-fs-xs); color: var(--ls-ink-muted);", "{preview}" }
        }
    }
}

fn summarize(v: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(obj) = v.as_object() {
        for key in [
            "status",
            "host",
            "hostname",
            "release",
            "generation",
            "version",
            "role",
            "connected",
            "path",
            "usedBytes",
            "totalBytes",
            "reserveBytes",
            "lastRun",
        ] {
            if let Some(val) = obj.get(key) {
                let s = match val {
                    Value::String(s) => s.clone(),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    _ => continue,
                };
                out.push((key.to_string(), s));
            }
        }
        for key in [
            "apps",
            "releases",
            "entries",
            "snapshots",
            "generations",
            "options",
            "items",
            "results",
            "edges",
        ] {
            if let Some(arr) = obj.get(key).and_then(Value::as_array) {
                out.push((key.to_string(), format!("{} item(s)", arr.len())));
            }
        }
    } else if let Some(arr) = v.as_array() {
        out.push(("items".into(), format!("{} item(s)", arr.len())));
    }
    out.truncate(14);
    out
}

#[component]
pub fn SettingsPane(config: BoxConfig, on_change: EventHandler<BoxConfig>) -> Element {
    let mut host = use_signal(|| config.host.clone());
    let mut token = use_signal(|| config.token.clone());
    let mut demo = use_signal(|| config.demo);
    rsx! {
        div { class: "card",
            h2 { class: "card-title", "Connection" }
            p { class: "muted", "Dioxus frontend (Kotlin companion rewrite, cross-platform)." }
            div { class: "stack",
                div { label { "Box address" } input { r#type: "text", value: "{host}", oninput: move |e| host.set(e.value()) } }
                div { label { "Access token" } input { r#type: "password", value: "{token}", oninput: move |e| token.set(e.value()) } }
                label { class: "row", input { r#type: "checkbox", checked: *demo.read(), onchange: move |e| demo.set(e.checked()) } span { "Demo mode" } }
                div { class: "row",
                    button { class: "btn primary", onclick: { let config=config.clone(); let on_change=on_change; move |_| {
                        let mut next=config.clone(); next.host=host.read().trim().to_string(); next.token=token.read().trim().to_string(); next.demo=*demo.read(); on_change.call(next);
                    }}, "Save" }
                    button { class: "btn danger", onclick: { let config=config.clone(); move |_| {
                        let mut next=config.clone(); next.host.clear(); next.token.clear(); next.tls_pin=None; next.demo=true; on_change.call(next);
                    }}, "Disconnect" }
                }
            }
        }
        div { class: "card",
            h2 { class: "card-title", "Code reuse" }
            p { class: "muted",
                "Reuses TUI/client patterns from `admin-ui/tui` (trust semantics, formatting) and `backend/access` (`losos-access`). "
                "Next: extract a shared cross-platform client core used by both the TUI and this frontend."
            }
        }
    }
}

#[component]
pub fn HistoryPane(config: BoxConfig, on_change: EventHandler<BoxConfig>, refresh: u64) -> Element {
    let cfg = config.clone();
    let mut nonce = use_signal(|| 0u64);
    let data = use_resource(move || {
        let cfg = cfg.clone();
        let _ = refresh;
        let _ = nonce();
        async move { fetch_json(&cfg, "/api/config", false).await }
    });
    let mut busy = use_signal(|| false);
    let mut msg = use_signal(String::new);
    let mut err = use_signal(String::new);
    if let Some(Err((401, _))) = data.read().as_ref() {
        handle_401(401, &config, &on_change);
    }
    let do_sync = {
        let config = config.clone();
        let on_change = on_change;
        move |_| {
            let config = config.clone();
            let on_change = on_change;
            spawn(async move {
                busy.set(true);
                err.set(String::new());
                msg.set(String::new());
                match make_api(&config) {
                    Err(m) => err.set(m),
                    Ok(api) => match api.post("/api/config/sync").await {
                        Ok(_) => {
                            msg.set("Sync requested.".into());
                            nonce += 1;
                        }
                        Err(e) => {
                            if !handle_401(e.status, &config, &on_change) {
                                err.set(e.message);
                            }
                        }
                    },
                }
                busy.set(false);
            });
        }
    };
    rsx! { div { class: "card",
        h2 { class: "card-title", "History" }
        match &*data.read() {
            Some(Ok(v)) => rsx! { JsonSummary { value: v.clone(), path: "/api/config" } button { class: "btn", style: "margin-top:10px;", disabled: busy(), onclick: do_sync, if busy() { "Syncing…" } else { "Sync config" } } },
            Some(Err((_,m))) => rsx!{ div{ class:"banner error", "{m}" } },
            None => rsx!{ span{ class:"muted", "Loading…" } },
        }
        if !msg.read().is_empty() { div { class:"banner info", style:"margin-top:8px;", "{msg}" } }
        if !err.read().is_empty() { div { class:"banner error", style:"margin-top:8px;", "{err}" } }
    }}
}

#[component]
pub fn BackupPane(config: BoxConfig, on_change: EventHandler<BoxConfig>, refresh: u64) -> Element {
    let cfg = config.clone();
    let mut nonce = use_signal(|| 0u64);
    let data = use_resource(move || {
        let cfg = cfg.clone();
        let _ = refresh;
        let _ = nonce();
        async move { fetch_json(&cfg, "/api/backup", false).await }
    });
    let mut busy = use_signal(|| false);
    let mut msg = use_signal(String::new);
    let mut err = use_signal(String::new);
    if let Some(Err((401, _))) = data.read().as_ref() {
        handle_401(401, &config, &on_change);
    }
    let do_run = {
        let config = config.clone();
        let on_change = on_change;
        move |_| {
            let config = config.clone();
            let on_change = on_change;
            spawn(async move {
                busy.set(true);
                err.set(String::new());
                msg.set(String::new());
                match make_api(&config) {
                    Err(m) => err.set(m),
                    Ok(api) => match api.post("/api/backup/run").await {
                        Ok(_) => {
                            msg.set("Backup run requested.".into());
                            nonce += 1;
                        }
                        Err(e) => {
                            if !handle_401(e.status, &config, &on_change) {
                                err.set(e.message);
                            }
                        }
                    },
                }
                busy.set(false);
            });
        }
    };
    rsx! { div { class: "card",
        h2 { class: "card-title", "Backup" }
        match &*data.read() {
            Some(Ok(v)) => rsx! { JsonSummary { value: v.clone(), path: "/api/backup" } button { class: "btn", style: "margin-top:10px;", disabled: busy(), onclick: do_run, if busy() { "Running…" } else { "Run backup" } } },
            Some(Err((_,m))) => rsx!{ div{ class:"banner error", "{m}" } },
            None => rsx!{ span{ class:"muted", "Loading…" } },
        }
        if !msg.read().is_empty() { div { class:"banner info", style:"margin-top:8px;", "{msg}" } }
        if !err.read().is_empty() { div { class:"banner error", style:"margin-top:8px;", "{err}" } }
    }}
}

#[component]
pub fn CloudPane(config: BoxConfig, on_change: EventHandler<BoxConfig>, refresh: u64) -> Element {
    let mut path = use_signal(|| "/".to_string());
    let cfg = config.clone();
    let data = use_resource(move || {
        let cfg = cfg.clone();
        let p = path();
        let _ = refresh;
        async move {
            fetch_json(
                &cfg,
                &format!("/api/cloud/files?path={}", api::encode_query(&p)),
                false,
            )
            .await
        }
    });
    if let Some(Err((401, _))) = data.read().as_ref() {
        handle_401(401, &config, &on_change);
    }
    let go_up = move |_| {
        let cur = path();
        if cur == "/" {
            return;
        }
        let trimmed = cur.trim_end_matches('/');
        let parent = match trimmed.rsplit_once('/') {
            Some(("", _)) | None => "/".to_string(),
            Some((p, _)) if p.is_empty() => "/".to_string(),
            Some((p, _)) => p.to_string(),
        };
        path.set(parent);
    };
    rsx! {
        div { class: "card",
            div { class: "row-between",
                h2 { class: "card-title", "Cloud files" }
                button { class: "btn", disabled: path() == "/", onclick: go_up, "Up" }
            }
            div { class: "muted", "Path: {path}" }
            match &*data.read() {
                Some(Ok(v)) => {
                    let entries = v.get("entries").and_then(Value::as_array).or_else(|| v.get("files").and_then(Value::as_array)).cloned().unwrap_or_default();
                    if entries.is_empty() {
                        rsx! { JsonSummary { value: v.clone(), path: "/api/cloud/files" } }
                    } else {
                        rsx! { div { class: "stack", style: "margin-top:10px;",
                            for (i, e) in entries.iter().enumerate() {
                                {
                                    let name = api::opt_str(e, "name").unwrap_or_else(|| "item".into());
                                    let kind = api::opt_str(e, "type").or_else(|| api::opt_str(e, "kind")).unwrap_or_default();
                                    let is_dir = kind == "directory" || kind == "dir" || kind == "folder" || api::opt_bool(e, "isDirectory").unwrap_or(false);
                                    let cur = path();
                                    let next = if cur == "/" { format!("/{name}") } else { format!("{}/{}", cur.trim_end_matches('/'), name) };
                                    rsx! { div { key: "{i}", class: "row-between",
                                        div { class: "row",
                                            span { class: "chip neutral", if is_dir { "dir" } else { "file" } }
                                            span { "{name}" }
                                        }
                                        if is_dir {
                                            button { class: "btn", onclick: move |_| path.set(next.clone()), "Open" }
                                        }
                                    } }
                                }
                            }
                        } }
                    }
                }
                Some(Err((_, m))) => rsx! { div { class: "banner error", "{m}" } },
                None => rsx! { span { class: "muted", "Loading…" } },
            }
        }
    }
}
