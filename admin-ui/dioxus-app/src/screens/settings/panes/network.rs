use super::super::shell::SettingsFormSignal;
use crate::components::ui::Card;
use dioxus::prelude::*;

#[component]
pub fn NetworkPane() -> Element {
    let mut form: SettingsFormSignal = use_context();
    let f = form();
    let Some(draft) = f.draft.clone() else {
        return rsx! { Card { p { class: "muted", "Settings not loaded." } } };
    };
    let probs = f.problems();

    rsx! {
        Card {
            h3 { "Network" }
            p { class: "muted", "Host name and network-related settings. Changes apply via a full rebuild (overrides.nix)." }

            div { class: "field",
                label { r#for: "hostName", "Host name" }
                input {
                    id: "hostName",
                    class: if probs.host_name.is_some() { "input error" } else { "input" },
                    r#type: "text",
                    value: "{draft.host_name}",
                    oninput: move |e| {
                        let mut f = form();
                        if let Some(d) = f.draft.as_mut() {
                            d.host_name = e.value();
                        }
                        form.set(f);
                    },
                }
                if let Some(err) = probs.host_name.as_ref() {
                    p { class: "error", "{err}" }
                } else {
                    p { class: "muted", "The box will be reachable at {draft.host_name}.local (when mDNS works)." }
                }
            }

            div { class: "field",
                label { class: "row",
                    input {
                        r#type: "checkbox",
                        checked: draft.proxy_enable,
                        onchange: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.proxy_enable = e.checked();
                            }
                            form.set(f);
                        },
                    }
                    span { "Enable proxy features (losos.proxy.enable)" }
                }
            }
        }
    }
}
