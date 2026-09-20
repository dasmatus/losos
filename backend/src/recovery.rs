//! The appliance recovery code — one UUID, minted once, shown on demand.
//!
//! # What it is for
//!
//! The **reinstall ISO** wipes `/persist`, and `/persist` is the only durable
//! storage this box has: the persisted set in `modules/impermanence.nix` lives
//! inside the LUKS volume, and `backend/src/installer.rs` passes
//! `--yes-wipe-all-disks`, so the ESP goes too. The box comes back up with a
//! brand-new identity — a fresh `machine-id`, a fresh node password under
//! `/etc/rancher`, and no `/var/secrets/losos-proxy-token`.
//!
//! `losos-ctl factory-reset` does **not** do that, whatever an earlier draft of
//! this comment said. It restores the committed defaults and rebuilds; see
//! [`crate::losos::cmd_factory_reset`], which is explicit that the destructive
//! tier is the installer ISO, not the command.
//!
//! # What it does not solve — read this before wiring an edge half
//!
//! This module was conceived as the proof of continuity for that rejoin: to the
//! edge, a re-imaged box looks like a stranger claiming a hostname the registry
//! already knows. That lockout is real. It is also **already prevented by a
//! different mechanism**, landed the day before this file: `/cluster/join` in
//! `backend-registrar` deletes the caller's stale `Node` object and its
//! node-password `Secret` before handing back a mesh token, precisely so a
//! re-imaged box can rejoin under the same name. It authenticates that with the
//! appliance's existing `/var/secrets/losos-proxy-token` and mints no second
//! credential. The module header of `backend-registrar/src/server.rs` describes
//! this exact failure — "rke2 then refuses the rejoin *permanently* as a
//! duplicate hostname" — and the cleanup that fixes it.
//!
//! So the only thing a recovery code could still buy is self-service re-issue
//! of the proxy token, for an owner who kept no copy of it (nothing in the
//! flake creates that file — `modules/options.nix` says to provision it with
//! agenix or by hand, so the owner or the edge operator already holds one).
//! That is a different feature with a different threat model: an
//! unauthenticated, internet-reachable route trading a UUID for a tenant's live
//! credential. It is not what this module's text used to claim, and it should
//! be decided on its own merits.
//!
//! Until that decision is made, this module has exactly one job — make a code,
//! keep it stable for the life of the installation, and hand it back when the
//! first-run wizard asks. **Nothing consumes it**, and the wizard step that
//! shows it still tells the owner it is "the only proof that the new box is the
//! old one", which is the claim `modules/recovery.nix` warns against.
//!
//! # Why the durable copy is not on the box
//!
//! It cannot be. Everything this appliance can write is inside the volume that
//! a reinstall destroys. TPM2 NV storage is the one physical exception and it
//! is deliberately rejected: `losos.tpm.enable` may be false (the keyfile path
//! in `modules/disko.nix` is a supported, tested configuration), and a
//! reinstall that re-enrols LUKS is exactly when a TPM is most likely to have
//! been cleared. A recovery mechanism that silently does not exist on half the
//! fleet is worse than none.
//!
//! `/var/secrets/losos-recovery-code` is therefore a **cache for re-display**,
//! nothing more. The copies that matter are the owner's — paper, password
//! manager — and, once the edge half of this feature exists, the edge's. Please
//! do not "fix" this later by persisting it harder; there is nowhere harder to
//! persist it to.
//!
//! # Minted once, and only once
//!
//! Every read goes through [`plan_ensure`], which returns [`Ensure::Keep`] for
//! any well-formed file. A code that changed between two reads would make every
//! copy the owner already wrote down worthless, which is the whole failure this
//! module exists to prevent — so "mint" is reachable only from *no usable file
//! at all*, and the two ways of getting there are distinguished
//! ([`MintReason`]) because one of them is a fresh box and the other is
//! corruption worth a line in the journal.
//!
//! This is a deliberate divergence from `http::ensure_token`, which discards a
//! malformed admin token without ceremony. Rotating the admin token costs the
//! owner one visit to the admin UI; rotating this costs them the piece of paper
//! in their desk drawer.
//!
//! # Why the read is gated exactly like everything else, and no harder
//!
//! The code is a credential — it is the thing that will let a re-imaged box
//! claim an existing identity — so it is stored 0600, it is never an argv, and
//! it is never logged (see the hand-written [`Debug`] on [`Recovery`]: a
//! derived one would put it in the journal the first time anyone added a
//! `?recovery` field to a tracing call).
//!
//! But the *endpoint* that returns it gets the ordinary treatment: the same
//! Bearer token as every other `/api` route, behind the same `lanOnly` guard in
//! `modules/containers.nix`. Not a second factor, not a re-authentication, not
//! a one-shot reveal. The reasoning is that the admin token already authorises
//! `POST /api/apply`, which writes arbitrary Nix and runs `nixos-rebuild
//! switch` as root on this box. Guarding a recovery code more heavily than the
//! endpoint that already grants root would be theatre — it would add friction
//! for the owner and nothing for an attacker who, by the time they can call
//! this at all, can simply rebuild the machine into whatever they like.
//!
//! What that argument does *not* license is leaking it anywhere cheaper than
//! the gate. Hence: no logging, no argv, no `Debug`, and a full value in the
//! response body only.
//!
//! # Shape
//!
//! Same split as `grow.rs`: a pure planner ([`plan_ensure`]) plus formatting
//! and validation that need no filesystem, with the one effectful step behind
//! the [`CodeStore`] trait. `MemoryStore` is the in-memory interpreter the
//! tests run against, so "minted once, stable across reads" is asserted as
//! behaviour rather than inspected as a file.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Environment variable `modules/recovery.nix` points at the code file with.
pub const RECOVERY_FILE_ENV: &str = "LOSOS_RECOVERY_FILE";

