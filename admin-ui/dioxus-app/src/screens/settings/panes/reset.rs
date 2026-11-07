use super::super::shell::SettingsFormSignal;
use crate::app::ConfigSignal;
use crate::components::ui::{Button, Card};
use dioxus::prelude::*;

#[component]
pub fn ResetPane() -> Element {
    let mut form: SettingsFormSignal = use_context();
    let config: ConfigSignal = use_context();
    let mut confirming = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let f = form();
    let blocked = f.locked || !f.ready || f.applying;

    rsx! {
        Card {
            h3 { "Start over" }
            p { class: "muted",
                if f.applying {
                    "A change is being applied. Wait for it to finish before resetting."
                } else {
                    "Factory reset restores default settings and rebuilds the box. It cannot be taken back."
                }
            }

            if !*confirming.read() {
                div { class: "actions",
                    Button {
                        kind: "danger".to_string(),
                        disabled: blocked,
                        onclick: move |_| {
                            error.set(None);
                            confirming.set(true);
                        },
                        "Reset to factory defaults"
                    }
                }
            } else {
                div { class: "card", style: "border: 1px solid var(--danger, #c00); padding: 12px; margin-top: 8px;",
                    h4 { "Reset this box?" }
                    p { "This restores settings to their defaults and rebuilds the box." }
                    ul {
                        li { "Settings and overrides return to defaults." }
                        li { "Your files and data stay where they are." }
                        li { "The box rebuilds to apply the reset." }
                    }
                    if let Some(err) = error.read().as_ref() {
                        p { class: "error", "{err}" }
                    }
                    div { class: "actions", style: "display: flex; gap: 8px; flex-wrap: wrap;",
                        Button {
                            kind: "ghost".to_string(),
                            disabled: f.applying,
                            onclick: move |_| confirming.set(false),
                            "Keep current settings"
                        }
                        Button {
                            kind: "danger".to_string(),
                            disabled: blocked,
                            onclick: move |_| {
                                let cfg = config();
                                spawn(async move {
                                    let mut f = form();
                                    if let Err(e) = f.factory_reset(&cfg).await {
                                        error.set(Some(e));
                                        confirming.set(true);
                                    } else {
                                        confirming.set(false);
                                        error.set(None);
                                    }
                                    form.set(f);
                                });
                            },
                            "Reset and rebuild"
                        }
                    }
                }
            }
        }

        Card {
            h3 { "Erase" }
            p { class: "muted",
                "Full erase (including data) is an own-request flow managed by the box. This RSX settings pane does not yet expose the erase countdown/cancel UI; use the dashboard/backup-erase surfaces where available."
            }
        }
    }
}
