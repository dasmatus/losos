//! `losos-ctl set-password` — replace the Nextcloud admin password.
//!
//! `modules/nextcloud-common.nix` generates the install-time password with
//! `head -c 24 /dev/urandom | base64`, writes it 0600, and shows it to nobody.
//! `flake/images.nix` has already run `occ maintenance:install` with it by the
//! time anyone looks. The appliance has no SSH and no shell logins, so on a
//! fresh box the only account that can administer Nextcloud is one whose
//! password exists solely as bytes in a root-only file. This command is the way
//! out: it **replaces** that password with one the owner chose. It never reads
//! it back out. Surfacing the generated one would turn a root-only 0600 file
//! into an API-readable secret and buy nothing — the owner still has to type a
//! password they want either way.
//!
//! Same shape as [`crate::grow`]: a pure planner producing an ordered
//! [`OccAction`] list, and a thin executor behind the [`crate::losos::Losos`]
//! trait. The reason the plan is data here is narrower than `grow`'s ordering
//! argument, and it is this:
//!
//! > **The password must never appear in an argv.** `/proc/<pid>/cmdline` is
//! > world-readable and nothing in `modules/` sets `hidepid`, so
//! > `occ user:resetpassword --password hunter2` publishes it to every process
//! > on the box, including the two data domains' own workloads. With the argv
//! > in the plan as data, "no rendered argv contains the password" is a
//! > property the test suite checks by construction instead of by reading the
//! > code and hoping.
//!
//! The password is not a field of [`OccAction`] at all. It travels beside the
//! plan, as the `secret` argument to [`crate::losos::Losos::run_occ`], so it
//! cannot reach an argv — and cannot reach a serialized plan in a debug log
//! either.
//!
//! ## The two modes take different routes, and neither is optional
//!
//! ```text
//!   container   crictl ps   → the running workload container's id
//!               stage       → /var/lib/nextcloud/.losos-setpass, 0600, uid 1002
//!               crictl exec → /bin/losos-nextcloud-occ-setpass <user>
//!               clear       → unlink the staged file, success or failure
//!
//!   native      nextcloud-occ user:resetpassword <user> --password-from-env
//!               with OC_PASS in the child's environment
//! ```
//!
//! Container mode stages a file because **`crictl exec` has no environment
//! flag**. That is checked, not assumed: `crictl exec --help` on the pinned
//! `cri-tools 1.36.0` lists `--ignore-errors/-e`, `--image`, `--interactive`,
//! `--label`, `--last`, `--latest`, `--name`, `--parallel`, `--pod`, `--quiet`,
//! `--state`, `--sync`, `--timeout`, `--tls-*`, `--transport` and `--tty`, and
//! nothing else. `-e` is `--ignore-errors`, which is a trap of its own. The
//! staged file is the only channel left that is not an argv, and
//! `/var/lib/nextcloud` is a read-write `hostPath` mount shared by the host and
//! the pod (`modules/workloads.nix`), so both ends can reach it.

use serde::{Deserialize, Serialize};

/// The Nextcloud admin account this appliance installs.
/// `modules/nextcloud-stack.nix` sets `adminUser = "notshared"`, matching the
/// uid-1000 data domain in `modules/configuration.nix`.
pub const DEFAULT_ADMIN_USER: &str = "notshared";

/// Shortest password this daemon will set.
///
/// Above Nextcloud's own `password_policy` default of 10, and deliberately a
/// server-side rule rather than a browser one: the admin HTTP API is reachable
/// by anything that holds the admin token, and the wizard's `<input>` is not a
/// boundary. The floor is length only. Composition rules ("one symbol, one
/// digit") push people towards shorter passwords they reuse, and Nextcloud's
/// own policy app is the right place to add more if an owner wants it.
pub const MIN_PASSWORD_CHARS: usize = 12;

/// Longest password accepted, in bytes.
///
/// Not a security limit — a cap on what gets handed to a password hasher. Note
/// that Nextcloud hashes with bcrypt, which itself stops at 72 bytes, so
/// anything past that adds no entropy on Nextcloud's side either.
pub const MAX_PASSWORD_BYTES: usize = 256;

