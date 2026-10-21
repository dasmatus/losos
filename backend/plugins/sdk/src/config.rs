//! The manifest's `config` table, with `${VAR}` already filled in.

/// One value, or `None` when the manifest has no such key (or its
/// `${VAR:-}` came out empty).
#[must_use]
pub fn get(key: &str) -> Option<String> {
    crate::wit::config::get(key).filter(|v| !v.is_empty())
}

/// One value, or `default`.
#[must_use]
pub fn get_or(key: &str, default: &str) -> String {
    get(key).unwrap_or_else(|| default.to_string())
}
