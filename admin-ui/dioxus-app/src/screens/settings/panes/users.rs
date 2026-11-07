//! Users/people pane (own requests). Standalone-aware basics.

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
                    p { class: "muted", "People on this box (standalone). Role/group editing UI is incremental; data shown for parity." }
                    if let Some(you) = &v.you { p { "Signed in as " strong { "{you}" } } }
                    if v.people.is_empty() {
                        p { class: "muted", "No people found." }
                    } else {
                        ul { class: "list",
                            for p in v.people.iter().cloned() {
                                li { key: "{p.name}",
                                    strong { "{p.name}" }
                                    if !p.display_name.is_empty() { span { class: "muted", " ({p.display_name})" } }
                                    if let Some(r) = &p.role { span { class: "muted", " · {r}" } }
                                    if p.disabled { span { class: "muted", " · disabled" } }
                                    if p.suspended.is_some() { span { class: "muted", " · suspended" } }
                                    if p.muted.is_some() { span { class: "muted", " · muted" } }
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
