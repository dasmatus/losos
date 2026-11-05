//! Native LosOS Office capabilities: which file types LosOS Office opens,
//! and how.
//!
//! Collabora/coolwsd discovery is no longer used. This module defines the
//! format policy for native LosOS Office:
//!
//! - `.md`, `.paper`, `.slides` open in native editors (handled by the web
//!   app; listed here for completeness).
//! - Legacy Microsoft/ODF formats are *importable* into native LosOS Office
//!   formats (import to native + export). They are not edited in place by an
//!   embedded legacy engine.
//! - Other types are not offered by Office.

use std::collections::BTreeMap;

/// How LosOS Office handles a file type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OfficeMode {
    /// Open in a native LosOS Office editor.
    Native,
    /// Import into a native LosOS Office file (copy/conversion flow).
    Import,
    /// View only (no edit/import flow implemented here).
    View,
}

/// One supported extension.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficeType {
    pub extension: String,
    pub mode: OfficeMode,
    /// Whether the web app can edit this type directly (native editor).
    pub edit: bool,
    /// The native target kind for imports, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_target: Option<&'static str>,
}

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    types: BTreeMap<String, OfficeType>,
}

impl Capabilities {
    pub fn native() -> Self {
        let mut types = BTreeMap::new();
        let mut add = |ext: &str, mode: OfficeMode, edit: bool, import_target: Option<&'static str>| {
            types.insert(
                ext.to_ascii_lowercase(),
                OfficeType {
                    extension: ext.to_ascii_lowercase(),
                    mode,
                    edit,
                    import_target,
                },
            );
        };

        // Native LosOS Office formats.
        add("md", OfficeMode::Native, true, None);
        add("markdown", OfficeMode::Native, true, None);
        add("paper", OfficeMode::Native, true, None);
        add("slides", OfficeMode::Native, true, None);

        // Legacy document formats: import to native (target chosen by client/importer).
        for ext in ["doc", "docx", "docm", "dotx", "odt", "rtf", "txt"] {
            add(ext, OfficeMode::Import, false, Some("paper"));
        }
        // Presentations: import to native slides/deck.
        for ext in ["ppt", "pptx", "pptm", "ppsx", "odp", "odg"] {
            add(ext, OfficeMode::Import, false, Some("slides"));
        }
        // Spreadsheets: importable, but target is limited initially.
        for ext in ["xls", "xlsx", "xlsm", "xlsb", "ods", "csv"] {
            add(ext, OfficeMode::Import, false, Some("paper"));
        }
        // View-only / import as content where supported later.
        add("pdf", OfficeMode::View, false, None);

        Self { types }
    }

    pub fn action(&self, ext: &str) -> Option<&OfficeType> {
        self.types.get(&ext.to_ascii_lowercase())
    }

    pub fn extensions(&self) -> impl Iterator<Item = &OfficeType> {
        self.types.values()
    }

    pub fn list(&self) -> Vec<OfficeType> {
        self.types.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_formats_are_editable() {
        let c = Capabilities::native();
        for ext in ["md", "paper", "slides"] {
            let t = c.action(ext).unwrap();
            assert_eq!(t.mode, OfficeMode::Native);
            assert!(t.edit);
        }
    }

    #[test]
    fn legacy_formats_are_importable() {
        let c = Capabilities::native();
        let docx = c.action("docx").unwrap();
        assert_eq!(docx.mode, OfficeMode::Import);
        assert!(!docx.edit);
        assert_eq!(docx.import_target, Some("paper"));
    }

    #[test]
    fn pdf_is_view_only() {
        let c = Capabilities::native();
        assert_eq!(c.action("PDF").unwrap().mode, OfficeMode::View);
    }
}
