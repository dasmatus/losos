use crate::settings::form::SettingsFormState;
use dioxus::prelude::*;

#[component]
pub fn ApplyBar(
    form: Signal<SettingsFormState>,
    on_apply: EventHandler<MouseEvent>,
    on_discard: EventHandler<MouseEvent>,
    #[props(default)] on_refresh_status: Option<EventHandler<MouseEvent>>,
) -> Element {
    let f = form();
    let show = f.dirty() || f.applying || f.rebuild.is_some();
    if !show {
        return rsx! {};
    }
    let can_apply = f.can_apply();
    let changes = f.change_count();
    let status_msg = f
        .rebuild
        .as_ref()
        .map(|r| format!("{}: {}", r.title, r.message))
        .unwrap_or_else(|| {
            if changes == 1 {
                "1 pending change".to_string()
            } else {
                format!("{changes} pending changes")
            }
        });
    rsx! {
        div { class: "apply-bar card",
            div { class: "apply-bar-status",
                if f.applying { span { class: "spinner", role: "status" } }
                span { "{status_msg}" }
                if let Some(err) = &f.load_error {
                    span { class: "error", " {err}" }
                }
            }
            div { class: "apply-bar-actions",
                if let Some(on_refresh) = on_refresh_status {
                    if f.applying || f.rebuild.is_some() {
                        button {
                            class: "btn btn-secondary",
                            disabled: false,
                            onclick: move |e| on_refresh.call(e),
                            "Refresh status"
                        }
                    }
                }
                button {
                    class: "btn btn-secondary",
                    disabled: f.applying || !f.dirty(),
                    onclick: move |e| on_discard.call(e),
                    "Discard"
                }
                button {
                    class: "btn btn-primary",
                    disabled: !can_apply,
                    onclick: move |e| on_apply.call(e),
                    if f.applying { "Applying…" } else { "Apply changes" }
                }
            }
        }
    }
}
