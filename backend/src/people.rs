//! The people who may sign in to this box, and the groups they are in.
//!
//! Authelia (`modules/identity.nix`) signs people in against its file
//! backend, and this daemon is that file's only writer: it lives at
//! `$LOSOS_PEOPLE_FILE` (`/var/lib/losos-identity/users.yml`), owned by
//! root and readable by Authelia's group. Authelia watches the directory and
//! reloads the file when it changes, so a write here takes effect without a
//! restart. The file is JSON, which is YAML too, so neither side needs a
//! YAML writer.
//!
//! Passwords are argon2id hashes in PHC form with Authelia's own default
//! parameters, so a hash made here is one Authelia checks and the other way
//! round. The plaintext never reaches a file or an argv.
//!
//! Groups decide what a person may do in the admin pages
//! (`crate::sections`). Authelia hands the groups to nginx too, but lososd
//! reads them from this file on every call, so a person taken out of a group
//! loses the section at once rather than at their next sign-in.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::setup::Secret;

/// Where `modules/identity.nix` points Authelia.
const DEFAULT_FILE: &str = "/var/lib/losos-identity/users.yml";
/// Directory that may contain the people file.
const PEOPLE_DIR: &str = "/var/lib/losos-identity";
/// The group every first person lands in.
pub const ADMINS: &str = "losos-admins";
/// The person who claimed the box. nginx signs them in to LosOS Cloud and
/// LosOS Git as the account that holds their files and repositories
/// (`modules/app-sign-in.nix`); everyone else arrives under their own name.
pub const OWNER: &str = "losos-owner";
/// The group that may read every section and change none
/// (`backend/sections.json`'s `viewers`).
pub const VIEWERS: &str = "losos-viewers";
/// The person has an authenticator app or a passkey, so LosOS Login asks
/// for it at every sign-in (`crate::login`, `modules/identity.nix`).
pub const TWO_FACTOR: &str = "losos-two-factor";
/// A rule of LosOS Login holds the person to their own account
/// (`crate::login`): LosOS Login keeps them out of LosOS Cloud and LosOS
/// Git, and this daemon out of everything but their account.
pub const LOCKED: &str = "losos-locked";
/// The marks this daemon keeps in step itself. Nobody hands them out, and a
/// change of access keeps them.
pub const MARKS: [&str; 2] = [TWO_FACTOR, LOCKED];
/// Names already taken by the box's own accounts in LosOS Cloud and LosOS
/// Git: the owner's, the two sharing domains' and the bot's, and LosOS Git's
/// account in chat (`crate::chat::GIT_BOT`). A person signing in under
/// one of them would arrive as that account.
const RESERVED: [&str; 5] = [
    "notshared",
    "shared",
    "kindashared",
    "losos",
    crate::chat::GIT_BOT,
];
/// Sign-in names: what Authelia and the audit log show. Lower case so two
/// people cannot differ by case alone.
const MAX_NAME_CHARS: usize = 32;

/// One person as Authelia's file backend stores them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    #[serde(default)]
    pub disabled: bool,
    pub displayname: String,
    pub password: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub groups: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Document {
    #[serde(default)]
    users: BTreeMap<String, Record>,
}

/// A change to the people file that cannot be made, in words the Users pane
/// shows as they are.
#[derive(Debug, Clone, PartialEq)]
pub enum Refused {
    /// Nobody by that name.
    Unknown(String),
    /// The name is taken.
    Taken(String),
    /// The person who claimed the box keeps their account and their access:
    /// their files in LosOS Cloud and LosOS Git hang off it.
    Owner(String),
    /// A person cannot remove themselves or change their own access, so
    /// nobody locks themselves out by a slip.
    Yourself,
    /// The change would leave nobody who can sign in as an administrator.
    LastAdmin,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Unknown(n) => write!(f, "{n} has no account"),
            Refused::Taken(n) => write!(f, "{n} already has an account"),
            Refused::Owner(n) => write!(
                f,
                "{n} owns this box; their account and access stay as they are"
            ),
            Refused::Yourself => {
                f.write_str("you cannot remove yourself or change your own access")
            }
            Refused::LastAdmin => f.write_str("the box needs at least one administrator"),
        }
    }
}

impl std::error::Error for Refused {}

