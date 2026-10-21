//! Files under the manifest's `files` roots.
//!
//! Paths are absolute strings. A missing file is [`crate::Error::NotFound`],
//! so the "absent means default" reads every module does are
//! [`read_optional`] and friends.

use crate::wit::files as w;
use crate::Result;

pub use w::{Entry, Kind, Metadata};

/// How a new file is written.
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteOptions {
    pub mode: Option<u32>,
    pub owner: Option<u32>,
    pub create_dirs: bool,
}

impl WriteOptions {
    /// 0o600, parents created: a secret, or anything only root reads.
    #[must_use]
    pub fn secret() -> Self {
        WriteOptions {
            mode: Some(0o600),
            owner: None,
            create_dirs: true,
        }
    }

    /// 0o644, parents created.
    #[must_use]
    pub fn public() -> Self {
        WriteOptions {
            mode: Some(0o644),
            owner: None,
            create_dirs: true,
        }
    }
}

pub fn read(path: &str) -> Result<Vec<u8>> {
    w::read(path).map_err(Into::into)
}

/// The file, or `None` when it is not there.
pub fn read_optional(path: &str) -> Result<Option<Vec<u8>>> {
    match w::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(w::Error::NotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The file as text (lossy), or `None` when it is not there.
pub fn read_string_optional(path: &str) -> Result<Option<String>> {
    Ok(read_optional(path)?.map(|b| String::from_utf8_lossy(&b).into_owned()))
}

pub fn read_string(path: &str) -> Result<String> {
    Ok(String::from_utf8_lossy(&read(path)?).into_owned())
}

/// At most `max` bytes from the end.
pub fn read_tail(path: &str, max: u32) -> Result<Vec<u8>> {
    w::read_tail(path, max).map_err(Into::into)
}

/// Replace the file atomically.
pub fn write(path: &str, data: &[u8], options: WriteOptions) -> Result<()> {
    w::write(
        path,
        data,
        w::WriteOptions {
            mode: options.mode,
            owner: options.owner,
            create_dirs: options.create_dirs,
        },
    )
    .map_err(Into::into)
}

pub fn append(path: &str, data: &[u8]) -> Result<()> {
    w::append(path, data).map_err(Into::into)
}

/// Remove a file or an empty directory; fine when it is already gone.
pub fn remove_if_present(path: &str) -> Result<()> {
    match w::remove(path) {
        Ok(()) | Err(w::Error::NotFound) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Remove a directory and what is in it; fine when it is already gone.
pub fn remove_tree_if_present(path: &str) -> Result<()> {
    match w::remove_tree(path) {
        Ok(()) | Err(w::Error::NotFound) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn rename(src: &str, dst: &str) -> Result<()> {
    w::rename(src, dst).map_err(Into::into)
}

pub fn create_dir(path: &str, mode: Option<u32>) -> Result<()> {
    w::create_dir(path, mode).map_err(Into::into)
}

pub fn list(dir: &str) -> Result<Vec<Entry>> {
    w::read_dir(dir).map_err(Into::into)
}

pub fn stat(path: &str) -> Result<Metadata> {
    w::stat(path).map_err(Into::into)
}

/// Whether anything is at `path`.
pub fn exists(path: &str) -> Result<bool> {
    match w::stat(path) {
        Ok(_) => Ok(true),
        Err(w::Error::NotFound) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// `dir` and `name` joined with one `/`.
#[must_use]
pub fn join(dir: &str, name: &str) -> String {
    format!(
        "{}/{}",
        dir.trim_end_matches('/'),
        name.trim_start_matches('/')
    )
}
