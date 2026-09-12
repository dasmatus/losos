//! Compute windows: when an appliance is willing to run other people's pods.
//!
//! An appliance owner opts in with `losos.cluster.shareCompute` and a
//! `losos.cluster.computeWindow.{start,end}` pair, and `losos-mesh-join`
//! carries all three to the edge on `/cluster/join`. The edge records them per
//! node and the reconciler republishes the whole set to
//! `--compute-windows-file`, which `losos-mesh-taint.service` reads every five
//! minutes and turns into `losos.dev/compute-window=closed:NoSchedule` on the
//! nodes whose window is shut.
//!
//! **The taint has to be applied from the edge.** Kubernetes' `NodeRestriction`
//! admission plugin forbids a kubelet from editing its own node's taints, so
//! no appliance-side timer can do this, and `--node-taint` at agent start is
//! fixed for the life of the process. That is the whole reason the window
//! travels over the wire at all instead of staying a local setting.
//!
//! The published file's shape, which the edge's timer script parses, is:
//!
//! ```json
//! {
//!   "nodes": [
//!     { "node_name": "mattbox-01", "share_compute": true,
//!       "window_start": "23:00", "window_end": "07:00" }
//!   ]
//! }
//! ```
//!
//! A list, not a map, so a shell consumer can walk it with one `jq` expression;
//! sorted by `node_name`, because the reconciler byte-compares the rendered
//! file against what is already on disk and a map iterated in hash order would
//! rewrite it on every tick. `end` earlier than `start` wraps midnight — the
//! common case, since the window is meant to be the hours the owner is asleep
//! — so the consumer must not assume `start < end`.
//!
//! Times are wall-clock in the *edge's* `time.timeZone`. Nothing here converts
//! them, and nothing should start: an owner who writes "23:00" means eleven at
//! night, and the appliance and the edge are expected to agree on the zone.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One appliance's compute-sharing preference, as recorded by `/cluster/join`.
///
/// `share_compute == false` still carries a window: the owner may flip sharing
/// back on from the admin UI without touching the hours, and the edge should
/// then already know them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeWindow {
    pub share_compute: bool,
    pub window_start: String,
    pub window_end: String,
    /// The IANA zone the two bounds are wall-clock times *in* — the
    /// appliance's own `time.timeZone`, sent at join time.
    ///
    /// This is not decoration. The taint that enforces the window is written
    /// by the edge, because that is the only node the NodeRestriction admission
    /// plugin lets write it, so the comparison happens on the edge's clock. The
    /// edge is a VPS and runs UTC; the appliance ships `Europe/Berlin`. Without
    /// carrying the zone, a 23:00-07:00 window entered by an owner in Berlin
    /// was enforced 00:00-08:00 in winter and 01:00-09:00 in summer, sliding an
    /// hour at each DST change — handing strangers' pods the first hours of
    /// that owner's working day, which is the exact harm the feature exists to
    /// prevent.
    ///
    /// `#[serde(default)]` so a registry written before this field existed
    /// still loads: a failed load is fatal at boot for the registrar.
    #[serde(default = "default_tz")]
    pub tz: String,
}

/// `UTC` — what a window means when its row predates the `tz` field.
///
/// Chosen because it is what the edge's clock already was, so an old row keeps
/// behaving exactly as it did rather than shifting the moment this ships.
fn default_tz() -> String {
    "UTC".to_string()
}

/// One row of the published file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeWindow {
    pub node_name: String,
    pub share_compute: bool,
    pub window_start: String,
    pub window_end: String,
    #[serde(default = "default_tz")]
    pub tz: String,
    /// Whether this box was idle as of its last heartbeat.
    ///
    /// The window says which hours the owner is willing to lend the machine
    /// out; this says whether they are using it right now. The taint comes off
    /// only when both agree, and this one can only ever withdraw availability
    /// inside a window, never grant it outside one.
    #[serde(default)]
    pub idle: bool,
}

/// The published file itself. A named `nodes` field rather than a bare array
/// so the format can gain a sibling key later without breaking a consumer that
/// already indexes `.nodes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeWindows {
    pub nodes: Vec<NodeWindow>,
}

/// Render the windows the registry holds into the bytes of
/// `--compute-windows-file`.
///
/// Pure: no IO, no clock. The reconciler compares the result against the file
/// on disk and only writes when it differs, so this must be a deterministic
/// function of its input — which the `BTreeMap` input and the trailing newline
/// both exist to guarantee.
#[must_use]
pub fn render(
    windows: &BTreeMap<String, ComputeWindow>,
    idle: &std::collections::BTreeSet<String>,
) -> String {
    let doc = ComputeWindows {
        nodes: windows
            .iter()
            .map(|(node_name, w)| NodeWindow {
                node_name: node_name.clone(),
                share_compute: w.share_compute,
                window_start: w.window_start.clone(),
                window_end: w.window_end.clone(),
                tz: w.tz.clone(),
                idle: idle.contains(node_name),
            })
            .collect(),
    };
    // Infallible for a struct of String/bool/Vec fields — there is no map with
    // non-string keys and no enum for serde_json to reject.
    let mut json = serde_json::to_string_pretty(&doc)
        .expect("serializing ComputeWindows is infallible for String/bool/Vec fields");
    json.push('\n');
    json
}

/// Whether `s` is a plausible IANA timezone name (`Europe/Berlin`, `UTC`).
///
/// Deliberately a shape check, not a lookup against the tz database: the
/// registrar has no tzdata of its own and the name is resolved by the edge's
/// `date`, not here. What this must catch is the two ways a bad value hurts.
/// A name `date` cannot resolve is silently treated as UTC — the window then
/// slides by the offset with nothing logged, which is the whole bug this field
/// was added to fix, reintroduced one layer down. And the value reaches a
/// shell, so anything outside this character class has no business being in it.
pub fn valid_tz(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('/')
        && !s.ends_with('/')
        && !s.contains("..")
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'/' | b'_' | b'-' | b'+'))
}

/// Whether `s` is a `HH:MM` clock time in `00:00`-`23:59`.
///
/// The single implementation of the rule for this crate: the `join` client
/// checks its own flags with it, and the edge re-checks every value that
/// arrives on the wire. `/cluster/join` is on the public internet, so a
/// browser (or lososd, or anything else holding a token) having already
/// validated the value is not a reason to trust it — a window that reaches
/// `compute-windows.json` malformed is a `kubectl taint` argument the edge's
/// timer builds out of attacker-supplied text.
///
/// Byte-wise rather than `chrono`: the input is exactly what an
/// `<input type="time">` emits, five ASCII characters, and a date library for
/// that is a dependency the appliance's closure would carry for nothing.
#[must_use]
pub fn valid_hhmm(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && (b[0] - b'0') * 10 + (b[1] - b'0') < 24
        && (b[3] - b'0') * 10 + (b[4] - b'0') < 60
}
