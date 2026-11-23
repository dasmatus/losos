/*
 * The build's contract checks, run by Cargo before either binary compiles.
 *
 * Two jobs, both of which used to live elsewhere and drift:
 *
 *   * The version. Cargo.toml says a plain counter while the release truth —
 *     what boot menus, os-release, `losos --version` and the Git API's
 *     /version route show — is one tag string in nix/version.nix, kept
 *     current by .github/workflows/version.yml (the sync contract: nobody
 *     bumps it by hand). build.rs reads that file and generates OUT_DIR/
 *     version.rs, so `losos::version::VERSION` is the tag the tree is.
 *     LOSOS_VERSION is the escape hatch for the release jobs, which spell
 *     the tag over nix/version.nix anyway: the environment wins, the tree
 *     file is next, and Cargo.toml's own version is the fallback for a
 *     source checkout that does not carry the nix/ tree (nix/tests/verify
 *     the tagged spelling). backend/tests/version_build.rs anchors it: the
 *     test re-parses nix/version.nix itself and demands the constant agree.
 *
 *   * The two tree JSON documents this crate reads at run time —
 *     `backend/sections.json`, the access map behind src/sections.rs and
 *     modules/identity.nix, and `backend/schema.json`, the normative wire
 *     spec named by src/lib.rs's doc comment every producer on the box
 *     checks its output against — are parsed and eyeballed here, so a
 *     malformed edit fails the build naming its file instead of lososd
 *     failing at start-up on a box.
 *
 * serde_json rides only as a build-dependency, and only at the version the
 * crate already links, so Cargo.lock — and therefore the cargoHash in
 * nix/packages.nix — does not move when this script grows.
 */

use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|e| panic!("build.rs: {e}")));
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap_or_else(|e| panic!("build.rs: {e}")));

    stamp_version(&manifest_dir, &out_dir);
    check_data(&manifest_dir);
}

/// Generate OUT_DIR/version.rs: `pub const VERSION` is the tag the tree is.
fn stamp_version(manifest_dir: &Path, out_dir: &Path) {
    // backend/ is one directory in, so the repo root is one up: ../nix.
    let version_nix = manifest_dir.join("../nix/version.nix");
    println!("cargo:rerun-if-env-changed=LOSOS_VERSION");
    println!("cargo:rerun-if-changed={}", version_nix.display());

    let stamped = if let Ok(from_env) = env::var("LOSOS_VERSION") {
        tag(&from_env, "LOSOS_VERSION")
    } else {
        match fs::read_to_string(&version_nix) {
            Ok(text) => tag(&quoted_first(&text, &version_nix), "nix/version.nix"),
            Err(..) => env!("CARGO_PKG_VERSION").to_string(),
        }
    };

    let generated = out_dir.join("version.rs");
    fs::write(
        &generated,
        format!("pub const VERSION: &str = {stamped:?};\n"),
    )
    .unwrap_or_else(|e| panic!("build.rs: writing {}: {e}", generated.display()));
}

/// The first double-quoted run in the file is the tag line — nix/version.nix
/// is comments then one string, so this can't land on a wrong quote pair; the
/// failure text names the file rather than the build script.
fn quoted_first(text: &str, named: &Path) -> String {
    let start = match text.find('"') {
        Some(i) => i + 1,
        None => panic!(
            "build.rs: {} has no quoted version to read",
            named.display()
        ),
    };
    let end = start
        + text[start..].find('"').unwrap_or_else(|| {
            panic!(
                "build.rs: {} starts a quote it never closes",
                named.display()
            )
        });
    text[start..end].to_string()
}

/// Trim quotes and whitespace; a bare semver gains the tag's leading `v`, so
/// `LOSOS_VERSION=0.6.2` spells `v0.6.2`. Anything else the release job wrote
/// is kept: it owns the spelling, the same way its push does.
fn tag(raw: &str, from: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim();
    if trimmed.is_empty() {
        panic!("build.rs: {from} names an empty version");
    }
    if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("v{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// Parse both tree documents. The run-time readers keep their own semantics;
/// the build refuses only documents that are not what their names claim.
fn check_data(manifest_dir: &Path) {
    let sections = manifest_dir.join("sections.json");
    let schema = manifest_dir.join("schema.json");

    let sections = parse(&sections);
    if sections.as_object().is_none_or(|o| o.is_empty()) {
        panic!("build.rs: sections.json is not a populated object; lososd reads it at run time and would fail on a box instead");
    }

    let schema = parse(&schema);
    let namespaces = schema
        .as_object()
        .unwrap_or_else(|| panic!("build.rs: schema.json is not an object"));
    for namespace in ["commands", "http", "dbus"] {
        match namespaces.get(namespace) {
            Some(Value::Object(..)) => {}
            _ => panic!("build.rs: schema.json lost its `{namespace}` namespace — the wire spec is checked at build time, by this very script"),
        }
    }
}

fn parse(path: &PathBuf) -> Value {
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("build.rs: reading {}: {e}", path.display()));
    serde_json::from_str::<Value>(&text)
        .unwrap_or_else(|e| panic!("build.rs: {} is not valid JSON: {e}", path.display()))
}
