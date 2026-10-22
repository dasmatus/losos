//! Each person's LosOS Cloud files as a Windows network share, signed in to
//! with their LosOS name and password (`modules/windows.nix`).
//!
//! Samba serves the shares and answers the sign-in. A Windows sign-in is
//! NTLM, which the server can only check against the NT hash of the
//! password (MD4 of its UTF-16), never against the argon2id hash LosOS Login
//! keeps. So this daemon makes the NT hash whenever a password passes
//! through it in the clear (an account made, a password set, a password
//! checked: [`crate::people::People`] calls [`Windows::learn`]), and writes
//! everything Samba reads from the people file:
//!
//!   * `smbpasswd`, Samba's password file, one line per person who is ready
//!     (root's, 0600);
//!   * `shares.conf`, one share per ready person, named after them, holding
//!     their LosOS Cloud files (`<cloud>/<login>/files`), every file
//!     touched as the cloud's own user so the cloud sees it as its own;
//!   * `users.map`, which turns the name a person types into the account
//!     Samba knows them by, `losos-win-<name>`. The prefix keeps a person
//!     called `root` or `nginx` from naming a real account;
//!   * `/run/userdb/losos-win-<name>.user`, the Unix account Samba insists
//!     every one of its users has, as a systemd userdb record: nothing on
//!     the box can sign in as it, and it is gone at the next boot until this
//!     daemon writes it again.
//!
//! A person is ready while the NT hash was made from the password they have
//! now: `learned.json` keeps, per person, the argon2id hash it was learned
//! next to. A password changed anywhere else, an account disabled, locked
//! or removed, and their line is gone from Samba at the next write.
//!
//! The owner's files are the `notshared` account's in LosOS Cloud, as on
//! every other door (`modules/app-sign-in.nix`).

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::people::{LOCKED, OWNER};
use crate::setup::Secret;

/// The prefix of the accounts Samba knows people by.
pub const ACCOUNT_PREFIX: &str = "losos-win-";
/// The owner's login in LosOS Cloud.
const OWNER_LOGIN: &str = "notshared";
/// Names smb.conf gives a meaning of its own.
const SPECIAL_SECTIONS: [&str; 3] = ["global", "homes", "printers"];

/// One person as the people file has them, for [`Windows::sync`].
#[derive(Debug, Clone)]
pub struct Account {
    pub name: String,
    pub display_name: String,
    pub groups: Vec<String>,
    pub disabled: bool,
    /// The argon2id hash LosOS Login checks.
    pub hash: String,
}

impl Account {
    fn may_use(&self) -> bool {
        !self.disabled && !self.groups.iter().any(|g| g == LOCKED)
    }

    fn login(&self) -> &str {
        if self.groups.iter().any(|g| g == OWNER) {
            OWNER_LOGIN
        } else {
            &self.name
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Learned {
    /// The argon2id hash the person had when the NT hash was made.
    argon: String,
    /// Upper-case hex, as Samba writes it.
    nt: String,
}

/// What the Account pane shows a person about their Windows sign-in.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Whether Windows takes their LosOS password now.
    pub ready: bool,
    /// The share's name, under `\\<box>\`.
    pub share: String,
}

pub struct Windows {
    dir: PathBuf,
    userdb: PathBuf,
    cloud: PathBuf,
    smbcontrol: Option<PathBuf>,
    lock: Mutex<()>,
}

impl Windows {
    /// From `LOSOS_WINDOWS_DIR` (`modules/windows.nix`); `None` while the
    /// share is off, so no password is ever turned into an NT hash then.
    pub fn from_env() -> Option<Windows> {
        let dir = std::env::var_os("LOSOS_WINDOWS_DIR").filter(|d| !d.is_empty())?;
        let var = |k: &str, d: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(d))
        };
        Some(Windows::new(
            PathBuf::from(dir),
            var("LOSOS_WINDOWS_USERDB", "/run/userdb"),
            var("LOSOS_CLOUD_ROOT", "/var/lib/losos-cloud"),
            std::env::var_os("LOSOS_WINDOWS_SMBCONTROL").map(PathBuf::from),
        ))
    }

