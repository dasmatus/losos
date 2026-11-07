use super::super::shell::SettingsFormSignal;
use crate::components::ui::Card;
use dioxus::prelude::*;

#[component]
pub fn AboutPane() -> Element {
    // About must read saved/running values, never draft (React parity).
    let form: SettingsFormSignal = use_context();
    let f = form();
    let Some(saved) = f.saved.as_ref() else {
        return rsx! { Card { p { class: "muted", "Settings not loaded." } } };
    };
    let host = if saved.host_name.is_empty() { "this box".to_string() } else { saved.host_name.clone() };
    rsx! {
        Card {
            h3 { "About this box" }
            p { "Host name: " strong { "{host}" } }
            p { class: "muted", "Local address (when mDNS works): {host}.local" }
            dl { class: "facts",
                dt { "Sharing storage (mesh)" } dd { if saved.sharing_my_storage { "On" } else { "Off" } }
                dt { "Sharing locally" } dd { if saved.sharing_locally { "On" } else { "Off" } }
                dt { "Cluster" } dd { if saved.cluster_enable { "On" } else { "Off" } }
                dt { "Share compute" } dd { if saved.share_compute { "On" } else { "Off" } }
                dt { "Proxy" } dd { if saved.proxy_enable { "On" } else { "Off" } }
            }
            p { class: "muted", "Values shown are the saved/running configuration, not pending draft edits." }
        }
    }
}