/// Where the code lives when the environment says nothing.
///
/// Under `/var/secrets`, which impermanence keeps across the tmpfs-root reboot
/// (via the whole-`/var` bind mount) and which `atomic_write_secret` creates at
/// 0700 if it is not already there. Same directory, and the same 0600, as the
/// admin token — these are the two secrets the daemon mints for itself.
pub const DEFAULT_RECOVERY_FILE: &str = "/var/secrets/losos-recovery-code";

/// Length of a canonical hyphenated UUID: 32 hex digits plus 4 hyphens.
pub const UUID_LEN: usize = 36;

/// Byte offsets a canonical UUID puts its hyphens at.
const HYPHENS: [usize; 4] = [8, 13, 18, 23];

/// The code file's path, from `$LOSOS_RECOVERY_FILE` or the default.
///
/// The only place this module touches the outside world at all, and it is here
/// rather than in `io_backend` so that the constant, the variable name and the
/// fallback stay in one file with the reason they were chosen.
///
/// The lookup itself is split out into [`code_file_from`] so the fallback rule
/// is testable: `std::env::set_var` is process-global, and in a test binary
/// that runs its cases on threads it is a race rather than a fixture.
pub fn code_file() -> PathBuf {
    code_file_from(std::env::var(RECOVERY_FILE_ENV).ok().as_deref())
}

/// [`code_file`]'s rule, with the environment passed in.
///
/// A malformed value is impossible — any non-empty string is a path — so unlike
/// `http::admin_port` there is nothing to reject. An *empty* or whitespace
/// value is treated as unset, because a unit that writes
/// `LOSOS_RECOVERY_FILE=` meant to leave the default alone, and writing a
/// secret to `""` fails with something nobody could act on.
pub fn code_file_from(raw: Option<&str>) -> PathBuf {
    match raw.map(str::trim) {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(DEFAULT_RECOVERY_FILE),
    }
}

