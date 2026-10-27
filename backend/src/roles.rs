//! Roles: names for bundles of the groups `backend/sections.json` already
//! reads. Nothing here decides access on its own; a role is the set of
//! groups it puts a person in, and [`crate::sections`] goes on reading
//! the groups.
//!
//! - owner: `losos-owner` and `losos-admins`, given by the claim, never
//!   handed out.
//! - admin: `losos-admins`.
//! - moderator: `losos-moderation`, the moderation section's group, beside
//!   whatever other parts they have.
//! - member: anything else; the parts they have are their own.
//! - guest: `losos-guests` and their own account, nothing more; a guest
//!   reads what anyone signed in reads and changes only what the table's
//!   `guests` list names.
//! - a custom role: `losos-role-<id>` and the sections the administrator
//!   saved under that name (`/var/lib/losos/roles.json`). Change the saved
//!   sections and everyone with the role moves with them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::people::{ADMINS, OWNER};

/// Guests: their own account, and no part of the box to change.
pub const GUESTS: &str = "losos-guests";
/// The moderation section's group (`sections.json`'s `moderation`).
pub const MODERATORS: &str = "losos-moderation";
/// What a custom role's mark starts with; the role's id follows.
pub const CUSTOM: &str = "losos-role-";
/// Where custom roles are kept.
const DEFAULT_FILE: &str = "/var/lib/losos/roles.json";
/// At most this many custom roles, so the role picker stays a picker.
pub const MAX_CUSTOM: usize = 16;

/// What a person is on this box, in rank order: a guest lowest, the owner
/// highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Guest,
    Member,
    Moderator,
    Admin,
    Owner,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Guest => "guest",
            Role::Member => "member",
            Role::Moderator => "moderator",
            Role::Admin => "admin",
            Role::Owner => "owner",
        }
    }
}

/// A person's role, and the custom role they hold, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub role: Role,
    pub custom: Option<String>,
}

/// The role `groups` add up to.
pub fn role_of(groups: &[String]) -> Standing {
    let has = |g: &str| groups.iter().any(|x| x == g);
    let custom = groups
        .iter()
        .find_map(|g| g.strip_prefix(CUSTOM))
        .map(str::to_string);
    let role = if has(OWNER) {
        Role::Owner
    } else if has(ADMINS) {
        Role::Admin
    } else if has(GUESTS) {
        Role::Guest
    } else if has(MODERATORS) {
        Role::Moderator
    } else {
        Role::Member
    };
    let custom = custom.filter(|_| matches!(role, Role::Member | Role::Moderator));
    Standing { role, custom }
}

/// Whether someone of `actor`'s role may mute, suspend or hide the words
/// of someone of `target`'s: an administrator anyone but the owner, a
/// moderator members and guests. Nobody acts on themselves; the caller
/// checks that by name.
pub fn may_moderate(actor: Role, target: Role) -> bool {
    match actor {
        Role::Owner | Role::Admin => target != Role::Owner,
        Role::Moderator => target <= Role::Member,
        Role::Member | Role::Guest => false,
    }
}

/// One custom role as the administrator saved it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    /// Section names, without the `losos-` prefix.
    pub sections: Vec<String>,
}

/// The groups a custom role puts a person in.
pub fn preset_groups(id: &str, preset: &Preset) -> Vec<String> {
    let mut out = vec![format!("{CUSTOM}{id}")];
    out.extend(preset.sections.iter().map(|s| format!("losos-{s}")));
    out
}

/// The groups to write for a request's `groups`: a role's bundle replaces
/// whatever else came with it, so the people file never holds an
/// administrator who is also a guest. `Err` names a custom role nobody
/// saved.
pub fn normalize(
    groups: &[String],
    presets: &BTreeMap<String, Preset>,
) -> Result<Vec<String>, String> {
    if groups.iter().any(|g| g == ADMINS) {
        return Ok(vec![ADMINS.to_string()]);
    }
    if groups.iter().any(|g| g == GUESTS) {
        return Ok(vec![GUESTS.to_string(), "losos-account".to_string()]);
    }
    if let Some(id) = groups.iter().find_map(|g| g.strip_prefix(CUSTOM)) {
        let preset = presets
            .get(id)
            .ok_or_else(|| format!("there is no role called {id} on this box"))?;
        return Ok(preset_groups(id, preset));
    }
    Ok(groups
        .iter()
        .filter(|g| g.as_str() != OWNER)
        .cloned()
        .collect())
}

/// The id a role's name gets: lower-case letters, digits and `-`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 24 {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Document {
    #[serde(default)]
    roles: BTreeMap<String, Preset>,
}

/// The custom roles. One lock around every read-modify-write.
pub struct Presets {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Presets {
    pub fn new(path: impl Into<PathBuf>) -> Presets {
        Presets {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    pub fn from_env() -> Presets {
        Presets::new(
            std::env::var_os("LOSOS_ROLES_FILE")
                .filter(|v| !v.is_empty())
                .map_or_else(|| PathBuf::from(DEFAULT_FILE), PathBuf::from),
        )
    }

    fn read(&self) -> anyhow::Result<Document> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("reading {}: {e}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Document::default()),
            Err(e) => Err(anyhow::anyhow!("reading {}: {e}", self.path.display())),
        }
    }

    fn write(&self, doc: &Document) -> anyhow::Result<()> {
        crate::moderation::write_atomic(&self.path, &serde_json::to_vec_pretty(doc)?)
    }

    /// Every custom role, by id.
    pub fn all(&self) -> BTreeMap<String, Preset> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.read().map(|d| d.roles).unwrap_or_default()
    }

