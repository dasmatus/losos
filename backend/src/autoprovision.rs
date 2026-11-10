//! autoprovision — the box installer for machines the mesh netboots, and the
//! configuration sync across the local mesh.
//!
//! A variant of `losos-ctl install` (`losos-ctl provision`) serves a netbooted
//! machine: it has no stick to carry settings on, so the modified box
//! configuration comes from a *mirror* — a git repository holding the mesh's
//! shared `modules/overrides.nix` on a branch, and each provisioned box's own
//! facts under `boxes/<name>/`. [`plan_provision`] decides whether the mirror
//! yields a `Stage::StageIn`; the *plan* is data, exactly like the installer's,
//! so a revision of either side cannot silently skip a step.
//!
//!     refs/heads/provision                a record per provisioned box
//!     ├── modules/overrides.nix           ├── <name>/overrides.nix
//!     └── boxes/                          └── <name>/install-target.nix
//!
//! The same mirror keeps the boxes in step afterwards: lososd, as the one
//! writer of `/etc/nixos`, checks `provision` at its own pace and either
//! takes the mesh's text (through the same gates an Apply goes through) or
//! pushes its own ([`plan_mesh`], tested here and with the [`crate::fake`]).
//! Nothing secret is in either copy: the mirror carries only the files
//! [`crate::config_repo::BOX_FILES`] names, and no token.
//!
//! Two separate processes use this module, and they share only this pure
//! code and its constants:
//!
//!   * `losos-ctl provision` — the installer's driver (backend/src/bin/
//!     losos-ctl.rs), which fetches the mirror before any destructive work
//!     and stages what the install plan lays down ([`crate::installer`]);
//!   * the daemon, through [`crate::losos::cmd_mesh_sync`].

use serde::{Deserialize, Serialize};

/// The mirror's branch: the shared, modified box configuration every mesh
/// box provisioned from it runs, plus the `boxes/` records.
pub const BRANCH: &str = "provision";

/// The branch directory holding one record per provisioned box.
pub const BOXES_DIR: &str = "boxes";

/// The mesh's overrides on the mirror (and the box's own: [`crate::config_repo::BOX_FILES`]).
pub const OVERRIDES_PATH: &str = "modules/overrides.nix";

/// The box's machine facts on the mirror (and the file in the box's own tree).
pub const TARGET_PATH: &str = "modules/install-target.nix";

/// The mirror's URL comes from the module; the medium and the box carry it the
/// same way.
pub const MIRROR_URL_ENV: &str = "LOSOS_MESH_CONFIG_URL";

/// The box's name on the mirror (`losos.autoprovision.box`, the host name by
/// default). The medium's operator may name a netbooted machine differently
/// with `--name`.
pub const MIRROR_BOX_ENV: &str = "LOSOS_MESH_CONFIG_BOX";

/// The provision driver stages the mirror's configuration here, then the
/// install plan copies it into the flake clone. Set by `losos-ctl provision`
/// before the install half takes over, never read another way.
pub const STAGE_ENV: &str = "LOSOS_PROVISION_STAGE";

/// Where the stage defaults to. The medium has a writable `/run`.
pub const STAGE_DIR: &str = "/run/losos-provision";

/// The staged copy of the mesh's shared `modules/overrides.nix`, laid into
/// the clone by the plan.
pub const STAGE_OVERRIDES_FILE: &str = "overrides.nix";

/// Who staged what, when: written by the provision driver before the install
/// begins, so a report of a provision the install never finished can still
/// name the mirror.
pub const STAGE_INFO_FILE: &str = "box.json";

/// The record paths one provisioned box takes on the mirror, in the order a
/// human would read them. The complete set: the mirror's shared half is
/// `modules/overrides.nix`, a record is only one box's facts.
pub fn box_paths(name: &str) -> [String; 2] {
    [
        format!("{BOXES_DIR}/{name}/install-target.nix"),
        format!("{BOXES_DIR}/{name}/overrides.nix"),
    ]
}

/// The subject and body of a registration push (`boxes/<name>` fresh on the
/// mirror, from the netboot medium's own machine).
pub fn provision_register_message(name: &str) -> (String, String) {
    (
        format!("autoprovision: {name} provisioned from the netboot image"),
        format!(
            "Provisioned: {name}\nThe box's own {OVERRIDES_PATH} rides the record, as installed."
        ),
    )
}

// ── The mesh from the environment ───────────────────────────────────────────

/// Which mirror this process talks to, and under what name.
///
/// For the installer the operator gives both (`--mirror`, `--name`); for the
/// daemon the module writes the environment. `None` is "not provisioned into
/// a mesh": the installer stops before it fetches anything, and the daemon
/// commits its configuration locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mesh {
    /// The git URL of the mirror (a `git ls-remote` address).
    pub url: String,
    /// The name this box takes under [`BOXES_DIR`], and what the subject of a
    /// sync push names.
    pub box_name: String,
}