/// Where the host stages the new password for the pod to read.
///
/// Inside `/var/lib/nextcloud`, which `modules/workloads.nix` mounts into the
/// pod as a read-write `Directory` hostPath owned `1002:1002`. Deliberately
/// *not* `/var/secrets/nextcloud-admin-pass`: that one is mounted `type: File`,
/// a bind mount of a single inode, so the temp-and-rename in
/// [`crate::io_backend::atomic_write_secret`] would swap the directory entry
/// and leave the pod reading the old inode forever.
pub const STAGED_SECRET: &str = "/var/lib/nextcloud/.losos-setpass";

/// The uid the Nextcloud workload runs as (`modules/workloads.nix`), and so the
/// uid that has to be able to read the staged file. It is 0600, so ownership is
/// the whole of its access control.
pub const NEXTCLOUD_UID: u32 = 1002;

/// The native-mode entry point: the wrapper `services.nextcloud` puts on
/// `environment.systemPackages`. It handles `cd webroot`,
/// `NEXTCLOUD_CONFIG_DIR`, dropping to the `nextcloud` user, and — the reason
/// it is the right entry point rather than calling `php occ` ourselves — it
/// forwards `OC_PASS` across that privilege drop.
pub const NATIVE_OCC: &str = "nextcloud-occ";

/// The container-mode entry point, which lives **inside the image**.
///
/// `occ` is not on the pod's `PATH`: `flake/images.nix` defines it as a shell
/// function in the entrypoint, after a `cd` to the webroot and an
/// `export NEXTCLOUD_CONFIG_DIR`, and an external `crictl exec` inherits none
/// of that. The wrapper named here is the contract this module depends on:
///
/// > `/bin/losos-nextcloud-occ-setpass <user>` reads the password from
/// > [`STAGED_SECRET`], exports it as `OC_PASS`, runs
/// > `occ user:resetpassword <user> --password-from-env`, and exits with occ's
/// > status.
///
/// Read the file with a plain `$(cat …)`: the daemon writes it with no trailing
/// newline and [`validate_password`] rejects control characters, so command
/// substitution round-trips it exactly.
pub const IMAGE_OCC_SETPASS: &str = "/bin/losos-nextcloud-occ-setpass";

/// The CRI socket of the box's **own** k3s cluster's containerd
/// (`modules/cluster.nix`, `localCriSocket`).
///
/// Not `/run/k3s/containerd/containerd.sock` — that one belongs to the mesh
/// rke2 agent, which runs the edge's workloads and has never heard of this
/// box's Nextcloud.
pub const DEFAULT_CRI_SOCKET: &str = "unix:///run/containerd/containerd.sock";

/// `crictl ps --name` takes a regular expression, so this is anchored: an
/// unanchored `nextcloud` would also match a `nextcloud-cron` sidecar if one is
/// ever added, and [`parse_container_ids`] would then refuse to guess between
/// them.
const CONTAINER_NAME_RE: &str = "^nextcloud$";
/// `modules/workloads.nix` puts every static pod in `kube-system`.
const CONTAINER_NAMESPACE_RE: &str = "^kube-system$";

/// Which deployment mode Nextcloud is **running** in.
///
/// Read from `$LOSOS_NEXTCLOUD_MODE`, set by `modules/daemon.nix` from
/// `config.losos.nextcloud.mode`. Deliberately not derived from
/// [`crate::losos::cmd_settings`]: `overrides.nix` describes what the *next*
/// rebuild will be, and between `POST /api/apply` and that rebuild finishing
/// the two disagree. The pod that has to be talked to is the running one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NcMode {
    Container,
    Native,
}

