use super::apply_bar::ApplyBar;
use super::panes;
use crate::app::{AuthSignal, AuthState, ConfigSignal};
use crate::auth::Gate;
use crate::components::ui::{Card, Spinner};
use crate::routes::Route;
use crate::settings::form::SettingsFormState;
use crate::store;
use dioxus::prelude::*;

pub type SettingsFormSignal = Signal<SettingsFormState>;

fn is_draft_pane(pane: &str) -> bool {
    // Draft panes edit the unified settings document and apply via full /api/apply.
    // Own-request panes (storage grow, mesh edge actions details, apps install, etc.)
    // manage their own requests; they can still show form applying/lock state.
    matches!(
        pane,
        "network" | "hardware" | "security" | "mesh" | "market" | "history"
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
        "about" => "About",
        "apps" => "Apps",
        "market" => "Market",
        _ => "Settings",
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}
#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    0.0
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

#[component]
pub fn SettingsShell(pane: String) -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth_sig: AuthSignal = use_context();
    let mut form = use_context_provider(|| Signal::new(SettingsFormState::new()));
    let mut refresh = use_signal(|| 0u64);

    // Load settings when config/refresh changes.
    let cfg = config();
    use_effect(use_reactive((&cfg.host, &cfg.token, &*refresh.read()), move |_| {
        let cfg = config();
        spawn(async move {
            let mut f = form();
            f.load(&cfg).await;
            form.set(f);
            if form().locked {
                let mut next = config();
                crate::auth::drop_token(&mut next);
                store::save(&next);
                config.set(next);
                auth_sig.set(AuthState {
                    gate: Gate::Signin,
                    me: None,
                    claim: auth_sig().claim,
                });
            }
        });
    }));

    // Rebuild status polling while applying (native best-effort; web can refresh/retry).
    let applying = form().applying;
    use_effect(use_reactive((&applying,), move |(is_applying,)| {
        if !is_applying {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let cfg = config();
            spawn(async move {
                loop {
                    sleep_ms(crate::settings::STATUS_PERIOD_MS).await;
                    if !form().applying {
                        break;
                    }
                    match crate::settings::api::get_status(&cfg).await {
                        Ok(st) => {
                            let mut f = form();
                            f.on_status(st, now_ms());
                            form.set(f);
                        }
                        Err(e) if e.status == 401 => {
                            let mut f = form();
                            f.locked = true;
                            f.applying = false;
                            form.set(f);
                            let mut next = config();
                            crate::auth::drop_token(&mut next);
                            store::save(&next);
                            config.set(next);
                            auth_sig.set(AuthState {
                                gate: Gate::Signin,
                                me: None,
                                claim: auth_sig().claim,
                            });
                            break;
                        }
                        Err(_) => {}
                    }
                }
            });
        }
    }));

    let f = form();
    let draft_pane = is_draft_pane(&pane);
    let links = [
        "storage", "network", "hardware", "security", "credentials", "problems", "account",
        "users", "moderation", "access", "look", "tag", "advanced", "history", "logs",
        "backup", "reset", "about", "mesh", "apps", "market",
    ];

    rsx! {
        div { class: "page settings-layout",
            aside { class: "settings-nav card",
                h2 { "Settings" }
                ul { class: "pane-list",
                    for p in links {
                        {
                            let active = p == pane.as_str();
                            let cls = if active { "nav-item active" } else { "nav-item" };
                            let label = pane_label(p);
                            rsx! {
                                li {
                                    Link {
                                        to: Route::SettingsPane { pane: p.to_string() },
                                        class: "{cls}",
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            section { class: "settings-pane",
                h1 { "{pane_label(&pane)}" }
                if f.locked {
                    Card { p { class: "error", "Sign in to change settings." } }
                } else if !f.ready && f.load_error.is_none() {
                    Spinner {}
                } else if let Some(err) = &f.load_error {
                    Card {
                        p { class: "error", "{err}" }
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                let v = *refresh.read() + 1;
                                refresh.set(v);
                            },
                            "Retry"
                        }
                    }
                } else {
                    PaneRouter { pane: pane.clone() }
                    if draft_pane {
                        ApplyBar {
                            form: form,
                            on_refresh_status: move |_| {
                                let cfg = config();
                                spawn(async move {
                                    match crate::settings::api::get_status(&cfg).await {
                                        Ok(st) => {
                                            let mut f = form();
                                            f.on_status(st, now_ms());
                                            form.set(f);
                                        }
                                        Err(e) if e.status == 401 => {
                                            let mut f = form();
                                            f.locked = true;
                                            f.applying = false;
                                            form.set(f);
                                            let mut next = config();
                                            crate::auth::drop_token(&mut next);
                                            store::save(&next);
                                            config.set(next);
                                            auth_sig.set(AuthState {
                                                gate: Gate::Signin,
                                                me: None,
                                                claim: auth_sig().claim,
                                            });
                                        }
                                        Err(_) => {}
                                    }
                                });
                            },
                            on_discard: move |_| {
                                let mut f = form();
                                f.discard();
                                form.set(f);
                            },
                            on_apply: move |_| {
                                let cfg = config();
                                spawn(async move {
                                    let mut f = form();
                                    let _ = f.apply(&cfg).await;
                                    form.set(f);
                                    if form().locked {
                                        let mut next = config();
                                        crate::auth::drop_token(&mut next);
                                        store::save(&next);
                                        config.set(next);
                                        auth_sig.set(AuthState {
                                            gate: Gate::Signin,
                                            me: None,
                                            claim: auth_sig().claim,
                                        });
                                    }
                                });
                            },
                        }
                    }
                    }
            }
        }
    }
}

#[component]
fn PaneRouter(pane: String) -> Element {
    match pane.as_str() {
        "network" => rsx! { panes::network::NetworkPane {} },
        "hardware" => rsx! { panes::hardware::HardwarePane {} },
        "security" => rsx! { panes::security::SecurityPane {} },
        "mesh" => rsx! { panes::mesh::MeshPane {} },
        "reset" => rsx! { panes::reset::ResetPane {} },
        "storage" => rsx! { panes::storage::StoragePane {} },
        "about" => rsx! { panes::about::AboutPane {} },
        "credentials" => rsx! { panes::credentials::CredentialsPane {} },
        "access" => rsx! { panes::access::AccessPane {} },
        "backup" => rsx! { panes::backup::BackupPane {} },
        "users" => rsx! { panes::users::UsersPane {} },
        "apps" => rsx! { panes::apps::AppsPane {} },
        "look" => rsx! { panes::look::LookPane {} },
        _ => rsx! {
            Card {
                p { "{pane_label(&pane)} pane — RSX port in progress." }
                p { class: "muted", "Draft panes use full /api/apply (overrides.nix) with extras preserved; own-request panes use their /api/* endpoints." }
            }
        },
    }
}
