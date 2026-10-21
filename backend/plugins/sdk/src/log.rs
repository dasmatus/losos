//! Lines in the journal, under the plugin's name.

use crate::wit::log::{log, Level};

pub fn trace(message: &str) {
    log(Level::Trace, message);
}

pub fn debug(message: &str) {
    log(Level::Debug, message);
}

pub fn info(message: &str) {
    log(Level::Info, message);
}

pub fn warn(message: &str) {
    log(Level::Warn, message);
}

pub fn error(message: &str) {
    log(Level::Error, message);
}
