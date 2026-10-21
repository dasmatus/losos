//! The `files` import: the manifest's file roots, and nothing past them.
//!
//! A root is a file or a directory with an access (`ro` or `rw`). A path is
//! allowed when it is a root or lies under a directory root, and when no
//! component below the root is a symlink: lososd is root, and a link a
//! process with less could plant must not walk it out of the root. The
//! plugin has no way to make a link, so a legitimate one is never needed.

use super::bindings::losos::plugin::files::{Entry, Kind, Metadata, WriteOptions};
use super::bindings::losos::plugin::types::Error;
use super::manifest::{is_clean_path, Access, FileRoot};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

/// The most a `read` returns.
pub const MAX_READ: u64 = 64 * 1024 * 1024;

fn denied(why: impl Into<String>) -> Error {
    Error::Denied(why.into())
}

pub(crate) fn io_error(e: &std::io::Error, what: &str) -> Error {
    match e.kind() {
        std::io::ErrorKind::NotFound => Error::NotFound,
        _ => Error::Failed(format!("{what}: {e}")),
    }
}

/// A path the plugin may use, and the root it is under.
pub struct Allowed<'a> {
    pub path: PathBuf,
    pub root: &'a FileRoot,
}

/// Find the root `path` lies in, and check the components below it.
pub fn resolve<'a>(roots: &'a [FileRoot], path: &str, write: bool) -> Result<Allowed<'a>, Error> {
    if !is_clean_path(path) {
        return Err(denied(format!("{path:?} is not a clean absolute path")));
    }
    // The longest root wins, so a narrower `ro` inside an `rw` holds.
    let root = roots
        .iter()
        .filter(|r| path == r.path || path.starts_with(&format!("{}/", r.path)))
        .max_by_key(|r| r.path.len())
        .ok_or_else(|| denied(format!("{path} is outside the manifest's files")))?;
    if write && root.access != Access::Rw {
        return Err(denied(format!(
            "{} is read-only for this plugin",
            root.path
        )));
    }
    let below = &path[root.path.len()..];
    // A path below a root needs the root to be a directory already. A file
    // root grants that one file, and a root that does not exist yet is
    // made with `create-dir` first, so `create-dirs` on a write cannot turn
    // a file root into a directory.
    if !below.is_empty() && !std::fs::metadata(&root.path).is_ok_and(|m| m.is_dir()) {
        return Err(denied(format!("{} is not a directory", root.path)));
    }
    let mut at = PathBuf::from(&root.path);
    for seg in below.split('/').filter(|s| !s.is_empty()) {
        at.push(seg);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(denied(format!("{} is a symlink", at.display())))
            }
            Ok(_) => {}
            // Nothing further down exists either.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(io_error(&e, "checking the path")),
        }
    }
    // A root itself may be a symlink the box set up; what it names is
    // trusted like the root. A file root must not have become a link to
    // somewhere else, though: that would be a whole file elsewhere.
    if path == root.path {
        if let Ok(m) = std::fs::symlink_metadata(path) {
            if m.file_type().is_symlink() && write {
                return Err(denied(format!("{path} is a symlink")));
            }
        }
    }
    Ok(Allowed {
        path: PathBuf::from(path),
        root,
    })
}

fn kind_of(ft: std::fs::FileType) -> Kind {
    if ft.is_symlink() {
        Kind::Symlink
    } else if ft.is_dir() {
        Kind::Dir
    } else if ft.is_file() {
        Kind::File
    } else {
        Kind::Other
    }
}

/// Open a file for reading without following a final symlink.
fn open_nofollow(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
}

pub fn read(roots: &[FileRoot], path: &str) -> Result<Vec<u8>, Error> {
    let a = resolve(roots, path, false)?;
    let mut f = open_nofollow(&a.path).map_err(|e| io_error(&e, path))?;
    let mut out = Vec::new();
    f.by_ref()
        .take(MAX_READ + 1)
        .read_to_end(&mut out)
        .map_err(|e| io_error(&e, path))?;
    if out.len() as u64 > MAX_READ {
        return Err(Error::Failed(format!(
            "{path} is larger than {MAX_READ} bytes"
        )));
    }
    Ok(out)
}

