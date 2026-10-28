//! Who is asking: the person LosOS Login signed in.
//!
//! nginx asks LosOS Login before every request under `/calls/api/` and hands
//! the SFU the person it named as `Remote-User` (`modules/calls.nix`), on a
//! socket only nginx can open. That person's Matrix id on this box is
//! `@<name>:<server>`, the id lososd signs chat in with
//! (`backend/src/chat.rs`). A guest's socket gets an empty `Remote-User`
//! and comes in on its invite alone.
//!
//! The SFU admits only people of this box, so a Matrix OpenID token would
//! only repeat what LosOS Login already said.

use super::admit::{Refusal, UserId};

pub trait Resolve: Send + Sync {
    /// The Matrix id of the person nginx named, if it named one.
    fn resolve(&self, remote_user: Option<&str>) -> Result<UserId, Refusal>;
}

/// People of this box's homeserver, as LosOS Login names them.
pub struct Login {
    pub server_name: String,
}

impl Resolve for Login {
    fn resolve(&self, remote_user: Option<&str>) -> Result<UserId, Refusal> {
        let name = remote_user
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .ok_or(Refusal::Unauthenticated)?;
        if name.contains(':') || name.contains('@') {
            return Err(Refusal::Unauthenticated);
        }
        UserId::parse(&format!("@{name}:{}", self.server_name))
            .map_err(|_| Refusal::Unauthenticated)
    }
}

/// The `Remote-User` header of `req`, as nginx set it.
pub fn remote_user(req: &actix_web::HttpRequest) -> Option<&str> {
    req.headers()
        .get("Remote-User")
        .and_then(|v| v.to_str().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login() -> Login {
        Login {
            server_name: "b".into(),
        }
    }

    #[test]
    fn the_signed_in_person_is_their_matrix_id() {
        assert_eq!(login().resolve(Some("eva")).unwrap().as_str(), "@eva:b");
    }

    #[test]
    fn nobody_named_is_unauthenticated() {
        assert_eq!(login().resolve(None), Err(Refusal::Unauthenticated));
        assert_eq!(login().resolve(Some("")), Err(Refusal::Unauthenticated));
        assert_eq!(login().resolve(Some("  ")), Err(Refusal::Unauthenticated));
    }

    #[test]
    fn a_name_cannot_reach_another_server_or_person() {
        for bad in [
            "eva:elsewhere",
            "@eva:b",
            "Eva",
            "e va",
            "eva\r\nx",
            &"a".repeat(300),
        ] {
            assert_eq!(
                login().resolve(Some(bad)),
                Err(Refusal::Unauthenticated),
                "{bad}"
            );
        }
    }
}
