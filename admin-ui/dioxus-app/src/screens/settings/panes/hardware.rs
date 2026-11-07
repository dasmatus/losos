use super::super::shell::SettingsFormSignal;
use crate::components::ui::Card;
use dioxus::prelude::*;

#[component]
pub fn HardwarePane() -> Element {
    let mut form: SettingsFormSignal = use_context();
    let f = form();
    let Some(draft) = f.draft.clone() else {
        return rsx! { Card { p { class: "muted", "Settings not loaded." } } };
    };
    rsx! {
        Card {
            h3 { "Hardware" }
            div { class: "field",
                label { class: "row",
                    input {
                        r#type: "checkbox",
                        checked: draft.gpu_enable,
                        onchange: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.gpu_enable = e.checked();
                            }
                            form.set(f);
                        },
                    }
                    span { "Enable GPU support (losos.gpu.enable)" }
                }
                p { class: "muted", "Applies on next rebuild." }
            }
        }
    }
}
