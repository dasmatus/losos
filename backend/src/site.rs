//! The settings of LosOS cloud and LosOS Git, on a route of their own.
//!
//! `GET /api/site` and `POST /api/site` are the apps section's way to change
//! what Nextcloud's Administration settings and LosOS Git's site settings
//! would otherwise hold, without `POST /api/apply`, which writes any option
//! and is admin only. A company edge the owner lets manage the box
//! (`crate::edge::manageable`) reaches these two routes and nothing that
//! writes the rest of `overrides.nix`.
//!
//! The change is still an apply: [`patch`] rewrites only the named lines,
//! and `cmd_site_set` hands the whole file to `cmd_apply`, so the typed gate,
//! the commit in the box's history and the rebuild are the same ones.

use serde_json::Value;

use crate::options::{fits, join, OptionsDoc, Rejected};
use crate::overrides::parse_line;

/// The options this route may read and write, by name or by prefix (a name
/// ending in `.`), without the `losos.` prefix.
pub const OPTIONS: &[&str] = &[
    "nextcloud.site.",
    "git.site.",
    "nextcloud.federation.enable",
];

pub fn is_site(name: &str) -> bool {
    OPTIONS.iter().any(|o| match o.strip_suffix('.') {
        Some(_) => name.starts_with(o) && name.len() > o.len(),
        None => name == *o,
    })
}

/// `GET /api/site`: [`join`] cut down to the site options. `stray` is left
/// empty, because the lines it lists are none of this route's business.
pub fn view(doc: &OptionsDoc, overrides: &str) -> Value {
    let mut v = join(doc, overrides);
    if let Some(obj) = v.as_object_mut() {
        if let Some(Value::Array(options)) = obj.get_mut("options") {
            options.retain(|o| o["name"].as_str().is_some_and(is_site));
        }
        obj.insert("stray".to_string(), Value::Array(Vec::new()));
        obj.insert("excluded".to_string(), Value::Object(Default::default()));
    }
    v
}

/// Check one change against the document: a site option the box declares,
/// not fixed by someone else, and a literal of its kind on one line.
pub fn check(doc: &OptionsDoc, name: &str, raw: Option<&str>) -> Result<(), Rejected> {
    if !is_site(name) {
        return Err(Rejected(format!(
            "losos.{name} is not a setting of LosOS cloud or LosOS Git"
        )));
    }
    let Some(opt) = doc.find(name) else {
        return Err(Rejected(format!(
            "losos.{name} is not an option this box declares"
        )));
    };
    if let Some(owner) = &opt.fixed {
        return Err(Rejected(format!(
            "losos.{name} is set by the {owner} and cannot be changed from here"
        )));
    }
    let Some(raw) = raw else { return Ok(()) };
    if raw.contains("${") {
        return Err(Rejected(format!(
            "losos.{name} must not contain Nix interpolation"
        )));
    }
    if raw.contains(['\n', '\r']) {
        return Err(Rejected(format!("losos.{name} must be one line")));
    }
    fits(&opt.editor, raw).map_err(|why| Rejected(format!("losos.{name} {why}")))
}

