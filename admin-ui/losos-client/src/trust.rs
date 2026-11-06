//! TLS trust modes, aligned with `admin-ui/tui/src/http.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    Public,
    Pinned(String),
    FirstUse,
    Permissive,
}
