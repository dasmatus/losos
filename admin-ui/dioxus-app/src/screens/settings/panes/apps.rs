//! Apps pane (own requests): installed apps + catalogue basics.

use crate::app::{AuthSignal, AuthState, ConfigSignal};
use crate::auth::Gate;
use crate::components::ui::{Button, Card, Spinner};
use crate::settings::api;
use crate::store;
use dioxus::prelude::*;

#[component]
pub fn AppsPane() -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth_sig: AuthSignal = use_context();
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let mut view = use_signal(|| None::<api::AppsResponse>);
    let mut query = use_signal(String::new);
    let mut results = use_signal(|| None::<api::CatalogueResults>);
    let mut busy = use_signal(|| false);
    let mut refresh = use_signal(|| 0u64);

    let cfg = config();
    use_effect(use_reactive((&cfg.host, &cfg.token, &*refresh.read()), move |_| {
        let cfg = config();
        spawn(async move {
            loading.set(true);
            error.set(None);
            match api::get_apps(&cfg).await {
                Ok(v) => view.set(Some(v)),
                Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
            }
            loading.set(false);
        });
    }));

    rsx! {
        Card {
            h3 { "Apps" }
            if *loading.read() && view().is_none() { Spinner {} }
            if let Some(err) = error.read().as_ref() { p { class: "error", "{err}" } }
            if let Some(v) = view() {
                if !v.available {
                    p { class: "muted", "Apps are not available: {v.reason.clone().unwrap_or_else(|| \"unknown\".into())}" }
                } else if v.apps.is_empty() {
                    p { class: "muted", "No apps installed." }
                } else {
                    h4 { "Installed" }
                    ul { class: "list",
                        for app in v.apps.iter().cloned() {
                            {
                                let rel = app.release.clone();
                                rsx! {
                                    li { key: "{app.release}", class: "row",
                                        div {
                                            strong { "{app.title.clone().unwrap_or(app.release.clone())}" }
                                            span { class: "muted", " · {app.phase} · {app.release}" }
                                            if let Some(msg) = &app.message { span { class: "muted", " · {msg}" } }
                                        }
                                        Button {
                                            kind: "ghost".to_string(),
                                            disabled: *busy.read(),
                                            onclick: move |_| {
                                                let cfg = config();
                                                let rel = rel.clone();
                                                spawn(async move {
                                                    busy.set(true);
                                                    match api::post_app_remove(&cfg, &rel).await {
                                                        Ok(_) => { let n = *refresh.read() + 1; refresh.set(n); }
                                                        Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                                                    }
                                                    busy.set(false);
                                                });
                                            },
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h4 { style: "margin-top:16px;", "Catalogue (basic)" }
            div { class: "row",
                input { class: "input", placeholder: "Search apps…", value: "{query}", oninput: move |e| query.set(e.value()) }
                Button {
                    kind: "secondary".to_string(),
                    disabled: *busy.read() || query.read().trim().is_empty(),
                    onclick: move |_| {
                        let cfg = config();
                        let q = query.read().trim().to_string();
                        spawn(async move {
                            busy.set(true);
                            match api::search_catalogue(&cfg, &q).await {
                                Ok(r) => results.set(Some(r)),
                                Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                            }
                            busy.set(false);
                        });
                    },
                    "Search"
                }
            }
            if let Some(r) = results() {
                if r.results.is_empty() {
                    p { class: "muted", "No results." }
                } else {
                    ul { class: "list",
                        for item in r.results.iter().cloned() {
                            {
                                let chart = item.chart.clone();
                                let release = chart
                                    .as_ref()
                                    .map(|c| c.name.clone())
                                    .unwrap_or_else(|| item.id.clone());
                                rsx! {
                                    li { key: "{item.id}", class: "row",
                                        div {
                                            strong { "{item.name}" }
                                            if let Some(s) = &item.summary { span { class: "muted", " — {s}" } }
                                            if let Some(v) = &item.version { span { class: "muted", " ({v})" } }
                                        }
                                        if let Some(chart) = chart {
                                            Button {
                                                kind: "secondary".to_string(),
                                                disabled: *busy.read(),
                                                onclick: move |_| {
                                                    let cfg = config();
                                                    let release = release.clone();
                                                    let chart = chart.clone();
                                                    spawn(async move {
                                                        busy.set(true);
                                                        error.set(None);
                                                        let req = serde_json::json!({
                                                            "release": release,
                                                            "chart": {
                                                                "repo": chart.repo,
                                                                "name": chart.name,
                                                                "version": chart.version,
                                                            },
                                                            "runAs": "user",
                                                            "values": {},
                                                            "valuesYaml": null,
                                                        });
                                                        match api::post_app_install(&cfg, req).await {
                                                            Ok(_) => { let n = *refresh.read() + 1; refresh.set(n); }
                                                            Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                                                        }
                                                        busy.set(false);
                                                    });
                                                },
                                                "Install"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "actions", style: "margin-top:12px;",
                Button { kind: "ghost".to_string(), onclick: move |_| { let n = *refresh.read() + 1; refresh.set(n); }, "Refresh" }
            }
        }
    }
}

fn clear_auth(mut config: ConfigSignal, mut auth_sig: AuthSignal) {
    let mut next = config();
    crate::auth::drop_token(&mut next);
    store::save(&next);
    config.set(next);
    auth_sig.set(AuthState { gate: Gate::Signin, me: None, claim: auth_sig().claim });
}
