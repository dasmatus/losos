//! Settings draft/apply engine (P0 foundation), aligned with React `use-settings-form`.

pub mod api;
pub mod form;
pub mod model;
pub mod nix;

pub use model::{Extras, OptionsResponse, SettingsResponse, StatusResponse};
pub use nix::{build_overrides_nix, is_valid_host_name, is_valid_time, nix_string};

pub const DEFAULT_HOST_NAME: &str = "mattbox";
pub const DEFAULT_WINDOW_START: &str = "23:00";
pub const DEFAULT_WINDOW_END: &str = "07:00";
pub const SETTLE_MS: u64 = 20_000;
pub const STATUS_PERIOD_MS: u64 = 2_000;
