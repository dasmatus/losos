use crate::actions::{self, Action};
use crate::app::{AppLayout, AuthSignal, AuthState, ConfigSignal};
use crate::auth::{self, Gate};
use crate::components::ui::{Button, Card, Empty, Spinner};
use crate::pages::{blocks, page::{Page, PageId}};
use crate::store::{self, BoxConfig};
use dioxus::prelude::*;
use serde_json::Value;

/// Top-level app routes, mirroring the React admin UI structure.
/// Incremental: routes render functional screens as ported; placeholders for the rest.
#[derive(Clone, Routable, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppLayout)]
        #[route("/")]
        Home {},
        #[route("/apps")]
        Apps {},
        #[route("/storage")]
        Storage {},
        #[route("/mesh")]
        Mesh {},
        #[route("/mesh/market")]
        Market {},
        #[route("/machines")]
        Machines {},
        #[route("/mesh/machines")]
        MeshMachines {},
        #[route("/diagrams")]
        Diagrams {},
        #[route("/wikis")]
        Wikis {},
        #[route("/settings")]
        SettingsRoot {},
        #[route("/settings/:pane")]
        SettingsPane { pane: String },
        #[route("/about")]
        About {},
        #[route("/:..segments")]
        NotFound { segments: Vec<String> },
}

fn config_signal() -> Signal<BoxConfig> {
    // Re-read from storage so route components get current config; AppLayout/App persist.
    use_context::<ConfigSignal>()
}

fn clear_auth_on_401(mut config: ConfigSignal, mut auth_sig: AuthSignal) {
    let mut next = config();
    auth::drop_token(&mut next);
    store::save(&next);
    config.set(next);
    auth_sig.set(AuthState {
        gate: Gate::Signin,
        me: None,
        claim: auth_sig().claim,
    });
}

#[component]
fn Home() -> Element {
    let config = config_signal();
    let cfg = config();
    rsx! {
        Page { id: PageId::Overview,
            blocks::BoxName {}
            blocks::Applying {}
            blocks::Apps { config: cfg.clone() }
            blocks::Glance { config: cfg }
        }
    }
}

#[component]
fn Apps() -> Element {
    let config = config_signal();
    let auth_sig = use_context::<AuthSignal>();
    let mut apps = use_signal(|| None::<Result<Value, String>>);
    let cfg = config();
    use_future(move || {
        let cfg = cfg.clone();
        async move {
            let res = match actions::dispatch(&cfg, Action::AppsList).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    if actions::is_unauthorized(&e) {
                        clear_auth_on_401(config, auth_sig);
                    }
                    Err(e.message)
                }
            };
            apps.set(Some(res));
        }
    });
    rsx! {
        div { class: "page",
            h1 { "Apps" }
            p { class: "muted", "Apps available on this box and from the market." }
            match apps() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { Card { p { class: "error", "{e}" } } },
                Some(Ok(v)) => rsx! { AppsList { data: v } },
            }
        }
    }
}