impl NcMode {
    /// The wire spelling, and the spelling `losos.nextcloud.mode` uses.
    pub fn as_str(self) -> &'static str {
        match self {
            NcMode::Container => "container",
            NcMode::Native => "native",
        }
    }

    /// Exact-match parse; anything else is `None`.
    ///
    /// No lenient fallback, unlike [`crate::model::Mode`]. A wrong guess here
    /// does not degrade gracefully — it runs `crictl` on a box with no local
    /// cluster, or `nextcloud-occ` on a box whose Nextcloud lives in an image.
    pub fn parse(s: &str) -> Option<NcMode> {
        match s {
            "container" => Some(NcMode::Container),
            "native" => Some(NcMode::Native),
            _ => None,
        }
    }
}

/// A password, wrapped so it cannot be printed by accident.
///
/// The `Debug` impl redacts, and there is deliberately no `Display`, no
/// `Serialize` and no `Deref`. `crate::http::run` logs failures as
/// `tracing::error!(error = ?e, …)`, and `dbus::reply` renders the whole
/// context chain into the error message: a `String` password caught up in
/// either would land in the journal or on the bus.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// The plaintext. Every call site is one of exactly two: writing the staged
    /// file, or setting `OC_PASS` on a child process.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// Where the `occ` invocation is going to run, once it has been located.
///
/// The container id is an *input* to the planner rather than something the plan
/// discovers, for the same reason [`crate::grow::plan_grow`] takes a
/// [`crate::grow::VgFree`]: the look and the do are separate, so the do is a
/// pure function of what was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A running workload container, addressed by CRI id on a CRI socket.
    Container { socket: String, id: String },
    /// The host's own `nextcloud-occ` wrapper.
    Native,
}

impl Target {
    pub fn mode(&self) -> NcMode {
        match self {
            Target::Container { .. } => NcMode::Container,
            Target::Native => NcMode::Native,
        }
    }
}

/// How the password reaches `occ`. Both arms exist because neither mode can use
/// the other's channel, and naming the channel in the plan is what stops a
/// later edit from quietly adding a third one that happens to be an argv.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretChannel {
    /// `OC_PASS` in the child process's environment.
    /// `/proc/<pid>/environ` is mode 0400 owner-only and the child is root.
    Env,
    /// A file staged beforehand, read by the in-image wrapper. The only channel
    /// left in container mode, because `crictl exec` has no environment flag.
    File { path: String },
}

/// One step of a password change. Data rather than a call, so the argv is
/// testable without running anything — and so the password, which is *not* a
/// field here, provably cannot be in one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OccAction {
    /// Write the new password to `path`, mode 0600, owned by `uid`.
    ///
    /// 0600 plus the chown is the whole access control: the pod's process runs
    /// as `uid` and there is no group anyone shares.
    StageSecret { path: String, uid: u32 },
    /// Run `occ`. A non-zero exit is reported as data, not as a spawn failure —
    /// see [`interpret_occ`].
    RunOcc {
        argv: Vec<String>,
        secret: SecretChannel,
    },
    /// Remove the staged file.
    ///
    /// Runs on the failure path too. It holds a plaintext password, and a
    /// `crictl exec` that fails because the pod is still installing would
    /// otherwise leave it lying in `/var/lib/nextcloud` until the next attempt.
    ClearSecret { path: String },
}

/// What a finished `occ` run reported. Interpreted purely by
/// [`interpret_occ`], so "which failures are which" is unit-tested rather than
/// discovered in a journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OccOutcome {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// The argv that finds the running Nextcloud container.
///
/// The "look" half, matching [`crate::losos::Losos::vg_free`]. Two details are
/// load-bearing and both are flag traps in `crictl`:
///
///   * `--runtime-endpoint` comes **before** the subcommand. It is a global
///     flag; after `ps` the short form `-r` means `--resolve-image-path`, and
///     after `exec` it means `--transport`. `tests/cluster-vm.nix` already
///     spells its calls this way.
///   * every flag is spelled long. `-s` is `--state` on `ps` but `--sync` on
///     `exec`, and `-t` is `--timeout` globally but `--tty` on `exec`.
pub fn resolve_argv(socket: &str) -> Vec<String> {
    vec![
        "crictl".into(),
        "--runtime-endpoint".into(),
        socket.to_string(),
        "ps".into(),
        "--name".into(),
        CONTAINER_NAME_RE.into(),
        "--namespace".into(),
        CONTAINER_NAMESPACE_RE.into(),
        "--state".into(),
        "Running".into(),
        // Ids only, untruncated — a truncated id is not a valid `exec` target.
        "--quiet".into(),
        "--no-trunc".into(),
    ]
}

