//! Each person's name and address from LosOS Login, in LosOS cloud and
//! LosOS Git.
//!
//! Both apps make a person's account at their first sign-in through LosOS
//! Login (`modules/app-sign-in.nix`), with the name nginx passes on, and
//! take the name and address again at every later sign-in. The owner's
//! LosOS Git account is made by this daemon before anyone signs in
//! (`crate::config_repo`), without them. So this daemon sets both apps
//! itself: for everyone when it starts, and for one person when they change
//! their details on their own account.
//!
//! An app that is not up yet is asked again later; an account an app has
//! not made yet is left to that first sign-in.

use crate::losos::Losos;

/// One person as the two apps should show them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Their account in both apps: the owner's own for the person who
    /// claimed the box, their sign-in name for everyone else.
    pub account: String,
    pub name: String,
    pub email: String,
}

/// What the apps should show for `person`.
pub fn profile(person: &crate::people::Listed) -> Profile {
    let account = if person.groups.iter().any(|g| g == crate::people::OWNER) {
        OWNER_ACCOUNT.to_string()
    } else {
        person.name.clone()
    };
    let name = match person.display_name.trim() {
        "" => person.name.clone(),
        shown => shown.to_string(),
    };
    Profile {
        account,
        name,
        email: person.email.trim().to_string(),
    }
}

/// `modules/app-sign-in.nix`, `ownerAccount`.
pub const OWNER_ACCOUNT: &str = "notshared";

/// How one app took a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The app shows it now.
    Set,
    /// The app has no account for the person yet; it makes one, with the
    /// name, at their first sign-in.
    NoAccount,
    /// The app does not run on this box.
    Off,
}

/// Give `who` their name and address in LosOS cloud. `Err` when LosOS cloud
/// could not be asked, which is worth asking again.
pub fn nextcloud<L: Losos>(l: &mut L, who: &Profile) -> anyhow::Result<Outcome> {
    let mode = l.nextcloud_mode()?;
    let target = l.nextcloud_target(mode)?;
    let native = target == crate::setup::Target::Native;
    let steps = crate::setup::plan_person(&target, &who.account, &who.name, &who.email);
    for (i, step) in steps.iter().enumerate() {
        let out = l.nextcloud_run(step)?;
        if out.code == 0 {
            continue;
        }
        if (native && i == 0) || (!native && out.code == crate::setup::NO_ACCOUNT) {
            return Ok(Outcome::NoAccount);
        }
        let why = out
            .stderr
            .lines()
            .chain(out.stdout.lines())
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string();
        anyhow::bail!("occ exited {} for {}: {why}", out.code, who.account);
    }
    Ok(Outcome::Set)
}

/// Give `who` their name and address in LosOS Git, making the account if
/// they have none yet. `Err` when LosOS Git could not be asked, or refused.
pub fn git<L: Losos>(l: &mut L, who: &Profile) -> anyhow::Result<Outcome> {
    let op = crate::config_repo::GitOp::EnsureUser {
        login: who.account.clone(),
        full_name: who.name.clone(),
        email: who.email.clone(),
        admin: None,
    };
    match l.git_request(&op) {
        Ok((code, _)) if (200..300).contains(&code) => Ok(Outcome::Set),
        Ok((code, body)) => {
            let answer = crate::config_repo::classify(code, &body);
            anyhow::bail!("LosOS Git refused {}'s name: {answer:?}", who.account)
        }
        Err(e) if e.is::<crate::config_repo::NotUp>() => Err(e),
        // No LosOS Git on this box (no configuration repository either).
        Err(_) => Ok(Outcome::Off),
    }
}

/// Both apps, for one person. True when nothing is left to do for them.
pub fn sync<L: Losos>(l: &mut L, who: &Profile) -> bool {
    let mut settled = true;
    for (app, outcome) in [
        ("LosOS cloud", nextcloud(l, who)),
        ("LosOS Git", git(l, who)),
    ] {
        if let Err(e) = outcome {
            tracing::info!(app, account = %who.account, error = %e, "a name waits for its app");
            settled = false;
        }
    }
    settled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(name: &str, display: &str, groups: &[&str]) -> crate::people::Listed {
        crate::people::Listed {
            name: name.into(),
            display_name: display.into(),
            email: format!("{name}@example.org"),
            groups: groups.iter().map(ToString::to_string).collect(),
            disabled: false,
        }
    }

    #[test]
    fn the_owner_is_their_own_account_and_a_blank_name_is_the_sign_in_name() {
        let owner = profile(&listed(
            "matus",
            "Matus K",
            &["losos-admins", "losos-owner"],
        ));
        assert_eq!(owner.account, "notshared");
        assert_eq!(owner.name, "Matus K");
        let eva = profile(&listed("eva", "  ", &["losos-backup"]));
        assert_eq!((eva.account.as_str(), eva.name.as_str()), ("eva", "eva"));
        assert_eq!(eva.email, "eva@example.org");
    }

    #[test]
    fn the_name_goes_to_both_apps_and_never_as_an_option() {
        let mut l = crate::fake::FakeLosos::new();
        l.git_routes
            .insert("PUT /api/v1/admin/users/eva".into(), (200, "{}".into()));
        let who = profile(&listed("eva", "--help", &[]));
        assert!(sync(&mut l, &who));
        assert!(!l.nextcloud_runs.is_empty());
        for step in &l.nextcloud_runs {
            let dashes = step.iter().position(|a| a == "--").expect("options end");
            let name = step.iter().position(|a| a == "--help");
            assert!(name.is_none_or(|n| n > dashes), "{step:?}");
        }
        assert!(l.git_calls.iter().any(|k| k == "PUT /api/v1/admin/users/eva"));
    }

    #[test]
    fn a_missing_account_waits_for_the_first_sign_in_and_a_down_app_is_retried() {
        let mut l = crate::fake::FakeLosos::new();
        l.nextcloud_run_exit = crate::setup::NO_ACCOUNT;
        l.git_routes
            .insert("PUT /api/v1/admin/users/eva".into(), (200, "{}".into()));
        let who = profile(&listed("eva", "Eva", &[]));
        assert_eq!(nextcloud(&mut l, &who).unwrap(), Outcome::NoAccount);
        assert_eq!(git(&mut l, &who).unwrap(), Outcome::Set);
        assert!(sync(&mut l, &who));
        // The fake LosOS Git answers nothing else: not up, so asked again.
        let oma = profile(&listed("oma", "Oma", &[]));
        assert!(!sync(&mut l, &oma));
    }

    #[test]
    fn git_gets_the_name_and_keeps_an_address_it_was_not_given() {
        let op = |email: &str| crate::config_repo::GitOp::EnsureUser {
            login: "eva".into(),
            full_name: "Eva N".into(),
            email: email.into(),
            admin: None,
        };
        let body: serde_json::Value =
            serde_json::from_str(&op("eva@example.org").body().unwrap()).unwrap();
        assert_eq!(body["full_name"], "Eva N");
        assert_eq!(body["email"], "eva@example.org");
        // An empty address is sent empty, and LosOS Git keeps what it had.
        let body: serde_json::Value = serde_json::from_str(&op("").body().unwrap()).unwrap();
        assert_eq!(body["email"], "");
    }
}
