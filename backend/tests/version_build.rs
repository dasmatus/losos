//! The version stamping contract, anchored against the tree.
//!
//! `backend/build.rs` reads the tag from `nix/version.nix` (or the
//! `LOSOS_VERSION` environment, or Cargo.toml's counter when no nix tree is
//! there) and generates `OUT_DIR/version.rs`, which src/version.rs includes
//! as `VERSION`. This test re-parses `nix/version.nix` itself and demands
//! the constant agree, so a build script that starts inventing versions or
//! drifts into reading something else fails here, exactly where a malformed
//! `sections.json` or `schema.json` edit fails the build instead.
//!
//! The same pass stands in for build.rs's check_data job: the two documents
//! lososd reads at run time must parse and keep their shape, and the test
//! re-reads them with the run time's own reader (serde_json).

use serde_json::Value;

/// The tag a release intends, re-derived the same way build.rs does: the
/// first double-quoted run in nix/version.nix.
fn tree_tag() -> Option<String> {
    let text =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../nix/version.nix")).ok()?;
    let start = text.find('"')? + 1;
    let end = start + text[start..].find('"')?;
    Some(text[start..end].trim_matches('"').trim().to_string())
}

#[test]
fn version_matches_the_tag_commit() {
    match tree_tag() {
        Some(tag) => assert_eq!(
            losos_ctl::version::VERSION,
            if tag.starts_with('v') { tag } else { format!("v{tag}") },
            "build.rs stamped a version that is not the tag the tree is",
        ),
        None => panic!("nix/version.nix is missing: this checkout is the flake's source root and must carry it"),
    }
}

#[test]
fn wire_spec_keeps_every_namespace() {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/schema.json"))
            .unwrap_or_else(|e| panic!("reading schema.json: {e}")),
    )
    .unwrap_or_else(|e| panic!("schema.json parses at run time too: {e}"));
    let namespaces = schema.as_object().expect("schema.json is an object");
    for namespace in ["commands", "http", "dbus"] {
        assert!(
            namespaces.get(namespace).is_some_and(|ns| ns.is_object()),
            "schema.json: `{namespace}` must stay a namespace object",
        );
    }
}

#[test]
fn access_map_stays_a_populated_object() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/sections.json"))
        .unwrap_or_else(|e| panic!("reading sections.json: {e}"));
    let sections: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("sections.json parses at run time too: {e}"));
    assert!(
        sections.as_object().is_some_and(|o| !o.is_empty()),
        "sections.json: lososd reads the access map from this object at run time",
    );
}