/// `overrides` with each named option set to its literal, or its line
/// removed for `None`. A line is replaced in place, a new one goes before
/// the closing brace. Only lines that hold one assignment are touched; an
/// option set on a line that also sets something else is refused rather
/// than half rewritten.
pub fn patch(overrides: &str, changes: &[(String, Option<String>)]) -> Result<String, Rejected> {
    let mut lines: Vec<String> = overrides.lines().map(str::to_string).collect();
    for (name, raw) in changes {
        let alone = |l: &String| {
            parse_line(l).is_some_and(|(k, _)| k == *name)
                && l.matches('=').count() == 1
                && !l.contains('{')
        };
        let held = |l: &String| {
            crate::overrides::assignments(l)
                .iter()
                .any(|(k, _)| k == name)
        };
        if lines.iter().any(|l| held(l) && !alone(l)) {
            return Err(Rejected(format!(
                "losos.{name} shares its line in overrides.nix with another setting; change it on the Advanced pane"
            )));
        }
        match raw {
            None => lines.retain(|l| !alone(l)),
            Some(raw) => {
                let line = format!("  losos.{name} = {raw};");
                if let Some(i) = lines.iter().position(alone) {
                    lines[i] = line;
                    lines.retain({
                        let mut first = true;
                        move |l| {
                            if alone(l) {
                                let keep = first;
                                first = false;
                                keep
                            } else {
                                true
                            }
                        }
                    });
                } else {
                    match lines.iter().rposition(|l| l.trim() == "}") {
                        Some(i) => lines.insert(i, line),
                        None => {
                            return Err(Rejected(
                                "overrides.nix has no closing brace on a line of its own"
                                    .to_string(),
                            ))
                        }
                    }
                }
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overrides::{lookup_nix, DEFAULT_OVERRIDES_NIX};
    use serde_json::json;

    fn doc() -> OptionsDoc {
        serde_json::from_value(json!({
            "version": 1,
            "options": [
                { "name": "nextcloud.site.trashDays", "group": "nextcloud",
                  "editor": { "kind": "int", "min": 1, "max": 3650 }, "nixType": "int" },
                { "name": "git.site.defaultPrivate", "group": "git",
                  "editor": { "kind": "enum", "values": ["last", "private", "public"] }, "nixType": "enum" },
                { "name": "nextcloud.federation.enable", "group": "nextcloud",
                  "editor": { "kind": "bool" }, "nixType": "bool" },
                { "name": "hostName", "group": "general",
                  "editor": { "kind": "str" }, "nixType": "str" },
                { "name": "git.site.fixedOne", "group": "git",
                  "editor": { "kind": "bool" }, "nixType": "bool", "fixed": "installer" }
            ]
        }))
        .expect("a test document")
    }

    #[test]
    fn only_the_two_apps_settings_are_site_settings() {
        assert!(is_site("nextcloud.site.trashDays"));
        assert!(is_site("git.site.defaultPrivate"));
        assert!(is_site("nextcloud.federation.enable"));
        assert!(!is_site("nextcloud.site."));
        assert!(!is_site("nextcloud.siteX.a"));
        assert!(!is_site("nextcloud.federation.enabled"));
        assert!(!is_site("hostName"));
        assert!(!is_site("sharingMyStorage"));
        assert!(!is_site("nextcloud.mode"));
    }

    #[test]
    fn the_view_carries_only_site_options() {
        let v = view(&doc(), DEFAULT_OVERRIDES_NIX);
        let names: Vec<&str> = v["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["name"].as_str().unwrap())
            .collect();
        assert!(!names.contains(&"hostName"), "{names:?}");
        assert!(names.contains(&"nextcloud.site.trashDays"));
        assert_eq!(v["stray"], json!([]));
    }

    #[test]
    fn each_change_is_checked_by_name_and_kind() {
        let d = doc();
        assert!(check(&d, "nextcloud.site.trashDays", Some("30")).is_ok());
        assert!(check(&d, "nextcloud.site.trashDays", None).is_ok());
        assert!(check(&d, "nextcloud.site.trashDays", Some("0")).is_err());
        assert!(check(
            &d,
            "nextcloud.site.trashDays",
            Some("30; losos.hostName = \"x\"")
        )
        .is_err());
        assert!(check(&d, "git.site.defaultPrivate", Some("\"public\"")).is_ok());
        assert!(check(
            &d,
            "git.site.defaultPrivate",
            Some("\"public\"; losos.hostName = \"x\"")
        )
        .is_err());
        assert!(check(&d, "git.site.defaultPrivate", Some("\"pub\nlic\"")).is_err());
        assert!(check(
            &d,
            "git.site.defaultPrivate",
            Some("\"${builtins.readFile \"/x\"}\"")
        )
        .is_err());
        assert!(check(&d, "hostName", Some("\"x\"")).is_err());
        assert!(check(&d, "nextcloud.site.notDeclared", Some("1")).is_err());
        assert!(check(&d, "git.site.fixedOne", Some("true")).is_err());
    }

    #[test]
    fn a_patch_touches_only_the_named_lines() {
        let one = patch(
            DEFAULT_OVERRIDES_NIX,
            &[("nextcloud.site.trashDays".into(), Some("30".into()))],
        )
        .unwrap();
        assert_eq!(
            lookup_nix("nextcloud.site.trashDays", &one).as_deref(),
            Some("30")
        );
        assert_eq!(lookup_nix("hostName", &one).as_deref(), Some("\"mattbox\""));
        assert!(one.trim_end().ends_with('}'));
        assert_eq!(
            crate::overrides::assignments(&one).len(),
            crate::overrides::assignments(DEFAULT_OVERRIDES_NIX).len() + 1
        );

        let two = patch(
            &one,
            &[("nextcloud.site.trashDays".into(), Some("60".into()))],
        )
        .unwrap();
        assert_eq!(
            lookup_nix("nextcloud.site.trashDays", &two).as_deref(),
            Some("60")
        );
        assert_eq!(two.matches("trashDays").count(), 1);

        let back = patch(&two, &[("nextcloud.site.trashDays".into(), None)]).unwrap();
        assert_eq!(back, DEFAULT_OVERRIDES_NIX);
    }

    #[test]
    fn a_shared_line_is_refused_not_half_rewritten() {
        let body =
            "{ ... }:\n{\n  losos.hostName = \"a\"; losos.nextcloud.site.trashDays = 5;\n}\n";
        assert!(patch(
            body,
            &[("nextcloud.site.trashDays".into(), Some("9".into()))]
        )
        .is_err());
        assert!(patch(body, &[("nextcloud.site.trashDays".into(), None)]).is_err());
    }
}