/// A name or email address a person cannot give their account, in words
/// the Account pane shows as they are (a 400).
#[derive(Debug, Clone, PartialEq)]
pub struct BadDetails(pub String);

impl std::fmt::Display for BadDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BadDetails {}

impl Refused {
    /// 404 for nobody by that name, 409 for everything else.
    pub fn status(&self) -> u16 {
        match self {
            Refused::Unknown(_) => 404,
            _ => 409,
        }
    }
}

/// One person as the Users pane lists them: never the password hash.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub name: String,
    pub display_name: String,
    pub email: String,
    pub groups: Vec<String>,
    pub disabled: bool,
}

/// Everyone in `doc`, as the Windows shares need them.
fn accounts(doc: &Document) -> Vec<crate::windows::Account> {
    doc.users
        .iter()
        .map(|(name, r)| crate::windows::Account {
            name: name.clone(),
            display_name: r.displayname.clone(),
            groups: r.groups.clone(),
            disabled: r.disabled,
            hash: r.password.clone(),
        })
        .collect()
}

/// Whether anyone enabled in `doc` is an administrator.
fn any_admin(doc: &Document) -> bool {
    doc.users
        .values()
        .any(|r| !r.disabled && r.groups.iter().any(|g| g == ADMINS))
}

/// A signed-in person, as the rest of the daemon sees them.
#[derive(Debug, Clone, PartialEq)]
pub struct Person {
    pub name: String,
    pub display_name: String,
    pub groups: Vec<String>,
}

impl Person {
    pub fn is_admin(&self) -> bool {
        self.groups.iter().any(|g| g == ADMINS)
    }
}

/// A name Authelia and this daemon will both accept: 1 to 32 characters,
/// lower-case letters, digits, `.`, `-` and `_`, starting with a letter.
pub fn validate_name(name: &str) -> Result<&str, String> {
    let ok_start = name.chars().next().is_some_and(|c| c.is_ascii_lowercase());
    let ok_rest = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'));
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || !ok_start || !ok_rest {
        return Err(format!(
            "a sign-in name is 1 to {MAX_NAME_CHARS} characters: lower-case letters, digits, \
             '.', '-' and '_', starting with a letter"
        ));
    }
    if RESERVED.contains(&name) {
        return Err(format!(
            "{name} is the name of one of the box's own accounts"
        ));
    }
    Ok(name)
}

