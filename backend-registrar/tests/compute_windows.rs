//! `window::render` and `window::valid_hhmm` — the pure core of the mesh's
//! compute windows.
//!
//! `render`'s output is a file the edge's `losos-mesh-taint.service` parses and
//! turns into `kubectl taint` arguments, and the reconciler byte-compares it
//! against what is already on disk, so both its *shape* and its *stability* are
//! contracts. `valid_hhmm` is the only thing standing between an arbitrary
//! request body and those arguments.

use std::collections::{BTreeMap, BTreeSet};

use losos_registrar::window::{render, valid_hhmm, ComputeWindow};
use losos_registrar::ComputeWindow as ReExported;

fn window(share: bool, start: &str, end: &str) -> ComputeWindow {
    ComputeWindow {
        share_compute: share,
        window_start: start.to_string(),
        window_end: end.to_string(),
        tz: "UTC".to_string(),
    }
}

/// An edge that has never seen a join still publishes a file. The taint timer
/// needs to read an empty set in order to *remove* a taint it applied earlier;
/// "no file" would leave it with nothing to act on and the taint stuck.
#[test]
fn no_windows_render_an_empty_node_list() {
    let rendered = render(&BTreeMap::new(), &BTreeSet::new());
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("valid JSON");
    assert_eq!(
        parsed["nodes"].as_array().expect("nodes is a list").len(),
        0
    );
    assert!(rendered.ends_with('\n'), "got: {rendered:?}");
}

#[test]
fn a_window_renders_every_field_the_taint_timer_reads() {
    let mut windows = BTreeMap::new();
    windows.insert("mattbox-01".to_string(), window(true, "23:00", "07:00"));
    let parsed: serde_json::Value =
        serde_json::from_str(&render(&windows, &BTreeSet::new())).expect("valid JSON");

    let node = &parsed["nodes"][0];
    assert_eq!(node["node_name"], "mattbox-01");
    assert_eq!(node["share_compute"], true);
    assert_eq!(node["window_start"], "23:00");
    assert_eq!(node["window_end"], "07:00");
}

/// Sorted by node name, because the reconciler only writes when the rendered
/// bytes differ from the file on disk. A hash-ordered list would reshuffle on
/// every pass and rewrite the file every `reconcile_interval` forever, which on
/// a five-second tick is a pointless fsync storm on the edge's root filesystem.
#[test]
fn output_is_ordered_by_node_name_and_stable() {
    let mut windows = BTreeMap::new();
    for id in ["zeta", "alpha", "mattbox-01", "beta"] {
        windows.insert(id.to_string(), window(true, "23:00", "07:00"));
    }
    let first = render(&windows, &BTreeSet::new());
    assert_eq!(
        first,
        render(&windows, &BTreeSet::new()),
        "render is not deterministic"
    );

    let parsed: serde_json::Value = serde_json::from_str(&first).expect("valid JSON");
    let names: Vec<&str> = parsed["nodes"]
        .as_array()
        .expect("nodes is a list")
        .iter()
        .map(|n| n["node_name"].as_str().expect("node_name is a string"))
        .collect();
    assert_eq!(names, ["alpha", "beta", "mattbox-01", "zeta"]);
}

/// Sharing off still carries a window: the owner may flip sharing back on from
/// the admin UI without touching the hours, and the edge should already know
/// them when they do.
#[test]
fn a_node_that_is_not_sharing_still_publishes_its_hours() {
    let mut windows = BTreeMap::new();
    windows.insert("mattbox-01".to_string(), window(false, "01:00", "05:30"));
    let parsed: serde_json::Value =
        serde_json::from_str(&render(&windows, &BTreeSet::new())).expect("valid JSON");

    assert_eq!(parsed["nodes"][0]["share_compute"], false);
    assert_eq!(parsed["nodes"][0]["window_start"], "01:00");
}

/// A window whose end is before its start wraps midnight — which is the common
/// case, since the window is meant to be the hours the owner is asleep. Nothing
/// in the renderer may normalise or reject it.
#[test]
fn a_window_that_wraps_midnight_is_carried_through_untouched() {
    let mut windows = BTreeMap::new();
    windows.insert("mattbox-01".to_string(), window(true, "23:00", "07:00"));
    let parsed: serde_json::Value =
        serde_json::from_str(&render(&windows, &BTreeSet::new())).expect("valid JSON");

    assert_eq!(parsed["nodes"][0]["window_start"], "23:00");
    assert_eq!(parsed["nodes"][0]["window_end"], "07:00");
}

#[test]
fn valid_hhmm_accepts_every_real_clock_time() {
    for value in ["00:00", "07:00", "12:34", "23:00", "23:59"] {
        assert!(valid_hhmm(value), "{value:?} should be accepted");
    }
}

