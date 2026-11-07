//! Access tokens pane (own requests, not draft/apply).

use crate::app::{AuthSignal, AuthState, ConfigSignal};
use crate::auth::Gate;
use crate::components::ui::{Button, Card, Spinner};
use crate::settings::api::{self, AccessResponse};
use crate::store;
use dioxus::prelude::*;

#[component]
pub fn AccessPane() -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth_sig: AuthSignal = use_context();
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let mut view = use_signal(|| None::<AccessResponse>);
    let mut busy = use_signal(|| false);
    let mut created_token = use_signal(|| None::<String>); // show once
    let mut name = use_signal(String::new);
    let mut fingerprint = use_signal(String::new);
    let mut scope = use_signal(|| "read".to_string());
    let mut refresh = use_signal(|| 0u64);

    let cfg = config();
    use_effect(use_reactive((&cfg.host, &cfg.token, &*refresh.read()), move |_| {
        let cfg = config();
        spawn(async move {
            loading.set(true);
            error.set(None);
            match api::get_access(&cfg).await {
                Ok(v) => view.set(Some(v)),
                Err(e) => {
                    if e.status == 401 {
                        clear_auth(config, auth_sig);
                    }
                    error.set(Some(e.message));
                }
            }
            loading.set(false);
        });
    }));

    rsx! {
        Card {
            h3 { "Access tokens" }
            p { class: "muted", "Tokens are bound to keys. A created token secret is shown once — copy it now." }

            if *loading.read() && view().is_none() {
                Spinner {}
            }
            if let Some(err) = error.read().as_ref() {
                p { class: "error", "{err}" }
            }
            if let Some(tok) = created_token.read().as_ref() {
                Card { class: "notice ok",
                    h4 { "Token created (shown once)" }
                    pre { class: "mono", "{tok}" }
                    p { class: "muted", "Save this token now. It cannot be shown again." }
                    Button { kind: "secondary".to_string(), onclick: move |_| created_token.set(None), "Dismiss" }
                }
            }

            if let Some(v) = view() {
                if let Some(hk) = &v.host_key {
                    p { class: "muted", "Host key: " span { class: "mono", "{hk.fingerprint}" } }
                }
                h4 { "Tokens" }
                if v.tokens.is_empty() {
                    p { class: "muted", "No tokens." }
                } else {
                    ul { class: "list",
                        for t in v.tokens.iter().cloned() {
                            {
                                let id = t.id.clone();
                                let label = t.name.clone();
                                rsx! {
                                    li { key: "{t.id}", class: "row",
                                        div {
                                            strong { "{t.name}" }
                                            span { class: "muted", " · {t.scope} · {t.fingerprint}" }
                                        }
                                        Button {
                                            kind: "ghost".to_string(),
                                            disabled: *busy.read(),
                                            onclick: move |_| {
                                                let id = id.clone();
                                                let label = label.clone();
                                                let cfg = config();
                                                spawn(async move {
                                                    busy.set(true);
                                                    match api::post_access_revoke(&cfg, &id).await {
                                                        Ok(_) => {
                                                            let n = *refresh.read() + 1;
                                                            refresh.set(n);
                                                        }
                                                        Err(e) => {
                                                            if e.status == 401 { clear_auth(config, auth_sig); }
                                                            error.set(Some(e.message));
                                                        }
                                                    }
                                                    busy.set(false);
                                                });
                                            },
                                            "Revoke"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                h4 { "Create token" }
                div { class: "field",
                    label { "Name" }
                    input { class: "input", value: "{name}", oninput: move |e| name.set(e.value()) }
                }
                div { class: "field",
                    label { "Key fingerprint" }
                    input { class: "input", placeholder: "GPG fingerprint bound to this token", value: "{fingerprint}", oninput: move |e| fingerprint.set(e.value()) }
                    if !v.keys.is_empty() {
                        p { class: "muted", "Known keys: {v.keys.iter().map(|k| k.fingerprint.clone()).collect::<Vec<_>>().join(\", \")}" }
                    }
                }
                div { class: "field",
                    label { "Scope" }
                    select { class: "input", value: "{scope}", onchange: move |e| scope.set(e.value()),
                        option { value: "read", "read" }
                        option { value: "full", "full" }
                    }
                }
                Button {
                    kind: "primary".to_string(),
                    disabled: *busy.read() || name.read().trim().is_empty() || fingerprint.read().trim().is_empty(),
                    onclick: move |_| {
                        let cfg = config();
                        let n = name.read().trim().to_string();
                        let fp = fingerprint.read().trim().to_string();
                        let sc = scope.read().clone();
                        spawn(async move {
                            busy.set(true);
                            error.set(None);
                            match api::post_access_token(&cfg, &n, &fp, &sc, None).await {
                                Ok(created) => {
                                    created_token.set(Some(created.token));
                                    name.set(String::new());
                                    let n = *refresh.read() + 1;
                                    refresh.set(n);
                                }
                                Err(e) => {
                                    if e.status == 401 { clear_auth(config, auth_sig); }
                                    error.set(Some(e.message));
                                }
                            }
                            busy.set(false);
                        });
                    },
                    if *busy.read() { "Working…" } else { "Create token" }
                }
            }

            div { class: "actions", style: "margin-top: 12px;",
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
    auth_sig.set(AuthState {
        gate: Gate::Signin,
        me: None,
        claim: auth_sig().claim,
    });
}
