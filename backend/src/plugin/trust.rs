//! Where lososd will load a plugin from.
//!
//! A plugin runs inside lososd, which is root, and its manifest is its whole
//! grant. So the only plugins lososd loads are ones nobody on the box can
//! change: files inside the Nix store, on a read-only mount, owned by root
//! and with no write bit for anyone. The NixOS module points
//! `LOSOS_PLUGIN_DIR` at a store path (`losos.backend.plugins`); a
//! directory anywhere else, or one file in it that fails a check, and lososd
//! refuses to start rather than run with part of its API missing.
//!
//! The directory may be a `symlinkJoin` of several plugin packages, so each
//! file is checked where its links lead, not where it was found.

use anyhow::{bail, Context};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// What a plugin's files must satisfy.
#[derive(Debug, Clone)]
pub struct Trust {
    /// Every file, links resolved, must be under this directory.
    pub root: PathBuf,
    /// And on a filesystem mounted read-only.
    pub read_only_mount: bool,
    /// And owned by root.
    pub root_owned: bool,
}

impl Trust {
    /// The rule lososd runs under: the Nix store, read-only, root's.
    #[must_use]
    pub fn nix_store() -> Self {
        Trust {
            root: PathBuf::from("/nix/store"),
            read_only_mount: true,
            root_owned: true,
        }
    }

    /// [`Trust::nix_store`], always, in a release build. A debug build, which
    /// is what `cargo test` runs as the daemon, also takes
    /// `LOSOS_PLUGIN_TRUST_ROOT`: a directory whose sealed files stand in
    /// for the store, without the mount and owner checks a test cannot meet.
    /// No package ever ships a debug build (nix/packages.nix builds release).
    #[must_use]
    pub fn from_env() -> Self {
        #[cfg(debug_assertions)]
        if let Some(root) = std::env::var_os("LOSOS_PLUGIN_TRUST_ROOT").filter(|r| !r.is_empty()) {
            tracing::warn!(
                "debug build: plugins load from LOSOS_PLUGIN_TRUST_ROOT, unchecked mount"
            );
            return Trust {
                root: PathBuf::from(root),
                read_only_mount: false,
                root_owned: false,
            };
        }
        Trust::nix_store()
    }
}

/// One plugin found in the directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub wasm: PathBuf,
    pub manifest: PathBuf,
}

/// Check one path: resolved, inside the root, read-only, nobody's to write.
fn check(path: &Path, trust: &Trust, want_dir: bool) -> anyhow::Result<PathBuf> {
    let real =
        std::fs::canonicalize(path).with_context(|| format!("resolving {}", path.display()))?;
    let root = std::fs::canonicalize(&trust.root)
        .with_context(|| format!("resolving {}", trust.root.display()))?;
    if !real.starts_with(&root) || real == root {
        bail!(
            "{} is outside {}; plugins load only from there",
            real.display(),
            root.display()
        );
    }
    // Everything from the file up to the root: a writable directory on the
    // way could swap what the path names.
    let mut at = real.as_path();
    loop {
        let meta = std::fs::metadata(at).with_context(|| format!("reading {}", at.display()))?;
        if at == real {
            if want_dir && !meta.is_dir() {
                bail!("{} is not a directory", at.display());
            }
            if !want_dir && !meta.is_file() {
                bail!("{} is not a regular file", at.display());
            }
        }
        if at != root.as_path() {
            if meta.mode() & 0o222 != 0 {
                bail!(
                    "{} is writable (mode {:o}); plugins load only from read-only files",
                    at.display(),
                    meta.mode() & 0o7777
                );
            }
            if trust.root_owned && meta.uid() != 0 {
                bail!("{} is not root's", at.display());
            }
        }
        match at.parent() {
            Some(parent) if at != root.as_path() => at = parent,
            _ => break,
        }
    }
    if trust.read_only_mount {
        let vfs =
            rustix::fs::statvfs(&real).with_context(|| format!("statvfs {}", real.display()))?;
        if !vfs.f_flag.contains(rustix::fs::StatVfsMountFlags::RDONLY) {
            bail!(
                "{} is on a filesystem mounted read-write; plugins load only from a read-only one",
                real.display()
            );
        }
    }
    Ok(real)
}