pub fn read_tail(roots: &[FileRoot], path: &str, max: u32) -> Result<Vec<u8>, Error> {
    let a = resolve(roots, path, false)?;
    let mut f = open_nofollow(&a.path).map_err(|e| io_error(&e, path))?;
    let size = f.metadata().map_err(|e| io_error(&e, path))?.len();
    let max = u64::from(max).min(MAX_READ);
    if size > max {
        f.seek(SeekFrom::Start(size - max))
            .map_err(|e| io_error(&e, path))?;
    }
    let mut out = Vec::new();
    f.take(max)
        .read_to_end(&mut out)
        .map_err(|e| io_error(&e, path))?;
    Ok(out)
}

/// Create `dir` and its missing parents, all inside a root.
fn create_parents(roots: &[FileRoot], dir: &Path, mode: u32) -> Result<(), Error> {
    if dir.exists() {
        return Ok(());
    }
    let dir_str = dir.to_string_lossy();
    // Every level that gets created must itself be allowed.
    resolve(roots, &dir_str, true)?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(mode)
        .create(dir)
        .map_err(|e| io_error(&e, &dir_str))
}

pub fn write(
    roots: &[FileRoot],
    path: &str,
    data: &[u8],
    options: &WriteOptions,
) -> Result<(), Error> {
    let a = resolve(roots, path, true)?;
    if let Some(uid) = options.owner {
        if !a.root.chown.contains(&uid) {
            return Err(denied(format!(
                "{} may not hand files to uid {uid}",
                a.root.path
            )));
        }
    }
    let mode = options.mode.unwrap_or(0o644) & 0o777;
    let parent = a
        .path
        .parent()
        .ok_or_else(|| denied("a file needs a directory"))?;
    if options.create_dirs {
        create_parents(roots, parent, 0o755)?;
    }
    crate::io_backend::write_atomically(&a.path, data, mode, 0o755, options.owner)
        .map_err(|e| Error::Failed(format!("{e:#}")))
}

pub fn append(roots: &[FileRoot], path: &str, data: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let a = resolve(roots, path, true)?;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(&a.path)
        .map_err(|e| io_error(&e, path))?;
    f.write_all(data).map_err(|e| io_error(&e, path))
}

pub fn remove(roots: &[FileRoot], path: &str) -> Result<(), Error> {
    let a = resolve(roots, path, true)?;
    let meta = std::fs::symlink_metadata(&a.path).map_err(|e| io_error(&e, path))?;
    if meta.is_dir() {
        if path == a.root.path {
            return Err(denied(format!("{path} is a root; it stays")));
        }
        std::fs::remove_dir(&a.path).map_err(|e| io_error(&e, path))
    } else {
        std::fs::remove_file(&a.path).map_err(|e| io_error(&e, path))
    }
}

pub fn remove_tree(roots: &[FileRoot], path: &str) -> Result<(), Error> {
    let a = resolve(roots, path, true)?;
    if path == a.root.path {
        return Err(denied(format!("{path} is a root; it stays")));
    }
    let meta = std::fs::symlink_metadata(&a.path).map_err(|e| io_error(&e, path))?;
    if meta.is_dir() {
        // remove_dir_all does not follow symlinks inside the tree.
        std::fs::remove_dir_all(&a.path).map_err(|e| io_error(&e, path))
    } else {
        std::fs::remove_file(&a.path).map_err(|e| io_error(&e, path))
    }
}

pub fn rename(roots: &[FileRoot], src: &str, dst: &str) -> Result<(), Error> {
    let from = resolve(roots, src, true)?;
    let to = resolve(roots, dst, true)?;
    if src == from.root.path || dst == to.root.path && to.root.path != dst {
        return Err(denied(format!("{src} is a root; it stays")));
    }
    std::fs::rename(&from.path, &to.path).map_err(|e| io_error(&e, src))
}

pub fn create_dir(roots: &[FileRoot], path: &str, mode: Option<u32>) -> Result<(), Error> {
    let a = resolve(roots, path, true)?;
    create_parents(roots, &a.path, mode.unwrap_or(0o755) & 0o777)
}

