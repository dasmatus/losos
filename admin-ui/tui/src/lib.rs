//! `losos`, the terminal client for LosOS boxes and edges: the web UI's
//! panes in a terminal, and a few commands for scripts, over the signed and
//! encrypted envelope of docs/terminal-access.md.

pub mod agents;
pub mod cli;
pub mod commands;
pub mod computer;
pub mod envelope;
pub mod http;
pub mod i18n;
pub mod keychain;
pub mod logging;
pub mod problem;
pub mod prompt;
pub mod theme;
pub mod tui;
pub mod vault;