impl Mesh {
    /// [`from_env`]'s rule, with the environment passed in, so it is
    /// testable without racing other tests over process-global state.
    pub fn mesh_from(url: Option<&str>, box_name: Option<&str>) -> Option<Mesh> {
        let url = url.map(str::trim).filter(|s| !s.is_empty())?;
        let box_name = box_name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("mesh-box")
            .to_string();
        Some(Mesh {
            url: url.to_string(),
            box_name,
        })
    }

    /// `None` when `LOSOS_MESH_CONFIG_URL` is unset, empty or whitespace.
    /// The box name falls back to `mesh-box`: something with a letter in it,
    /// for a commit subject, while the module decides the real one.
    pub fn from_env() -> Option<Mesh> {
        Self::mesh_from(
            std::env::var(MIRROR_URL_ENV).ok().as_deref(),
            std::env::var(MIRROR_BOX_ENV).ok().as_deref(),
        )
    }
}

/// The name a provisioned machine takes on the mirror ([`BOXES_DIR`] entry it
/// records itself by and what its sync pushes name it by): lowercase letters,
/// digits and dashes, at most 32 characters: a git path a human reads the
/// same way on every box.
/// The name a provisioned machine takes on the mirror ([`BOXES_DIR`] entry it
/// records itself by and what its sync pushes name it by): lowercase letters,
/// digits and dashes, at most 32 characters, and not starting with a dash:
/// a git path a human reads the same way on every box.
pub fn check_box_name(name: &str) -> Result<&str, String> {
    let valid = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
    let ok = !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && name.chars().all(valid);
    if ok {
        Ok(name)
    } else {
        Err(format!(
            "--name must be lowercase letters, digits and dashes, at most 32 characters, got {name:?}"
        ))
    }
}

/// A box with no operator in front: the machine ID, as `box-<8 hex>` from the
/// installer medium's own `/etc/machine-id`.
///
/// The file is written by stage 1 on any NixOS system; a missing or short one
/// falls back to "box", which would collide — so an operator naming the
/// machine themselves beats the fallback, and a collision is refused before
/// it can matter.
pub fn default_box_name(id: &str) -> String {
    let hex: String = id
        .chars()
        .filter(char::is_ascii_hexdigit)
        .collect::<String>()
        .chars()
        .take(8)
        .collect();
    if hex.len() < 8 {
        "mesh-box".to_string()
    } else {
        format!("box-{hex}")
    }
}

/// `git ls-remote --heads` output as a map from branch to sha. The last line
/// for a branch wins (a mirror in mid-motion cannot branch); parse errors are
/// skipped so a mirror with human noise at stdin still yields its heads.
pub fn remote_heads(out: &str) -> std::collections::BTreeMap<String, String> {
    let mut heads = std::collections::BTreeMap::new();
    for line in out.lines() {
        let Some((sha, name)) = line.split_once('\t') else {
            continue;
        };
        if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        if let Some(branch) = name.strip_prefix("refs/heads/") {
            heads.insert(branch.to_string(), sha.to_string());
        }
    }
    heads
}

// ── What one provision decides ──────────────────────────────────────────────

/// The steps of one provision, decided from what the mirror yields.
///
/// The mirror must carry a shared configuration for provisioning to be
/// worth its destructive work: a machine PXE-booted from nothing that installs
/// a stock box would be better done by the regular stick. Failing (details in
/// the message) happens *before* any disk is touched, so a half-built mirror
/// is recoverable by pointing the netboot at another machine.
///
/// The `provision` half of `losos-ctl` runs [`StageIn`] first, then calls
/// `install` — the destructive half is the installer's own plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The mirror yielded nothing for this machine: stop, and say why.
    Refuse,
    /// Take the mirror: stage its overrides, install with them, and record
    /// the box under [`BOXES_DIR`] when the install is done.
    StageIn,
}

/// The pure decision: there is a shared configuration, the machine names
/// itself legally, both are fit for tracking ([`check_box_name`]), and
/// everything is there for [`Stage::StageIn`].
pub fn plan_provision(overrides: Option<&str>, box_name: &str) -> Stage {
    match overrides {
        Some(text) if !text.trim().is_empty() && check_box_name(box_name).is_ok() => Stage::StageIn,
        _ => Stage::Refuse,
    }
}

// ── The sync decision ───────────────────────────────────────────────────────

