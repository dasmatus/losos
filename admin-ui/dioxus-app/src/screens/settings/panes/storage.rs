use super::super::shell::SettingsFormSignal;
use crate::app::ConfigSignal;
use crate::components::ui::{Button, Card, Spinner};
use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
struct StorageFacts {
    total_bytes: Option<f64>,
    used_bytes: Option<f64>,
    reserve_bytes: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
struct GrowResponse {
    #[serde(default)]
    grew: bool,
    #[serde(default)]
    before_bytes: f64,
    #[serde(default)]
    after_bytes: f64,
    #[serde(default)]
    claimed_bytes: f64,
}

fn fmt_bytes(v: Option<f64>) -> String {
    let Some(n) = v else {
        return "Not reported".to_string();
    };
    if n < 0.0 {
        return "Not reported".to_string();
    }
    // Keep it simple and dependency-free; losos_client::format_bytes exists but
    // this keeps the settings pane self-contained and readable.
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    if n >= TB {
        format!("{:.1} TB", n / TB)
    } else if n >= GB {
        format!("{:.1} GB", n / GB)
    } else if n >= MB {
        format!("{:.0} MB", n / MB)
    } else if n >= KB {
        format!("{:.0} KB", n / KB)
    } else {
        format!("{:.0} B", n)
    }
}

#[component]
pub fn StoragePane() -> Element {
    let form: SettingsFormSignal = use_context();
    let config: ConfigSignal = use_context();
    let mut facts = use_signal(|| None::<StorageFacts>);
    let mut load_error = use_signal(|| None::<String>);
    let mut loading = use_signal(|| false);
    let mut growing = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);
    let mut confirm_grow = use_signal(|| false);
    let mut refresh = use_signal(|| 0u64);

    // Load storage facts. This is an own request (GET /api/storage), not part of
    // settings apply. Re-read after grow or manual refresh.
    let cfg_key = {
        let c = config();
        (c.host.clone(), c.token.clone(), *refresh.read())
    };
    use_effect(use_reactive(&cfg_key, move |_| {
        let cfg = config();
        spawn(async move {
            loading.set(true);
            load_error.set(None);
            let api = match crate::settings::api::api_for_config(&cfg) {
                Ok(a) => a,
                Err(e) => {
                    loading.set(false);
                    load_error.set(Some(e.message));
                    return;
                }
            };
            match api.get("/api/storage").await {
                Ok(v) => {
                    match serde_json::from_value::<StorageFacts>(v) {
                        Ok(f) => facts.set(Some(f)),
                        Err(e) => load_error.set(Some(format!("storage: {e}"))),
                    }
                }
                Err(e) => {
                    // 404 / not present is reported honestly.
                    load_error.set(Some(e.message));
                }
            }
            loading.set(false);
        });
    }));

    let f = form();
    let form_blocked = f.locked || f.applying;
    let current = facts().unwrap_or_default();
    let reserve = current.reserve_bytes;
    let has_reserve = reserve.map(|n| n > 0.0).unwrap_or(false);
    let grow_disabled = form_blocked || *growing.read() || *loading.read() || !has_reserve;
    let mut facts_sig = facts;

    rsx! {
        Card {
            h3 { "Storage" }
            p { class: "muted",
                "Disk facts are read directly from the box (own request). Claiming reserve is an own request too, integrated with settings form lock/applying awareness and never mixed into /api/apply."
            }

            if *loading.read() && facts().is_none() {
                Spinner {}
            } else if let Some(err) = load_error.read().as_ref() {
                p { class: "error", "{err}" }
                Button {
                    kind: "secondary".to_string(),
                    onclick: move |_| {
                        let n = *refresh.read() + 1;
                        refresh.set(n);
                    },
                    "Retry"
                }
            }

            if let Some(facts_val) = facts() {
                div { class: "facts",
                    dt { "Total" } dd { "{fmt_bytes(facts_val.total_bytes)}" }
                    dt { "Used" } dd { "{fmt_bytes(facts_val.used_bytes)}" }
                    dt { "Reserve available" } dd { "{fmt_bytes(facts_val.reserve_bytes)}" }
                }

                div { class: "field", style: "margin-top: 12px;",
                    h4 { "Held reserve" }
                    p { class: "muted",
                        if reserve == Some(0.0) && !*growing.read() {
                            "No unallocated reserve is available to claim."
                        } else {
                            "Claim unallocated volume-group space into the filesystem. This is slow and cannot be cancelled."
                        }
                    }
                    if let Some(msg) = message.read().as_ref() {
                        p { "{msg}" }
                    }
                    if !*confirm_grow.read() {
                        Button {
                            kind: "secondary".to_string(),
                            disabled: grow_disabled,
                            onclick: move |_| {
                                message.set(None);
                                confirm_grow.set(true);
                            },
                            if *growing.read() { "Claiming reserve…" } else if has_reserve { "Use reserve" } else { "Reserve not available" }
                        }
                    } else {
                        div { class: "actions", style: "display: flex; gap: 8px; flex-wrap: wrap; align-items: center;",
                            span { class: "muted", "Claim {fmt_bytes(reserve)} into the filesystem?" }
                            Button {
                                kind: "ghost".to_string(),
                                disabled: *growing.read(),
                                onclick: move |_| confirm_grow.set(false),
                                "Cancel"
                            }
                            Button {
                                kind: "primary".to_string(),
                                disabled: form_blocked || *growing.read(),
                                onclick: move |_| {
                                    let cfg = config();
                                    spawn(async move {
                                        growing.set(true);
                                        message.set(None);
                                        let api = match crate::settings::api::api_for_config(&cfg) {
                                            Ok(a) => a,
                                            Err(e) => {
                                                growing.set(false);
                                                confirm_grow.set(false);
                                                message.set(Some(e.message));
                                                return;
                                            }
                                        };
                                        // POST /api/grow is an own request. Do not route through /api/apply.
                                        match api.post_json("/api/grow", json!({})).await {
                                            Ok(v) => {
                                                match serde_json::from_value::<GrowResponse>(v) {
                                                    Ok(r) => {
                                                        if r.grew {
                                                            facts.set(Some(StorageFacts {
                                                                total_bytes: Some(r.after_bytes),
                                                                used_bytes: facts().and_then(|f| f.used_bytes),
                                                                reserve_bytes: Some(0.0),
                                                            }));
                                                            message.set(Some(format!(
                                                                "Reserve claimed: {} → {}.",
                                                                fmt_bytes(Some(r.before_bytes)),
                                                                fmt_bytes(Some(r.after_bytes))
                                                            )));
                                                        } else if r.claimed_bytes > 0.0 {
                                                            message.set(Some(format!(
                                                                "Nothing changed. {} was available to claim.",
                                                                fmt_bytes(Some(r.claimed_bytes))
                                                            )));
                                                        } else {
                                                            message.set(Some("Nothing changed. No reserve was available.".into()));
                                                        }
                                                    }
                                                    Err(e) => message.set(Some(format!("grow: {e}"))),
                                                }
                                            }
                                            Err(e) => message.set(Some(e.message)),
                                        }
                                        growing.set(false);
                                        confirm_grow.set(false);
                                        let n = *refresh.read() + 1;
                                        refresh.set(n);
                                    });
                                },
                                "Confirm"
                            }
                        }
                    }
                }
            }

            div { class: "actions", style: "margin-top: 12px;",
                Button {
                    kind: "ghost".to_string(),
                    disabled: *loading.read() || *growing.read(),
                    onclick: move |_| {
                        let n = *refresh.read() + 1;
                        refresh.set(n);
                    },
                    "Refresh"
                }
            }
        }
    }
}