/// Pick the one container id out of `crictl ps --quiet` output.
///
/// Zero and many are both errors rather than a guess. The filters in
/// [`resolve_argv`] *should* select exactly one container, but that is an
/// inference from `modules/workloads.nix`, not something this code can see —
/// and the cost of guessing wrong is resetting a password inside the wrong
/// container and reporting success.
pub fn parse_container_ids(stdout: &str) -> Result<String, String> {
    let ids: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    match ids.as_slice() {
        [id] => Ok((*id).to_string()),
        [] => Err(format!(
            "no running container matches name {CONTAINER_NAME_RE} in namespace \
             {CONTAINER_NAMESPACE_RE}. On a fresh box the pod takes minutes to \
             come up — its entrypoint runs `occ maintenance:install` first. \
             Check `systemctl status k3s` and try again."
        )),
        many => Err(format!(
            "{} containers match name {CONTAINER_NAME_RE} in namespace \
             {CONTAINER_NAMESPACE_RE}; refusing to guess which one is Nextcloud",
            many.len()
        )),
    }
}

/// Reject a user name that could be read as a flag, or that could not be a
/// Nextcloud account.
///
/// The leading-dash rule is the one with teeth: `occ user:resetpassword
/// --version --password-from-env` parses, and would report success having
/// changed nothing.
pub fn validate_user(user: &str) -> Result<&str, String> {
    if user.is_empty() {
        return Err("user must not be empty".to_string());
    }
    if user.len() > 64 {
        return Err("user must be at most 64 characters".to_string());
    }
    if user.starts_with('-') {
        return Err(format!(
            "user must not start with '-' ({user:?} would be read as a flag by occ)"
        ));
    }
    if !user
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'@'))
    {
        return Err(format!(
            "user may only contain letters, digits and . _ - @ (got {user:?})"
        ));
    }
    Ok(user)
}

/// Check a candidate password and wrap it.
///
/// No error here ever quotes the password: these messages travel to the
/// journal, to the D-Bus error name and to the SPA.
pub fn validate_password(password: &str) -> Result<Secret, String> {
    // Characters, not bytes, so a passphrase in a non-Latin script is measured
    // the way its owner would count it.
    let chars = password.chars().count();
    if chars < MIN_PASSWORD_CHARS {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_CHARS} characters (got {chars})"
        ));
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(format!(
            "password must be at most {MAX_PASSWORD_BYTES} bytes"
        ));
    }
    // A newline is the one that would actually bite: the in-image wrapper reads
    // the staged file with `$(cat …)`, so a password containing one would be
    // silently truncated there while the owner believes they set the whole
    // thing — and would then be unable to log in. Every other control character
    // is refused with it, because none of them survive a round trip through a
    // shell any more reliably.
    if let Some(c) = password.chars().find(|c| c.is_control()) {
        return Err(format!(
            "password must not contain control characters (found U+{:04X})",
            c as u32
        ));
    }
    Ok(Secret(password.to_string()))
}

