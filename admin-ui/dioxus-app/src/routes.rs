use crate::actions::{self, Action};
use crate::app::{AppLayout, AuthSignal, AuthState, ConfigSignal};
use crate::auth::{self, Gate, Me};
use crate::components::ui::{Button, Card, Empty, Spinner};
use crate::pages::{blocks, page::{Page, PageId}};
use crate::screens::settings::SettingsShell;
use crate::store::{self, BoxConfig};
use dioxus::prelude::*;
use serde_json::Value;

/// Top-level pane IDs that get top-level-style addresses or direct pane parity.
/// Matches React `lib/routes.ts` + settings pane set used by the Dioxus shell.
pub const TOP_LEVEL_PANES: &[&str] = &["storage", "mesh", "machines", "apps"];
pub const SETTINGS_LANDING: &str = "network";

pub fn pane_href(pane: &str) -> String {
    if TOP_LEVEL_PANES.contains(&pane) {
        format!("/{pane}")
    } else {
        format!("/settings/{pane}")
    }
}

pub fn is_settings_pane_id(pane: &str) -> bool {
    matches!(
        pane,
        "network"
            | "hardware"
            | "security"
            | "credentials"
            | "problems"
            | "account"
            | "users"
            | "moderation"
            | "access"
            | "look"
            | "tag"
            | "advanced"
            | "history"
            | "logs"
            | "backup"
            | "reset"
            | "statistics"
            | "storage"
            | "mesh"
            | "market"
            | "machines"
            | "apps"
            | "about"
    )
}

pub fn pane_label(pane: &str) -> &'static str {
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
        "storage" => "Storage",
        "mesh" => "Mesh",
        "market" => "Market",
        "machines" => "Machines",
        "apps" => "Apps",
        "about" => "About",
        _ => "Settings",
    }
}

pub fn is_wide_route(route: &Route) -> bool {
    matches!(
        route,
        Route::Diagrams {}
            | Route::DiagramsPath { .. }
            | Route::Calendar {}
            | Route::CalendarPath { .. }
            | Route::Wikis {}
            | Route::WikisPath { .. }
    )
}

pub fn held_account_lock_target(me: &Option<Me>) -> Option<Route> {
    if me.as_ref().map(|m| m.is_held()).unwrap_or(false) {
        Some(Route::SettingsPane {
            pane: "account".to_string(),
        })
    } else {
        None
    }
}

/// Top-level app routes, mirroring the React admin UI structure.
/// Slice 6: expanded route tree with placeholders for parity.
#[derive(Clone, Routable, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppLayout)]
        #[route("/")]
        Home {},
        #[route("/apps")]
        Apps {},
        #[route("/apps/new")]
        AppNew {},
        #[route("/apps/:id")]
        AppScreen { id: String },
        #[route("/apps/:id/edit")]
        AppEdit { id: String },
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
        #[route("/office")]
        Office {},
        #[route("/office/:..segments")]
        OfficePath { segments: Vec<String> },
        #[route("/cloud")]
        Cloud {},
        #[route("/mail")]
        Mail {},
        #[route("/contacts")]
        Contacts {},
        #[route("/calendar")]
        Calendar {},
        #[route("/calendar/:..segments")]
        CalendarPath { segments: Vec<String> },
        #[route("/chat")]
        Chat {},
        #[route("/chat/:..segments")]
        ChatPath { segments: Vec<String> },
        #[route("/ci")]
        Ci {},
        #[route("/ci/runs/:id")]
        CiRun { id: String },
        #[route("/diagrams")]
        Diagrams {},
        #[route("/diagrams/:..segments")]
        DiagramsPath { segments: Vec<String> },
        #[route("/wikis")]
        Wikis {},
        #[route("/wikis/:..segments")]
        WikisPath { segments: Vec<String> },
        #[route("/settings")]
        SettingsRoot {},
        #[route("/settings/:pane")]
        SettingsPane { pane: String },
        #[route("/about")]
        About {},
        #[route("/:..segments")]
        PlaceholderPath { segments: Vec<String> },
}

