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
    Ok(name)
}

/// argon2id with Authelia's defaults (`m=65536,t=3,p=4`, 32-byte key).
pub fn hash_password(secret: &Secret) -> anyhow::Result<String> {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    let params = argon2::Params::new(65_536, 3, 4, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 parameters: {e}"))?;
    let hasher = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let salt = SaltString::generate(&mut OsRng);
    Ok(hasher
        .hash_password(secret.expose().as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hashing a password: {e}"))?
        .to_string())
}

/// Whether `secret` is the password behind `hash`.
pub fn verify_password(hash: &str, secret: &Secret) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
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
    lock: Mutex<()>,
}

impl People {
    fn sanitize_people_path(path: PathBuf) -> PathBuf {
        let default = PathBuf::from(DEFAULT_FILE);
        let allowed_base = default
            .parent()
            .unwrap_or(Path::new("/var/lib/losos-identity"));

        let has_parent = path
            .components()
            .any(|c| matches!(c, Component::ParentDir));
        let is_absolute = path.is_absolute();
        let in_allowed_base = path.starts_with(allowed_base);

            .map(Self::sanitize_people_path)
        if is_absolute && !has_parent && in_allowed_base {
            path
        } else {
            tracing::warn!(
                path = %path.display(),
                default = DEFAULT_FILE,
                "invalid LOSOS_PEOPLE_FILE path; falling back to default"
            );
            default
    fn sanitize_people_path(input: PathBuf) -> PathBuf {
        let base = PathBuf::from(PEOPLE_DIR);
        let candidate = if input.is_absolute() { input } else { base.join(input) };

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

        }
    }

    pub fn new(path: impl Into<PathBuf>, group: Option<String>) -> People {
        People {
            path: Self::sanitize_people_path(path.into()),
            headers: false,
            group,
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
        let mut people = People::new(path, group);
        people.headers = std::env::var("LOSOS_IDENTITY").is_ok_and(|v| v == "authelia");
        people
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
        Ok(())
    }

    /// Whether anyone can sign in as an administrator yet.
    pub fn has_admin(&self) -> anyhow::Result<bool> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self
            .read()?
            .users
            .values()
            .any(|r| !r.disabled && r.groups.iter().any(|g| g == ADMINS)))
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
            anyhow::bail!("{name} already has an account");
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
        self.write(&doc)
    }

    /// Replace `name`'s password.
    pub fn set_password(&self, name: &str, secret: &Secret) -> anyhow::Result<()> {
        let hash = hash_password(secret)?;
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let record = doc
            .users
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("{name} has no account"))?;
        record.password = hash;
        self.write(&doc)
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
    fn names_are_lower_case_and_start_with_a_letter() {
        assert!(validate_name("matus").is_ok());
        assert!(validate_name("a.b-c_9").is_ok());
        for bad in ["", "Matus", "9lives", "two words", "x/y", &"a".repeat(33)] {
            assert!(validate_name(bad).is_err(), "{bad:?}");
        }
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
}