/// Every plugin in `dir`: each `<name>.wasm` with its `<name>.json`, both
/// checked. Sorted by name, so the load order (and with it which plugin
/// reports a route clash) does not depend on the directory's order.
pub fn discover(dir: &Path, trust: &Trust) -> anyhow::Result<Vec<Found>> {
    let dir = check(dir, trust, true)?;
    let mut found = Vec::new();
    let entries = std::fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".wasm") else {
            continue;
        };
        if !super::manifest::is_name(stem) {
            bail!("{name}: not a plugin name");
        }
        let wasm = check(&entry.path(), trust, false)?;
        let manifest_path = dir.join(format!("{stem}.json"));
        if !manifest_path.exists() {
            bail!("{name} has no {stem}.json beside it");
        }
        let manifest = check(&manifest_path, trust, false)?;
        found.push(Found {
            name: stem.to_string(),
            wasm,
            manifest,
        });
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A throwaway "store": the mount and owner checks are off, because a
    /// test can make neither, and they are covered by the VM test.
    fn store() -> (PathBuf, Trust) {
        let root = std::env::temp_dir().join(format!(
            "losos-trust-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let trust = Trust {
            root: root.clone(),
            read_only_mount: false,
            root_owned: false,
        };
        (root, trust)
    }

    fn put(path: &Path, body: &str, mode: u32) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    fn seal(dir: &Path) {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    }

    fn unseal(dir: &Path) {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn a_sealed_directory_loads() {
        let (root, trust) = store();
        let pkg = root.join("abc-plugins");
        put(&pkg.join("look.wasm"), "w", 0o444);
        put(&pkg.join("look.json"), "{}", 0o444);
        put(&pkg.join("README"), "x", 0o444);
        seal(&pkg);
        let found = discover(&pkg, &trust).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "look");
        unseal(&pkg);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_writable_file_or_directory_is_refused() {
        let (root, trust) = store();
        let pkg = root.join("abc-plugins");
        put(&pkg.join("look.wasm"), "w", 0o644);
        put(&pkg.join("look.json"), "{}", 0o444);
        seal(&pkg);
        let e = discover(&pkg, &trust).unwrap_err().to_string();
        assert!(e.contains("writable"), "{e}");
        unseal(&pkg);

        std::fs::set_permissions(
            pkg.join("look.wasm"),
            std::fs::Permissions::from_mode(0o444),
        )
        .unwrap();
        // The directory itself is writable: someone could swap the files.
        let e = discover(&pkg, &trust).unwrap_err().to_string();
        assert!(e.contains("writable"), "{e}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_link_out_of_the_store_is_refused() {
        let (root, trust) = store();
        let outside = std::env::temp_dir().join(format!("losos-outside-{}", std::process::id()));
        put(&outside.join("evil.wasm"), "w", 0o444);
        let pkg = root.join("abc-plugins");
        std::fs::create_dir_all(&pkg).unwrap();
        std::os::unix::fs::symlink(outside.join("evil.wasm"), pkg.join("evil.wasm")).unwrap();
        put(&pkg.join("evil.json"), "{}", 0o444);
        seal(&pkg);
        let e = discover(&pkg, &trust).unwrap_err().to_string();
        assert!(e.contains("outside"), "{e}");
        unseal(&pkg);
        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&outside).ok();

        // And the directory itself must be inside the root.
        let (root, trust) = store();
        let e = discover(Path::new("/tmp"), &trust).unwrap_err().to_string();
        assert!(e.contains("outside"), "{e}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_plugin_without_its_manifest_is_refused() {
        let (root, trust) = store();
        let pkg = root.join("abc-plugins");
        put(&pkg.join("look.wasm"), "w", 0o444);
        seal(&pkg);
        let e = discover(&pkg, &trust).unwrap_err().to_string();
        assert!(e.contains("look.json"), "{e}");
        unseal(&pkg);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_shipped_rule_is_the_store_read_only_and_roots() {
        let t = Trust::nix_store();
        assert_eq!(t.root, PathBuf::from("/nix/store"));
        assert!(t.read_only_mount && t.root_owned);
    }
}
