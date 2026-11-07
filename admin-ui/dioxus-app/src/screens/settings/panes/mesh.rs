use super::super::shell::SettingsFormSignal;
use crate::components::ui::Card;
use dioxus::prelude::*;

#[component]
pub fn MeshPane() -> Element {
    let mut form: SettingsFormSignal = use_context();
    let f = form();
    let Some(draft) = f.draft.clone() else {
        return rsx! { Card { p { class: "muted", "Settings not loaded." } } };
    };
    let probs = f.problems();
    let disabled = f.locked || !f.ready;
    let joined = draft.cluster_enable;
    let sharing = draft.share_compute;
    let window_disabled = disabled || !joined;

    rsx! {
        Card {
            h3 { "Mesh / edge" }
            p { class: "muted",
                "Edge discovery, cluster membership, and live gateway details are shown here as honest placeholders until full API/hook parity is ported. Changes below are draft settings and apply through the full /api/apply settings engine (overrides.nix), preserving unrelated keys."
            }
            div { class: "facts",
                dt { "Cluster (draft)" } dd { if joined { "Enabled" } else { "Disabled" } }
                dt { "Share compute (draft)" } dd { if sharing { "Enabled" } else { "Disabled" } }
                dt { "Compute window (draft)" } dd { "{draft.compute_window_start} – {draft.compute_window_end}" }
            }
        }

        Card {
            h3 { "Other boxes" }
            div { class: "field",
                label { class: "row", r#for: "clusterEnable",
                    input {
                        id: "clusterEnable",
                        r#type: "checkbox",
                        checked: joined,
                        disabled: disabled,
                        onchange: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.cluster_enable = e.checked();
                            }
                            form.set(f);
                        },
                    }
                    span { "Join the mesh (losos.cluster.enable)" }
                }
                p { class: "muted",
                    "Full edge reachability/gating parity is pending in this RSX port. Applying uses the normal settings flow; the box may refuse settings it cannot honor."
                }
            }
        }

        Card {
            h3 { "Spare time" }
            div { class: "field",
                label { class: "row", r#for: "shareCompute",
                    input {
                        id: "shareCompute",
                        r#type: "checkbox",
                        checked: sharing,
                        disabled: window_disabled,
                        onchange: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.share_compute = e.checked();
                            }
                            form.set(f);
                        },
                    }
                    span { "Lend spare compute time (losos.cluster.shareCompute)" }
                }
                if window_disabled && !disabled {
                    p { class: "muted", "Join the mesh first to set compute sharing hours." }
                }
            }

            div { class: "field",
                label { "Compute window (local time)" }
                div { class: "row", style: "gap: 8px; flex-wrap: wrap; align-items: center;",
                    label { class: "muted", r#for: "computeWindowStart", "From" }
                    input {
                        id: "computeWindowStart",
                        class: if probs.compute_window.is_some() { "input error" } else { "input" },
                        r#type: "time",
                        value: "{draft.compute_window_start}",
                        disabled: window_disabled,
                        oninput: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.compute_window_start = e.value();
                            }
                            form.set(f);
                        },
                    }
                    span { class: "muted", "until" }
                    label { class: "muted", r#for: "computeWindowEnd", "To" }
                    input {
                        id: "computeWindowEnd",
                        class: if probs.compute_window.is_some() { "input error" } else { "input" },
                        r#type: "time",
                        value: "{draft.compute_window_end}",
                        disabled: window_disabled,
                        oninput: move |e| {
                            let mut f = form();
                            if let Some(d) = f.draft.as_mut() {
                                d.compute_window_end = e.value();
                            }
                            form.set(f);
                        },
                    }
                }
                if let Some(err) = probs.compute_window.as_ref() {
                    p { class: "error", "{err}" }
                } else {
                    p { class: "muted",
                        "Hours are interpreted in your local time and travel with the setting. Windows that cross midnight are allowed."
                    }
                }
            }
        }
    }
}