    /// Save a role under `id`, or under a new id made from its name.
    /// Returns the id.
    pub fn put(&self, id: Option<&str>, name: &str, sections: &[String]) -> anyhow::Result<String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
            anyhow::bail!(crate::people::BadDetails(
                "a role's name is one line of 1 to 40 characters".into()
            ));
        }
        let known = crate::sections::names();
        let mut parts = Vec::new();
        for s in sections {
            if !known.contains(&s.as_str()) {
                anyhow::bail!(crate::people::BadDetails(format!(
                    "{s} is not a part of the box"
                )));
            }
            if !parts.contains(s) {
                parts.push(s.clone());
            }
        }
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        let id = match id {
            Some(id) if doc.roles.contains_key(id) => id.to_string(),
            Some(id) => anyhow::bail!(crate::people::Refused::Unknown(id.to_string())),
            None => {
                if doc.roles.len() >= MAX_CUSTOM {
                    anyhow::bail!(crate::people::BadDetails(format!(
                        "a box keeps at most {MAX_CUSTOM} roles of its own"
                    )));
                }
                let base = match slug(name) {
                    s if s.is_empty() || !s.starts_with(|c: char| c.is_ascii_lowercase()) => {
                        format!("role-{s}").trim_end_matches('-').to_string()
                    }
                    s => s,
                };
                let mut id = base.clone();
                let mut n = 2;
                while doc.roles.contains_key(&id) {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                id
            }
        };
        if doc
            .roles
            .iter()
            .any(|(other, p)| *other != id && p.name.eq_ignore_ascii_case(name))
        {
            anyhow::bail!(crate::people::Refused::Taken(name.to_string()));
        }
        doc.roles.insert(
            id.clone(),
            Preset {
                name: name.to_string(),
                sections: parts,
            },
        );
        self.write(&doc)?;
        Ok(id)
    }

    /// Forget a custom role.
    pub fn remove(&self, id: &str) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut doc = self.read()?;
        if doc.roles.remove(id).is_none() {
            anyhow::bail!(crate::people::Refused::Unknown(id.to_string()));
        }
        self.write(&doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_role_is_read_off_the_groups() {
        assert_eq!(role_of(&g(&[OWNER, ADMINS])).role, Role::Owner);
        assert_eq!(role_of(&g(&[ADMINS])).role, Role::Admin);
        assert_eq!(
            role_of(&g(&[MODERATORS, "losos-backup"])).role,
            Role::Moderator
        );
        assert_eq!(role_of(&g(&["losos-backup"])).role, Role::Member);
        assert_eq!(role_of(&g(&[])).role, Role::Member);
        assert_eq!(role_of(&g(&[GUESTS, "losos-account"])).role, Role::Guest);
        let custom = role_of(&g(&["losos-role-helpers", "losos-backup"]));
        assert_eq!(custom.role, Role::Member);
        assert_eq!(custom.custom.as_deref(), Some("helpers"));
        // A custom role with the moderation section moderates.
        let mods = role_of(&g(&["losos-role-desk", MODERATORS]));
        assert_eq!(mods.role, Role::Moderator);
        assert_eq!(mods.custom.as_deref(), Some("desk"));
    }

    #[test]
    fn moderators_act_on_members_and_guests_only() {
        assert!(may_moderate(Role::Moderator, Role::Member));
        assert!(may_moderate(Role::Moderator, Role::Guest));
        assert!(!may_moderate(Role::Moderator, Role::Moderator));
        assert!(!may_moderate(Role::Moderator, Role::Admin));
        assert!(may_moderate(Role::Admin, Role::Admin));
        assert!(!may_moderate(Role::Admin, Role::Owner));
        assert!(!may_moderate(Role::Member, Role::Guest));
    }

    #[test]
    fn a_role_bundle_replaces_what_came_with_it() {
        let none = BTreeMap::new();
        assert_eq!(
            normalize(&g(&[ADMINS, GUESTS]), &none).unwrap(),
            g(&[ADMINS])
        );
        assert_eq!(
            normalize(&g(&[GUESTS, "losos-backup"]), &none).unwrap(),
            g(&[GUESTS, "losos-account"])
        );
        assert_eq!(
            normalize(&g(&[MODERATORS, "losos-account"]), &none).unwrap(),
            g(&[MODERATORS, "losos-account"])
        );
        assert!(normalize(&g(&["losos-role-ghost"]), &none).is_err());
        let mut presets = BTreeMap::new();
        presets.insert(
            "helpers".to_string(),
            Preset {
                name: "Helpers".into(),
                sections: g(&["backup", "account"]),
            },
        );
        assert_eq!(
            normalize(&g(&["losos-role-helpers", "losos-storage"]), &presets).unwrap(),
            g(&["losos-role-helpers", "losos-backup", "losos-account"])
        );
    }

    #[test]
    fn custom_roles_are_saved_renamed_and_forgotten() {
        let dir = assert_fs::TempDir::new().unwrap();
        let presets = Presets::new(dir.path().join("roles.json"));
        let id = presets
            .put(None, "Backup helpers", &g(&["backup", "account", "backup"]))
            .unwrap();
        assert_eq!(id, "backup-helpers");
        assert_eq!(presets.all()[&id].sections, g(&["backup", "account"]));
        // The same name twice is refused, whatever its case.
        assert!(presets.put(None, "backup HELPERS", &[]).is_err());
        // A name with no letters still gets an id that is a group's tail.
        let odd = presets.put(None, "42", &[]).unwrap();
        assert_eq!(odd, "role-42");
        assert!(presets.put(None, "Mystery", &g(&["vault"])).is_err());
        presets
            .put(Some(&id), "Backup desk", &g(&["backup"]))
            .unwrap();
        assert_eq!(presets.all()[&id].name, "Backup desk");
        presets.remove(&id).unwrap();
        assert!(!presets.all().contains_key(&id));
        assert!(presets.remove(&id).is_err());
    }
}