/// What the daemon recorded the last time it had one of the two words
/// (`{"sha":"<mirror head>","text":"<this box's overrides then>"}`).
/// `pushed_at`: the moment this box itself sealed a push, when it did: the
/// tie-break in last-writer-wins.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Mark {
    /// The mirror head the mark corresponds to.
    pub sha: String,
    /// This box's own overrides.nix text at that moment.
    pub text: String,
    /// Seconds since the epoch, this box's own last push. `None`: the mark is
    /// a take, or was recorded before pushes were counted.
    #[serde(default)]
    pub pushed_at: Option<i64>,
}

/// What one look at the mirror asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshPlan {
    /// Either side is identical to what both saw last: the caller still
    /// refreshes the mark with the more recent mirror head.
    Nothing,
    /// The mirror holds something this box has not seen: take it.
    Take,
    /// It is this box's word to place in the mirror.
    Push,
}

/// Decide the sync from the box's current text, the mirror head, and the
/// mark.
///
/// With no mark, a later take is the rule: this box has either just been
/// provisioned or joined a standing mirror, and in both cases the mesh's
/// choice is what it falls in line with, not the other way.
///
/// With a mark, the box and the mirror each *remember what happened between
/// the two*: anything one recorded, the other has too, and whatever changed
/// since is the one thing left to decide. When both sides changed,
/// last-writer-wins with the mirror head's committer time (the whole
/// state that decided the winner travels in the commit, so the next, faster
/// writer refutes it by time alone). An unknown time loses to a remembered
/// one: a mirror to be woken up by hand (0) never beats a box that pushed.
pub fn plan_mesh(local: &str, remote: Option<(&str, &str, i64)>, mark: Option<&Mark>) -> MeshPlan {
    let Some((remote_text, remote_sha, remote_at)) = remote else {
        // No branch: a mirror that lost its head (rebuilt, moved). This
        // box's word seeds it again; a push would also be the first push
        // of a box that has no mark yet.
        return MeshPlan::Push;
    };
    let Some(mark) = mark else {
        return if local == remote_text {
            MeshPlan::Nothing
        } else {
            MeshPlan::Take
        };
    };
    if local != mark.text {
        // The box moved on its own since the mark: it pushes unless the
        // mirror caught a *newer* original push.
        if remote_sha == mark.sha {
            MeshPlan::Push
        } else if remote_at > mark.pushed_at.unwrap_or(0) {
            MeshPlan::Take
        } else {
            MeshPlan::Push
        }
    } else if remote_sha != mark.sha {
        // Only the mirror moved.
        MeshPlan::Take
    } else {
        MeshPlan::Nothing
    }
}

/// The subject and body a sync push names: what the mirror's history reads if
/// the same mutation came from either side, and the mirror-admin reads again
/// to see who won.
pub fn mesh_push_message(box_name: &str, head: &str, pushed_at: i64) -> (String, String) {
    (
        format!("autoprovision: {box_name} pushed this box's configuration"),
        format!("Sync: {box_name} {head} {pushed_at}\nThe box's commit that carried it: {head}"),
    )
}

