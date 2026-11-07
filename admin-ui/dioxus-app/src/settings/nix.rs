//! Nix generation for `/api/apply`, matching `admin-ui/app/src/lib/api.ts`
//! `buildOverridesNix`/`nixString`/`nixBool` and `OWNED_NAMES` behavior exactly.
//!
//! `/api/apply` REPLACES the entire `modules/overrides.nix` body — every key
//! that must be kept has to be emitted. Omitting an extra or owned key silently
//! resets it to defaults.

use super::model::{owned_name_set, Extras, SettingsResponse};

pub fn nix_string(value: &str) -> String {
    // Matches React `nixString`: backslashes, quotes, then `${` interpolation.
    let body = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace("${", "\\${");
    format!("\"{body}\"")
}

pub fn nix_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Build the full modules/overrides.nix body lososd writes.
/// Format matches backend's line-based parser/defaultOverridesNix.
pub fn build_overrides_nix(settings: &SettingsResponse, extra: &Extras) -> String {
    let owned = owned_name_set();
    let mut extra_lines: Vec<String> = extra
        .iter()
        .filter(|(name, _)| !owned.contains(name.as_str()))
        .map(|(name, lit)| format!("  losos.{name} = {lit};"))
        .collect();
    extra_lines.sort();

    let mut lines = vec![
        "{ ... }:".to_string(),
        "{".to_string(),
        format!("  losos.sharingMyStorage = {};", nix_bool(settings.sharing_my_storage)),
        format!("  losos.sharingLocally = {};", nix_bool(settings.sharing_locally)),
        format!("  losos.hostName = {};", nix_string(&settings.host_name)),
        format!("  losos.gpu.enable = {};", nix_bool(settings.gpu_enable)),
        format!("  losos.proxy.enable = {};", nix_bool(settings.proxy_enable)),
        format!("  losos.cluster.enable = {};", nix_bool(settings.cluster_enable)),
        format!("  losos.cluster.shareCompute = {};", nix_bool(settings.share_compute)),
        format!(
            "  losos.cluster.computeWindow.start = {};",
            nix_string(&settings.compute_window_start)
        ),
        format!(
            "  losos.cluster.computeWindow.end = {};",
            nix_string(&settings.compute_window_end)
        ),
        format!("  losos.hardening.apparmor = {};", nix_bool(settings.hardening_apparmor)),
        format!("  losos.hardening.malloc = {};", nix_bool(settings.hardening_malloc)),
        format!("  losos.hardening.nosmt = {};", nix_bool(settings.hardening_nosmt)),
        format!("  losos.hardening.usbguard = {};", nix_bool(settings.hardening_usbguard)),
    ];
    lines.extend(extra_lines);
    lines.push("}".to_string());
    lines.push(String::new());
    lines.join("\n")
}

pub fn is_valid_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return false;
    }
    let Ok(h) = value[0..2].parse::<u32>() else { return false };
    let Ok(m) = value[3..5].parse::<u32>() else { return false };
    h <= 23 && m <= 59
}

pub fn is_valid_host_name(value: &str) -> bool {
    let len = value.len();
    if len == 0 || len > 63 {
        return false;
    }
    let bytes = value.as_bytes();
    let ok_char = |b: u8| b.is_ascii_alphanumeric() || b == b'-';
    if !bytes[0].is_ascii_alphanumeric() || !bytes[len - 1].is_ascii_alphanumeric() {
        return false;
    }
    bytes.iter().all(|&b| ok_char(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nix_string_escapes_like_react() {
        assert_eq!(nix_string("mattbox"), "\"mattbox\"");
        assert_eq!(nix_string("a\"b"), "\"a\\\"b\"");
        assert_eq!(nix_string("a\\b"), "\"a\\\\b\"");
        assert_eq!(nix_string("${x}"), "\"\\${x}\"");
    }

    #[test]
    fn build_overrides_nix_matches_react_format() {
        let s = SettingsResponse::default();
        let extra = Extras::new();
        let nix = build_overrides_nix(&s, &extra);
        assert!(nix.starts_with("{ ... }:\n{\n"));
        assert!(nix.contains("  losos.sharingMyStorage = false;"));
        assert!(nix.contains("  losos.sharingLocally = false;"));
        assert!(nix.contains("  losos.hostName = \"mattbox\";"));
        assert!(nix.contains("  losos.gpu.enable = false;"));
        assert!(nix.contains("  losos.cluster.computeWindow.start = \"23:00\";"));
        assert!(nix.contains("  losos.cluster.computeWindow.end = \"07:00\";"));
        assert!(nix.contains("  losos.hardening.usbguard = false;"));
        assert!(nix.ends_with("}\n"));
        // 13 owned lines + header/footer
        let losos_lines = nix.lines().filter(|l| l.trim_start().starts_with("losos.")).count();
        assert_eq!(losos_lines, 13);
    }

    #[test]
    fn extras_sorted_and_owned_filtered() {
        let s = SettingsResponse::default();
        let mut extra = Extras::new();
        extra.insert("z.opt".to_string(), "true".to_string());
        extra.insert("a.opt".to_string(), "\"x\"".to_string());
        extra.insert("hostName".to_string(), "\"hijack\"".to_string()); // owned, must be filtered
        let nix = build_overrides_nix(&s, &extra);
        let a = nix.find("losos.a.opt").unwrap();
        let z = nix.find("losos.z.opt").unwrap();
        assert!(a < z, "extras must be sorted");
        assert_eq!(nix.matches("losos.hostName").count(), 1, "owned extra must not duplicate");
    }

    #[test]
    fn validators_match_react() {
        assert!(is_valid_time("23:00"));
        assert!(is_valid_time("00:59"));
        assert!(!is_valid_time("24:00"));
        assert!(!is_valid_time("23:00 "));
        assert!(is_valid_host_name("mattbox"));
        assert!(!is_valid_host_name("-bad"));
        assert!(!is_valid_host_name(""));
    }
}
