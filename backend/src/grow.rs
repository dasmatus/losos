//! `losos-ctl grow` — extend `/persist` into the volume group's free extents,
//! without unmounting it.
//!
//! The appliance stores `/nix` on `/persist` (impermanence binds
//! `/persist/nix` over `/nix`), so the store grows with every generation and
//! the 03:00 unattended rebuild is what eventually runs it out of room — on a
//! box with no shell to notice from. `modules/disko.nix` therefore leaves part
//! of the volume group unallocated (`losos.storage.fillPercent`), and this is
//! the command that claims it.
//!
//! Same shape as the installer: a pure planner producing an ordered
//! [`GrowAction`] list, and a thin executor. The ordering is the entire
//! correctness argument, so it is asserted on the *plan* rather than by
//! running anything:
//!
//! ```text
//!   lvextend           the LV takes the free extents
//!   cryptsetup resize  the LUKS mapping learns the LV got bigger
//!   resize2fs          the filesystem takes the new space, online
//! ```
//!
//! Getting that order wrong does not fail loudly. `resize2fs` asks the
//! *mapping* how big it is, so running it before `cryptsetup resize` reads the
//! old size and no-ops with a cheerful "the filesystem is already 123 blocks
//! long. Nothing to do!" — the command succeeds, the disk does not grow, and
//! nothing says otherwise until the box fills up again.

use serde::{Deserialize, Serialize};

/// The device names this layout fixes. They are not configurable because
/// `modules/disko.nix` hard-codes them: the volume group is `persist-vg`, its
/// one logical volume is `persist`, and the opened LUKS mapping is
/// `/dev/mapper/persist`.
pub const VG: &str = "persist-vg";
pub const LV: &str = "persist";
pub const LUKS_NAME: &str = "persist";
pub const MAPPER: &str = "/dev/mapper/persist";

/// The logical volume's device path, as lvextend wants it.
pub fn lv_path() -> String {
    format!("/dev/{VG}/{LV}")
}

/// One step of an online grow. Data rather than a call, so the ordering is
/// testable without a disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowAction {
    /// `lvextend -l +<extents> /dev/persist-vg/persist`
    ExtendLv { extents: u64 },
    /// `cryptsetup resize persist [--key-file <path>]`
    ///
    /// The key file is not optional for cosmetic reasons. `cryptsetup resize`
    /// needs the volume key to re-derive the mapping's size, and when it
    /// cannot find one in the kernel keyring it falls back to *prompting on
    /// stdin* — which, for a daemon with no terminal, fails with the
    /// magnificently unhelpful "Nothing to read on input." That happens after
    /// `lvextend` has already run, leaving the logical volume bigger than the
    /// mapping on top of it.
    ResizeLuks { key_file: Option<String> },
    /// `resize2fs /dev/mapper/persist`
    ResizeFs,
}

/// What the volume group looks like before a grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VgFree {
    /// Free (unallocated) extents in the volume group.
    pub free_extents: u64,
    /// Extent size in bytes, for reporting how much that actually is.
    pub extent_bytes: u64,
}

impl VgFree {
    pub fn free_bytes(self) -> u64 {
        self.free_extents.saturating_mul(self.extent_bytes)
    }
}

/// Turn the volume group's free space into an ordered action list.
///
/// Returns `Err` when there is nothing to do, rather than an empty plan: a
/// grow that silently does nothing is the failure mode this whole module
/// exists to avoid, so "no free extents" is a message, not a no-op.
pub fn plan_grow(vg: VgFree, key_file: Option<&str>) -> Result<Vec<GrowAction>, String> {
    if vg.free_extents == 0 {
        return Err(format!(
            "{VG} has no free extents; nothing to grow into. \
             Add a disk (pvcreate + vgextend), or reinstall with a lower \
             losos.storage.fillPercent."
        ));
    }
    Ok(vec![
        GrowAction::ExtendLv {
            extents: vg.free_extents,
        },
        GrowAction::ResizeLuks {
            key_file: key_file.map(str::to_string),
        },
        GrowAction::ResizeFs,
    ])
}