/// Turn a located target into an ordered action list.
///
/// `user` must already have been through [`validate_user`]; the planner
/// re-checks rather than trusting its caller, because this is the function that
/// puts the value into an argv.
pub fn plan_set_password(target: &Target, user: &str) -> Result<Vec<OccAction>, String> {
    let user = validate_user(user)?;
    Ok(match target {
        Target::Native => vec![OccAction::RunOcc {
            argv: vec![
                NATIVE_OCC.into(),
                "user:resetpassword".into(),
                user.to_string(),
                // The whole reason this command is safe to run at all. Without
                // it occ takes the password interactively, and a daemon has no
                // terminal; with `--password` it would take it from an argv.
                "--password-from-env".into(),
            ],
            secret: SecretChannel::Env,
        }],
        Target::Container { socket, id } => vec![
            OccAction::StageSecret {
                path: STAGED_SECRET.to_string(),
                uid: NEXTCLOUD_UID,
            },
            OccAction::RunOcc {
                argv: vec![
                    "crictl".into(),
                    "--runtime-endpoint".into(),
                    socket.clone(),
                    "exec".into(),
                    // ExecSync: one RPC returning stdout, stderr and the exit
                    // code. Without it crictl wants a streaming server and a
                    // stdin this daemon does not have.
                    "--sync".into(),
                    id.clone(),
                    IMAGE_OCC_SETPASS.into(),
                    user.to_string(),
                ],
                secret: SecretChannel::File {
                    path: STAGED_SECRET.to_string(),
                },
            },
            OccAction::ClearSecret {
                path: STAGED_SECRET.to_string(),
            },
        ],
    })
}

/// The argv an action becomes, or `None` for the two that touch only files.
///
/// Kept next to the planner so the test suite can assert the exact command line
/// — and assert what is *not* on it — without spawning anything.
pub fn action_argv(a: &OccAction) -> Option<&[String]> {
    match a {
        OccAction::RunOcc { argv, .. } => Some(argv),
        OccAction::StageSecret { .. } | OccAction::ClearSecret { .. } => None,
    }
}

/// Turn a finished `occ` run into a message, or an error worth reading.
///
/// Unlike `resize2fs`, occ's exit code is trustworthy here:
/// `core/Command/User/ResetPassword.php` returns 1 on every failure branch and
/// 0 only after `setPassword` has returned true. There is no
/// succeeded-while-doing-nothing path, so this needs none of `grow`'s
/// measure-before-and-after paranoia.
pub fn interpret_occ(outcome: &OccOutcome) -> Result<String, String> {
    if outcome.code == 0 {
        let msg = last_line(&outcome.stdout);
        return Ok(if msg.is_empty() {
            "password updated".to_string()
        } else {
            msg
        });
    }

    let combined = format!("{}\n{}", outcome.stdout, outcome.stderr);
    // `crictl ps --state Running` goes true long before Nextcloud is usable:
    // the entrypoint runs `occ maintenance:install` on first boot and that
    // takes minutes. Saying so is the difference between an owner waiting and
    // an owner re-imaging the box.
    if combined.contains("not installed") {
        return Err(
            "Nextcloud is not installed yet, so there is no account to reset. \
             The first boot runs `occ maintenance:install`, which takes several \
             minutes; try again once the Nextcloud page loads."
                .to_string(),
        );
    }
    // ResetPassword.php prints exactly this when the environment channel did
    // not arrive — which in container mode means the staged file never reached
    // the pod, not that the password was empty.
    if combined.contains("NC_PASS/OC_PASS is empty") {
        return Err(format!(
            "occ received no password. The staged file at {STAGED_SECRET} did not \
             reach the container — check that /var/lib/nextcloud is the pod's \
             hostPath mount and that {IMAGE_OCC_SETPASS} reads it."
        ));
    }
    let detail = {
        let stderr = last_line(&outcome.stderr);
        if stderr.is_empty() {
            last_line(&outcome.stdout)
        } else {
            stderr
        }
    };
    Err(if detail.is_empty() {
        format!("occ user:resetpassword failed (exit {})", outcome.code)
    } else {
        format!(
            "occ user:resetpassword failed (exit {}): {detail}",
            outcome.code
        )
    })
}

/// Last non-empty line, capped. occ writes one line worth reading and a
/// variable amount of Symfony framing around it.
///
/// Delegates to [`crate::supervisor::last_log_line`] so the daemon has one
/// capping rule rather than two that drift.
fn last_line(text: &str) -> String {
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    crate::supervisor::last_log_line(&lines)
}
