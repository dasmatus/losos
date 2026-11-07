use crate::auth::{self, ClaimState, Gate, Me};
use crate::components::auth_gate::{ConnectHostScreen, GateLoading, SetupScreen, SigninScreen};
use crate::components::shell::RoutedShell;
use crate::routes::Route;
use crate::screens::settings::SettingsShell;
use crate::store::{self, BoxConfig};
use dioxus::prelude::*;

const MAIN_CSS: Asset = asset!("/assets/main.css");

/// Shared application config signal type.
pub type ConfigSignal = Signal<BoxConfig>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthState {
    pub gate: Gate,
    pub me: Option<Me>,
    pub claim: Option<ClaimState>,
}

pub type AuthSignal = Signal<AuthState>;

#[component]
pub fn App() -> Element {
    let config = use_context_provider(|| Signal::new(store::load_with_session_token()));
    use_context_provider(|| Signal::new(AuthState::default()));

    use_effect(move || {
        store::save(&config.read());
    });

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn AppLayout() -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth: AuthSignal = use_context();
    // Pages store: /api/pages customizations + defaults renderer (best-effort).
    let pages = crate::pages::provide_pages_store();
    crate::pages::use_load_pages(pages);
    let cfg = config();
    let refresh = use_signal(|| 0u64);
    let gate_gen = use_signal(|| 0u64);

    // Gate resolution: rerun when host/token/demo/refresh changes.
    use_effect(use_reactive(
        (&cfg.host, &cfg.token, &cfg.demo, &*refresh.read()),
        move |(host, token, demo, _)| {
            let mut cfg_now = config();
            cfg_now.host = host;
            cfg_now.token = token;
            cfg_now.demo = demo;
            let mut gen = gate_gen;
            let next = *gen.read() + 1;
            gen.set(next);
            spawn(async move {
                resolve_gate(cfg_now, auth, gate_gen, next).await;
            });
        },
    ));

    let current = config();
    let state = auth();

    // Demo without a linked box: open the shell; API-backed screens will show
    // errors/empty states until a box address is configured.
    if current.demo && current.host.trim().is_empty() {
        return rsx! {
            div { class: "shell-root ls-shell",
                RoutedShell { me: state.me.clone() }
            }
        };
    }

    if current.host.trim().is_empty() {
        return rsx! {
            ConnectHostScreen {
                config: current,
                on_change: move |next: BoxConfig| {
                    config.set(next);
                },
            }
        };
    }

    match state.gate {
        Gate::Unknown => rsx! { GateLoading {} },
        Gate::Setup => rsx! {
            SetupScreen {
                config: current.clone(),
                claim: state.claim.clone().unwrap_or_default(),
                on_claimed: move |next: BoxConfig| {
                    config.set(next.clone());
                    auth.set(AuthState {
                        gate: Gate::Open,
                        me: None,
                        claim: None,
                    });
                },
            }
        },
        Gate::Signin => rsx! {
            SigninScreen {
                config: current.clone(),
                on_signed_in: move |(next, me): (BoxConfig, Option<Me>)| {
                    if next.host.trim().is_empty() {
                        config.set(next);
                        auth.set(AuthState::default());
                        return;
                    }
                    let open = me.is_some() || !next.token.trim().is_empty();
                    config.set(next);
                    auth.set(AuthState {
                        gate: if open { Gate::Open } else { Gate::Signin },
                        me,
                        claim: auth().claim,
                    });
                },
            }
        },
        Gate::Open => {
            // Held accounts (LosOS Login lock) are restricted to account settings.
            let held = state.me.as_ref().map(|m| m.is_held()).unwrap_or(false);
            if held {
                rsx! {
                    crate::components::shell::ShellFrame { me: state.me.clone(), wide: false,
                        div { class: "ls-held-notice muted", style: "margin-bottom: 12px;",
                            "Your account needs attention before you can use the rest of this box."
                        }
                        SettingsShell { pane: "account".to_string() }
                    }
                }
            } else {
                rsx! {
                    RoutedShell { me: state.me.clone() }
                }
            }
        }
    }
}

async fn resolve_gate(config: BoxConfig, mut auth: AuthSignal, gate_gen: Signal<u64>, gen: u64) {
    let stale = || *gate_gen.read() != gen;
    if config.demo {
        if !stale() {
            auth.set(AuthState {
                gate: Gate::Open,
                me: None,
                claim: None,
            });
        }
        return;
    }
    if config.host.trim().is_empty() {
        if !stale() {
            auth.set(AuthState::default());
        }
        return;
    }

    if stale() {
        return;
    }
    let prev = auth();
    auth.set(AuthState {
        gate: Gate::Unknown,
        me: prev.me,
        claim: prev.claim,
    });

    let claim = match auth::get_claim_state(&config).await {
        Ok(c) => Some(c),
        Err(_) => None,
    };
    if stale() {
        return;
    }
    let me = match auth::load_me(&config).await {
        Ok(m) => m,
        Err(_) => None,
    };
    if stale() {
        return;
    }

    let gate = if me.is_some() {
        Gate::Open
    } else if let Some(c) = &claim {
        if c.claimed {
            Gate::Signin
        } else {
            Gate::Setup
        }
    } else if auth::has_token(&config) {
        Gate::Signin
    } else {
        Gate::Signin
    };

    auth.set(AuthState { gate, me, claim });
}

/// Common result state for pane data fetches.
#[derive(Clone, PartialEq)]
pub enum Load<T: Clone + PartialEq + 'static> {
    Idle,
    Loading,
    Ready(T),
    Err(String),
    Unauthorized,
}
