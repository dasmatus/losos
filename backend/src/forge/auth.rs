//! Who is calling, and what they may do.
//!
//! A caller is one of three things. The site administrator holds the admin
//! token (lososd, through `$LOSOS_GIT_TOKEN_FILE`). A person is named by
//! nginx in `X-Losos-Cloud-User` after LosOS Login vouched for them (on the
//! socket only nginx can open), or holds one of their own access tokens.
//! Nobody is everyone else, who sees public repositories unless the box
//! asks for a sign-in first.
//!
//! A person signed in through LosOS Login may do everything their account
//! may. A token may do what its scopes say on top of that, and never more:
//! `write:repository` pushes, `read:repository` clones, and so on.

use super::db::{Permission, Repo, Store, Token, User, Visibility};

/// The scope families a token can carry, as `read:<family>` or
/// `write:<family>`, or `all`.
pub const SCOPE_FAMILIES: [&str; 4] = ["repository", "user", "issue", "admin"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Who {
    Nobody,
    Person(User),
    /// The admin token. lososd uses it to make accounts and repositories.
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub who: Who,
    /// `None` for a person LosOS Login signed in, and for the administrator:
    /// no token limits them. A token's scopes otherwise.
    pub scopes: Option<Vec<String>>,
}

impl Caller {
    pub fn nobody() -> Caller {
        Caller {
            who: Who::Nobody,
            scopes: None,
        }
    }

    pub fn person(user: User) -> Caller {
        Caller {
            who: Who::Person(user),
            scopes: None,
        }
    }

    pub fn with_token(user: User, token: &Token) -> Caller {
        Caller {
            who: Who::Person(user),
            scopes: Some(token.scopes.clone()),
        }
    }

    pub fn admin() -> Caller {
        Caller {
            who: Who::Admin,
            scopes: None,
        }
    }

    pub fn user(&self) -> Option<&User> {
        match &self.who {
            Who::Person(u) => Some(u),
            _ => None,
        }
    }

    pub fn is_site_admin(&self) -> bool {
        match &self.who {
            Who::Admin => true,
            Who::Person(u) => u.is_admin && self.allows("admin", true),
            Who::Nobody => false,
        }
    }

    /// Whether the token (if any) lets this caller read or write `family`.
    pub fn allows(&self, family: &str, write: bool) -> bool {
        match &self.scopes {
            None => true,
            Some(scopes) => scopes_allow(scopes, family, write),
        }
    }

    /// What this caller may do in `repo`, before the token's scopes. A
    /// public repository reads for anyone, an internal one for every
    /// account on the box; past that it is ownership and collaboration.
    pub fn permission(&self, store: &Store, repo: &Repo) -> anyhow::Result<Permission> {
        let read_if = |open: bool| {
            if open {
                Permission::Read
            } else {
                Permission::None
            }
        };
        Ok(match &self.who {
            Who::Admin => Permission::Admin,
            Who::Nobody => read_if(repo.visibility == Visibility::Public),
            Who::Person(u) if u.id == repo.owner_id || u.is_admin => Permission::Admin,
            Who::Person(u) => store
                .collaborator(repo.id, u.id)?
                .max(read_if(repo.visibility != Visibility::Private)),
        })
    }

    /// What this caller may do in `repo` with the code (clone, push,
    /// settings), the token's scopes applied.
    pub fn repo_access(&self, store: &Store, repo: &Repo) -> anyhow::Result<Permission> {
        let p = self.permission(store, repo)?;
        Ok(
            if p >= Permission::Write && !self.allows("repository", true) {
                if self.allows("repository", false) {
                    Permission::Read
                } else {
                    Permission::None
                }
            } else if p >= Permission::Read && !self.allows("repository", false) {
                Permission::None
            } else {
                p
            },
        )
    }
}

/// Whether `scopes` grant `family` (writing implies reading).
pub fn scopes_allow(scopes: &[String], family: &str, write: bool) -> bool {
    scopes.iter().any(|s| {
        s == "all" || *s == format!("write:{family}") || (!write && *s == format!("read:{family}"))
    })
}

/// A scope list as a person asked for it, checked.
pub fn validate_scopes(scopes: &[String]) -> Result<Vec<String>, String> {
    if scopes.is_empty() {
        return Err("a token needs at least one scope".to_string());
    }
    for s in scopes {
        let ok = s == "all"
            || s.split_once(':').is_some_and(|(level, family)| {
                matches!(level, "read" | "write") && SCOPE_FAMILIES.contains(&family)
            });
        if !ok {
            return Err(format!("\"{s}\" is not a scope"));
        }
    }
    let mut out = scopes.to_vec();
    out.sort();
    out.dedup();
    Ok(out)
}

/// What an `Authorization` header offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    /// `Basic`: git sends the user name and, from a credential helper, the
    /// token as the password.
    Basic { user: String, password: String },
    /// `token <value>` or `Bearer <value>`.
    Token(String),
}