pub fn read_dir(roots: &[FileRoot], dir: &str) -> Result<Vec<Entry>, Error> {
    let a = resolve(roots, dir, false)?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&a.path).map_err(|e| io_error(&e, dir))? {
        let entry = entry.map_err(|e| io_error(&e, dir))?;
        let kind = entry
            .file_type()
            .map(kind_of)
            .map_err(|e| io_error(&e, dir))?;
        out.push(Entry {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn stat(roots: &[FileRoot], path: &str) -> Result<Metadata, Error> {
    let a = resolve(roots, path, false)?;
    let m = std::fs::symlink_metadata(&a.path).map_err(|e| io_error(&e, path))?;
    Ok(Metadata {
        kind: kind_of(m.file_type()),
        size: m.len(),
        mode: m.mode() & 0o7777,
        uid: m.uid(),
        gid: m.gid(),
        modified: u64::try_from(m.mtime()).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "losos-files-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(d.join("state")).unwrap();
        d
    }

    fn roots(base: &Path) -> Vec<FileRoot> {
        vec![
            FileRoot {
                path: format!("{}/state", base.display()),
                access: Access::Rw,
                chown: vec![],
            },
            FileRoot {
                path: format!("{}/state/sealed", base.display()),
                access: Access::Ro,
                chown: vec![],
            },
            FileRoot {
                path: format!("{}/one.json", base.display()),
                access: Access::Rw,
                chown: vec![],
            },
        ]
    }

    fn opts() -> WriteOptions {
        WriteOptions {
            mode: Some(0o600),
            owner: None,
            create_dirs: true,
        }
    }

    #[test]
    fn files_inside_a_root_are_read_and_written() {
        let base = tmp();
        let r = roots(&base);
        let p = format!("{}/state/a/b.json", base.display());
        write(&r, &p, b"{}", &opts()).unwrap();
        assert_eq!(read(&r, &p).unwrap(), b"{}");
        let m = stat(&r, &p).unwrap();
        assert_eq!(m.mode, 0o600);
        assert_eq!(m.kind, Kind::File);
        append(&r, &p, b"x").unwrap();
        assert_eq!(read_tail(&r, &p, 2).unwrap(), b"}x");
        let names: Vec<String> = read_dir(&r, &format!("{}/state/a", base.display()))
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, ["b.json"]);
        let one = format!("{}/one.json", base.display());
        write(&r, &one, b"1", &opts()).unwrap();
        remove(&r, &one).unwrap();
        assert!(matches!(remove(&r, &one), Err(Error::NotFound)));
        assert!(matches!(read(&r, &one), Err(Error::NotFound)));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn paths_outside_the_roots_are_denied() {
        let base = tmp();
        let r = roots(&base);
        let b = base.display();
        for bad in [
            "/etc/shadow".to_string(),
            format!("{b}/stateful/x"),
            format!("{b}/state/../one.json"),
            format!("{b}/state//x"),
            format!("{b}/one.json/x"),
            format!("{b}/two.json"),
            "relative".to_string(),
        ] {
            assert!(
                matches!(read(&r, &bad), Err(Error::Denied(_))),
                "{bad} should be denied"
            );
        }
        // A narrower ro root inside an rw one holds.
        assert!(matches!(
            write(&r, &format!("{b}/state/sealed/x"), b"", &opts()),
            Err(Error::Denied(_))
        ));
        // Handing a file to someone needs the root's say-so.
        let with_owner = WriteOptions {
            owner: Some(1000),
            ..opts()
        };
        assert!(matches!(
            write(&r, &format!("{b}/state/x"), b"", &with_owner),
            Err(Error::Denied(_))
        ));
        // A root itself stays.
        assert!(matches!(
            remove_tree(&r, &format!("{b}/state")),
            Err(Error::Denied(_))
        ));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_symlink_below_a_root_is_not_followed() {
        let base = tmp();
        let r = roots(&base);
        std::fs::create_dir_all(base.join("state")).unwrap();
        std::fs::write(base.join("secret"), "s").unwrap();
        std::os::unix::fs::symlink(base.join("secret"), base.join("state/link")).unwrap();
        std::os::unix::fs::symlink(&base, base.join("state/dir")).unwrap();
        let b = base.display();
        assert!(matches!(
            read(&r, &format!("{b}/state/link")),
            Err(Error::Denied(_))
        ));
        assert!(matches!(
            read(&r, &format!("{b}/state/dir/secret")),
            Err(Error::Denied(_))
        ));
        assert!(matches!(
            write(&r, &format!("{b}/state/link"), b"x", &opts()),
            Err(Error::Denied(_))
        ));
        assert_eq!(std::fs::read_to_string(base.join("secret")).unwrap(), "s");
        std::fs::remove_dir_all(&base).ok();
    }
}