/// Render an action as the argv it becomes. Kept next to the planner so the
/// test suite can assert the exact command line without spawning anything.
pub fn action_argv(a: &GrowAction) -> Vec<String> {
    match a {
        GrowAction::ExtendLv { extents } => vec![
            "lvextend".into(),
            "-l".into(),
            format!("+{extents}"),
            lv_path(),
        ],
        GrowAction::ResizeLuks { key_file } => {
            let mut v = vec!["cryptsetup".into(), "resize".into(), LUKS_NAME.into()];
            if let Some(path) = key_file {
                v.push("--key-file".into());
                v.push(path.clone());
            }
            v
        }
        // No size argument: resize2fs grows to fill the device it is given,
        // which is what we want and what makes the ordering above load-bearing.
        GrowAction::ResizeFs => vec!["resize2fs".into(), MAPPER.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4 MiB extents, the lvm2 default.
    const EXTENT: u64 = 4 * 1024 * 1024;

    #[test]
    fn no_free_extents_is_an_error_not_an_empty_plan() {
        let err = plan_grow(
            VgFree {
                free_extents: 0,
                extent_bytes: EXTENT,
            },
            None,
        )
        .unwrap_err();
        assert!(err.contains("no free extents"), "unhelpful message: {err}");
        // It must say what to do about it — there is no shell on this box to
        // go and investigate from.
        assert!(err.contains("vgextend") || err.contains("fillPercent"));
    }

    #[test]
    fn the_plan_is_exactly_extend_then_luks_then_fs() {
        // This ordering is the entire point of the module. resize2fs asks the
        // LUKS mapping how big it is, so running it before `cryptsetup resize`
        // reads the pre-grow size and no-ops while reporting success.
        let plan = plan_grow(
            VgFree {
                free_extents: 512,
                extent_bytes: EXTENT,
            },
            None,
        )
        .unwrap();
        assert_eq!(
            plan,
            vec![
                GrowAction::ExtendLv { extents: 512 },
                GrowAction::ResizeLuks { key_file: None },
                GrowAction::ResizeFs,
            ]
        );
    }

    #[test]
    fn lvextend_claims_every_free_extent_with_a_plus() {
        let plan = plan_grow(
            VgFree {
                free_extents: 77,
                extent_bytes: EXTENT,
            },
            None,
        )
        .unwrap();
        let argv = action_argv(&plan[0]);
        // `-l +77`, not `-l 77`: without the plus lvextend reads it as an
        // absolute extent count and *shrinks* a volume that already has more,
        // which on a mounted ext4 destroys it.
        assert_eq!(
            argv,
            vec!["lvextend", "-l", "+77", "/dev/persist-vg/persist"]
        );
    }

    #[test]
    fn resize_commands_address_the_mapping_not_the_logical_volume() {
        let plan = plan_grow(
            VgFree {
                free_extents: 1,
                extent_bytes: EXTENT,
            },
            None,
        )
        .unwrap();
        assert_eq!(
            action_argv(&plan[1]),
            vec!["cryptsetup", "resize", "persist"]
        );
        // resize2fs must be pointed at /dev/mapper/persist. Given the LV it
        // would read the LUKS header as a filesystem and refuse, or worse.
        assert_eq!(
            action_argv(&plan[2]),
            vec!["resize2fs", "/dev/mapper/persist"]
        );
    }

    #[test]
    fn a_key_file_is_passed_to_cryptsetup_and_to_nothing_else() {
        // Found by tests/resize.nix rather than reasoned about: without a key,
        // `cryptsetup resize` looks for the volume key in the kernel keyring,
        // does not find it, and falls back to prompting on stdin. A daemon has
        // no stdin, so it dies with "Nothing to read on input." — *after*
        // lvextend has already grown the logical volume.
        let plan = plan_grow(
            VgFree {
                free_extents: 8,
                extent_bytes: EXTENT,
            },
            Some("/etc/keys/persist-keyfile"),
        )
        .unwrap();
        assert_eq!(
            action_argv(&plan[1]),
            vec![
                "cryptsetup",
                "resize",
                "persist",
                "--key-file",
                "/etc/keys/persist-keyfile"
            ]
        );
        // lvextend and resize2fs take no key and must not be handed one.
        assert!(!action_argv(&plan[0]).contains(&"--key-file".to_string()));
        assert!(!action_argv(&plan[2]).contains(&"--key-file".to_string()));
    }

    #[test]
    fn free_bytes_multiplies_out_and_does_not_overflow() {
        let vg = VgFree {
            free_extents: 512,
            extent_bytes: EXTENT,
        };
        assert_eq!(vg.free_bytes(), 2 * 1024 * 1024 * 1024);
        // Saturating rather than wrapping: a bogus reading from `vgs` should
        // report an absurd number, not a small one.
        let silly = VgFree {
            free_extents: u64::MAX,
            extent_bytes: EXTENT,
        };
        assert_eq!(silly.free_bytes(), u64::MAX);
    }
}