/// Render 16 random bytes as a canonical RFC 4122 version 4 UUID.
///
/// Pure, and takes its randomness as an argument, so the formatting is tested
/// against fixed vectors instead of against whatever `/dev/urandom` felt like.
///
/// Two nibbles are overwritten rather than random: the version (`4`) and the
/// two top bits of the variant octet (`10`). That costs 6 of the 128 bits and
/// leaves 122, which is well past anything a person could guess and is what
/// makes the result recognisable as a UUID to every tool that will ever read it
/// back.
pub fn format_uuid_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let mut out = String::with_capacity(UUID_LEN);
    for (i, b) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push(nibble(b >> 4));
        out.push(nibble(b & 0x0f));
    }
    out
}

/// One lowercase hex digit. Lowercase throughout so there is exactly one
/// spelling of any given code and [`is_well_formed`] can compare without
/// folding case.
fn nibble(n: u8) -> char {
    char::from_digit(u32::from(n), 16).unwrap_or('0')
}

/// Whether `candidate` is a code this daemon would have minted.
///
/// Canonical lowercase hyphenated form, version nibble `4`, RFC 4122 variant.
///
/// Note the asymmetry before changing this: *loosening* the check is always
/// safe — it can only start accepting a file that is already on disk — whereas
/// *tightening* it retroactively condemns codes owners have already written
/// down, because a code that stops validating is a code this module mints over.
/// If UUIDv7 ever looks appealing, accept both; do not swap one for the other.
pub fn is_well_formed(candidate: &str) -> bool {
    if candidate.len() != UUID_LEN {
        return false;
    }
    let b = candidate.as_bytes();
    if !HYPHENS.iter().all(|&i| b[i] == b'-') {
        return false;
    }
    let hex_ok = b
        .iter()
        .enumerate()
        .all(|(i, c)| HYPHENS.contains(&i) || (c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    if !hex_ok {
        return false;
    }
    // Byte 6 is at character offset 14 (two hyphens precede it); byte 8 is at
    // offset 19 (three hyphens). Those are the version and variant nibbles.
    b[14] == b'4' && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// Why a mint is about to happen. Kept apart because the two cases deserve
/// different volumes in the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MintReason {
    /// No file at all — a fresh installation, and the expected path exactly
    /// once in this appliance's life.
    Absent,
    /// A file that is not a code. `atomic_write_secret` is temp-then-rename, so
    /// this cannot be a torn write; it means the storage or the file was
    /// tampered with, and whatever the owner wrote down is unrecoverable from
    /// here. Minting is the only way back to having a code at all, but it is
    /// worth shouting about.
    Malformed,
}

/// What [`ensure_code`] should do about the file it found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ensure {
    /// The file already holds a usable code. Return it untouched — this is the
    /// branch that makes the code stable, and it is the branch taken on every
    /// call after the first.
    Keep(String),
    /// Nothing usable on disk; generate one and write it.
    Mint(MintReason),
}

/// Decide between keeping what is on disk and minting.
///
/// Pure: `existing` is the file's contents (already read), or `None` when there
/// is no file. Trimming happens here so that a trailing newline — which is how
/// the file is written, and how any text editor would leave it — is not
/// mistaken for corruption.
pub fn plan_ensure(existing: Option<&str>) -> Ensure {
    match existing {
        None => Ensure::Mint(MintReason::Absent),
        Some(raw) => {
            let candidate = raw.trim();
            if candidate.is_empty() {
                // An empty file is a fresh box whose write never landed, not a
                // corrupted code. Same branch, quieter.
                Ensure::Mint(MintReason::Absent)
            } else if is_well_formed(candidate) {
                Ensure::Keep(candidate.to_string())
            } else {
                Ensure::Mint(MintReason::Malformed)
            }
        }
    }
}

/// The effects [`ensure_code`] needs: a file it can read and write, and 16
/// random bytes.
///
/// Three methods rather than one so the tests can drive each independently —
/// in particular so "the second call writes nothing" is an assertion about
/// [`MemoryStore::writes`] rather than about a file's mtime.
pub trait CodeStore {
    /// Contents of the code file, or `None` if it does not exist. A read error
    /// that is *not* "missing" must be an `Err`, not a `None`: minting over an
    /// EIO would destroy a perfectly good code.
    fn read_code(&mut self) -> anyhow::Result<Option<String>>;
    /// Write the code out at 0600, atomically.
    fn write_code(&mut self, code: &str) -> anyhow::Result<()>;
    /// 16 bytes of cryptographic randomness.
    fn fresh_bytes(&mut self) -> anyhow::Result<[u8; 16]>;
}

/// What a caller gets back: the code, and whether this call is the one that
/// created it.
///
/// `minted` exists so the first-run wizard can say "write this down now"
/// exactly once and "here it is again" afterwards. It is not persisted state —
/// it describes this call.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recovery {
    pub code: String,
    pub minted: bool,
}

