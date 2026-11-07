pub use crate::components::app_bar::AppBar;
pub use crate::components::shell::{RoutedShell, ShellFrame};
pub use crate::components::sidebar::Sidebar;

use crate::app::AuthSignal;
use crate::auth::Me;
use dioxus::prelude::*;

/// Backward-compatible sidebar accessor reading me from auth context if available.
#[component]
pub fn SidebarCompat() -> Element {
    let auth = try_use_context::<AuthSignal>();
    let me = auth.and_then(|a| a().me);
    rsx! { crate::components::sidebar::Sidebar { me } }
}
