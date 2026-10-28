//! Account and repository names.
//!
//! A name ends up in a path on disk (`repositories/<owner>/<name>.git`) and
//! in every URL of the pages, so it is checked before anything else looks at
//! it. The rules are close to Forgejo's, which is what existing clones and
//! the configuration repository were made under: letters, digits, `.`, `-`
//! and `_`, no leading dot, and no `.git` at the end of a repository name
//! (that suffix belongs to the clone URL, and is taken off before the
//! check). Case is kept for display; uniqueness and the path ignore it.

/// Longest account name. LosOS Login names are shorter still.
pub const MAX_OWNER: usize = 40;
/// Longest repository name, Forgejo's limit.
pub const MAX_REPO: usize = 100;

/// Names that are the first segment of a page address under `/git/`, so no
/// account may have them (`/git/settings` is the settings page, not the
/// account `settings`).
pub const RESERVED_OWNERS: [&str; 14] = [
    "api",
    "assets",
    "explore",
    "settings",
    "new",
    "admin",
    "internal",
    "login",
    "user",
    "users",
    "issues",
    "pulls",
    "notifications",
    "repo",
];

/// Repository names a page address takes for itself below the account.
pub const RESERVED_REPOS: [&str; 2] = ["settings", "repositories"];

fn shape_ok(name: &str, max: usize) -> Result<(), String> {
    if name.is_empty() {
        return Err("the name is empty".to_string());
    }
    if name.chars().count() > max {
        return Err(format!("the name is longer than {max} characters"));
    }
    if name.starts_with('.') || name.starts_with('-') {
        return Err("the name may not start with a dot or a dash".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err("the name may hold letters, digits, dots, dashes and underscores".to_string());
    }
    if name.contains("..") {
        return Err("the name may not hold two dots in a row".to_string());
    }
    Ok(())
}

/// An account name as LosOS Login or an administrator gave it.
pub fn owner(name: &str) -> Result<&str, String> {
    shape_ok(name, MAX_OWNER)?;
    if RESERVED_OWNERS.contains(&name.to_ascii_lowercase().as_str()) {
        return Err(format!("\"{name}\" is a page of LosOS Git, not an account"));
    }
    Ok(name)
}

/// A repository name, with the `.git` of a clone URL already taken off by
/// [`strip_git`].
pub fn repo(name: &str) -> Result<&str, String> {
    shape_ok(name, MAX_REPO)?;
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".git") {
        return Err("a repository name may not end in .git".to_string());
    }
    // `<name>.wiki.git` is the wiki of `<name>`, as on Forgejo and GitHub.
    if lower.ends_with(".wiki") {
        return Err("a repository name may not end in .wiki".to_string());
    }
    if RESERVED_REPOS.contains(&lower.as_str()) {
        return Err(format!(
            "\"{name}\" is a page of LosOS Git, not a repository"
        ));
    }
    Ok(name)
}

/// `notes.wiki` → `notes`: the repository whose wiki a clone URL names.
pub fn wiki_of(name: &str) -> Option<&str> {
    let n = name.len().checked_sub(5)?;
    (name.is_char_boundary(n) && name[n..].eq_ignore_ascii_case(".wiki")).then(|| &name[..n])
}

/// `losos-config.git` → `losos-config`, as git clients spell a repository.
pub fn strip_git(name: &str) -> &str {
    name.strip_suffix(".git").unwrap_or(name)
}

/// A branch name git would take, checked without running git: what
/// `git check-ref-format --branch` refuses, near enough to keep a name that
/// reaches a command line from being an option or a revision expression.
pub fn branch(name: &str) -> Result<&str, String> {
    let bad = name.is_empty()
        || name.len() > 250
        || name.starts_with('-')
        || name.starts_with('/')
        || name.ends_with('/')
        || name.ends_with('.')
        || name.ends_with(".lock")
        || name.contains("..")
        || name.contains("//")
        || name.contains("@{")
        || name == "@"
        || name.split('/').any(|part| part.starts_with('.'))
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\'));
    if bad {
        Err(format!("\"{name}\" is not a branch name git accepts"))
    } else {
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owners_are_plain_names_and_never_a_page() {
        assert!(owner("notshared").is_ok());
        assert!(owner("Jana.Novak_2").is_ok());
        assert!(owner("").is_err());
        assert!(owner(".hidden").is_err());
        assert!(owner("-flag").is_err());
        assert!(owner("a/b").is_err());
        assert!(owner("a..b").is_err());
        assert!(owner("ján").is_err());
        assert!(owner("settings").is_err());
        assert!(owner("API").is_err());
        assert!(owner(&"a".repeat(MAX_OWNER + 1)).is_err());
    }

    #[test]
    fn repositories_lose_their_git_suffix_first() {
        assert_eq!(strip_git("losos-config.git"), "losos-config");
        assert_eq!(strip_git("losos-config"), "losos-config");
        assert!(repo("losos-config").is_ok());
        assert!(repo("dots.in.name").is_ok());
        assert!(repo("x.git").is_err());
        assert!(repo("notes.wiki").is_err());
        assert!(repo("notes.Wiki").is_err());
        assert_eq!(wiki_of(strip_git("notes.wiki.git")), Some("notes"));
        assert_eq!(wiki_of("Notes.WIKI"), Some("Notes"));
        assert_eq!(wiki_of("notes"), None);
        assert_eq!(wiki_of("wiki"), None);
        assert!(repo("..").is_err());
        assert!(repo("../etc").is_err());
        assert!(repo("settings").is_err());
        assert!(repo("a b").is_err());
    }

    #[test]
    fn branches_cannot_be_options_or_revision_syntax() {
        for good in ["main", "feature/login", "v1.2", "fix_7"] {
            assert!(branch(good).is_ok(), "{good}");
        }
        for bad in [
            "", "-f", "a..b", "x@{1}", "@", "a b", "x~1", "x^", "a:b", "x.lock", "/x", "x/",
            ".hidden", "a/.b",
        ] {
            assert!(branch(bad).is_err(), "{bad}");
        }
    }
}
