//! Moving a Nextcloud data folder's files into the cloud, once, when a box
//! switches over.
//!
//! Nextcloud keeps each account's files at `<data>/<login>/files/`. They are
//! moved (a rename, so no second copy on disk) into the person's home and
//! then scanned. A name the person already has in the cloud keeps the cloud's
//! file; the Nextcloud one lands beside it as "<name> (from Nextcloud)".
//! The apps keep a journal of file ids, and these files get new ones, so
//! each computer does one full pass after the switch. Nothing is lost by it:
//! the content and the mtimes come across, and the desktop app matches
//! files by both before it transfers anything.

use crate::cloud::store::{join, valid_name, valid_owner, StoreError};
use crate::cloud::Cloud;
use std::fs;
use std::path::Path;

/// Import every account found under `data`. Returns each login and how
/// many entries its scan picked up.
pub fn nextcloud(cloud: &Cloud, data: &Path) -> Result<Vec<(String, usize)>, StoreError> {
    let mut out = Vec::new();
    for e in fs::read_dir(data)? {
        let e = e?;
        let Some(login) = e.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let files = e.path().join("files");
        if !valid_owner(&login) || !files.is_dir() {
            continue;
        }
        cloud.store.ensure_home(&login)?;
        let home = cloud.store.disk_path(&login, "");
        move_tree(&files, &home, "")?;
        let n = cloud.store.scan(&login)?;
        out.push((login, n));
    }
    Ok(out)
}

fn move_tree(from: &Path, to: &Path, rel: &str) -> Result<(), StoreError> {
    for e in fs::read_dir(from.join(rel))? {
        let e = e?;
        let Some(name) = e.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !valid_name(&name) {
            continue;
        }
        let child = join(rel, &name);
        let src = from.join(&child);
        let dst = to.join(&child);
        let meta = fs::symlink_metadata(&src)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if !dst.exists() {
            fs::rename(&src, &dst)?;
        } else if meta.is_dir() && dst.is_dir() {
            move_tree(from, to, &child)?;
        } else {
            let (stem, ext) = match name.rsplit_once('.') {
                Some((s, x)) if !s.is_empty() && !meta.is_dir() => (s.to_owned(), format!(".{x}")),
                _ => (name.clone(), String::new()),
            };
            let mut alt = dst.with_file_name(format!("{stem} (from Nextcloud){ext}"));
            let mut n = 2;
            while alt.exists() {
                alt = dst.with_file_name(format!("{stem} (from Nextcloud {n}){ext}"));
                n += 1;
            }
            fs::rename(&src, &alt)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::Config;

    #[test]
    fn a_nextcloud_folder_moves_in_and_clashes_keep_both() {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let nc = data.path().join("notshared/files");
        fs::create_dir_all(nc.join("Photos")).unwrap();
        fs::write(nc.join("Photos/a.jpg"), b"jpeg").unwrap();
        fs::write(nc.join("notes.txt"), b"from nc").unwrap();
        fs::create_dir_all(data.path().join("appdata_oc123")).unwrap();
        let cloud = Cloud::open(root.path(), Config::default()).unwrap();
        cloud.store.ensure_home("notshared").unwrap();
        cloud.store.write_bytes("notshared", "notes.txt", b"ours").unwrap();
        let r = nextcloud(&cloud, data.path()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(cloud.store.read_bytes("notshared", "notes.txt").unwrap(), b"ours");
        assert_eq!(
            cloud.store.read_bytes("notshared", "notes (from Nextcloud).txt").unwrap(),
            b"from nc"
        );
        assert_eq!(cloud.store.stat("notshared", "Photos/a.jpg").unwrap().size, 4);
        assert!(!nc.join("Photos/a.jpg").exists(), "moved, not copied");
    }
}
