//! Each person's name and address from LosOS Login, in LosOS Git.
//!
//! Both apps make a person's account at their first sign-in through LosOS
//! Login (`modules/app-sign-in.nix`), with the name nginx passes on. LosOS
//! Cloud takes the name and address again with every request; LosOS Git
//! never does, and the owner's LosOS Git account, which this daemon makes
//! before anyone signs in (`crate::config_repo`), never had them. So this
//! daemon sets them in LosOS Git itself: for everyone when it starts, and
//! for one person when they change their details on their own account.
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

/// Give `who` their name and address in LosOS Git. `Err` when LosOS Git
/// could not be asked, or refused.
pub fn forgejo<L: Losos>(l: &mut L, who: &Profile) -> anyhow::Result<Outcome> {
    let op = crate::config_repo::ForgejoOp::SetProfile {
        login: who.account.clone(),
        full_name: who.name.clone(),
        email: who.email.clone(),
    };
    match l.forgejo_request(&op, None) {
        Ok((code, _)) if (200..300).contains(&code) => Ok(Outcome::Set),
        Ok((404, _)) => Ok(Outcome::NoAccount),
        Ok((code, body)) => {
            let answer = crate::config_repo::classify(code, &body);
            anyhow::bail!("LosOS Git refused {}'s name: {answer:?}", who.account)
        }
        Err(e) if e.is::<crate::config_repo::NotUp>() => Err(e),
        // No LosOS Git on this box (no configuration repository either).
        Err(_) => Ok(Outcome::Off),
    }
}

/// One person. True when nothing is left to do for them.
pub fn sync<L: Losos>(l: &mut L, who: &Profile) -> bool {
    match forgejo(l, who) {
        Ok(_) => true,
        Err(e) => {
            tracing::info!(app = "LosOS Git", account = %who.account, error = %e, "a name waits for its app");
            false
        }
    }
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
    fn the_name_goes_to_losos_git() {
        let mut l = crate::fake::FakeLosos::new();
        l.forgejo_routes
            .insert("PATCH /api/v1/admin/users/eva".into(), (200, "{}".into()));
        let who = profile(&listed("eva", "--help", &[]));
        assert!(sync(&mut l, &who));
        assert!(l
            .forgejo_calls
            .iter()
            .any(|(k, _)| k == "PATCH /api/v1/admin/users/eva"));
    }

    #[test]
    fn a_missing_account_waits_for_the_first_sign_in_and_a_down_app_is_retried() {
        let mut l = crate::fake::FakeLosos::new();
        l.forgejo_routes.insert(
            "PATCH /api/v1/admin/users/eva".into(),
            (404, r#"{"message":"user does not exist"}"#.into()),
        );
        let who = profile(&listed("eva", "Eva", &[]));
        assert_eq!(forgejo(&mut l, &who).unwrap(), Outcome::NoAccount);
        assert!(sync(&mut l, &who));
        // The fake LosOS Git answers nothing else: not up, so asked again.
        let oma = profile(&listed("oma", "Oma", &[]));
        assert!(!sync(&mut l, &oma));
    }

    #[test]
    fn forgejo_gets_the_name_and_keeps_an_address_it_was_not_given() {
        let op = |email: &str| crate::config_repo::ForgejoOp::SetProfile {
            login: "eva".into(),
            full_name: "Eva N".into(),
            email: email.into(),
        };
        let body: serde_json::Value =
            serde_json::from_str(&op("eva@example.org").body(None).unwrap().unwrap()).unwrap();
        assert_eq!(body["full_name"], "Eva N");
        assert_eq!(body["email"], "eva@example.org");
        assert_eq!(body["login_name"], "eva");
        let body: serde_json::Value =
            serde_json::from_str(&op("").body(None).unwrap().unwrap()).unwrap();
        assert!(body.get("email").is_none(), "{body}");
    }
}