/// argon2id with Authelia's defaults (`m=65536,t=3,p=4`, 32-byte key).
pub fn hash_password(secret: &Secret) -> anyhow::Result<String> {
    use argon2::password_hash::{phc::PasswordHash, PasswordHasher};
    let params = argon2::Params::new(65_536, 3, 4, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 parameters: {e}"))?;
    let hasher = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    // The salt is 16 bytes from the OS, drawn by `hash_password` itself.
    let hash: PasswordHash = hasher
        .hash_password(secret.expose().as_bytes())
        .map_err(|e| anyhow::anyhow!("hashing a password: {e}"))?;
    Ok(hash.to_string())
}

/// Whether `secret` is the password behind `hash`.
pub fn verify_password(hash: &str, secret: &Secret) -> bool {
    use argon2::password_hash::{phc::PasswordHash, PasswordVerifier};
    PasswordHash::new(hash).is_ok_and(|parsed| {
        argon2::Argon2::default()
            .verify_password(secret.expose().as_bytes(), &parsed)
            .is_ok()
    })
}

/// The people file. One lock around every read-modify-write.
pub struct People {
    path: PathBuf,
    /// Whether nginx asks Authelia and passes `Remote-User` on
    /// (`$LOSOS_IDENTITY`, set by `modules/identity.nix`). Without it the
    /// header is ignored, whoever sends it.
    pub headers: bool,
    /// The group Authelia runs as, given read access on each write.
    group: Option<String>,
    /// The Windows shares, while they are on: told of every write and of
    /// every password seen in the clear (`crate::windows`).
    windows: Option<crate::windows::Windows>,
    lock: Mutex<()>,
}

impl People {
    /// [`PEOPLE_DIR`], always, in a release build. A debug build, which is
    /// what `cargo test` runs as the daemon, also takes `LOSOS_PEOPLE_DIR`,
    /// so a test can keep its people file in a temporary directory.
    fn people_dir() -> PathBuf {
        #[cfg(debug_assertions)]
        if let Some(dir) = std::env::var_os("LOSOS_PEOPLE_DIR").filter(|d| !d.is_empty()) {
            return PathBuf::from(dir);
        }
        PathBuf::from(PEOPLE_DIR)
    }

    fn sanitize_people_path(input: PathBuf) -> PathBuf {
        let base = Self::people_dir();
        let candidate = if input.is_absolute() {
            input
        } else {
            base.join(input)
        };

        if candidate
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            return PathBuf::from(DEFAULT_FILE);
        }

        let Some(file_name) = candidate.file_name() else {
            return PathBuf::from(DEFAULT_FILE);
        };
        let Some(parent) = candidate.parent() else {
            return PathBuf::from(DEFAULT_FILE);
        };
        let Ok(parent_real) = std::fs::canonicalize(parent) else {
            return PathBuf::from(DEFAULT_FILE);
        };
        let Ok(base_real) = std::fs::canonicalize(&base) else {
            return PathBuf::from(DEFAULT_FILE);
        };
        let resolved = parent_real.join(file_name);
        if resolved.starts_with(&base_real) {
            resolved
        } else {
            PathBuf::from(DEFAULT_FILE)
        }
    }

    pub fn new(path: impl Into<PathBuf>, group: Option<String>) -> People {
        People {
            path: path.into(),
            headers: false,
            group,
            windows: None,
            lock: Mutex::new(()),
        }
    }

    pub fn from_env() -> People {
        let path = std::env::var_os("LOSOS_PEOPLE_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_FILE));
        let group = std::env::var("LOSOS_PEOPLE_GROUP")
            .ok()
            .filter(|g| !g.is_empty());
        let mut people = People::new(Self::sanitize_people_path(path), group);
        people.headers = std::env::var("LOSOS_IDENTITY").is_ok_and(|v| v == "authelia");
        people.windows = crate::windows::Windows::from_env();
        people
    }

    /// The same file, with the Windows shares kept by `windows`.
    pub fn with_windows(mut self, windows: crate::windows::Windows) -> People {
        self.windows = Some(windows);
        self
    }

    /// Whether the Windows shares are on.
    pub fn windows_on(&self) -> bool {
        self.windows.is_some()
    }

    /// Where `name` stands with Windows, while the shares are on.
    pub fn windows_status(&self, name: &str) -> anyhow::Result<Option<crate::windows::Status>> {
        let Some(windows) = &self.windows else {
            return Ok(None);
        };
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let doc = self.read()?;
        Ok(accounts(&doc)
            .into_iter()
            .find(|a| a.name == name)
            .map(|a| windows.status(&a)))
    }

    /// Write what Samba reads from the file as it is: once at start, since
    /// its accounts live on /run.
    pub fn sync_windows(&self) -> anyhow::Result<()> {
        if let Some(windows) = &self.windows {
            let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
            windows.sync(&accounts(&self.read()?));
        }
        Ok(())
    }

    /// `secret` is `name`'s password in `doc`: Windows may take it too.
    fn learn_windows(&self, doc: &Document, name: &str, secret: &Secret) {
        if let Some(windows) = &self.windows {
            let all = accounts(doc);
            if let Some(account) = all.iter().find(|a| a.name == name) {
                windows.learn(account, secret, &all);
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> anyhow::Result<Document> {
        match std::fs::read(&self.path) {
            Ok(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Ok(Document::default()),
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Document::default()),
            Err(e) => Err(anyhow::anyhow!("reading {}: {e}", self.path.display())),
        }
    }

    fn write(&self, doc: &Document) -> anyhow::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let dir = self.path.parent().unwrap_or(Path::new("/"));
        std::fs::create_dir_all(dir)?;
        let canonical_dir = std::fs::canonicalize(dir)
            .map_err(|e| anyhow::anyhow!("resolving {}: {e}", dir.display()))?;
        let target_parent = self.path.parent().unwrap_or(Path::new("/"));
        let canonical_target_parent = std::fs::canonicalize(target_parent).map_err(|e| {
            anyhow::anyhow!("resolving target parent {}: {e}", target_parent.display())
        })?;
        if !canonical_target_parent.starts_with(&canonical_dir) {
            anyhow::bail!(
                "refusing to write people file outside base directory: {}",
                self.path.display()
            );
        }
        let tmp = dir.join(".users.yml.tmp");
        let bytes = serde_json::to_vec_pretty(doc)?;
        {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&bytes)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
        }
        if let Some(group) = &self.group {
            // chgrp through the system tool rather than a libc call: the
            // crate forbids unsafe code and the group is a name.
            let status = std::process::Command::new("chgrp")
                .arg(group)
                .arg(&tmp)
                .status();
            if !status.is_ok_and(|s| s.success()) {
                tracing::warn!(group, "could not hand the people file to Authelia's group");
            }
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o640))?;
        }
        std::fs::rename(&tmp, &self.path)?;
        if let Some(windows) = &self.windows {
            windows.sync(&accounts(doc));
        }
        Ok(())
    }

    /// Whether anyone can sign in as an administrator yet.
    pub fn has_admin(&self) -> anyhow::Result<bool> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(any_admin(&self.read()?))
    }

    /// Everyone in the file, by name.
    pub fn list(&self) -> anyhow::Result<Vec<Listed>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self
            .read()?
            .users
            .into_iter()
            .map(|(name, r)| Listed {
                name,
                display_name: r.displayname,
                email: r.email,
                groups: r.groups,
                disabled: r.disabled,
            })
            .collect())
    }

    /// Take `name` out of the file. `by` is the person asking, if a person
    /// is: nobody removes themselves, the owner, or the last administrator.
    /// Their accounts in LosOS Cloud and LosOS Git, and the files in them,
    /// stay where they are.
    pub fn remove(&self, name: &str, by: Option<&str>) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let Some(record) = doc.users.get(name) else {
            return Err(Refused::Unknown(name.to_string()).into());
        };
        if by == Some(name) {
            return Err(Refused::Yourself.into());
        }
        if record.groups.iter().any(|g| g == OWNER) {
            return Err(Refused::Owner(name.to_string()).into());
        }
        doc.users.remove(name);
        if !any_admin(&doc) {
            return Err(Refused::LastAdmin.into());
        }
        self.write(&doc)
    }

    /// Give `name` the access groups in `groups` in place of the ones they
    /// have. Groups that are not access (the owner's mark, anything written
    /// by hand without the `losos-` prefix) stay. Same refusals as
    /// [`People::remove`].
    pub fn set_access(
        &self,
        name: &str,
        groups: &[String],
        by: Option<&str>,
    ) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let Some(record) = doc.users.get_mut(name) else {
            return Err(Refused::Unknown(name.to_string()).into());
        };
        if by == Some(name) {
            return Err(Refused::Yourself.into());
        }
        if record.groups.iter().any(|g| g == OWNER) {
            return Err(Refused::Owner(name.to_string()).into());
        }
        let mut next: Vec<String> = record
            .groups
            .iter()
            .filter(|g| *g == OWNER || MARKS.contains(&g.as_str()) || !g.starts_with("losos-"))
            .cloned()
            .collect();
        for g in groups {
            if !next.contains(g) {
                next.push(g.clone());
            }
        }
        record.groups = next;
        if !any_admin(&doc) {
            return Err(Refused::LastAdmin.into());
        }
        self.write(&doc)
    }

    /// Disable or enable `name`'s account: LosOS Login, which reads the
    /// file on every request, stops letting them in at once. Same refusals
    /// as [`People::remove`] when `by` names who asked.
    pub fn set_disabled(&self, name: &str, disabled: bool, by: Option<&str>) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let Some(record) = doc.users.get_mut(name) else {
            return Err(Refused::Unknown(name.to_string()).into());
        };
        if by == Some(name) {
            return Err(Refused::Yourself.into());
        }
        if record.groups.iter().any(|g| g == OWNER) {
            return Err(Refused::Owner(name.to_string()).into());
        }
        if record.disabled == disabled {
            return Ok(());
        }
        record.disabled = disabled;
        if !any_admin(&doc) {
            return Err(Refused::LastAdmin.into());
        }
        self.write(&doc)
    }

    /// The person signed in as `name`, unless they are unknown or disabled.
    pub fn get(&self, name: &str) -> anyhow::Result<Option<Person>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self
            .read()?
            .users
            .get(name)
            .filter(|r| !r.disabled)
            .map(|r| Person {
                name: name.to_string(),
                display_name: r.displayname.clone(),
                groups: r.groups.clone(),
            }))
    }

    /// [`People::get`], with the stored hash beside the person, for a
    /// password check outside the lock (`crate::cloud_share`).
    pub fn with_hash(&self, name: &str) -> anyhow::Result<Option<(Person, String)>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self
            .read()?
            .users
            .get(name)
            .filter(|r| !r.disabled)
            .map(|r| {
                (
                    Person {
                        name: name.to_string(),
                        display_name: r.displayname.clone(),
                        groups: r.groups.clone(),
                    },
                    r.password.clone(),
                )
            }))
    }

    /// Add `name` with `groups`. Refuses a name already taken.
    pub fn add(
        &self,
        name: &str,
        display_name: &str,
        email: &str,
        secret: &Secret,
        groups: &[&str],
    ) -> anyhow::Result<()> {
        let name = validate_name(name).map_err(|e| anyhow::anyhow!(e))?;
        let hash = hash_password(secret)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        if doc.users.contains_key(name) {
            return Err(Refused::Taken(name.to_string()).into());
        }
        doc.users.insert(
            name.to_string(),
            Record {
                disabled: false,
                displayname: if display_name.trim().is_empty() {
                    name.to_string()
                } else {
                    display_name.trim().to_string()
                },
                password: hash,
                email: email.to_string(),
                groups: groups.iter().map(ToString::to_string).collect(),
            },
        );
        self.write(&doc)?;
        self.learn_windows(&doc, name, secret);
        Ok(())
    }

    /// Put `name` in `group` if they are not in it yet. Writes nothing when
    /// they are, so calling it on every start costs a read.
    pub fn ensure_group(&self, name: &str, group: &str) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let Some(record) = doc.users.get_mut(name) else {
            anyhow::bail!("{name} has no account");
        };
        if record.groups.iter().any(|g| g == group) {
            return Ok(());
        }
        record.groups.push(group.to_string());
        self.write(&doc)
    }

    /// Put each person in exactly the marks `marks` names for them (none
    /// for a person it leaves out). Writes only when something changes, and
    /// says whether it did.
    pub fn set_marks(&self, marks: &BTreeMap<String, Vec<&'static str>>) -> anyhow::Result<bool> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let mut changed = false;
        for (name, record) in &mut doc.users {
            let want = marks.get(name).map(Vec::as_slice).unwrap_or(&[]);
            let mut next: Vec<String> = record
                .groups
                .iter()
                .filter(|g| !MARKS.contains(&g.as_str()))
                .cloned()
                .collect();
            next.extend(want.iter().map(ToString::to_string));
            if next != record.groups {
                record.groups = next;
                changed = true;
            }
        }
        if changed {
            self.write(&doc)?;
        }
        Ok(changed)
    }

    /// A person's own name and email address, from their account.
    pub fn set_details(&self, name: &str, display_name: &str, email: &str) -> anyhow::Result<()> {
        let display_name = display_name.trim();
        let email = email.trim();
        if display_name.is_empty() || display_name.chars().count() > 100 {
            anyhow::bail!(BadDetails("a name is 1 to 100 characters".into()));
        }
        if display_name.chars().any(char::is_control) {
            anyhow::bail!(BadDetails("a name is one line of text".into()));
        }
        if !email.contains('@') || email.contains(char::is_whitespace) || email.len() > 254 {
            anyhow::bail!(BadDetails("that is not an email address".into()));
        }
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let record = doc
            .users
            .get_mut(name)
            .ok_or_else(|| Refused::Unknown(name.to_string()))?;
        record.displayname = display_name.to_string();
        record.email = email.to_string();
        self.write(&doc)
    }

    /// Whether `secret` is `name`'s password.
    pub fn check_password(&self, name: &str, secret: &Secret) -> anyhow::Result<bool> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let doc = self.read()?;
        let ok = doc
            .users
            .get(name)
            .is_some_and(|r| !r.disabled && verify_password(&r.password, secret));
        if ok {
            self.learn_windows(&doc, name, secret);
        }
        Ok(ok)
    }

    /// A password checked outside the lock against the hash
    /// [`People::with_hash`] gave (`crate::cloud_share`) turned out right.
    pub fn password_checked(&self, name: &str, secret: &Secret) {
        if self.windows.is_none() {
            return;
        }
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(doc) = self.read() {
            self.learn_windows(&doc, name, secret);
        }
    }

    /// Replace `name`'s password.
    pub fn set_password(&self, name: &str, secret: &Secret) -> anyhow::Result<()> {
        let hash = hash_password(secret)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let record = doc
            .users
            .get_mut(name)
            .ok_or_else(|| Refused::Unknown(name.to_string()))?;
        record.password = hash;
        self.write(&doc)?;
        self.learn_windows(&doc, name, secret);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn people() -> (assert_fs::TempDir, People) {
        let dir = assert_fs::TempDir::new().unwrap();
        let p = People::new(dir.path().join("users.yml"), None);
        (dir, p)
    }

    #[test]
    fn a_disabled_person_is_nobody_until_enabled() {
        let (_d, p) = people();
        let pw = Secret::new("Correct-Horse-Battery-9");
        p.add("ada", "Ada", "", &pw, &[ADMINS, OWNER]).unwrap();
        p.add("bob", "Bob", "", &pw, &[ADMINS]).unwrap();
        p.add("eva", "Eva", "", &pw, &["losos-account"]).unwrap();
        p.set_disabled("eva", true, Some("bob")).unwrap();
        assert!(p.get("eva").unwrap().is_none());
        assert!(!p.check_password("eva", &pw).unwrap());
        p.set_disabled("eva", false, None).unwrap();
        assert!(p.get("eva").unwrap().is_some());
        let refused = |r: anyhow::Result<()>| r.unwrap_err().downcast::<Refused>().unwrap();
        assert_eq!(
            refused(p.set_disabled("ada", true, Some("bob"))),
            Refused::Owner("ada".into())
        );
        assert_eq!(
            refused(p.set_disabled("bob", true, Some("bob"))),
            Refused::Yourself
        );
        assert_eq!(
            refused(p.set_disabled("zed", true, None)),
            Refused::Unknown("zed".into())
        );
    }

    #[test]
    fn names_are_lower_case_and_start_with_a_letter() {
        assert!(validate_name("matus").is_ok());
        assert!(validate_name("a.b-c_9").is_ok());
        for bad in ["", "Matus", "9lives", "two words", "x/y", &"a".repeat(33)] {
            assert!(validate_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_boxs_own_account_names_are_refused() {
        for taken in ["notshared", "shared", "kindashared", "losos", "losos-git"] {
            let e = validate_name(taken).unwrap_err();
            assert!(e.contains("box's own accounts"), "{taken}: {e}");
        }
        assert!(validate_name("notshared2").is_ok());
    }

    #[test]
    fn a_group_is_added_once_and_an_unknown_person_is_an_error() {
        let (_d, p) = people();
        let s = Secret::new("correct horse battery");
        p.add("matus", "", "", &s, &[ADMINS]).unwrap();
        p.ensure_group("matus", OWNER).unwrap();
        let before = std::fs::read(p.path()).unwrap();
        p.ensure_group("matus", OWNER).unwrap();
        assert_eq!(
            std::fs::read(p.path()).unwrap(),
            before,
            "a second call wrote"
        );
        assert_eq!(p.get("matus").unwrap().unwrap().groups, [ADMINS, OWNER]);
        assert!(p.ensure_group("nobody", OWNER).is_err());
    }

    #[test]
    fn a_hash_is_argon2id_with_authelias_parameters_and_checks_out() {
        let s = Secret::new("correct horse battery");
        let h = hash_password(&s).unwrap();
        assert!(h.starts_with("$argon2id$v=19$m=65536,t=3,p=4$"), "{h}");
        assert!(verify_password(&h, &s));
        assert!(!verify_password(&h, &Secret::new("wrong horse battery")));
    }

    #[test]
    fn the_first_admin_is_found_and_a_name_is_taken_once() {
        let (_d, p) = people();
        assert!(!p.has_admin().unwrap());
        let s = Secret::new("correct horse battery");
        p.add("matus", "Matus", "matus@losos.local", &s, &[ADMINS])
            .unwrap();
        assert!(p.has_admin().unwrap());
        let who = p.get("matus").unwrap().unwrap();
        assert!(who.is_admin());
        assert_eq!(who.display_name, "Matus");
        assert!(p.add("matus", "", "", &s, &[]).is_err());
        assert!(p.get("nobody").unwrap().is_none());
    }

    #[test]
    fn the_file_is_what_authelias_file_backend_reads() {
        let (_d, p) = people();
        let s = Secret::new("correct horse battery");
        p.add("eva", "", "eva@losos.local", &s, &["losos-backup"])
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(p.path()).unwrap()).unwrap();
        let eva = &v["users"]["eva"];
        assert_eq!(eva["displayname"], "eva");
        assert_eq!(eva["disabled"], false);
        assert_eq!(eva["groups"][0], "losos-backup");
        assert!(eva["password"].as_str().unwrap().starts_with("$argon2id$"));
    }

    #[test]
    fn a_new_password_replaces_the_old_one() {
        let (_d, p) = people();
        let old_password = format!(
            "test-old-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let new_password = format!(
            "test-new-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let old = Secret::new(&old_password);
        let new = Secret::new(&new_password);
        p.add("eva", "", "", &old, &[]).unwrap();
        p.set_password("eva", &new).unwrap();
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(p.path()).unwrap()).unwrap();
        let h = v["users"]["eva"]["password"].as_str().unwrap();
        assert!(verify_password(h, &new) && !verify_password(h, &old));
    }

    /// A password made up at run time, so no literal reaches a hash.
    fn a_password() -> Secret {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Secret::new(&format!("test-{nanos}"))
    }

    fn refusal(e: anyhow::Error) -> Refused {
        e.downcast::<Refused>().expect("a refusal")
    }

    #[test]
    fn people_are_listed_without_their_password() {
        let (_d, p) = people();
        let s = a_password();
        p.add("matus", "Matus", "m@x.local", &s, &[ADMINS, OWNER])
            .unwrap();
        p.add("eva", "Eva", "", &s, &[VIEWERS]).unwrap();
        let all = p.list().unwrap();
        assert_eq!(
            all.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            ["eva", "matus"]
        );
        assert_eq!(all[0].display_name, "Eva");
        let json = serde_json::to_string(&all).unwrap();
        assert!(!json.contains("argon2"), "{json}");
        assert!(json.contains("\"displayName\":\"Eva\""), "{json}");
    }

    #[test]
    fn a_removed_person_can_no_longer_sign_in() {
        let (_d, p) = people();
        let s = a_password();
        p.add("matus", "", "", &s, &[ADMINS, OWNER]).unwrap();
        p.add("eva", "", "", &s, &[VIEWERS]).unwrap();
        p.remove("eva", Some("matus")).unwrap();
        assert!(p.get("eva").unwrap().is_none());
        assert_eq!(
            refusal(p.remove("eva", Some("matus")).unwrap_err()),
            Refused::Unknown("eva".into())
        );
    }

    #[test]
    fn nobody_removes_themselves_the_owner_or_the_last_administrator() {
        let (_d, p) = people();
        let s = a_password();
        p.add("matus", "", "", &s, &[ADMINS, OWNER]).unwrap();
        p.add("eva", "", "", &s, &[ADMINS]).unwrap();
        assert_eq!(
            refusal(p.remove("eva", Some("eva")).unwrap_err()),
            Refused::Yourself
        );
        assert_eq!(
            refusal(p.remove("matus", Some("eva")).unwrap_err()),
            Refused::Owner("matus".into())
        );
        // The spare key is nobody in particular, and the owner still stays.
        assert_eq!(
            refusal(p.remove("matus", None).unwrap_err()),
            Refused::Owner("matus".into())
        );
        // A box whose only administrator is not the owner (a people file
        // written by hand) keeps them too.
        let (_d2, q) = people();
        q.add("solo", "", "", &s, &[ADMINS]).unwrap();
        q.add("lea", "", "", &s, &[]).unwrap();
        assert_eq!(
            refusal(q.remove("solo", Some("lea")).unwrap_err()),
            Refused::LastAdmin
        );
        assert!(q.get("solo").unwrap().is_some(), "a refused removal wrote");
    }

    #[test]
    fn marks_follow_the_map_survive_a_change_of_access_and_write_only_on_change() {
        let (_d, p) = people();
        let s = a_password();
        p.add("matus", "", "", &s, &[ADMINS, OWNER]).unwrap();
        p.add("eva", "", "", &s, &["losos-backup"]).unwrap();
        let mut marks = BTreeMap::new();
        marks.insert("eva".to_string(), vec![TWO_FACTOR, LOCKED]);
        assert!(p.set_marks(&marks).unwrap());
        assert!(
            !p.set_marks(&marks).unwrap(),
            "nothing changed, nothing written"
        );
        let eva = p.get("eva").unwrap().unwrap();
        assert_eq!(eva.groups, ["losos-backup", TWO_FACTOR, LOCKED]);
        p.set_access("eva", &["losos-storage".to_string()], Some("matus"))
            .unwrap();
        let eva = p.get("eva").unwrap().unwrap();
        assert_eq!(eva.groups, [TWO_FACTOR, LOCKED, "losos-storage"]);
        marks.insert("eva".to_string(), vec![TWO_FACTOR]);
        assert!(p.set_marks(&marks).unwrap());
        assert_eq!(
            p.get("eva").unwrap().unwrap().groups,
            ["losos-storage", TWO_FACTOR]
        );
        assert_eq!(p.get("matus").unwrap().unwrap().groups, [ADMINS, OWNER]);
    }

    #[test]
    fn a_person_sets_their_own_name_and_email_within_limits() {
        let (_d, p) = people();
        let s = a_password();
        p.add("eva", "", "eva@box.local", &s, &[]).unwrap();
        p.set_details("eva", " Eva Novak ", "eva@example.org")
            .unwrap();
        let eva = p.list().unwrap().remove(0);
        assert_eq!(
            (eva.display_name.as_str(), eva.email.as_str()),
            ("Eva Novak", "eva@example.org")
        );
        for (name, email) in [
            ("", "eva@example.org"),
            ("Eva\nNovak", "eva@example.org"),
            ("Eva", "not an address"),
        ] {
            let e = p.set_details("eva", name, email).unwrap_err();
            assert!(
                e.downcast_ref::<BadDetails>().is_some(),
                "{name:?} {email:?}"
            );
        }
        assert!(p.check_password("eva", &s).unwrap());
        assert!(!p
            .check_password("eva", &Secret::new("something else"))
            .unwrap());
        assert!(!p.check_password("nobody", &s).unwrap());
    }

    #[test]
    fn access_replaces_the_access_groups_and_keeps_the_rest() {
        let (_d, p) = people();
        let s = a_password();
        p.add("matus", "", "", &s, &[ADMINS, OWNER]).unwrap();
        p.add("eva", "", "", &s, &["losos-backup", "family"])
            .unwrap();
        p.set_access(
            "eva",
            &["losos-storage".into(), VIEWERS.into()],
            Some("matus"),
        )
        .unwrap();
        assert_eq!(
            p.get("eva").unwrap().unwrap().groups,
            ["family", "losos-storage", VIEWERS]
        );
        assert_eq!(
            refusal(p.set_access("matus", &[], Some("eva")).unwrap_err()),
            Refused::Owner("matus".into())
        );
        assert_eq!(
            refusal(p.set_access("eva", &[], Some("eva")).unwrap_err()),
            Refused::Yourself
        );
        assert_eq!(
            refusal(p.set_access("nobody", &[], None).unwrap_err()),
            Refused::Unknown("nobody".into())
        );
        let (_d2, q) = people();
        q.add("solo", "", "", &s, &[ADMINS]).unwrap();
        assert_eq!(
            refusal(q.set_access("solo", &[], None).unwrap_err()),
            Refused::LastAdmin
        );
    }

    #[test]
    fn a_taken_name_is_a_refusal() {
        let (_d, p) = people();
        let s = a_password();
        p.add("eva", "", "", &s, &[]).unwrap();
        assert_eq!(
            refusal(p.add("eva", "", "", &s, &[]).unwrap_err()),
            Refused::Taken("eva".into())
        );
    }
}
