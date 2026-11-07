use super::super::shell::SettingsFormSignal;
use crate::app::{AuthSignal, AuthState, ConfigSignal};
use crate::auth::Gate;
use crate::components::ui::{Button, Card, Spinner};
use crate::settings::api;
use crate::store;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn MeshPane() -> Element {
    let mut form: SettingsFormSignal = use_context();
    let mut config: ConfigSignal = use_context();
    let mut auth_sig: AuthSignal = use_context();
    let mut edge = use_signal(|| None::<Result<Value, String>>);
    let mut busy = use_signal(|| false);
    let mut refresh = use_signal(|| 0u64);

    let cfg = config();
    use_effect(use_reactive((&cfg.host, &cfg.token, &*refresh.read()), move |_| {
        let cfg = config();
        spawn(async move {
            edge.set(None);
            match api::get_edge(&cfg).await {
                Ok(v) => edge.set(Some(Ok(v))),
                Err(e) => {
                    if e.status == 401 {
                        clear_auth(config, auth_sig);
                    }
                    edge.set(Some(Err(e.message)));
                }
            }
        });
    }));

    let f = form();
    let Some(draft) = f.draft.clone() else {
        return rsx! { Card { p { class: "muted", "Settings not loaded." } } };
    };
    let probs = f.problems();
    let disabled = f.locked || !f.ready;
    let joined = draft.cluster_enable;
    let sharing = draft.share_compute;
    let window_disabled = disabled || !joined;

    let edge_manage = edge()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|v| v.get("manage"))
        .cloned()
        .unwrap_or(Value::Null);
    let manage_active = edge_manage.get("active").and_then(Value::as_bool).unwrap_or(false);
    let manage_candidate = edge_manage.get("candidate").is_some() && !edge_manage.get("candidate").unwrap_or(&Value::Null).is_null();

    rsx! {
        Card {
            h3 { "Edge (live)" }
            p { class: "muted",
                "Live edge scan and management actions. Join/compute settings below are draft values applied through the full /api/apply settings flow (13 owned keys + extras preserved)."
            }
            match edge() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                Some(Ok(v)) => rsx! {
                    div { class: "facts",
                        dt { "Reachable" } dd { if v.get("reachable").and_then(Value::as_bool).unwrap_or(false) { "Yes" } else { "No" } }
                        dt { "Official edge" } dd { if v.get("official").and_then(Value::as_bool).unwrap_or(false) { "Yes" } else { "No" } }
                        dt { "Management" } dd { if manage_active { "Active" } else if manage_candidate { "Available" } else { "Unavailable" } }
                    }
                    details {
                        summary { "Raw edge status" }
                        pre { class: "mono", "{serde_json::to_string_pretty(&v).unwrap_or_default()}" }
                    }
                }
            }
            div { class: "actions", style: "display:flex; gap:8px; flex-wrap:wrap; margin-top:12px;",
                Button {
                    kind: "ghost".to_string(),
                    disabled: *busy.read(),
                    onclick: move |_| { let n = *refresh.read() + 1; refresh.set(n); },
                    "Refresh edge"
                }
                if manage_candidate || manage_active {
                    Button {
                        kind: "secondary".to_string(),
                        disabled: *busy.read(),
                        onclick: move |_| {
                            let cfg = config();
                            let enable = !manage_active;
                            spawn(async move {
                                busy.set(true);
                                match api::post_edge_manage(&cfg, enable).await {
                                    Ok(_) => { let n = *refresh.read() + 1; refresh.set(n); }
                                    Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } edge.set(Some(Err(e.message))); }
                                }
                                busy.set(false);
                            });
                        },
                        if manage_active { "Stop edge management" } else { "Allow edge management" }
                    }
                }
            }
        }

        Card {
            h3 { "Other boxes" }
            div { class: "field",
                label { class: "row", r#for: "clusterEnable",
                    input {
                        id: "clusterEnable",
                        r#type: "checkbox",
                        checked: joined,
                        disabled: disabled,
                        onchange: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.cluster_enable = e.checked();
                            }
                            form.set(f);
                        },
                    }
                    span { "Join the mesh (losos.cluster.enable)" }
                }
                p { class: "muted",
                    "The box may refuse sharing/cluster settings it cannot honor without a reachable edge."
                }
            }
        }

        Card {
            h3 { "Spare time" }
            div { class: "field",
                label { class: "row", r#for: "shareCompute",
                    input {
                        id: "shareCompute",
                        r#type: "checkbox",
                        checked: sharing,
                        disabled: window_disabled,
                        onchange: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.share_compute = e.checked();
                            }
                            form.set(f);
                        },
                    }
                    span { "Lend spare compute time (losos.cluster.shareCompute)" }
                }
                if window_disabled && !disabled {
                    p { class: "muted", "Join the mesh first to set compute sharing hours." }
                }
            }

            div { class: "field",
                label { "Compute window (local time)" }
                div { class: "row", style: "gap: 8px; flex-wrap: wrap; align-items: center;",
                    label { class: "muted", r#for: "computeWindowStart", "From" }
                    input {
                        id: "computeWindowStart",
                        class: if probs.compute_window.is_some() { "input error" } else { "input" },
                        r#type: "time",
                        value: "{draft.compute_window_start}",
                        disabled: window_disabled,
                        oninput: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.compute_window_start = e.value();
                            }
                            form.set(f);
                        },
                    }
                    span { class: "muted", "until" }
                    label { class: "muted", r#for: "computeWindowEnd", "To" }
                    input {
                        id: "computeWindowEnd",
                        class: if probs.compute_window.is_some() { "input error" } else { "input" },
                        r#type: "time",
                        value: "{draft.compute_window_end}",
                        disabled: window_disabled,
                        oninput: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.compute_window_end = e.value();
                            }
                            form.set(f);
                        },
                    }
                }
                if let Some(err) = probs.compute_window.as_ref() {
                    p { class: "error", "{err}" }
                } else {
                    p { class: "muted",
                        "Hours are interpreted in your local time and travel with the setting. Windows that cross midnight are allowed."
                    }
                }
            }
        }
    }
}

fn clear_auth(mut config: ConfigSignal, mut auth_sig: AuthSignal) {
    let mut next = config();
    crate::auth::drop_token(&mut next);
    store::save(&next);
    config.set(next);
    auth_sig.set(AuthState {
        gate: Gate::Signin,
        me: None,
        claim: auth_sig().claim,
    });
}