/// Redacted on purpose. `tracing` macros take `?value`, and a derived `Debug`
/// would put the recovery code in the journal the first time anyone wrote
/// `tracing::info!(?recovery, ...)` — which is not a mistake anyone would catch
/// in review, because it looks exactly like every other log line in this crate.
impl std::fmt::Debug for Recovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recovery")
            .field("code", &"<redacted>")
            .field("minted", &self.minted)
            .finish()
    }
}

/// Read the recovery code, minting one only if there is none to read.
///
/// Idempotent by construction: everything after the first successful call takes
/// the [`Ensure::Keep`] branch and touches nothing.
pub fn ensure_code<S: CodeStore + ?Sized>(store: &mut S) -> anyhow::Result<Recovery> {
    let existing = store.read_code()?;
    match plan_ensure(existing.as_deref()) {
        Ensure::Keep(code) => Ok(Recovery {
            code,
            minted: false,
        }),
        Ensure::Mint(reason) => {
            if reason == MintReason::Malformed {
                tracing::error!(
                    "the recovery code file does not contain a UUID; minting a new code — \
                     any code the owner wrote down before now is no longer the one this \
                     appliance will present"
                );
            }
            let code = format_uuid_v4(store.fresh_bytes()?);
            store.write_code(&code)?;
            // Deliberately no `code = %code` here, and no `path` either: the
            // path belongs to the store, and the value belongs nowhere.
            tracing::info!(?reason, "minted the appliance recovery code");
            Ok(Recovery { code, minted: true })
        }
    }
}

/// An in-memory [`CodeStore`].
///
/// Public rather than `#[cfg(test)]` so the daemon's own test suite can reach
/// it once `Losos`/`FakeLosos` grow a recovery method — the fake backend has no
/// business opening `/dev/urandom` either.
/// `Clone` so it can sit in [`crate::fake::FakeLosos`], which is cloned by the
/// tests. A clone is an independent store, not a second handle — which is the
/// right semantics here: two clones model two appliances, and that is exactly
/// what `two_appliances_do_not_get_the_same_code` needs.
#[derive(Debug, Default, Clone)]
pub struct MemoryStore {
    /// The "file". `None` is a missing file.
    pub file: Option<String>,
    /// How many times [`CodeStore::write_code`] ran. The mint-once assertion.
    pub writes: usize,
    /// Counter the fake randomness is derived from, so two mints differ.
    seed: u64,
}

impl MemoryStore {
    /// An empty store — no file, as on a fresh appliance.
    pub fn empty() -> Self {
        Self::default()
    }

    /// A store whose file already holds `contents`, byte for byte.
    pub fn with_file(contents: &str) -> Self {
        Self {
            file: Some(contents.to_string()),
            ..Self::default()
        }
    }
}

impl CodeStore for MemoryStore {
    fn read_code(&mut self) -> anyhow::Result<Option<String>> {
        Ok(self.file.clone())
    }

    fn write_code(&mut self, code: &str) -> anyhow::Result<()> {
        self.file = Some(format!("{code}\n"));
        self.writes += 1;
        Ok(())
    }

    /// Not random and not trying to be — a counter run through splitmix64, so
    /// successive mints differ and every test run produces the same bytes.
    fn fresh_bytes(&mut self) -> anyhow::Result<[u8; 16]> {
        let mut out = [0u8; 16];
        for chunk in out.chunks_mut(8) {
            self.seed = self.seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.seed;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^= z >> 31;
            chunk.copy_from_slice(&z.to_le_bytes());
        }
        Ok(out)
    }
}
