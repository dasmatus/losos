//! Reading and writing the two Nix files the control plane owns.
//!
//! This module is pure: it takes and returns strings, touches no filesystem,
//! and is where the whole `overrides.nix` contract lives. Two separate files
//! are involved and they are deliberately **not** the same one:
//!
//!   * `modules/overrides.nix` (`LOSOS_OVERRIDES`) — read by `settings`,
//!     overwritten wholesale by `apply` and `factory-reset`.
//!   * `defaults.nix` (`LOSOS_CONFIG`) — only ever line-patched by
//!     `change --mode`, via [`inject_line`].
//!
//! Parsing is line-based on purpose. A real Nix parser would reject the
//! hand-edited files this appliance tolerates, and the only values that need
//! reading are scalar assignments on a single line.

use crate::model::Settings;

/// The committed default body. Returned when `overrides.nix` is missing, and
/// written back verbatim by `factory-reset`.
pub const DEFAULT_OVERRIDES_NIX: &str = r#"{ ... }:

{
  losos.sharingMyStorage = true;
  losos.nextcloud.mode = "container";
  losos.forgejo.mode = "container";
  losos.hostName = "mattbox";
  losos.nextcloud.https = false;
  losos.gpu.enable = true;
  losos.nextcloud.apachePort = 11000;
  losos.proxy.enable = false;
}
"#;

/// Find the value assigned to `losos.<key>`.
///
/// The first line that mentions the key, contains an `=`, and is not commented
/// out wins. The value is everything after the first `=`, minus surrounding
/// whitespace and one trailing `;`.
pub fn lookup_nix(key: &str, content: &str) -> Option<String> {
    let needle = format!("losos.{key}");
    content
        .lines()
        .find(|l| l.contains(&needle) && l.contains('=') && !l.trim_start().starts_with('#'))
        .map(|l| {
            let after = &l[l.find('=').expect("checked above") + 1..];
            after.trim().trim_end_matches(';').trim().to_string()
        })
}

