//! The names lososd and its plugins both take, in one place so a rule that
//! changes on one side changes on the other. Each check is a plain yes or
//! no; the caller says what was wrong in its own words.

/// Longest release name of an installed app: the namespace prefix lososd
/// puts in front of it still has to fit a DNS label.
pub const MAX_RELEASE_CHARS: usize = 40;
/// Longest owner or repository name on LosOS Git.
pub const MAX_REPO_PART_CHARS: usize = 100;
/// Longest LosOS Cloud path, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;

/// A release name of an app from the Apps search: small letters, digits
/// and dashes, a letter first, no dash last.
#[must_use]
pub fn release(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_RELEASE_CHARS
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// An owner or a repository name on LosOS Git: letters, digits, `-`, `_`
/// and `.`, never `.` or `..` alone.
#[must_use]
pub fn repo_part(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part.chars().count() <= MAX_REPO_PART_CHARS
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A path inside LosOS Cloud: from the top (`/`), no empty, `.` or `..`
/// part, no backslash, nothing that is not printable. `/` itself is one.
#[must_use]
pub fn cloud_path(path: &str) -> bool {
    if !path.starts_with('/') || path.len() > MAX_PATH_BYTES {
        return false;
    }
    if path == "/" {
        return true;
    }
    path.trim_end_matches('/')
        .split('/')
        .skip(1)
        .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains('\\'))
        && !path.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_names() {
        for ok in ["a", "immich", "my-app2", &"a".repeat(MAX_RELEASE_CHARS)] {
            assert!(release(ok), "{ok}");
        }
        for bad in [
            "",
            "2app",
            "app-",
            "App",
            "my_app",
            &"a".repeat(MAX_RELEASE_CHARS + 1),
        ] {
            assert!(!release(bad), "{bad}");
        }
    }

    #[test]
    fn repo_parts() {
        for ok in ["matus", "site", "a.b_c-d", "X1"] {
            assert!(repo_part(ok), "{ok}");
        }
        for bad in [
            "",
            ".",
            "..",
            "a/b",
            "a b",
            &"a".repeat(MAX_REPO_PART_CHARS + 1),
        ] {
            assert!(!repo_part(bad), "{bad}");
        }
    }

    #[test]
    fn cloud_paths() {
        for ok in ["/", "/Documents", "/Documents/", "/a b/plan.pdf"] {
            assert!(cloud_path(ok), "{ok}");
        }
        for bad in ["", "Documents", "//a", "/a/../b", "/./a", "/a\\b", "/a\nb"] {
            assert!(!cloud_path(bad), "{bad:?}");
        }
        assert!(!cloud_path(&format!("/{}", "a".repeat(MAX_PATH_BYTES))));
    }
}
