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
    content.lines().find_map(|l| {
        if l.trim_start().starts_with('#') {
            return None;
        }
        let (lhs, rhs) = l.split_once('=')?;
        // The left side must BE the option, not merely contain it. Matching on
        // `contains` made `losos.proxy.enable` also match `losos.proxy.enabled`,
        // and `losos.gpu.enable = true; # see losos.hostName` match hostName.
        // A compact one-line body (`{ losos.hostName = "x"; }`) puts a brace
        // before the key, so strip that before comparing.
        if lhs.trim().trim_start_matches('{').trim() != needle {
            return None;
        }
        // Drop a trailing comment before anything else, or `= 11000; # default`
        // reads back as `11000; # default`.
        let v = rhs.split('#').next().unwrap_or(rhs);
        // Then the statement terminator, then a closing brace from a one-line
        // body, then the terminator again in case the brace hid it.
        let v = v.trim().trim_end_matches(';').trim();
        let v = v.trim_end_matches('}').trim().trim_end_matches(';').trim();
        Some(v.to_string())
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

/// Whether `h` is a valid single DNS label, per RFC 1123: 1-63 characters,
/// alphanumeric at both ends, hyphens allowed in between.
///
/// `losos.hostName` becomes `networking.hostName`, the mDNS name the box is
/// reached by, Nextcloud's `trusted_domains` and `overwrite.cli.url`, and
/// Forgejo's `ROOT_URL`. A space, a slash, a leading hyphen or a 64th
/// character produces either a config that will not evaluate or an appliance
/// that boots unreachable — and there is no SSH and no login shell to repair
/// it from. The admin UI checks this too, but a browser is not a trust
/// boundary: `POST /api/apply` accepts a body from anything holding the token.
pub fn valid_host_name(h: &str) -> bool {
    let b = h.as_bytes();
    !h.is_empty()
        && h.len() <= 63
        && b[0].is_ascii_alphanumeric()
        && b[b.len() - 1].is_ascii_alphanumeric()
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
}

/// Gate an `apply` payload before it overwrites `overrides.nix`.
///
/// Deliberately shallow on syntax — no brace balancing, no Nix parsing.
/// `nixos-rebuild` is the real syntax checker; this only blocks payloads that
/// are obviously empty or aimed at the wrong file, which would otherwise
/// silently blank the appliance's configuration.
///
/// It is *not* shallow about `hostName`, because a bad one is the single value
/// here that can leave the box unreachable with no way back in. Rules are
/// checked in order and the messages are part of the HTTP contract.
pub fn validate_apply(t: &str) -> Result<&str, &'static str> {
    if t.trim().is_empty() {
        return Err("empty nix config");
    }
    if !t.contains("losos.") {
        return Err("nix config must reference losos.* options");
    }
    if !t.contains('{') {
        return Err("nix config must be a module body (missing '{')");
    }
    // Only checked when the body actually sets it; an apply that leaves
    // hostName alone is none of this function's business.
    if let Some(raw) = lookup_nix("hostName", t) {
        let h = strip_quotes(&raw);
        // A `${...}` here is Nix interpolation, evaluated as root by the
        // rebuild — `${builtins.readFile "/var/secrets/losos-admin-token"}`
        // would splice the token into the config. The character class below
        // already excludes `$`, `{` and `}`; this arm exists to give that
        // case a message that says what is wrong rather than "invalid".
        if h.contains("${") {
            return Err("hostName must not contain Nix interpolation");
        }
        if !valid_host_name(h) {
            return Err(
                "hostName must be 1-63 chars, alphanumeric at both ends, hyphens allowed between",
            );
        }
    }
    Ok(t)
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

    #[test]
    fn a_trailing_comment_does_not_poison_the_value() {
        // Split on the first `=` and this read back as `11000; # default`,
        // which parsed to the default by luck. `= 12345; # x` would have
        // silently returned the wrong port.
        let s = parse_settings("{\n  losos.nextcloud.apachePort = 12345; # default\n}\n");
        assert_eq!(s.apache_port, 12345);
    }

    #[test]
    fn a_key_is_not_matched_by_a_longer_key_that_contains_it() {
        // `contains("losos.proxy.enable")` also matched `losos.proxy.enabled`.
        let s = parse_settings("{\n  losos.proxy.enabled = true;\n}\n");
        assert!(!s.proxy_enable, "a different option must not be read");
    }

    #[test]
    fn a_mention_in_a_trailing_comment_is_not_an_assignment() {
        // This line assigns gpu.enable; it merely *mentions* hostName.
        let s = parse_settings("{\n  losos.gpu.enable = true; # see losos.hostName\n}\n");
        assert_eq!(s.host_name, Settings::default().host_name);
        assert!(s.gpu_enable);
    }

    #[test]
    fn a_one_line_module_body_parses() {
        // The closing brace used to end up inside the value.
        let s = parse_settings("{ losos.hostName = \"box2\"; }");
        assert_eq!(s.host_name, "box2");
    }

    #[test]
    fn valid_host_names_are_accepted() {
        for h in ["x", "mattbox", "box-2", "a-b-c", &"a".repeat(63)] {
            assert!(valid_host_name(h), "{h:?} should be valid");
        }
    }

    #[test]
    fn invalid_host_names_are_rejected() {
        for h in [
            "",              // empty
            "-box",          // leading hyphen
            "box-",          // trailing hyphen
            "my box",        // space
            "a/b",           // slash
            "box.local",     // dot: this is one label, not an FQDN
            "box_2",         // underscore is not legal in a hostname
            &"a".repeat(64), // 63 is the limit
        ] {
            assert!(!valid_host_name(h), "{h:?} should be rejected");
        }
    }

    #[test]
    fn apply_rejects_a_hostname_that_would_brick_the_box() {
        // The browser checks these too, but a token holder can POST directly.
        assert!(validate_apply("{ losos.hostName = \"my box\"; }").is_err());
        assert!(validate_apply("{ losos.hostName = \"-box\"; }").is_err());
        assert!(validate_apply("{ losos.hostName = \"\"; }").is_err());
    }

    #[test]
    fn apply_rejects_nix_interpolation_in_the_hostname() {
        // Evaluated as root by nixos-rebuild; this one splices the admin token
        // into the generated config.
        let body =
            "{ losos.hostName = \"a${builtins.readFile \"/var/secrets/losos-admin-token\"}b\"; }";
        assert_eq!(
            validate_apply(body),
            Err("hostName must not contain Nix interpolation")
        );
    }

    #[test]
    fn apply_without_a_hostname_is_left_alone() {
        // Not every apply touches hostName; the check must not demand one.
        assert!(validate_apply("{ losos.sharingMyStorage = true; }").is_ok());
    }
}
