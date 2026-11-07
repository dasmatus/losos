//! Backup pane (own requests): status, target basics, run/restore basics.

use crate::app::{AuthSignal, AuthState, ConfigSignal};
use crate::auth::Gate;
use crate::components::ui::{Button, Card, Spinner};
use crate::settings::api;
use crate::store;
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn BackupPane() -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth_sig: AuthSignal = use_context();
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let mut view = use_signal(|| None::<Value>);
    let mut busy = use_signal(|| false);
    let mut restore_code = use_signal(String::new);
    let mut refresh = use_signal(|| 0u64);

    let cfg = config();
    use_effect(use_reactive((&cfg.host, &cfg.token, &*refresh.read()), move |_| {
        let cfg = config();
        spawn(async move {
            loading.set(true);
            error.set(None);
            match api::api_for_config(&cfg).and_then(|a| Ok(a)) {
                Ok(a) => match a.get("/api/backup").await {
                    Ok(v) => view.set(Some(v)),
                    Err(e) => {
                        if e.status == 401 { clear_auth(config, auth_sig); }
                        error.set(Some(e.message));
                    }
                },
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    }));

    rsx! {
        Card {
            h3 { "Backup" }
            p { class: "muted", "Backup target and restore. Own requests (not /api/apply). Some restore paths may require thaw/time; this UI shows honest status/errors." }

            if *loading.read() && view().is_none() { Spinner {} }
            if let Some(err) = error.read().as_ref() { p { class: "error", "{err}" } }
            if let Some(v) = view() {
                pre { class: "mono", "{serde_json::to_string_pretty(&v).unwrap_or_default()}" }
            }

            div { class: "actions", style: "display:flex; gap:8px; flex-wrap:wrap; margin-top:12px;",
                Button {
                    kind: "secondary".to_string(),
                    disabled: *busy.read(),
                    onclick: move |_| {
                        let cfg = config();
                        spawn(async move {
                            busy.set(true);
                            error.set(None);
                            match api::post_backup_run(&cfg).await {
                                Ok(_) => { let n = *refresh.read() + 1; refresh.set(n); }
                                Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                            }
                            busy.set(false);
                        });
                    },
                    if *busy.read() { "Working…" } else { "Run backup" }
                }
                if view().as_ref().and_then(|v| v.get("target")).is_some_and(|t| !t.is_null()) {
                    Button {
                        kind: "ghost".to_string(),
                        disabled: *busy.read(),
                        onclick: move |_| {
                            let cfg = config();
                            spawn(async move {
                                busy.set(true);
                                error.set(None);
                                match api::delete_backup_target(&cfg).await {
                                    Ok(_) => { let n = *refresh.read() + 1; refresh.set(n); }
                                    Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                                }
                                busy.set(false);
                            });
                        },
                        "Remove backup target"
                    }
                }
                Button { kind: "ghost".to_string(), onclick: move |_| { let n = *refresh.read() + 1; refresh.set(n); }, "Refresh" }
            }

            h4 { style: "margin-top:16px;", "Restore" }
            p { class: "muted", "Restore uses a recovery code. Review backup state and any thaw/warning requirements before restoring." }
            div { class: "field",
                label { "Recovery code" }
                input { class: "input", r#type: "password", value: "{restore_code}", oninput: move |e| restore_code.set(e.value()) }
            }
            Button {
                kind: "primary".to_string(),
                disabled: *busy.read() || restore_code.read().trim().is_empty(),
                onclick: move |_| {
                    let cfg = config();
                    let code = restore_code.read().trim().to_string();
                    spawn(async move {
                        busy.set(true);
                        error.set(None);
                        match api::post_backup_restore(&cfg, &code).await {
                            Ok(_) => { restore_code.set(String::new()); let n = *refresh.read() + 1; refresh.set(n); }
                            Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                        }
                        busy.set(false);
                    });
                },
                "Restore"
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