    pub fn new(
        dir: PathBuf,
        userdb: PathBuf,
        cloud: PathBuf,
        smbcontrol: Option<PathBuf>,
    ) -> Windows {
        Windows {
            dir,
            userdb,
            cloud,
            smbcontrol,
            lock: Mutex::new(()),
        }
    }

    fn learned_file(&self) -> PathBuf {
        self.dir.join("learned.json")
    }

    fn read_learned(&self) -> BTreeMap<String, Learned> {
        std::fs::read(self.learned_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// The share's name for `name`.
    pub fn share_name(name: &str) -> String {
        if SPECIAL_SECTIONS.contains(&name) {
            format!("{name}-files")
        } else {
            name.to_string()
        }
    }

    /// Where `account` stands.
    pub fn status(&self, account: &Account) -> Status {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let learned = self.read_learned();
        Status {
            ready: account.may_use()
                && learned
                    .get(&account.name)
                    .is_some_and(|l| l.argon == account.hash),
            share: Self::share_name(&account.name),
        }
    }

    /// `secret` is `account`'s password now: make its NT hash, unless the
    /// one kept is already for that password. Then [`Windows::sync`].
    pub fn learn(&self, account: &Account, secret: &Secret, everyone: &[Account]) {
        {
            let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
            let mut learned = self.read_learned();
            if learned
                .get(&account.name)
                .is_some_and(|l| l.argon == account.hash)
            {
                return;
            }
            learned.insert(
                account.name.clone(),
                Learned {
                    argon: account.hash.clone(),
                    nt: hex_upper(&nt_hash(secret.expose())),
                },
            );
            if let Err(e) = self.write_private(
                &self.learned_file(),
                &serde_json::to_vec_pretty(&learned).unwrap_or_default(),
            ) {
                tracing::error!(error = ?e, "the Windows sign-in could not be kept");
                return;
            }
        }
        self.sync(everyone);
    }

    /// Write what Samba reads from `everyone`, and drop what a person no
    /// longer may have.
    pub fn sync(&self, everyone: &[Account]) {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = self.sync_locked(everyone) {
            tracing::error!(error = ?e, "the Windows shares could not be written");
        }
    }

    fn sync_locked(&self, everyone: &[Account]) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let mut learned = self.read_learned();
        // Forget the hashes of people who are gone or changed their
        // password since: nothing may open a share with an old password.
        let before = learned.len();
        learned.retain(|name, l| {
            everyone
                .iter()
                .any(|a| a.name == *name && a.hash == l.argon)
        });
        if learned.len() != before {
            self.write_private(&self.learned_file(), &serde_json::to_vec_pretty(&learned)?)?;
        }
        let ready: Vec<(&Account, &Learned)> = everyone
            .iter()
            .filter(|a| a.may_use())
            .filter_map(|a| learned.get(&a.name).map(|l| (a, l)))
            .collect();

        // The cloud's own user, who owns its store and every file in it.
        let store = std::fs::metadata(&self.cloud)?;
        let (uid, gid) = (store.uid(), store.gid());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut passwd = String::new();
        let mut map = String::new();
        let mut shares =
            String::from("# Written by lososd (backend/src/windows.rs) from the people file.\n");
        for (i, (account, l)) in ready.iter().enumerate() {
            let user = format!("{ACCOUNT_PREFIX}{}", account.name);
            let own = account_uid(i);
            passwd.push_str(&format!(
                "{user}:{own}:XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX:{}:[U          ]:LCT-{now:08X}:\n",
                l.nt
            ));
            map.push_str(&format!("{user} = {}\n", account.name));
            let files = self.cloud.join(account.login()).join("files");
            self.ensure_files(&files, uid, gid)?;
            shares.push_str(&format!(
                "\n[{}]\n  comment = LosOS Cloud\n  path = {}\n  valid users = +{user}\n  read only = no\n  server smb encrypt = desired\n{SHARE_COMMON}",
                Self::share_name(&account.name),
                files.display()
            ));
        }
        // smb.conf includes this file from inside [global], and what follows
        // the include would land in the last share. Going back to [global]
        // keeps the rest of the settings global, but a share takes its
        // defaults when it is read, so each share carries its own above.
        shares.push_str("\n[global]\n");
        let mut changed = false;
        changed |= self.replace(&self.dir.join("smbpasswd"), passwd.as_bytes(), 0o600)?;
        changed |= self.replace(&self.dir.join("users.map"), map.as_bytes(), 0o644)?;
        changed |= self.replace(&self.dir.join("shares.conf"), shares.as_bytes(), 0o644)?;
        changed |= self.write_userdb(&ready)?;
        if changed {
            self.reload();
        }
        Ok(())
    }

    /// The person's files folder, made for someone who never opened LosOS
    /// Cloud yet; the cloud picks a new home up as it appears.
    fn ensure_files(&self, files: &Path, uid: u32, gid: u32) -> anyhow::Result<()> {
        if files.is_dir() {
            return Ok(());
        }
        use std::os::unix::fs::DirBuilderExt;
        let home = files.parent().unwrap_or(&self.cloud);
        // Made under a dot name and moved into place once they are LosOS
        // Cloud's: the cloud watches a new folder the moment it appears
        // (cloud/watch.rs), and one still root's would be refused.
        let (stage, target) = if home.is_dir() {
            (home.join(".files-new"), files)
        } else {
            let login = home
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            (self.cloud.join(format!(".new-{login}")), home)
        };
        let _ = std::fs::remove_dir_all(&stage);
        let mut made = vec![stage.clone()];
        if target == home {
            made.push(stage.join("files"));
        }
        for dir in &made {
            std::fs::DirBuilder::new().mode(0o700).create(dir)?;
            std::os::unix::fs::chown(dir, Some(uid), Some(gid))?;
        }
        std::fs::rename(&stage, target)?;
        Ok(())
    }

    fn write_userdb(&self, ready: &[(&Account, &Learned)]) -> anyhow::Result<bool> {
        std::fs::create_dir_all(&self.userdb)?;
        let mut changed = false;
        let mut keep = Vec::new();
        for (i, (account, _)) in ready.iter().enumerate() {
            let user = format!("{ACCOUNT_PREFIX}{}", account.name);
            let id = account_uid(i);
            let record = serde_json::json!({
                "userName": user,
                "uid": id,
                "gid": id,
                "realName": account.display_name,
                "homeDirectory": "/var/empty",
                "shell": "/run/current-system/sw/bin/nologin",
                "disposition": "regular",
                "locked": true,
            });
            let file = format!("{user}.user");
            changed |= self.replace(
                &self.userdb.join(&file),
                &serde_json::to_vec(&record)?,
                0o644,
            )?;
            // Each account has a group of its own, which is what the share
            // names: Samba's share list matches a person by group, not by
            // user name.
            let group = format!("{user}.group");
            changed |= self.replace(
                &self.userdb.join(&group),
                &serde_json::to_vec(&serde_json::json!({ "groupName": user, "gid": id }))?,
                0o644,
            )?;
            // A lookup by number, which Samba makes at every sign-in, finds
            // a record only through `<uid>.user` and `<gid>.group`.
            for (target, link) in [
                (&file, format!("{id}.user")),
                (&group, format!("{id}.group")),
            ] {
                let path = self.userdb.join(&link);
                if std::fs::read_link(&path).ok().as_deref() != Some(Path::new(target)) {
                    let _ = std::fs::remove_file(&path);
                    std::os::unix::fs::symlink(target, &path)?;
                    changed = true;
                }
                keep.push(link);
            }
            keep.push(file);
            keep.push(group);
        }
        for entry in std::fs::read_dir(&self.userdb)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let ours = name.starts_with(ACCOUNT_PREFIX)
                || std::fs::read_link(entry.path())
                    .is_ok_and(|t| t.to_string_lossy().starts_with(ACCOUNT_PREFIX));
            let record = name.ends_with(".user") || name.ends_with(".group");
            if ours && record && !keep.contains(&name) {
                std::fs::remove_file(entry.path())?;
                changed = true;
            }
        }
        Ok(changed)
    }

