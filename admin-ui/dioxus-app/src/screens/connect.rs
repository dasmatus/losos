use crate::store::BoxConfig;
use dioxus::prelude::*;

#[component]
pub fn ConnectScreen(config: BoxConfig, on_change: EventHandler<BoxConfig>) -> Element {
    let mut host = use_signal(|| config.host.clone());
    let mut token = use_signal(|| config.token.clone());
    let mut demo = use_signal(|| config.demo);
    let mut error = use_signal(String::new);

    let mut submit = move || {
        let mut next = config.clone();
        next.host = host.read().trim().to_string();
        next.token = token.read().trim().to_string();
        next.demo = *demo.read();
        if next.demo {
            if next.host.is_empty() {
                // Demo without a host still shows the UI shell; allow entering.
                // Keep demo on and clear token requirement.
            }
            error.set(String::new());
            on_change.call(next);
            return;
        }
        if next.host.is_empty() {
            error.set("Enter the box address (for example, losos.local or an IP).".into());
            return;
        }
        if next.token.is_empty() {
            error.set("Enter an access token from the box's Settings → Access tokens.".into());
            return;
        }
        error.set(String::new());
        on_change.call(next);
    };

    rsx! {
        div { class: "content",
            div { class: "card",
                h1 { class: "card-title", "Connect to a LosOS box" }
                p { class: "muted",
                    "This cross-platform frontend (Dioxus) replaces the Kotlin companion. "
                    "Link a box with its address and an access token from "
                    strong { "Settings → Access tokens" }
                    "."
                }
                if !error.read().is_empty() {
                    div { class: "banner error", "{error}" }
                }
                div { class: "stack", style: "margin-top: 12px;",
                    div {
                        label { r#for: "host", "Box address" }
                        input {
                            id: "host",
                            r#type: "text",
                            placeholder: "losos.local or 192.168.1.20",
                            value: "{host}",
                            oninput: move |e| host.set(e.value()),
                        }
                    }
                    div {
                        label { r#for: "token", "Access token" }
                        input {
                            id: "token",
                            r#type: "password",
                            placeholder: "Paste token",
                            value: "{token}",
                            oninput: move |e| token.set(e.value()),
                        }
                    }
                    label { class: "row",
                        input {
                            r#type: "checkbox",
                            checked: *demo.read(),
                            onchange: move |e| demo.set(e.checked()),
                        }
                        span { "Demo mode (show the interface without a linked box token)" }
                    }
                    div { class: "row", style: "margin-top: 8px;",
                        button { class: "btn primary", onclick: move |_| submit(), "Continue" }
                    }
                }
            }
            div { class: "card",
                h2 { class: "card-title", "About this rewrite" }
                p { class: "muted",
                    "The goal is one Dioxus codebase for web, desktop, and mobile, "
                    "removing the Kotlin/Jetpack Compose companion. It reuses patterns "
                    "from the Rust TUI client (TLS trust semantics, API/formatting helpers)."
                }
            }
        }
    }
}
