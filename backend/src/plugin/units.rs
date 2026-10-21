//! The `units` import: transient systemd units under the manifest's prefix,
//! running programs the manifest lists.
//!
//! The host writes the `systemd-run` line itself. A plugin chooses the
//! name, the program and its arguments, `--setenv`s and a few properties;
//! it cannot pick who the unit runs as or slip in an `ExecStart=` of its
//! own, which would make the program list meaningless.

use super::bindings::losos::plugin::types::Error;
use super::bindings::losos::plugin::units::{Spec, UnitState};
use super::manifest::{FileRoot, Units};

/// Properties the host keeps for itself.
fn forbidden_property(key: &str) -> bool {
    key.starts_with("Exec")
        || matches!(
            key,
            "User"
                | "Group"
                | "DynamicUser"
                | "SupplementaryGroups"
                | "AmbientCapabilities"
                | "CapabilityBoundingSet"
                | "PAMName"
                | "RootDirectory"
                | "RootImage"
                | "BindPaths"
                | "BindReadOnlyPaths"
                | "LoadCredential"
                | "LoadCredentialEncrypted"
                | "SetCredential"
                | "StandardOutput"
                | "StandardError"
                | "StandardInput"
        )
}

fn check_name(units: &Units, name: &str) -> Result<(), Error> {
    let rest = name.strip_prefix(&units.prefix).unwrap_or("");
    if rest.is_empty()
        || name.len() > 200
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.@".contains(&b))
        || name.ends_with(".service")
    {
        return Err(Error::Denied(format!(
            "{name} is not a unit name under {}",
            units.prefix
        )));
    }
    Ok(())
}

/// The `systemd-run` arguments for `spec`, or why not.
pub fn launch_args(
    units: Option<&Units>,
    roots: &[FileRoot],
    spec: &Spec,
) -> Result<Vec<String>, Error> {
    let units = units.ok_or_else(|| Error::Denied("the manifest grants no units".to_string()))?;
    check_name(units, &spec.name)?;
    let program = spec
        .argv
        .first()
        .ok_or_else(|| Error::Denied("a unit needs a program".to_string()))?;
    if !units.programs.iter().any(|p| p == program) {
        return Err(Error::Denied(format!(
            "{program} is not on the manifest's units list"
        )));
    }
    let mut args = vec![
        format!("--unit={}", spec.name),
        format!("--description=losos {}", spec.name),
        // A bare program is resolved against lososd's PATH, and the unit
        // runs with that PATH too (supervisor.rs says why).
        "--setenv=PATH".to_string(),
    ];
    for prop in &spec.properties {
        let (key, value) = prop
            .split_once('=')
            .ok_or_else(|| Error::Denied(format!("{prop:?} is not Key=Value")))?;
        if forbidden_property(key)
            || !key.bytes().all(|b| b.is_ascii_alphanumeric())
            || value.contains('\n')
        {
            return Err(Error::Denied(format!("{key}= is lososd's to set")));
        }
        args.push(format!("--property={prop}"));
    }
    for (k, v) in &spec.env {
        if k.is_empty()
            || k == "PATH"
            || k.starts_with("LD_")
            || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || v.contains('\n')
        {
            return Err(Error::Denied(format!("{k} cannot be set for a unit")));
        }
        args.push(format!("--setenv={k}={v}"));
    }
    if let Some(log) = &spec.log {
        let allowed = super::files::resolve(roots, log, true)?;
        let log = allowed.path.display();
        args.push(format!("--property=StandardOutput=append:{log}"));
        args.push(format!("--property=StandardError=append:{log}"));
    }
    if spec.argv.iter().any(|a| a.contains('\0')) {
        return Err(Error::Denied("an argument holds a NUL".to_string()));
    }
    // Every option before the program, or systemd-run hands them to it.
    args.extend(spec.argv.iter().cloned());
    Ok(args)
}

