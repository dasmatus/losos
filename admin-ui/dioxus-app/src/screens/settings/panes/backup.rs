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
    let mut bought_order = use_signal(String::new);
    let mut target_endpoint = use_signal(String::new);
    let mut target_bucket = use_signal(String::new);
    let mut target_prefix = use_signal(String::new);
    let mut target_region = use_signal(String::new);
    let mut target_access_key = use_signal(String::new);
    let mut target_secret = use_signal(String::new);
    let mut target_storage_class = use_signal(|| "standard".to_string());
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

            h4 { style: "margin-top:16px;", "Backup target (S3-compatible)" }
            p { class: "muted", "Set where backups go. Leave secret key empty to keep an existing stored secret when supported by the box." }
            div { class: "field", label { "Endpoint" } input { class: "input", value: "{target_endpoint}", oninput: move |e| target_endpoint.set(e.value()) } }
            div { class: "field", label { "Bucket" } input { class: "input", value: "{target_bucket}", oninput: move |e| target_bucket.set(e.value()) } }
            div { class: "field", label { "Prefix" } input { class: "input", value: "{target_prefix}", oninput: move |e| target_prefix.set(e.value()) } }
            div { class: "field", label { "Region" } input { class: "input", value: "{target_region}", oninput: move |e| target_region.set(e.value()) } }
            div { class: "field", label { "Access key ID" } input { class: "input", value: "{target_access_key}", oninput: move |e| target_access_key.set(e.value()) } }
            div { class: "field", label { "Secret access key" } input { class: "input", r#type: "password", value: "{target_secret}", oninput: move |e| target_secret.set(e.value()) } }
            div { class: "field",
                label { "Storage class" }
                select { class: "input", value: "{target_storage_class}", onchange: move |e| target_storage_class.set(e.value()),
                    option { value: "standard", "Standard" }
                    option { value: "infrequent", "Infrequent access" }
                    option { value: "archive", "Archive" }
                }
            }
            Button {
                kind: "secondary".to_string(),
                disabled: *busy.read() || target_bucket.read().trim().is_empty(),
                onclick: move |_| {
                    let cfg = config();
                    let mut body = serde_json::json!({
                        "endpoint": target_endpoint.read().trim(),
                        "bucket": target_bucket.read().trim(),
                        "prefix": target_prefix.read().trim(),
                        "region": target_region.read().trim(),
                        "accessKeyId": target_access_key.read().trim(),
                        "storageClass": target_storage_class.read().trim(),
                    });
                    let secret = target_secret.read().trim().to_string();
                    if !secret.is_empty() {
                        body["secretAccessKey"] = serde_json::Value::String(secret);
                    }
                    spawn(async move {
                        busy.set(true);
                        error.set(None);
                        match api::post_backup_target(&cfg, body).await {
                            Ok(_) => { target_secret.set(String::new()); let n = *refresh.read() + 1; refresh.set(n); }
                            Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                        }
                        busy.set(false);
                    });
                },
                "Save backup target"
            }

            h4 { style: "margin-top:16px;", "Bought storage (mesh)" }
            p { class: "muted", "Use storage bought on the mesh for backups (POST /api/backup/target/bought)." }
            div { class: "row", style: "gap:8px; flex-wrap:wrap; align-items:flex-end;",
                div { class: "field", style: "flex:1; min-width:220px;",
                    label { "Order ID" }
                    input { class: "input", value: "{bought_order}", oninput: move |e| bought_order.set(e.value()) }
                }
                Button {
                    kind: "secondary".to_string(),
                    disabled: *busy.read() || bought_order.read().trim().is_empty(),
                    onclick: move |_| {
                        let cfg = config();
                        let order = bought_order.read().trim().to_string();
                        spawn(async move {
                            busy.set(true);
                            error.set(None);
                            match api::post_backup_target_bought(&cfg, &order).await {
                                Ok(_) => { bought_order.set(String::new()); let n = *refresh.read() + 1; refresh.set(n); }
                                Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                            }
                            busy.set(false);
                        });
                    },
                    "Use bought storage"
                }
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

            h4 { style: "margin-top:16px;", "Erase (destructive)" }
            p { class: "muted", "Erase is destructive and follows the box's erase flow. Use only when you understand the consequences; cancel may be possible before it reaches the edge." }
            div { class: "actions", style: "display:flex; gap:8px; flex-wrap:wrap; margin-top:8px;",
                Button {
                    kind: "ghost".to_string(),
                    disabled: *busy.read(),
                    onclick: move |_| {
                        let cfg = config();
                        spawn(async move {
                            busy.set(true);
                            error.set(None);
                            match api::post_erase_cancel(&cfg).await {
                                Ok(_) => { let n = *refresh.read() + 1; refresh.set(n); }
                                Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                            }
                            busy.set(false);
                        });
                    },
                    "Cancel erase"
                }
                Button {
                    kind: "danger".to_string(),
                    disabled: *busy.read(),
                    onclick: move |_| {
                        let cfg = config();
                        spawn(async move {
                            busy.set(true);
                            error.set(None);
                            // `backup: false` — conservative default; full erase UX parity pending.
                            match api::post_erase(&cfg, false).await {
                                Ok(_) => { let n = *refresh.read() + 1; refresh.set(n); }
                                Err(e) => { if e.status == 401 { clear_auth(config, auth_sig); } error.set(Some(e.message)); }
                            }
                            busy.set(false);
                        });
                    },
                    "Erase box"
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
