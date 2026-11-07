//! Users/people pane (own requests). Standalone-aware basics + safe mutations.

use crate::app::{AuthSignal, AuthState, ConfigSignal};
use crate::auth::Gate;
use crate::components::ui::{Button, Card, Spinner};
use crate::settings::api;
use crate::store;
use dioxus::prelude::*;

#[component]
pub fn UsersPane() -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth_sig: AuthSignal = use_context();
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let mut view = use_signal(|| None::<api::PeopleResponse>);
    let mut busy = use_signal(|| false);
    let mut confirm_remove = use_signal(|| None::<String>);
    let mut refresh = use_signal(|| 0u64);

    let cfg = config();
    use_effect(use_reactive((&cfg.host, &cfg.token, &*refresh.read()), move |_| {
        let cfg = config();
        spawn(async move {
            loading.set(true);
            error.set(None);
            match api::get_people(&cfg).await {
                Ok(v) => view.set(Some(v)),
                Err(e) => {
                    if e.status == 401 { clear_auth(config, auth_sig); }
                    error.set(Some(e.message));
                }
            }
            loading.set(false);
        });
    }));

    rsx! {
        Card {
            h3 { "Users" }
            if *loading.read() && view().is_none() { Spinner {} }
            if let Some(err) = error.read().as_ref() { p { class: "error", "{err}" } }
            if let Some(v) = view() {
                if !v.standalone {
                    p { class: "muted", "Sign-in is managed elsewhere for this box. User management is unavailable here." }
                } else {
                    p { class: "muted", "People on this box (standalone). Destructive actions require confirmation." }
                    if let Some(you) = &v.you { p { "Signed in as " strong { "{you}" } } }
                    if v.people.is_empty() {
                        p { class: "muted", "No people found." }
                    } else {
                        ul { class: "list",
                            for p in v.people.iter().cloned() {
                                {
                                    let name = p.name.clone();
                                    let you_name = v.you.clone();
                                    let is_you = you_name.as_deref() == Some(name.as_str());
                                    let confirming = confirm_remove().as_deref() == Some(name.as_str());
                                    rsx! {
                                        li { key: "{p.name}", class: "row",
                                            div {
                                                strong { "{p.name}" }
                                                if !p.display_name.is_empty() { span { class: "muted", " ({p.display_name})" } }
                                                if let Some(r) = &p.role { span { class: "muted", " · {r}" } }
                                                if p.disabled { span { class: "muted", " · disabled" } }
                                                if p.suspended.is_some() { span { class: "muted", " · suspended" } }
                                                if p.muted.is_some() { span { class: "muted", " · muted" } }
                                            }
                                            if !is_you {
                                                if confirming {
                                                    div { class: "actions",
                                                        Button {
                                                            kind: "danger".to_string(),
                                                            disabled: *busy.read(),
                                                            onclick: move |_| {
                                                                let cfg = config();
                                                                let name = name.clone();
                                                                spawn(async move {
                                                                    busy.set(true);
                                                                    error.set(None);
                                                                    match api::post_people_remove(&cfg, &name).await {
                                                                        Ok(_) => { confirm_remove.set(None); let n = *refresh.read() + 1; refresh.set(n); }
                                                                        Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                                                                    }
                                                                    busy.set(false);
                                                                });
                                                            },
                                                            "Confirm remove"
                                                        }
                                                        Button { kind: "ghost".to_string(), disabled: *busy.read(), onclick: move |_| confirm_remove.set(None), "Cancel" }
                                                    }
                                                } else {
                                                    Button {
                                                        kind: "ghost".to_string(),
                                                        disabled: *busy.read(),
                                                        onclick: move |_| confirm_remove.set(Some(name.clone())),
                                                        "Remove"
                                                    }
                                                }
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
                Button { kind: "ghost".to_string(), disabled: *busy.read(), onclick: move |_| { let n = *refresh.read() + 1; refresh.set(n); }, "Refresh" }
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