fn config_signal() -> Signal<BoxConfig> {
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
        div { class: "page ls-page",
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
fn AppNew() -> Element {
    rsx! { PlaceholderPage { title: "New app".to_string(), description: "App editor (new) — route parity placeholder.".to_string() } }
}

#[component]
fn AppScreen(id: String) -> Element {
    rsx! { PlaceholderPage { title: format!("App: {id}"), description: "App screen — route parity placeholder.".to_string() } }
}

#[component]
fn AppEdit(id: String) -> Element {
    rsx! { PlaceholderPage { title: format!("Edit app: {id}"), description: "App editor — route parity placeholder.".to_string() } }
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
        div { class: "page ls-page",
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
        div { class: "page ls-page",
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
fn Office() -> Element {
    rsx! { PlaceholderPage { title: "Documents".to_string(), description: "Office/documents route parity placeholder.".to_string() } }
}
#[component]
fn OfficePath(segments: Vec<String>) -> Element {
    let path = format!("/office/{}", segments.join("/"));
    rsx! { PlaceholderPage { title: "Documents".to_string(), description: format!("Office route placeholder: {path}") } }
}
#[component]
fn Cloud() -> Element {
    rsx! { PlaceholderPage { title: "Cloud".to_string(), description: "LosOS Cloud entry (served by Cloud app on boxes where enabled).".to_string() } }
}
#[component]
fn Mail() -> Element {
    rsx! { PlaceholderPage { title: "Mail".to_string(), description: "LosOS Mail route parity placeholder (shared side).".to_string() } }
}
#[component]
fn Contacts() -> Element {
    rsx! { PlaceholderPage { title: "Contacts".to_string(), description: "LosOS Contacts route parity placeholder.".to_string() } }
}
#[component]
fn Calendar() -> Element {
    rsx! { PlaceholderPage { title: "Calendar".to_string(), description: "LosOS Calendar route parity placeholder (wide layout).".to_string() } }
}
#[component]
fn CalendarPath(segments: Vec<String>) -> Element {
    let path = format!("/calendar/{}", segments.join("/"));
    rsx! { PlaceholderPage { title: "Calendar".to_string(), description: format!("Calendar route placeholder: {path}") } }
}
#[component]
fn Chat() -> Element {
    rsx! { PlaceholderPage { title: "Chat".to_string(), description: "LosOS Chat route parity placeholder.".to_string() } }
}
#[component]
fn ChatPath(segments: Vec<String>) -> Element {
    let path = format!("/chat/{}", segments.join("/"));
    rsx! { PlaceholderPage { title: "Chat".to_string(), description: format!("Chat route placeholder: {path}") } }
}
#[component]
fn Ci() -> Element {
    rsx! { PlaceholderPage { title: "Mesh CI".to_string(), description: "Mesh CI route parity placeholder.".to_string() } }
}
#[component]
fn CiRun(id: String) -> Element {
    rsx! { PlaceholderPage { title: "Mesh CI run".to_string(), description: format!("CI run placeholder: {id}") } }
}
#[component]
fn Diagrams() -> Element {
    rsx! { PlaceholderPage { title: "Diagrams".to_string(), description: "LosOS Diagrams (Lab) — route parity placeholder (wide layout).".to_string() } }
}
#[component]
fn DiagramsPath(segments: Vec<String>) -> Element {
    let path = format!("/diagrams/{}", segments.join("/"));
    rsx! { PlaceholderPage { title: "Diagrams".to_string(), description: format!("Diagrams route placeholder: {path}") } }
}
#[component]
fn Wikis() -> Element {
    rsx! { PlaceholderPage { title: "Wikis".to_string(), description: "LosOS Wiki directory — route parity placeholder (wide layout).".to_string() } }
}
#[component]
fn WikisPath(segments: Vec<String>) -> Element {
    let path = format!("/wikis/{}", segments.join("/"));
    rsx! { PlaceholderPage { title: "Wikis".to_string(), description: format!("Wikis route placeholder: {path}") } }
}
#[component]
fn SettingsRoot() -> Element {
    rsx! { SettingsPane { pane: SETTINGS_LANDING.to_string() } }
}

#[component]
fn SettingsPane(pane: String) -> Element {
    rsx! { SettingsShell { pane } }
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
        div { class: "page ls-page",
            h1 { "{title}" }
            Card {
                p { "{description}" }
                p { class: "muted", "This screen is part of the ongoing Dioxus/RSX rewrite preserving the original design system. Actions are bound through the losos-client/TUI middle layer and /api/*." }
            }
        }
    }
}

#[component]
fn PlaceholderPath(segments: Vec<String>) -> Element {
    let path = format!("/{}", segments.join("/"));
    rsx! {
        div { class: "page ls-page",
            h1 { "Not found" }
            p { class: "muted", "No route for {path}" }
            Link { to: Route::Home {}, "Go to Overview" }
        }
    }
}