/// The subject and body a take commits on the box's own repository: the
/// History pane reads it as an ordinary line of `git log`.
pub fn mesh_take_message() -> (String, String) {
    (
        "autoprovision: took the mesh's configuration".to_string(),
        "The mesh mirror's modules/overrides.nix, through the Apply gates.".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_names_are_path_safe() {
        for good in [
            "box",
            "mattbox",
            "box-001",
            "abc-123",
            "a",
            "0123456789abcdef0123456789abcdef",
        ] {
            assert!(check_box_name(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "Box",
            "box/1",
            "box 2",
            "mayıldırım",
            // 33 characters, one past the boundary.
            "0123456789abcdefghijklmnopqrstuvwxyz9",
            "box..local",
            "-",
        ] {
            assert!(check_box_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_default_name_comes_from_the_machine_id() {
        assert_eq!(default_box_name("c9409e2e6daces-off"), "box-c9409e2e");
        // 0 or <8 hex leaves no distinctive suffix to read.
        assert_eq!(default_box_name(""), "mesh-box");
        assert_eq!(default_box_name("zzzz"), "mesh-box");
    }

    #[test]
    fn ls_remote_heads_parse_and_branch_names_strip() {
        let out = "\
abcdefabcdefabcdefabcdefabcdefabcdefabcd\trefs/heads/provision
1234561234561234561234561234561234561234\trefs/heads/boxes/one
short\trefs/heads/bogus
deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\tnot-a-head-ref
garbage without tabs
";
        let heads = remote_heads(out);
        assert_eq!(
            heads.get("provision").map(String::as_str),
            Some("abcdefabcdefabcdefabcdefabcdefabcdefabcd")
        );
        assert_eq!(
            heads.get("boxes/one").map(String::as_str),
            Some("1234561234561234561234561234561234561234")
        );
        // Bad lines leave nothing, instead of keeping half a name.
        assert!(!heads.contains_key("bogus"));
        assert!(!heads.contains_key("not-a-head-ref"));
        assert_eq!(heads.len(), 2);
    }

    #[test]
    fn a_provision_needs_the_shared_config_and_a_legal_name() {
        assert_eq!(plan_provision(Some("{ ... }: {}"), "box-1"), Stage::StageIn);
        // No overrides, empty overrides or a bad name — anything stops
        // before destructive work.
        assert_eq!(plan_provision(None, "box-1"), Stage::Refuse);
        assert_eq!(plan_provision(Some("   \n"), "box-1"), Stage::Refuse);
        assert_eq!(plan_provision(Some("{ ... }: {}"), "Box/1"), Stage::Refuse);
    }

    fn mark(sha: &str, text: &str) -> Mark {
        Mark {
            sha: sha.to_string(),
            text: text.to_string(),
            pushed_at: None,
        }
    }

    #[test]
    fn no_mark_yields_to_the_mirror() {
        // Provisioned boxes start with the mesh's text as their own.
        assert_eq!(
            plan_mesh("mesh's text", Some(("mesh's text", "h1", 10)), None),
            MeshPlan::Nothing
        );
        assert_eq!(
            plan_mesh("this box's text", Some(("mesh's text", "h1", 10)), None),
            MeshPlan::Take
        );
        // A brand-new mirror has nothing to take; this box seeds it.
        assert_eq!(plan_mesh("anything", None, None), MeshPlan::Push);
    }

    #[test]
    fn only_one_side_moving_creates_no_conflict() {
        let m = mark("h1", "the shared text");
        // Same on both sides.
        assert_eq!(
            plan_mesh(
                "the shared text",
                Some(("the shared text", "h1", 5)),
                Some(&m)
            ),
            MeshPlan::Nothing
        );
        // The mirror moved; the box did not. Take it.
        assert_eq!(
            plan_mesh("the shared text", Some(("newer text", "h2", 9)), Some(&m)),
            MeshPlan::Take
        );
        // The box moved; the mirror did not. Push.
        assert_eq!(
            plan_mesh("a newer edit", Some(("the shared text", "h1", 5)), Some(&m)),
            MeshPlan::Push
        );
        // A mirror with no branch gets seeded, even with a mark.
        assert_eq!(plan_mesh("a newer edit", None, Some(&m)), MeshPlan::Push);
    }

    #[test]
    fn both_moving_resolves_by_the_last_writer() {
        // The box pushed at 10; the mirror's head says 7 is a take at h2.
        let m = Mark {
            pushed_at: Some(10),
            ..mark("h1", "the shared text")
        };
        assert_eq!(
            plan_mesh("box's text", Some(("older mesh text", "h2", 7)), Some(&m)),
            MeshPlan::Push
        );
        // The mirror head is newer: it wins.
        assert_eq!(
            plan_mesh("box's text", Some(("newer mesh text", "h2", 11)), Some(&m)),
            MeshPlan::Take
        );
        // Same instant: a counted push of the box's own wins the tie, since
        // the mirror head already agrees with *something* of this box's.
        assert_eq!(
            plan_mesh("box's text", Some(("mesh text", "h2", 10)), Some(&m)),
            MeshPlan::Push
        );
        // A box that has never pushed loses to any mirror movement, however
        // old: there is no moment of its own to win with.
        let unpushed = mark("h1", "the shared text");
        assert_eq!(
            plan_mesh("box's text", Some(("mesh text", "h2", 1)), Some(&unpushed)),
            MeshPlan::Take
        );
    }

    #[test]
    fn push_and_take_messages_name_the_winner() {
        let (s, b) = mesh_push_message("mattbox", "c0ffee", 1690000000);
        assert_eq!(s, "autoprovision: mattbox pushed this box's configuration");
        assert!(b.contains("Sync: mattbox c0ffee 1690000000"));
        assert!(b.contains("c0ffee"));
        let (s, b) = mesh_take_message();
        assert!(s.starts_with("autoprovision: took"));
        assert!(b.contains("Apply gates"));
    }

    #[test]
    fn the_mesh_reads_the_environment() {
        assert!(Mesh::mesh_from(None, None).is_none());
        assert!(Mesh::mesh_from(Some("   "), None).is_none());
        let m = Mesh::mesh_from(Some("  /srv/mirror.git "), None).expect("a mirror in a var");
        assert_eq!(m.url, "/srv/mirror.git");
        assert_eq!(m.box_name, "mesh-box");
        assert_eq!(
            Mesh::mesh_from(Some("/srv/mirror.git"), Some(" mattbox "))
                .expect("a mirror and a name")
                .box_name,
            "mattbox"
        );
    }
}