    /// Replace `path` with `bytes` unless it already holds them.
    fn replace(&self, path: &Path, bytes: &[u8], mode: u32) -> anyhow::Result<bool> {
        if std::fs::read(path).is_ok_and(|b| b == bytes) {
            return Ok(false);
        }
        self.write_file(path, bytes, mode)?;
        Ok(true)
    }

    fn write_private(&self, path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        self.write_file(path, bytes, 0o600)
    }

    fn write_file(&self, path: &Path, bytes: &[u8], mode: u32) -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tmp = path.with_file_name(format!(".{name}.tmp"));
        {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(mode)
                .open(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Have smbd read its configuration again now rather than at its next
    /// look. Samba reads the password file on every sign-in anyway.
    fn reload(&self) {
        let Some(program) = &self.smbcontrol else {
            return;
        };
        match std::process::Command::new(program)
            // smbd and the helpers that list the shares.
            .args(["all", "reload-config"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
        {
            Ok(s) if s.success() => {}
            // Not running yet: it reads the files when it starts.
            Ok(_) => tracing::debug!("smbd did not take the reload"),
            Err(e) => tracing::warn!(error = %e, "smbcontrol could not be run"),
        }
    }
}

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// The NT hash: MD4 of the password in UTF-16LE.
pub fn nt_hash(password: &str) -> [u8; 16] {
    let bytes: Vec<u8> = password.encode_utf16().flat_map(u16::to_le_bytes).collect();
    md4(&bytes)
}

/// MD4 (RFC 1320). Here only because NTLM is built on it; nothing else on
/// the box may use it.
fn md4(input: &[u8]) -> [u8; 16] {
    let mut msg = input.to_vec();
    let bit_len = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let mut s: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let f = |x: u32, y: u32, z: u32| (x & y) | (!x & z);
    let g = |x: u32, y: u32, z: u32| (x & y) | (x & z) | (y & z);
    let h = |x: u32, y: u32, z: u32| x ^ y ^ z;
    for block in msg.as_chunks::<64>().0 {
        let mut x = [0u32; 16];
        for (i, w) in block.as_chunks::<4>().0.iter().enumerate() {
            x[i] = u32::from_le_bytes(*w);
        }
        let [mut a, mut b, mut c, mut d] = s;
        for &i in &[0usize, 4, 8, 12] {
            a = a.wrapping_add(f(b, c, d)).wrapping_add(x[i]).rotate_left(3);
            d = d
                .wrapping_add(f(a, b, c))
                .wrapping_add(x[i + 1])
                .rotate_left(7);
            c = c
                .wrapping_add(f(d, a, b))
                .wrapping_add(x[i + 2])
                .rotate_left(11);
            b = b
                .wrapping_add(f(c, d, a))
                .wrapping_add(x[i + 3])
                .rotate_left(19);
        }
        for &i in &[0usize, 1, 2, 3] {
            let k = 0x5a82_7999u32;
            a = a
                .wrapping_add(g(b, c, d))
                .wrapping_add(x[i])
                .wrapping_add(k)
                .rotate_left(3);
            d = d
                .wrapping_add(g(a, b, c))
                .wrapping_add(x[i + 4])
                .wrapping_add(k)
                .rotate_left(5);
            c = c
                .wrapping_add(g(d, a, b))
                .wrapping_add(x[i + 8])
                .wrapping_add(k)
                .rotate_left(9);
            b = b
                .wrapping_add(g(c, d, a))
                .wrapping_add(x[i + 12])
                .wrapping_add(k)
                .rotate_left(13);
        }
        for &i in &[0usize, 2, 1, 3] {
            let k = 0x6ed9_eba1u32;
            a = a
                .wrapping_add(h(b, c, d))
                .wrapping_add(x[i])
                .wrapping_add(k)
                .rotate_left(3);
            d = d
                .wrapping_add(h(a, b, c))
                .wrapping_add(x[i + 8])
                .wrapping_add(k)
                .rotate_left(9);
            c = c
                .wrapping_add(h(d, a, b))
                .wrapping_add(x[i + 4])
                .wrapping_add(k)
                .rotate_left(11);
            b = b
                .wrapping_add(h(c, d, a))
                .wrapping_add(x[i + 12])
                .wrapping_add(k)
                .rotate_left(15);
        }
        s[0] = s[0].wrapping_add(a);
        s[1] = s[1].wrapping_add(b);
        s[2] = s[2].wrapping_add(c);
        s[3] = s[3].wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (i, w) in s.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    out
}

/// What every share carries: every file as LosOS Cloud's own user, as
/// private as the cloud keeps them, and nothing reached through a link.
/// Per share, not in [global], where it would also make IPC$ (and with it
/// the share list) see every person as that user.
const SHARE_COMMON: &str = "  force user = losos-cloud
  force group = losos-cloud
  create mask = 0600
  force create mode = 0600
  directory mask = 0700
  force directory mode = 0700
  follow symlinks = no
  wide links = no
";

/// The Unix uid and gid of the `i`th ready person's account. Each has its
/// own: Samba
/// turns a file owner's uid into a Windows SID through its password file,
/// and two accounts on one uid made the cloud's files belong to whichever
/// came first, so the other was refused their own folder. Files stay the
/// cloud's (`force user`); the cloud's uid is in no Samba account at all.
/// Below systemd-homed's range (60001 and up), above every account NixOS
/// makes.
fn account_uid(i: usize) -> u32 {
    59000 + i as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn md4_matches_rfc_1320() {
        for (input, want) in [
            ("", "31d6cfe0d16ae931b73c59d7e0c089c0"),
            ("a", "bde52cb31de33e46245e05fbdbd6fb24"),
            ("abc", "a448017aaf21d8525fc10ae87aa6729d"),
            ("message digest", "d9130a8164549fe818874806e1c7014b"),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "e33b4ddc9c38f2199c3e7b164fcc0536",
            ),
        ] {
            assert_eq!(hex(&md4(input.as_bytes())), want, "{input:?}");
        }
    }

    #[test]
    fn nt_hash_is_md4_of_utf16() {
        // The value Samba's own `smbpasswd` writes for "password".
        assert_eq!(
            hex(&nt_hash("password")),
            "8846f7eaee8fb117ad06bdd830b7586c"
        );
    }

    struct Box_ {
        _dir: assert_fs::TempDir,
        w: Windows,
        root: PathBuf,
    }

    fn setup() -> Box_ {
        let dir = assert_fs::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("cloud")).unwrap();
        let w = Windows::new(
            root.join("windows"),
            root.join("userdb"),
            root.join("cloud"),
            None,
        );
        Box_ { _dir: dir, w, root }
    }

    fn account(name: &str, hash: &str, groups: &[&str]) -> Account {
        Account {
            name: name.into(),
            display_name: name.to_uppercase(),
            groups: groups.iter().map(|g| g.to_string()).collect(),
            disabled: false,
            hash: hash.into(),
        }
    }

    fn read(path: PathBuf) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn a_learned_password_opens_the_persons_cloud_files() {
        let b = setup();
        let matus = account("matus", "$argon-1", &[OWNER, "losos-admins"]);
        let eva = account("eva", "$argon-2", &[]);
        let all = [matus.clone(), eva.clone()];
        assert!(!b.w.status(&matus).ready);
        b.w.learn(&matus, &Secret::new("password"), &all);
        assert!(b.w.status(&matus).ready);
        assert!(!b.w.status(&eva).ready);

        let passwd = read(b.root.join("windows/smbpasswd"));
        assert!(
            passwd.starts_with("losos-win-matus:")
                && passwd.contains(":8846F7EAEE8FB117AD06BDD830B7586C:[U          ]:"),
            "the owner's smbpasswd line"
        );
        assert_eq!(passwd.lines().count(), 1);
        assert_eq!(
            read(b.root.join("windows/users.map")),
            "losos-win-matus = matus\n"
        );
        let shares = read(b.root.join("windows/shares.conf"));
        // The owner's files are the `notshared` account's.
        let path = b.root.join("cloud/notshared/files");
        assert!(shares.contains("[matus]"), "{shares}");
        assert!(
            shares.contains(&format!("path = {}", path.display())),
            "{shares}"
        );
        assert!(
            shares.contains("valid users = +losos-win-matus"),
            "{shares}"
        );
        assert!(shares.trim_end().ends_with("[global]"), "{shares}");
        assert!(path.is_dir(), "the files folder is made for a first-timer");
        let record: serde_json::Value =
            serde_json::from_str(&read(b.root.join("userdb/losos-win-matus.user"))).unwrap();
        assert_eq!(record["userName"], "losos-win-matus");
        assert_eq!(record["locked"], true);
        let by_uid = b.root.join(format!("userdb/{}.user", record["uid"]));
        assert_eq!(
            std::fs::read_link(by_uid).unwrap(),
            Path::new("losos-win-matus.user")
        );
        // A group of its own, under the same number, for the share to name.
        let group: serde_json::Value =
            serde_json::from_str(&read(b.root.join("userdb/losos-win-matus.group"))).unwrap();
        assert_eq!(group["groupName"], "losos-win-matus");
        assert_eq!(group["gid"], record["gid"]);
        assert_eq!(record["gid"], record["uid"]);
        assert_eq!(
            std::fs::read_link(b.root.join(format!("userdb/{}.group", group["gid"]))).unwrap(),
            Path::new("losos-win-matus.group")
        );

        // Everyone else's share is under their own login.
        b.w.learn(&eva, &Secret::new("another"), &all);
        assert!(read(b.root.join("windows/shares.conf")).contains(&format!(
            "path = {}",
            b.root.join("cloud/eva/files").display()
        )));
        let passwd = read(b.root.join("windows/smbpasswd"));
        assert_eq!(passwd.lines().count(), 2);
        // One uid each, and neither is the cloud's, which owns the files.
        let cloud_uid = std::fs::metadata(b.root.join("cloud")).unwrap().uid();
        let uids: Vec<u32> = passwd
            .lines()
            .map(|l| l.split(':').nth(1).unwrap().parse().unwrap())
            .collect();
        assert!(uids[0] != uids[1] && !uids.contains(&cloud_uid));
    }

    #[test]
    fn a_changed_password_a_lock_or_a_removal_closes_the_share() {
        let b = setup();
        let eva = account("eva", "$argon-2", &[]);
        b.w.learn(&eva, &Secret::new("another"), std::slice::from_ref(&eva));
        assert!(b.root.join("userdb/losos-win-eva.user").exists());

        // Locked by a rule of LosOS Login: out, but the hash is kept for
        // when the lock lifts.
        let locked = account("eva", "$argon-2", &[LOCKED]);
        b.w.sync(std::slice::from_ref(&locked));
        assert!(read(b.root.join("windows/smbpasswd")).is_empty());
        assert!(!b.root.join("userdb/losos-win-eva.user").exists());
        b.w.sync(std::slice::from_ref(&eva));
        assert!(b.w.status(&eva).ready);

        // A new password set somewhere this daemon did not see it in the
        // clear: the old one stops working on Windows too.
        let changed = account("eva", "$argon-3", &[]);
        b.w.sync(std::slice::from_ref(&changed));
        assert!(!b.w.status(&changed).ready);
        assert!(read(b.root.join("windows/smbpasswd")).is_empty());

        b.w.learn(
            &changed,
            &Secret::new("third"),
            std::slice::from_ref(&changed),
        );
        assert!(b.w.status(&changed).ready);
        b.w.sync(&[]);
        assert!(!read(b.root.join("windows/learned.json")).contains("eva"));
        assert!(!b.root.join("userdb/losos-win-eva.user").exists());
        // Its group and both by-number links went with it.
        assert_eq!(std::fs::read_dir(b.root.join("userdb")).unwrap().count(), 0);
    }

    #[test]
    fn names_smb_conf_keeps_for_itself_get_a_suffix() {
        assert_eq!(Windows::share_name("homes"), "homes-files");
        assert_eq!(Windows::share_name("global"), "global-files");
        assert_eq!(Windows::share_name("eva"), "eva");
    }
}
