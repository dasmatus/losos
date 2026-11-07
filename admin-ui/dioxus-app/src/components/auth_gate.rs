use crate::auth::{self, ClaimState, Me};
use crate::store::BoxConfig;
use dioxus::prelude::*;

#[component]
pub fn ConnectHostScreen(config: BoxConfig, on_change: EventHandler<BoxConfig>) -> Element {
    let mut host = use_signal(|| config.host.clone());
    let mut demo = use_signal(|| config.demo);
    let mut error = use_signal(String::new);

    let submit = {
        let config = config.clone();
        move |_| {
            let mut next = config.clone();
            next.host = host.read().trim().to_string();
            next.demo = *demo.read();
            if next.demo && next.host.trim().is_empty() {
                // Demo shell without a linked box.
                next.token.clear();
                auth::drop_token(&mut next);
                error.set(String::new());
                on_change.call(next);
                return;
            }
            if next.host.trim().is_empty() {
                error.set("Enter the box address (for example, losos.local or an IP).".into());
                return;
            }
            error.set(String::new());
            on_change.call(next);
        }
    };

    rsx! {
        div { class: "app-shell",
            div { class: "content",
                div { class: "card",
                    h1 { class: "card-title", "Connect to a LosOS box" }
                    p { class: "muted",
                        "Enter this box's address. If the box has not been set up yet, you'll be guided through setup. "
                        "If it's already set up, sign in with an access token."
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
                        label { class: "row",
                            input {
                                r#type: "checkbox",
                                checked: *demo.read(),
                                onchange: move |e| demo.set(e.checked()),
                            }
                            span { "Demo mode (show the interface without a linked box)" }
                        }
                        div { class: "row", style: "margin-top: 8px;",
                            button { class: "btn primary", onclick: submit, "Continue" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn SetupScreen(config: BoxConfig, claim: ClaimState, on_claimed: EventHandler<BoxConfig>) -> Element {
    let mut name = use_signal(|| "admin".to_string());
    let mut password = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);

    let waiting = claim.ready == Some(false);
    let submit = {
        let config = config.clone();
        move |_| {
            if *busy.read() || waiting {
                return;
            }
            let n = name.read().trim().to_string();
            let p = password.read().clone();
            if n.is_empty() {
                error.set("Enter a name for the first account.".into());
                return;
            }
            if p.is_empty() {
                error.set("Choose a password.".into());
                return;
            }
            let mut cfg = config.clone();
            spawn(async move {
                busy.set(true);
                error.set(String::new());
                match auth::claim_box(&cfg, &n, &p).await {
                    Ok(res) => {
                        auth::save_token(&mut cfg, res.token);
                        cfg.demo = false;
                        on_claimed.call(cfg);
                    }
                    Err(e) => error.set(e.message),
                }
                busy.set(false);
            });
        }
    };

    rsx! {
        div { class: "app-shell",
            div { class: "content",
                div { class: "card",
                    h1 { class: "card-title", "Set up this box" }
                    p { class: "muted",
                        "This box hasn't been claimed yet. Create the first administrator account to continue."
                    }
                    if waiting {
                        div { class: "banner info",
                            if let Some(msg) = claim.waiting_for.as_ref() {
                                "{msg}"
                            } else {
                                "The box is still getting ready. Try again in a moment."
                            }
                        }
                    }
                    if !error.read().is_empty() {
                        div { class: "banner error", "{error}" }
                    }
                    div { class: "stack", style: "margin-top: 12px;",
                        div {
                            label { r#for: "setup-name", "Administrator name" }
                            input {
                                id: "setup-name",
                                r#type: "text",
                                autocomplete: "username",
                                value: "{name}",
                                disabled: *busy.read(),
                                oninput: move |e| name.set(e.value()),
                            }
                        }
                        div {
                            label { r#for: "setup-password", "Password" }
                            input {
                                id: "setup-password",
                                r#type: "password",
                                autocomplete: "new-password",
                                value: "{password}",
                                disabled: *busy.read(),
                                oninput: move |e| password.set(e.value()),
                            }
                        }
                        div { class: "row", style: "margin-top: 8px;",
                            button { class: "btn primary", disabled: *busy.read() || waiting, onclick: submit,
                                if *busy.read() { "Setting up…" } else { "Create account" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn SigninScreen(config: BoxConfig, on_signed_in: EventHandler<(BoxConfig, Option<Me>)>) -> Element {
    let mut token = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);

    let submit = {
        let config = config.clone();
        move |_| {
            if *busy.read() {
                return;
            }
            let t = token.read().trim().to_string();
            if t.is_empty() {
                error.set("Enter an access token or spare key.".into());
                return;
            }
            let mut cfg = config.clone();
            spawn(async move {
                busy.set(true);
                error.set(String::new());
                match auth::sign_in(&cfg, &t).await {
                    Ok(Some(me)) => {
                        auth::save_token(&mut cfg, t);
                        cfg.demo = false;
                        on_signed_in.call((cfg, Some(me)));
                    }
                    Ok(None) => {
                        error.set("That token was not accepted by this box.".into());
                    }
                    Err(e) => error.set(e.message),
                }
                busy.set(false);
            });
        }
    };

    let disconnect = {
        let config = config.clone();
        move |_| {
            let mut next = config.clone();
            next.host.clear();
            next.token.clear();
            next.demo = true;
            auth::drop_token(&mut next);
            on_signed_in.call((next, None));
        }
    };

    rsx! {
        div { class: "app-shell",
            div { class: "content",
                div { class: "card",
                    h1 { class: "card-title", "Sign in" }
                    p { class: "muted",
                        "This box is set up. Sign in with an access token (Settings → Access tokens) or the spare admin key."
                    }
                    if !error.read().is_empty() {
                        div { class: "banner error", "{error}" }
                    }
                    div { class: "stack", style: "margin-top: 12px;",
                        div {
                            label { r#for: "signin-token", "Access token" }
                            input {
                                id: "signin-token",
                                r#type: "password",
                                autocomplete: "off",
                                value: "{token}",
                                disabled: *busy.read(),
                                oninput: move |e| token.set(e.value()),
                            }
                        }
                        div { class: "row", style: "margin-top: 8px;",
                            button { class: "btn primary", disabled: *busy.read(), onclick: submit,
                                if *busy.read() { "Checking…" } else { "Unlock" }
                            }
                            button { class: "btn", disabled: *busy.read(), onclick: disconnect, "Use a different box" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn GateLoading() -> Element {
    rsx! {
        div { class: "app-shell",
            div { class: "content",
                div { class: "card",
                    p { class: "muted", "Asking this box…" }
                }
            }
        }
    }
}
