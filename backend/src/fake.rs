//! In-memory [`Losos`] implementation for tests.
//!
//! The counterpart of the Haskell `TestM` interpreter: it lets the command
//! functions run with no filesystem, no `systemd-run` and no clock, so the
//! state machine is tested deterministically.
//!
//! One simplification is inherited from `TestM` and worth stating plainly:
//! [`FakeLosos::config`] stands in for *both* Nix files. In production
//! `rewrite_config` patches `defaults.nix` while `write_overrides` replaces
//! `overrides.nix`, and they are different files. The fake collapses them, so
//! these tests must not be read as evidence about which file wins at Nix
//! evaluation time.

use crate::losos::Losos;
use crate::model::State;
use crate::overrides::{inject_line, DEFAULT_OVERRIDES_NIX};

/// A fake world: everything the commands can observe or mutate.
#[derive(Debug, Clone)]
pub struct FakeLosos {
    pub state: State,
    /// Stands in for both Nix files (see the module docs).
    pub config: Vec<String>,
    /// Lines the "rebuild log" would contain.
    pub log: Vec<String>,
    pub job_counter: u32,
    pub spawned: bool,
    /// Unallocated extents the fake volume group reports.
    pub vg_free_extents: u64,
    /// Size of the fake `/persist`, in bytes.
    pub persist_bytes: u64,
    /// Every grow step that was executed, in order. The ordering is the thing
    /// under test, so the fake records rather than simulates.
    pub grow_ran: Vec<crate::grow::GrowAction>,
    /// Key file handed to `cryptsetup resize`; None models the TPM path.
    pub luks_key_file: Option<String>,
}

impl FakeLosos {
    /// A fresh appliance: default state, the committed overrides body, no log.
    pub fn new() -> Self {
        FakeLosos {
            state: State::default(),
            config: DEFAULT_OVERRIDES_NIX.lines().map(str::to_string).collect(),
            log: Vec::new(),
            job_counter: 0,
            spawned: false,
            // 512 extents of the lvm2 default 4 MiB = 2 GiB of headroom, on a
            // 20 GiB filesystem. Both are arbitrary; what matters is that they
            // are non-zero so the happy path is the default.
            vg_free_extents: 512,
            persist_bytes: 20 * 1024 * 1024 * 1024,
            grow_ran: Vec::new(),
            luks_key_file: None,
        }
    }
}

impl Default for FakeLosos {
    fn default() -> Self {
        Self::new()
    }
}

impl Losos for FakeLosos {
    fn load_state(&mut self) -> anyhow::Result<State> {
        Ok(self.state.clone())
    }

    fn save_state(&mut self, s: &State) -> anyhow::Result<()> {
        self.state = s.clone();
        Ok(())
    }

    fn rewrite_config(&mut self, sharing: bool) -> anyhow::Result<()> {
        // Runs the real line-injection routine, so that logic is genuinely
        // covered rather than stubbed out.
        self.config = inject_line(sharing, &self.config);
        Ok(())
    }

    fn write_overrides(&mut self, body: &str) -> anyhow::Result<()> {
        self.config = body.lines().map(str::to_string).collect();
        Ok(())
    }

    fn read_overrides(&mut self) -> anyhow::Result<String> {
        let mut s = self.config.join("\n");
        s.push('\n');
        Ok(s)
    }

    fn spawn_rebuild(&mut self, _job: &str) -> anyhow::Result<()> {
        // No watcher is simulated: the Building record was already written by
        // the command itself before it got here.
        self.spawned = true;
        Ok(())
    }

    fn rebuild_log_tail(&mut self) -> anyhow::Result<String> {
        Ok(crate::supervisor::last_log_line(&self.log))
    }

    fn next_job_id(&mut self) -> anyhow::Result<String> {
        let id = format!("job-{}", self.job_counter);
        self.job_counter += 1;
        Ok(id)
    }

    fn vg_free(&mut self) -> anyhow::Result<crate::grow::VgFree> {
        Ok(crate::grow::VgFree {
            free_extents: self.vg_free_extents,
            extent_bytes: 4 * 1024 * 1024,
        })
    }

    fn run_grow(&mut self, action: &crate::grow::GrowAction) -> anyhow::Result<()> {
        // Recorded, not simulated — except for the one effect the command
        // actually reads back. `ResizeFs` is what makes the filesystem bigger,
        // so only that step moves `persist_bytes`; a plan that ran the steps in
        // the wrong order would still "work" here, which is precisely why the
        // ordering is asserted against the plan in grow.rs rather than here.
        self.grow_ran.push(action.clone());
        if matches!(action, crate::grow::GrowAction::ResizeFs) {
            let claimed = self.vg_free_extents.saturating_mul(4 * 1024 * 1024);
            self.persist_bytes = self.persist_bytes.saturating_add(claimed);
            self.vg_free_extents = 0;
        }
        Ok(())
    }

    fn persist_bytes(&mut self) -> anyhow::Result<u64> {
        Ok(self.persist_bytes)
    }

    fn luks_key_file(&mut self) -> anyhow::Result<Option<String>> {
        Ok(self.luks_key_file.clone())
    }
}
