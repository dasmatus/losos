//! Capability discovery for LosOS Office.
//!
//! Collabora/coolwsd discovery has been removed. Native LosOS Office uses
//! a fixed capability registry; this module keeps the small API shape used
//! by session routes while the engine is migrated.

use crate::office::capabilities::{Capabilities, OfficeMode, OfficeType};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub edit: bool,
    pub mode: OfficeMode,
    pub import_target: Option<&'static str>,
}

#[derive(Debug, Clone, Default)]
pub struct Discovery {
    capabilities: Capabilities,
}

impl Discovery {
    pub fn native() -> Self {
        Self {
            capabilities: Capabilities::native(),
        }
    }

    pub fn parse(_xml: &str) -> Self {
        // Collabora XML discovery is no longer supported.
        Self::native()
    }

    pub fn fetch(_base: &str) -> std::io::Result<Self> {
        Ok(Self::native())
    }

    pub fn action(&self, ext: &str) -> Option<Action> {
        self.capabilities.action(ext).map(|t: &OfficeType| Action {
            edit: t.edit,
            mode: t.mode,
            import_target: t.import_target,
        })
    }

    pub fn extensions(&self) -> impl Iterator<Item = (&str, bool)> {
        self.capabilities
            .extensions()
            .map(|t| (t.extension.as_str(), t.edit))
    }

    pub fn types(&self) -> impl Iterator<Item = &OfficeType> {
        self.capabilities.extensions()
    }
}

/// Compatibility shim replacing the former coolwsd-backed discovery cache.
#[derive(Debug, Clone)]
pub struct Coolwsd {
    discovery: Arc<Discovery>,
}

impl Coolwsd {
    pub const REFRESH: Duration = Duration::from_secs(10 * 60);

    pub fn new(_base: &str) -> Self {
        Self {
            discovery: Arc::new(Discovery::native()),
        }
    }

    pub fn native() -> Self {
        Self::new("")
    }

    pub fn discovery(&self) -> std::io::Result<Arc<Discovery>> {
        Ok(self.discovery.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_discovery_lists_losos_formats() {
        let d = Discovery::native();
        assert!(d.action("paper").unwrap().edit);
        assert!(matches!(d.action("docx").unwrap().mode, OfficeMode::Import));
        assert!(d.action("unknownextxyz").is_none());
    }
}
