use crate::app::{AuthSignal, AuthState, ConfigSignal};
use crate::auth::Gate;
use crate::middle_layer::credentials::CredentialsSection;
use crate::store;
use dioxus::prelude::*;

#[component]
pub fn CredentialsPane() -> Element {
    let mut config: ConfigSignal = use_context();
    let mut auth_sig: AuthSignal = use_context();
    let cfg = config();
    rsx! {
        CredentialsSection {
            config: cfg,
            on_config: move |next| {
                store::save(&next);
                config.set(next);
            },
            on_unauthorized: move |_| {
                let mut next = config();
                crate::auth::drop_token(&mut next);
                store::save(&next);
                config.set(next);
                auth_sig.set(AuthState {
                    gate: Gate::Signin,
                    me: None,
                    claim: auth_sig().claim,
                });
            },
        }
    }
}
