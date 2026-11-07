//! LosOS Office: native document opening and format capabilities.
//!
//! Collabora Online / WOPI integration has been removed. Native LosOS
//! Office handles `.md`, `.paper`, and `.slides` directly in the web app.
//! Legacy Microsoft/ODF formats are offered for import into native LosOS
//! Office formats (import to native + export), rather than edited in place
//! by an embedded legacy engine.

pub mod actix;
pub mod capabilities;
pub mod discovery;
pub mod import;
pub mod store;
pub mod token;
pub mod utf7;

pub use capabilities::{Capabilities, OfficeMode, OfficeType};
pub use discovery::{Action, Coolwsd, Discovery};
pub use store::{FileInfo, OfficeStore, StoreError};
pub use token::{AccessToken, TokenKey};

use serde::{Deserialize, Serialize};

/// What the cloud/folder Office API answers for `open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenResponse {
    /// The file should be opened in a native LosOS Office editor. The web
    /// app routes by file name/id; this response mainly carries metadata.
    #[serde(rename_all = "camelCase")]
    Native {
        file_id: String,
        name: String,
        writable: bool,
        extension: String,
    },
    /// The file is a legacy format: import it into a native LosOS Office file.
    #[serde(rename_all = "camelCase")]
    Import {
        file_id: String,
        name: String,
        extension: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        import_target: Option<String>,
    },
    /// The file can be viewed but not edited/imported by Office.
    #[serde(rename_all = "camelCase")]
    View {
        file_id: String,
        name: String,
        extension: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("Office does not open .{0} files")]
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub login: String,
    pub display_name: String,
}

/// Resolve how `file_id` should be opened for `person`.
pub fn open(
    store: &dyn OfficeStore,
    capabilities: &Capabilities,
    person: &Person,
    file_id: &str,
) -> Result<OpenResponse, OpenError> {
    let info = store.stat(&person.login, file_id)?;
    let ext = extension(&info.name).unwrap_or_default();
    let action = capabilities
        .action(&ext)
        .ok_or_else(|| OpenError::Unsupported(ext.clone()))?;
    Ok(match action.mode {
        OfficeMode::Native => OpenResponse::Native {
            file_id: file_id.to_string(),
            name: info.name,
            writable: info.writable && action.edit,
            extension: ext,
        },
        OfficeMode::Import => OpenResponse::Import {
            file_id: file_id.to_string(),
            name: info.name,
            extension: ext,
            import_target: action.import_target.map(str::to_string),
        },
        OfficeMode::View => OpenResponse::View {
            file_id: file_id.to_string(),
            name: info.name,
            extension: ext,
        },
    })
}

/// The extension of a file name, lower case, without the dot.
pub fn extension(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::store::DirStore;

    fn setup() -> (assert_fs::TempDir, DirStore, Capabilities) {
        let dir = assert_fs::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Note.paper"), b"paper").unwrap();
        std::fs::write(dir.path().join("Report.docx"), b"doc").unwrap();
        std::fs::write(dir.path().join("archive.xyz"), b"x").unwrap();
        let store = DirStore::new(dir.path());
        (dir, store, Capabilities::native())
    }

    fn person() -> Person {
        Person {
            login: "matus".into(),
            display_name: "Matus".into(),
        }
    }

    #[test]
    fn native_format_opens_native() {
        let (_dir, store, caps) = setup();
        let id = DirStore::id_for("Note.paper");
        let r = open(&store, &caps, &person(), &id).unwrap();
        assert!(matches!(r, OpenResponse::Native { writable: true, .. }));
    }

    #[test]
    fn legacy_format_is_importable() {
        let (_dir, store, caps) = setup();
        let id = DirStore::id_for("Report.docx");
        let r = open(&store, &caps, &person(), &id).unwrap();
        assert!(matches!(r, OpenResponse::Import { .. }));
    }

    #[test]
    fn unsupported_format_is_refused() {
        let (_dir, store, caps) = setup();
        let id = DirStore::id_for("archive.xyz");
        let err = open(&store, &caps, &person(), &id).unwrap_err();
        assert!(matches!(err, OpenError::Unsupported(ext) if ext == "xyz"));
    }

    #[test]
    fn extensions_are_lower_case_and_need_a_stem() {
        assert_eq!(extension("A.DOCX").as_deref(), Some("docx"));
        assert_eq!(extension(".bashrc"), None);
        assert_eq!(extension("README"), None);
    }
}