/// Strip one matching pair of surrounding double quotes, if present.
fn strip_quotes(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && b[0] == b'"' && b[b.len() - 1] == b'"' {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// `"true"`/`"false"`; anything else (including a missing key) keeps `default`.
fn read_bool(default: bool, v: Option<String>) -> bool {
    match v.as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    }
}

/// Quote-stripped string, or `default` when the key is absent.
fn read_str(default: &str, v: Option<String>) -> String {
    v.map(|s| strip_quotes(&s).to_string())
        .unwrap_or_else(|| default.to_string())
}

/// Like [`read_str`], but folds the retired `"aio"` deployment mode into the
/// `"container"` it became, so a pre-migration overrides file still reads back
/// as something the admin UI's `<select>` can display.
fn read_mode(default: &str, v: Option<String>) -> String {
    let s = read_str(default, v);
    if s == "aio" {
        "container".to_string()
    } else {
        s
    }
}

/// Integer, or `default` when the key is absent or unparseable.
fn read_int(default: i64, v: Option<String>) -> i64 {
    v.and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

/// Parse a whole `overrides.nix` body into [`Settings`].
///
/// Missing keys fall back to [`Settings::default`] field by field, so a partial
/// file is still readable. Two keys carry legacy aliases that are honoured only
/// when the current spelling is absent: `losos.aio.apachePort` (pre-rename
/// Nextcloud AIO) and `losos.cfd.enable` (the retired Cloudflare tunnel, whose
/// role the master proxy took over).
pub fn parse_settings(content: &str) -> Settings {
    let d = Settings::default();
    Settings {
        sharing_my_storage: read_bool(
            d.sharing_my_storage,
            lookup_nix("sharingMyStorage", content),
        ),
        nextcloud_mode: read_mode(&d.nextcloud_mode, lookup_nix("nextcloud.mode", content)),
        forgejo_mode: read_str(&d.forgejo_mode, lookup_nix("forgejo.mode", content)),
        host_name: read_str(&d.host_name, lookup_nix("hostName", content)),
        https: read_bool(d.https, lookup_nix("nextcloud.https", content)),
        gpu_enable: read_bool(d.gpu_enable, lookup_nix("gpu.enable", content)),
        apache_port: read_int(
            d.apache_port,
            lookup_nix("nextcloud.apachePort", content)
                .or_else(|| lookup_nix("aio.apachePort", content)),
        ),
        proxy_enable: read_bool(
            d.proxy_enable,
            lookup_nix("proxy.enable", content).or_else(|| lookup_nix("cfd.enable", content)),
        ),
    }
}

/// Patch the `losos.sharingMyStorage` assignment in `defaults.nix`.
///
/// Replaces the first real assignment in place. When there is none, the line is
/// inserted before the closing brace, or appended if the file doesn't end in
/// one. The `contains('=')` guard means a comment merely *mentioning* the
/// option is left alone.
pub fn inject_line(sharing: bool, lines: &[String]) -> Vec<String> {
    let new = format!(
        "  losos.sharingMyStorage = {};",
        if sharing { "true" } else { "false" }
    );
    let is_assign = |l: &String| l.contains("losos.sharingMyStorage") && l.contains('=');

    if let Some(i) = lines.iter().position(is_assign) {
        let mut out = lines.to_vec();
        out[i] = new;
        return out;
    }
    let mut out = lines.to_vec();
    match out.last() {
        Some(last) if last.contains('}') => out.insert(out.len() - 1, new),
        _ => out.push(new),
    }
    out
}

/// Gate an `apply` payload before it overwrites `overrides.nix`.
///
/// Intentionally shallow — no brace balancing, no Nix parsing. `nixos-rebuild`
/// is the real syntax checker; this only blocks payloads that are obviously
/// empty or aimed at the wrong file, which would otherwise silently blank the
/// appliance's configuration. Rules are checked in order and the messages are
/// part of the HTTP contract.
pub fn validate_apply(t: &str) -> Result<&str, &'static str> {
    if t.trim().is_empty() {
        Err("empty nix config")
    } else if !t.contains("losos.") {
        Err("nix config must reference losos.* options")
    } else if !t.contains('{') {
        Err("nix config must be a module body (missing '{')")
    } else {
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_committed_default_body() {
        let s = parse_settings(DEFAULT_OVERRIDES_NIX);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn missing_keys_fall_back_per_field() {
        let s = parse_settings("{ ... }:\n{\n  losos.hostName = \"box2\";\n}\n");
        assert_eq!(s.host_name, "box2");
        // everything else stays at the default
        assert_eq!(s.apache_port, 11000);
        assert!(s.sharing_my_storage);
    }

    #[test]
    fn commented_out_assignments_are_ignored() {
        let s =
            parse_settings("{\n  # losos.hostName = \"ghost\";\n  losos.hostName = \"real\";\n}");
        assert_eq!(s.host_name, "real");
    }

    #[test]
    fn legacy_aio_apache_port_is_honoured() {
        let s = parse_settings("{\n  losos.aio.apachePort = 12345;\n}");
        assert_eq!(s.apache_port, 12345);
    }

    #[test]
    fn current_apache_port_wins_over_legacy_alias() {
        let s =
            parse_settings("{\n  losos.nextcloud.apachePort = 1;\n  losos.aio.apachePort = 2;\n}");
        assert_eq!(s.apache_port, 1);
    }

    #[test]
    fn legacy_cfd_enable_feeds_proxy_enable() {
        let s = parse_settings("{\n  losos.cfd.enable = true;\n}");
        assert!(s.proxy_enable);
    }

    #[test]
    fn legacy_aio_mode_reads_back_as_container() {
        let s = parse_settings("{\n  losos.nextcloud.mode = \"aio\";\n}");
        assert_eq!(s.nextcloud_mode, "container");
    }

    #[test]
    fn inject_replaces_an_existing_assignment_in_place() {
        let lines: Vec<String> = vec![
            "{".into(),
            "  losos.sharingMyStorage = true;".into(),
            "}".into(),
        ];
        let out = inject_line(false, &lines);
        assert_eq!(out[1], "  losos.sharingMyStorage = false;");
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn inject_inserts_before_the_closing_brace_when_absent() {
        let lines: Vec<String> = vec!["{".into(), "  losos.hostName = \"x\";".into(), "}".into()];
        let out = inject_line(true, &lines);
        assert_eq!(out[2], "  losos.sharingMyStorage = true;");
        assert_eq!(out[3], "}");
    }

    #[test]
    fn inject_ignores_a_comment_that_only_mentions_the_option() {
        let lines: Vec<String> = vec![
            "{".into(),
            "  # losos.sharingMyStorage is set by the admin UI".into(),
            "}".into(),
        ];
        let out = inject_line(true, &lines);
        // the comment survives untouched; the assignment is inserted separately
        assert!(out[1].starts_with("  #"));
        assert_eq!(out[2], "  losos.sharingMyStorage = true;");
    }

    #[test]
    fn validate_apply_rules_fire_in_order() {
        assert_eq!(validate_apply("   \n "), Err("empty nix config"));
        assert_eq!(
            validate_apply("{ services.foo = 1; }"),
            Err("nix config must reference losos.* options")
        );
        assert_eq!(
            validate_apply("losos.hostName = \"x\";"),
            Err("nix config must be a module body (missing '{')")
        );
        assert!(validate_apply("{ losos.hostName = \"x\"; }").is_ok());
    }
}