pub fn parse_authorization(header: &str) -> Option<Credential> {
    let (scheme, rest) = header.trim().split_once(' ')?;
    let rest = rest.trim();
    match scheme.to_ascii_lowercase().as_str() {
        "basic" => {
            use base64::Engine;
            let raw = base64::engine::general_purpose::STANDARD
                .decode(rest)
                .ok()?;
            let text = String::from_utf8(raw).ok()?;
            let (user, password) = text.split_once(':')?;
            Some(Credential::Basic {
                user: user.to_string(),
                password: password.to_string(),
            })
        }
        "token" | "bearer" if !rest.is_empty() => Some(Credential::Token(rest.to_string())),
        _ => None,
    }
}

/// Why a request was turned away before it reached a repository.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// Credentials were offered and were wrong: 401, never "nobody".
    #[error("the token is not one this box made")]
    BadCredentials,
    /// The account the header names cannot be an account here.
    #[error("{0}")]
    BadName(String),
}

/// Turn what a request carries into a caller.
///
/// `header_user` is the name nginx passed, believed only when `trusted`
/// (the request came in on the socket only nginx opens). `admin_hash` is
/// the hash of the admin token. `act_as` is `X-Losos-As`, which only the
/// administrator may use, to act as a person.
pub fn resolve(
    store: &Store,
    admin_hash: &str,
    trusted: bool,
    header_user: Option<(&str, &str, &str)>,
    authorization: Option<&str>,
    act_as: Option<&str>,
) -> anyhow::Result<Result<Caller, Refused>> {
    if let Some(cred) = authorization.and_then(parse_authorization) {
        // A Basic header from git carries the token as the password, but
        // some tools put it in the user name instead; try both.
        let candidates: Vec<&str> = match &cred {
            Credential::Token(t) => vec![t.as_str()],
            Credential::Basic { user, password } => vec![password.as_str(), user.as_str()],
        };
        for value in candidates.into_iter().filter(|v| !v.is_empty()) {
            if super::db::token_hash(value) == admin_hash {
                return Ok(Ok(match act_as.filter(|s| !s.is_empty()) {
                    Some(login) => match store.user(login)? {
                        Some(u) => Caller::person(u),
                        None => return Ok(Err(Refused::BadName(format!("no account {login}")))),
                    },
                    None => Caller::admin(),
                }));
            }
            if let Some((user, token)) = store.token_owner(value)? {
                return Ok(Ok(Caller::with_token(user, &token)));
            }
        }
        // A Basic header that is not a token is a person typing a password
        // where a token belongs; a header nginx vouched for still names them.
        if !trusted || header_user.is_none() {
            return Ok(Err(Refused::BadCredentials));
        }
    }
    if trusted {
        if let Some((login, name, email)) = header_user.filter(|(l, _, _)| !l.is_empty()) {
            return Ok(match store.ensure_user(login, name, email) {
                Ok(u) => Ok(Caller::person(u)),
                Err(e) => Err(Refused::BadName(e.to_string())),
            });
        }
    }
    Ok(Ok(Caller::nobody()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic(user: &str, password: &str) -> String {
        use base64::Engine;
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
        )
    }

    #[test]
    fn scopes_are_families_and_write_implies_read() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(scopes_allow(&s(&["write:repository"]), "repository", false));
        assert!(scopes_allow(&s(&["write:repository"]), "repository", true));
        assert!(!scopes_allow(&s(&["read:repository"]), "repository", true));
        assert!(!scopes_allow(&s(&["read:user"]), "repository", false));
        assert!(scopes_allow(&s(&["all"]), "admin", true));
        assert!(validate_scopes(&s(&["write:repository", "read:user"])).is_ok());
        assert!(validate_scopes(&s(&[])).is_err());
        assert!(validate_scopes(&s(&["write:everything"])).is_err());
        assert!(validate_scopes(&s(&["sudo"])).is_err());
    }

    #[test]
    fn headers_parse_both_ways_git_and_tools_send_them() {
        assert_eq!(
            parse_authorization(&basic("notshared", "abc")),
            Some(Credential::Basic {
                user: "notshared".into(),
                password: "abc".into()
            })
        );
        assert_eq!(
            parse_authorization("token abc"),
            Some(Credential::Token("abc".into()))
        );
        assert_eq!(
            parse_authorization("Bearer abc"),
            Some(Credential::Token("abc".into()))
        );
        assert_eq!(parse_authorization("Digest x"), None);
        assert_eq!(parse_authorization("Basic !!!"), None);
    }

    #[test]
    fn callers_resolve_by_token_header_or_nothing() {
        let store = Store::in_memory().unwrap();
        let admin = super::super::db::token_hash("admintoken");
        let u = store.ensure_user("notshared", "", "").unwrap();
        let (_, value) = store
            .create_token(u.id, "laptop", &["read:repository".into()])
            .unwrap();

        let c = resolve(
            &store,
            &admin,
            false,
            None,
            Some(&basic("notshared", &value)),
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(c.user().unwrap().login, "notshared");
        assert!(c.allows("repository", false) && !c.allows("repository", true));

        let c = resolve(
            &store,
            &admin,
            false,
            None,
            Some(&basic(&value, "x-oauth-basic")),
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(c.user().unwrap().login, "notshared");

        assert_eq!(
            resolve(
                &store,
                &admin,
                false,
                None,
                Some(&basic("notshared", "wrong")),
                None
            )
            .unwrap(),
            Err(Refused::BadCredentials)
        );

        let c = resolve(&store, &admin, false, None, Some("token admintoken"), None)
            .unwrap()
            .unwrap();
        assert!(c.is_site_admin());
        let c = resolve(
            &store,
            &admin,
            false,
            None,
            Some("token admintoken"),
            Some("notshared"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(c.user().unwrap().login, "notshared");

        // The header counts only where nginx is the one who sent it.
        let header = Some(("jana", "Jana Novak", "jana@example.org"));
        assert_eq!(
            resolve(&store, &admin, false, header, None, None)
                .unwrap()
                .unwrap(),
            Caller::nobody()
        );
        let c = resolve(&store, &admin, true, header, None, None)
            .unwrap()
            .unwrap();
        assert_eq!(c.user().unwrap().full_name, "Jana Novak");
        assert!(
            store.user("jana").unwrap().is_some(),
            "the first sign-in makes the account"
        );
        assert_eq!(
            resolve(&store, &admin, true, None, None, None)
                .unwrap()
                .unwrap(),
            Caller::nobody()
        );
    }

    #[test]
    fn permission_follows_ownership_collaboration_and_visibility() {
        let store = Store::in_memory().unwrap();
        let owner = store.ensure_user("notshared", "", "").unwrap();
        let jana = store.ensure_user("jana", "", "").unwrap();
        let repo = store
            .create_repo(owner.id, "r", "", Visibility::Private, "main", None)
            .unwrap();
        let p = |c: &Caller| c.permission(&store, &repo).unwrap();
        assert_eq!(p(&Caller::person(owner.clone())), Permission::Admin);
        assert_eq!(p(&Caller::person(jana.clone())), Permission::None);
        assert_eq!(p(&Caller::nobody()), Permission::None);
        store
            .set_collaborator(repo.id, jana.id, Permission::Write)
            .unwrap();
        assert_eq!(p(&Caller::person(jana.clone())), Permission::Write);
        let public = store
            .create_repo(owner.id, "p", "", Visibility::Public, "main", None)
            .unwrap();
        assert_eq!(
            Caller::nobody().permission(&store, &public).unwrap(),
            Permission::Read
        );
        // A read-only token cannot push even to the caller's own repository.
        let (t, _) = store
            .create_token(owner.id, "t", &["read:repository".into()])
            .unwrap();
        let c = Caller::with_token(owner.clone(), &t);
        assert_eq!(c.repo_access(&store, &repo).unwrap(), Permission::Read);
        let (t, _) = store
            .create_token(jana.id, "u", &["read:user".into()])
            .unwrap();
        let c = Caller::with_token(jana.clone(), &t);
        assert_eq!(c.repo_access(&store, &public).unwrap(), Permission::None);
        // Internal: every account on the box reads it, nobody else does.
        let ours = store
            .create_repo(owner.id, "i", "", Visibility::Internal, "main", None)
            .unwrap();
        let eva = store.ensure_user("eva", "", "").unwrap();
        assert_eq!(
            Caller::person(eva).permission(&store, &ours).unwrap(),
            Permission::Read
        );
        assert_eq!(
            Caller::nobody().permission(&store, &ours).unwrap(),
            Permission::None
        );
        store
            .set_collaborator(ours.id, jana.id, Permission::Write)
            .unwrap();
        assert_eq!(
            Caller::person(jana).permission(&store, &ours).unwrap(),
            Permission::Write
        );
    }
}
