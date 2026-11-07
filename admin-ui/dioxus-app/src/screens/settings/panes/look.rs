//! Look pane (own requests, immediate save; no Apply bar).

use crate::app::{AuthSignal, AuthState, ConfigSignal};
use crate::auth::Gate;
use crate::components::ui::{Button, Card, Spinner};
use crate::settings::api;
use crate::store;
use dioxus::prelude::*;

#[component]
pub fn LookPane() -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth_sig: AuthSignal = use_context();
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let mut view = use_signal(|| None::<api::LookResponse>);
    let mut busy = use_signal(|| false);
    let mut veil = use_signal(|| 60i64);
    let mut refresh = use_signal(|| 0u64);

    let cfg = config();
    use_effect(use_reactive((&cfg.host, &cfg.token, &*refresh.read()), move |_| {
        let cfg = config();
        spawn(async move {
            loading.set(true);
            error.set(None);
            match api::get_look(&cfg).await {
                Ok(v) => {
                    veil.set(v.veil);
                    view.set(Some(v));
                }
                Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
            }
            loading.set(false);
        });
    }));

    rsx! {
        Card {
            h3 { "Look" }
            p { class: "muted", "Appearance changes save immediately (no rebuild/Apply). Background/veil/widgets parity is incremental." }
            if *loading.read() && view().is_none() { Spinner {} }
            if let Some(err) = error.read().as_ref() { p { class: "error", "{err}" } }
            if let Some(v) = view() {
                p { "Background: " strong { "{v.background.kind}" } if let Some(n) = &v.background.name { span { " ({n})" } } }
                p { class: "muted", if v.personal { "Personal background." } else { "Box background." } }
                div { class: "field",
                    label { "Veil ({veil})" }
                    input {
                        r#type: "range",
                        min: "20",
                        max: "90",
                        value: "{veil}",
                        oninput: move |e| if let Ok(n) = e.value().parse::<i64>() { veil.set(n); }
                    }
                }
                div { class: "actions", style: "display:flex; gap:8px;",
                    Button {
                        kind: "primary".to_string(),
                        disabled: *busy.read(),
                        onclick: move |_| {
                            let cfg = config();
                            let v = *veil.read();
                            spawn(async move {
                                busy.set(true);
                                match api::post_look(&cfg, serde_json::json!({ "veil": v })).await {
                                    Ok(next) => { veil.set(next.veil); view.set(Some(next)); }
                                    Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                                }
                                busy.set(false);
                            });
                        },
                        "Save veil"
                    }
                    Button {
                        kind: "secondary".to_string(),
                        disabled: *busy.read(),
                        onclick: move |_| {
                            let cfg = config();
                            spawn(async move {
                                busy.set(true);
                                match api::delete_look_background(&cfg).await {
                                    Ok(next) => view.set(Some(next)),
                                    Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                                }
                                busy.set(false);
                            });
                        },
                        "Remove background"
                    }
                }
                if !v.shipped.is_empty() {
                    p { class: "muted", style: "margin-top:8px;", "Shipped backgrounds: {v.shipped.join(\", \")}" }
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