#[component]
fn AppsList(data: Value) -> Element {
    let items = data.as_array().cloned()
        .or_else(|| data.get("apps").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();
    if items.is_empty() {
        return rsx! { Empty { title: "No apps found".to_string(), description: "The box returned no apps.".to_string() } };
    }
    rsx! {
        div { class: "grid",
            for item in items {
                {
                    let name = item.get("name").or(item.get("id")).and_then(Value::as_str).unwrap_or("App").to_string();
                    let desc = item.get("description").or(item.get("summary")).and_then(Value::as_str).unwrap_or("").to_string();
                    let id = item.get("id").and_then(Value::as_str).unwrap_or(&name).to_string();
                    rsx! {
                        Card {
                            h3 { "{name}" }
                            if !desc.is_empty() { p { class: "muted", "{desc}" } }
                            p { class: "mono muted", "{id}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Storage() -> Element {
    let config = config_signal();
    let auth_sig = use_context::<AuthSignal>();
    let mut status = use_signal(|| None::<Result<Value, String>>);
    let mut busy = use_signal(|| false);
    let cfg = config();
    use_future(move || {
        let cfg = cfg.clone();
        async move {
            let res = match actions::dispatch(&cfg, Action::StorageStatus).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    if actions::is_unauthorized(&e) {
                        clear_auth_on_401(config, auth_sig);
                    }
                    Err(e.message)
                }
            };
            status.set(Some(res));
        }
    });
    rsx! {
        div { class: "page",
            h1 { "Storage" }
            p { class: "muted", "Disk and storage status for this box." }
            match status() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { Card { p { class: "error", "{e}" } } },
                Some(Ok(v)) => rsx! {
                    Card {
                        pre { class: "mono", "{serde_json::to_string_pretty(&v).unwrap_or_default()}" }
                        Button {
                            kind: "primary".to_string(),
                            disabled: busy(),
                            onclick: move |_| {
                                spawn(async move {
                                    busy.set(true);
                                    let cfg = config();
                                    if let Err(e) = actions::dispatch(&cfg, Action::StorageGrow).await {
                                        if actions::is_unauthorized(&e) {
                                            clear_auth_on_401(config, auth_sig);
                                        }
                                        status.set(Some(Err(e.message)));
                                        busy.set(false);
                                        return;
                                    }
                                    let res = match actions::dispatch(&cfg, Action::StorageStatus).await {
                                        Ok(v) => Ok(v),
                                        Err(e) => {
                                            if actions::is_unauthorized(&e) {
                                                clear_auth_on_401(config, auth_sig);
                                            }
                                            Err(e.message)
                                        }
                                    };
                                    status.set(Some(res));
                                    busy.set(false);
                                });
                            },
                            "Grow storage"
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn Mesh() -> Element {
    let config = config_signal();
    let auth_sig = use_context::<AuthSignal>();
    let mut status = use_signal(|| None::<Result<Value, String>>);
    let cfg = config();
    use_future(move || {
        let cfg = cfg.clone();
        async move {
            let res = match actions::dispatch(&cfg, Action::MeshStatus).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    if actions::is_unauthorized(&e) {
                        clear_auth_on_401(config, auth_sig);
                    }
                    Err(e.message)
                }
            };
            status.set(Some(res));
        }
    });
    rsx! {
        div { class: "page",
            h1 { "Mesh" }
            p { class: "muted", "Mesh status and participation." }
            match status() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { Card { p { class: "error", "{e}" } } },
                Some(Ok(v)) => rsx! { Card { pre { class: "mono", "{serde_json::to_string_pretty(&v).unwrap_or_default()}" } } },
            }
        }
    }
}

#[component]
fn Market() -> Element {
    rsx! { PlaceholderPage { title: "Market".to_string(), description: "Sell storage and compute, and browse the market.".to_string() } }
}
#[component]
fn Machines() -> Element {
    rsx! { PlaceholderPage { title: "Machines".to_string(), description: "Virtual machines on the mesh.".to_string() } }
}
#[component]
fn MeshMachines() -> Element {
    rsx! { Machines {} }
}
#[component]
fn Diagrams() -> Element {
    rsx! { PlaceholderPage { title: "Diagrams".to_string(), description: "LosOS Diagrams (Lab) — port in progress.".to_string() } }
}
#[component]
fn Wikis() -> Element {
    rsx! { PlaceholderPage { title: "Wikis".to_string(), description: "LosOS Wiki directory — port in progress.".to_string() } }
}
#[component]
fn SettingsRoot() -> Element {
    rsx! { SettingsPane { pane: "network".to_string() } }
}

#[component]
fn SettingsPane(pane: String) -> Element {
    let config = config_signal();
    rsx! {
        div { class: "page settings-layout",
            aside { class: "settings-nav",
                h2 { "Settings" }
                {settings_links(&pane)}
            }
            section { class: "settings-pane",
                h1 { "{pane_label(&pane)}" }
                PaneBody { pane: pane.clone(), config: config() }
            }
        }
    }
}

fn pane_label(pane: &str) -> &str {
    match pane {
        "network" => "Network",
        "hardware" => "Hardware",
        "security" => "Security",
        "credentials" => "Credentials",
        "problems" => "Problems",
        "account" => "Account",
        "users" => "Users",
        "moderation" => "Moderation",
        "access" => "Access tokens",
        "look" => "Look",
        "tag" => "Tag",
        "advanced" => "Advanced",
        "history" => "History",
        "logs" => "Logs",
        "backup" => "Backup",
        "reset" => "Reset",
        "statistics" => "Statistics",
        _ => "Settings",
    }
}

fn settings_links(current: &str) -> Element {
    let panes = ["network","hardware","security","credentials","problems","account","users","moderation","access","look","tag","advanced","history","logs","backup","reset"];
    rsx! {
        ul { class: "pane-list",
            for p in panes {
                {
                    let active = p == current;
                    let cls = if active { "nav-item active" } else { "nav-item" };
                    rsx! { li { Link { to: Route::SettingsPane { pane: p.to_string() }, class: "{cls}", "{pane_label(p)}" } } }
                }
            }
        }
    }
}

#[component]
fn PaneBody(pane: String, config: BoxConfig) -> Element {
    match pane.as_str() {
        "backup" => rsx! { ApiPane { config: config, action: Action::BackupStatus, title: "Backup".to_string() } },
        "users" => rsx! { ApiPane { config: config, action: Action::UsersList, title: "Users".to_string() } },
        "statistics" => rsx! { ApiPane { config: config, action: Action::Get("/api/statistics".into()), title: "Statistics".to_string() } },
        "history" => rsx! { ApiPane { config: config, action: Action::MeshHistory, title: "History".to_string() } },
        "logs" => rsx! { ApiPane { config: config, action: Action::Get("/api/logs".into()), title: "Logs".to_string() } },
        "access" => rsx! { ApiPane { config: config, action: Action::Get("/api/tokens".into()), title: "Access tokens".to_string() } },
        "network" | "hardware" | "security" | "credentials" | "problems" | "account" | "moderation" | "look" | "tag" | "advanced" | "reset" => {
            rsx! { PlaceholderPage { title: pane_label(&pane).to_string(), description: format!("{pane} pane — RSX port in progress. Actions bind via /api/* and losos middle layer.") } }
        }
        _ => rsx! { PlaceholderPage { title: "Settings".to_string(), description: "Select a settings pane.".to_string() } },
    }
}

#[component]
fn ApiPane(config: BoxConfig, action: Action, title: String) -> Element {
    let config_sig = config_signal();
    let auth_sig = use_context::<AuthSignal>();
    let mut data = use_signal(|| None::<Result<Value, String>>);
    let action2 = action.clone();
    let cfg = config.clone();
    use_future(move || {
        let cfg = cfg.clone();
        let action = action2.clone();
        async move {
            let res = match actions::dispatch(&cfg, action).await {
                Ok(v) => Ok(v),
                Err(e) => {
                    if actions::is_unauthorized(&e) {
                        clear_auth_on_401(config_sig, auth_sig);
                    }
                    Err(e.message)
                }
            };
            data.set(Some(res));
        }
    });
    match data() {
        None => rsx! { Spinner {} },
        Some(Err(e)) => rsx! { Card { p { class: "error", "{e}" } p { class: "muted", "This box may not expose this endpoint or access may be restricted." } } },
        Some(Ok(v)) => rsx! { Card { pre { class: "mono", "{serde_json::to_string_pretty(&v).unwrap_or_default()}" } } },
    }
}

#[component]
fn About() -> Element {
    let config = config_signal();
    rsx! {
        Page { id: PageId::SettingsAbout,
            blocks::AboutBlock { config: config() }
        }
    }
}

#[component]
fn PlaceholderPage(title: String, description: String) -> Element {
    rsx! {
        div { class: "page",
            h1 { "{title}" }
            Card {
                p { "{description}" }
                p { class: "muted", "This screen is part of the ongoing Dioxus/RSX rewrite preserving the original design system. Actions are bound through the losos-client/TUI middle layer and /api/*." }
            }
        }
    }
}

#[component]
fn NotFound(segments: Vec<String>) -> Element {
    let path = format!("/{}", segments.join("/"));
    rsx! {
        div { class: "page",
            h1 { "Not found" }
            p { class: "muted", "No route for {path}" }
            Link { to: Route::Home {}, "Go to Overview" }
        }
    }
}
