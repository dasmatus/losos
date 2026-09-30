//! The audit log's append path: one JSON line per record, created 0600,
//! parent directories made on demand, and never any secret in it.
//!
//! An integration test rather than a unit test in `guard.rs` on purpose: the
//! scratch directory comes from `assert_fs`, so no environment-derived path is
//! ever passed to the code that opens the file (CodeQL treats
//! `std::env::temp_dir()` as untrusted input and flags the open).

use assert_fs::TempDir;
use losos_ctl::guard::Audit;
use std::os::unix::fs::PermissionsExt;

#[test]
fn audit_appends_json_lines_at_mode_600() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nested/audit.log");
    let audit = Audit::new(&path);
    audit.record("/api/apply", "ok", "192.168.1.9");
    audit.record("/api/grow", "unauthorized", "192.168.1.10");

    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["route"], "/api/apply");
    assert_eq!(lines[0]["outcome"], "ok");
    assert_eq!(lines[1]["remote"], "192.168.1.10");
    assert!(lines[0]["ts"].as_str().is_some());
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