pub fn start(units: Option<&Units>, roots: &[FileRoot], spec: &Spec) -> Result<(), Error> {
    let args = launch_args(units, roots, spec)?;
    let out = std::process::Command::new("systemd-run")
        .args(&args)
        .output()
        .map_err(|e| Error::Failed(format!("running systemd-run: {e}")))?;
    if !out.status.success() {
        return Err(Error::Failed(format!(
            "systemd-run exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// `systemctl show -p LoadState -p ActiveState -p ExecMainStatus`, read.
pub fn classify(load: &str, active: &str, status: &str) -> UnitState {
    if load.trim() == "not-found" {
        return UnitState::Absent;
    }
    match active.trim() {
        "active" | "activating" | "deactivating" | "reloading" => UnitState::Running,
        "failed" => UnitState::Failed,
        "inactive" => match status.trim().parse::<i32>() {
            Ok(0) => UnitState::Succeeded,
            Ok(_) => UnitState::Failed,
            Err(_) => UnitState::Absent,
        },
        _ => UnitState::Absent,
    }
}

pub fn state(units: Option<&Units>, name: &str) -> Result<UnitState, Error> {
    let units = units.ok_or_else(|| Error::Denied("the manifest grants no units".to_string()))?;
    check_name(units, name)?;
    let out = std::process::Command::new("systemctl")
        .args([
            "show",
            &format!("{name}.service"),
            "--value",
            "-p",
            "LoadState",
            "-p",
            "ActiveState",
            "-p",
            "ExecMainStatus",
        ])
        .output()
        .map_err(|e| Error::Failed(format!("running systemctl: {e}")))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    match (lines.next(), lines.next(), lines.next()) {
        (Some(load), Some(active), Some(status)) => Ok(classify(load, active, status)),
        _ => Err(Error::Failed(format!("systemctl show {name}: {text:?}"))),
    }
}

pub fn stop(units: Option<&Units>, name: &str) -> Result<(), Error> {
    let units = units.ok_or_else(|| Error::Denied("the manifest grants no units".to_string()))?;
    check_name(units, name)?;
    let status = std::process::Command::new("systemctl")
        .args(["stop", "--no-block", &format!("{name}.service")])
        .status()
        .map_err(|e| Error::Failed(format!("running systemctl stop: {e}")))?;
    if !status.success() {
        return Err(Error::Failed(format!("systemctl stop exited {status}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::Access;

    fn units() -> Units {
        Units {
            prefix: "losos-backup-".to_string(),
            programs: vec!["/nix/store/x-backup/bin/backup".to_string()],
        }
    }

    fn spec(name: &str, argv: &[&str]) -> Spec {
        Spec {
            name: name.to_string(),
            argv: argv.iter().map(|a| a.to_string()).collect(),
            properties: vec![],
            env: vec![],
            log: None,
        }
    }

    #[test]
    fn a_unit_is_launched_with_options_before_the_program() {
        let dir = std::env::temp_dir().join(format!("losos-units-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.display().to_string();
        let roots = vec![FileRoot {
            path: dir.clone(),
            access: Access::Rw,
            chown: vec![],
        }];
        let mut s = spec(
            "losos-backup-1",
            &["/nix/store/x-backup/bin/backup", "--now"],
        );
        s.properties = vec!["EnvironmentFile=/var/lib/losos/backup/env".to_string()];
        s.env = vec![("RESTIC_PASSWORD_FILE".to_string(), "/x".to_string())];
        s.log = Some(format!("{dir}/log"));
        let args = launch_args(Some(&units()), &roots, &s).unwrap();
        let at = args
            .iter()
            .position(|a| a == "/nix/store/x-backup/bin/backup")
            .unwrap();
        assert!(args[..at].iter().all(|a| a.starts_with("--")), "{args:?}");
        assert_eq!(args[at + 1], "--now");
        assert!(args.contains(&"--unit=losos-backup-1".to_string()));
        assert!(args.contains(&"--setenv=RESTIC_PASSWORD_FILE=/x".to_string()));
        assert!(args.contains(&format!("--property=StandardOutput=append:{dir}/log")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn what_the_manifest_does_not_grant_is_denied() {
        let u = units();
        let ok = "/nix/store/x-backup/bin/backup";
        for s in [
            spec("losos-rebuild-1", &[ok]),
            spec("losos-backup-", &[ok]),
            spec("losos-backup-1.service", &[ok]),
            spec("losos-backup-a b", &[ok]),
            spec("losos-backup-1", &["/bin/sh", "-c", "id"]),
            spec("losos-backup-1", &[]),
        ] {
            assert!(
                matches!(launch_args(Some(&u), &[], &s), Err(Error::Denied(_))),
                "{s:?}"
            );
        }
        for prop in [
            "ExecStartPre=/bin/sh",
            "User=root",
            "DynamicUser=yes",
            "StandardOutput=file:/etc/shadow",
            "Nonsense",
            "Description=a\nExecStart=x",
        ] {
            let mut s = spec("losos-backup-1", &[ok]);
            s.properties = vec![prop.to_string()];
            assert!(
                matches!(launch_args(Some(&u), &[], &s), Err(Error::Denied(_))),
                "{prop}"
            );
        }
        let mut s = spec("losos-backup-1", &[ok]);
        s.log = Some("/etc/shadow".to_string());
        assert!(matches!(
            launch_args(Some(&u), &[], &s),
            Err(Error::Denied(_))
        ));
        assert!(matches!(
            launch_args(None, &[], &spec("losos-backup-1", &[ok])),
            Err(Error::Denied(_))
        ));
    }

    #[test]
    fn states_are_read_from_systemctl_show() {
        assert_eq!(classify("loaded", "active", "0"), UnitState::Running);
        assert_eq!(classify("loaded", "inactive", "0"), UnitState::Succeeded);
        assert_eq!(classify("loaded", "inactive", "3"), UnitState::Failed);
        assert_eq!(classify("loaded", "failed", "0"), UnitState::Failed);
        assert_eq!(classify("not-found", "inactive", "0"), UnitState::Absent);
    }
}
