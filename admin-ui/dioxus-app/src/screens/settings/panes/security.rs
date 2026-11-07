use super::super::shell::SettingsFormSignal;
use crate::components::ui::Card;
use dioxus::prelude::*;

#[component]
pub fn SecurityPane() -> Element {
    let form: SettingsFormSignal = use_context();
    let f = form();
    let Some(draft) = f.draft.clone() else {
        return rsx! { Card { p { class: "muted", "Settings not loaded." } } };
    };
    rsx! {
        Card {
            h3 { "Security / hardening" }
            p { class: "muted", "Hardening options have tradeoffs. Changes apply via rebuild." }
            {hardening_toggle(form, "hardeningApparmor", "AppArmor (losos.hardening.apparmor)", draft.hardening_apparmor, |d, v| d.hardening_apparmor = v, "Confinement is limited on this platform; enabling may affect workloads.")}
            {hardening_toggle(form, "hardeningMalloc", "Hardened malloc (losos.hardening.malloc)", draft.hardening_malloc, |d, v| d.hardening_malloc = v, "Applies to host daemons, not pods.")}
            {hardening_toggle(form, "hardeningNosmt", "Disable SMT / nosmt (losos.hardening.nosmt)", draft.hardening_nosmt, |d, v| d.hardening_nosmt = v, "Can halve available cores.")}
            {hardening_toggle(form, "hardeningUsbguard", "USBGuard (losos.hardening.usbguard)", draft.hardening_usbguard, |d, v| d.hardening_usbguard = v, "Blocks USB devices not present at boot.")}
        }
    }
}

fn hardening_toggle(
    mut form: SettingsFormSignal,
    id: &'static str,
    label: &'static str,
    checked: bool,
    set: fn(&mut crate::settings::SettingsResponse, bool),
    help: &'static str,
) -> Element {
    rsx! {
        div { class: "field",
            label { class: "row", r#for: "{id}",
                input {
                    id: "{id}",
                    r#type: "checkbox",
                    checked: checked,
                    onchange: move |e| {
                        let v = e.checked();
                        let mut f = form();
                        if let Some(d) = f.draft.as_mut() {
                            set(d, v);
                        }
                        form.set(f);
                    },
                }
                span { "{label}" }
            }
            p { class: "muted", "{help}" }
        }
    }
}