/// The same rejections lososd's `valid_hhmm` makes, so the two halves of the
/// wire agree: a value the admin UI accepted must not be one the edge refuses,
/// and vice versa.
#[test]
fn valid_hhmm_rejects_everything_else() {
    for value in [
        "24:00",
        "7:00",
        "23:60",
        "23:5",
        "",
        "1:2:3",
        "07:00:00",
        "0700",
        "23-00",
        "aa:bb",
        "23:0a",
        " 23:00",
        "23:00 ",
        "２３:００",
    ] {
        assert!(!valid_hhmm(value), "{value:?} should be rejected");
    }
}

/// The type is re-exported from the crate root beside `Registry` and
/// `TenantView`; `modules/edge.nix`'s consumers and the tests both reach for it
/// there.
#[test]
fn the_window_type_is_re_exported_from_the_crate_root() {
    let direct = window(true, "23:00", "07:00");
    let root = ReExported {
        share_compute: true,
        window_start: "23:00".to_string(),
        window_end: "07:00".to_string(),
        tz: "UTC".to_string(),
    };
    assert_eq!(direct, root);
}

/// The zone travels with the hours.
///
/// It has to: the edge writes the taint, so the comparison happens on the
/// edge's clock, and the edge is a VPS running UTC while the appliance ships
/// Europe/Berlin. Before this field existed a 23:00-07:00 window entered by an
/// owner in Berlin was enforced 00:00-08:00 in winter and 01:00-09:00 in
/// summer, sliding an hour at each DST change. The rendered file is the only
/// thing the edge's taint script reads, so if the zone is not in there it does
/// not exist.
#[test]
fn the_rendered_window_carries_the_zone() {
    let mut m = BTreeMap::new();
    m.insert(
        "box".to_string(),
        ComputeWindow {
            share_compute: true,
            window_start: "23:00".to_string(),
            window_end: "07:00".to_string(),
            tz: "Europe/Berlin".to_string(),
        },
    );
    let out = render(&m, &BTreeSet::new());
    assert!(
        out.contains("\"tz\""),
        "the file must name the field: {out}"
    );
    assert!(
        out.contains("Europe/Berlin"),
        "the appliance's zone must survive rendering: {out}"
    );
}

/// A registry row written before the field existed still loads.
///
/// A failed registry load is fatal at boot for the registrar (Restart=always
/// turns it into a crash loop), so a new field that is not `#[serde(default)]`
/// would brick an edge on upgrade rather than degrade it. UTC is the right
/// default because it is what the edge's clock already was, so an old row keeps
/// behaving exactly as it did instead of shifting the moment this ships.
#[test]
fn a_row_without_a_zone_still_decodes_and_means_utc() {
    let w: ComputeWindow = serde_json::from_str(
        r#"{"share_compute":true,"window_start":"23:00","window_end":"07:00"}"#,
    )
    .expect("a pre-tz row must still decode");
    assert_eq!(w.tz, "UTC");
}

/* The idle bit reaches the file the taint script reads.
 *
 * The window is permission and idle is reality; the edge removes the taint
 * only when both hold. The rendered file is the only channel between the
 * registrar and that script, so a bit that does not survive rendering does not
 * exist as far as scheduling is concerned. */
#[test]
fn an_idle_node_is_rendered_idle() {
    let mut m = BTreeMap::new();
    m.insert("awake".to_string(), window(true, "23:00", "07:00"));
    m.insert("busy".to_string(), window(true, "23:00", "07:00"));

    let mut idle = BTreeSet::new();
    idle.insert("awake".to_string());

    let out = render(&m, &idle);
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    let nodes = doc["nodes"].as_array().unwrap();

    let find = |name: &str| {
        nodes
            .iter()
            .find(|n| n["node_name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(find("awake")["idle"], true);
    assert_eq!(
        find("busy")["idle"],
        false,
        "a node nobody reported idle must render busy"
    );
}

/* Fail closed, and this is the assertion that matters most here.
 *
 * A registrar that has heard nothing from a box — because the edge restarted,
 * because the box went offline, because the report aged past the heartbeat TTL
 * — knows nothing about whether its owner is at the keyboard. "I could not
 * tell" and "the owner is away" must never be the same answer, because one of
 * them hands a stranger's workload to somebody who is using their machine. */
#[test]
fn a_node_nobody_reported_on_is_busy_not_idle() {
    let mut m = BTreeMap::new();
    m.insert("silent".to_string(), window(true, "23:00", "07:00"));

    let out = render(&m, &BTreeSet::new());
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["nodes"][0]["idle"], false);
}

/* A row written before idle reporting existed still decodes, and decodes as
 * busy. A failed registry load is fatal at boot for the registrar, so a new
 * field that is not defaulted would brick an edge on upgrade. */
#[test]
fn a_row_without_idle_decodes_as_busy() {
    let w: losos_registrar::window::NodeWindow = serde_json::from_str(
        r#"{"node_name":"box","share_compute":true,"window_start":"23:00","window_end":"07:00"}"#,
    )
    .expect("a pre-idle row must still decode");
    assert!(!w.idle);
    assert_eq!(w.tz, "UTC");
}
