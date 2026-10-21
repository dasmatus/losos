//! The owner's password: the rules for a new one, and the wrapper that
//! keeps it out of logs.
//!
//! The password lives in LosOS Login's people file (`crate::people`), which
//! lososd alone writes. `POST /api/setup/claim` sets the first one and
//! `POST /api/set-password` changes it; LosOS Cloud has no password of its
//! own for the owner and believes the person nginx names
//! (`modules/cloud.nix`).
//!
//! One rule holds everywhere a password is handled here: it never appears
//! in an argv. `/proc/<pid>/cmdline` is world-readable, so a password
//! travels as a [`Secret`] beside a request, never inside a command line.

/// The account that holds the owner's files in LosOS Cloud, matching the
/// uid-1000 data domain in `modules/configuration.nix`
/// (`modules/app-sign-in.nix`).
pub const DEFAULT_ADMIN_USER: &str = "notshared";

/// Shortest password this daemon will set.
///
/// Deliberately a server-side rule rather than a browser one: the admin
/// HTTP API is reachable by anything that holds the admin token, and the
/// wizard's `<input>` is not a boundary.
///
/// Length is not the only rule. The one password opens the admin pages
/// (`POST /api/apply` and a factory reset sit behind it, not only the
/// owner's files), so [`validate_password`] also wants both letter cases, a digit and a symbol. Four rules, all shown up
/// front by the wizard with a tick each, and nothing hidden: an owner who
/// meets what is on screen is never refused by a rule they did not see. A
/// symbol here is any character that is not a letter, a digit or
/// whitespace, so a hyphen between two words counts and nothing has to be
/// hunted for on a phone keyboard.
pub const MIN_PASSWORD_CHARS: usize = 12;

/// Longest password accepted, in bytes.
///
/// Not a security limit, a cap on what gets handed to a password hasher.
pub const MAX_PASSWORD_BYTES: usize = 256;

/// A password, wrapped so it cannot be printed by accident.
///
/// The `Debug` impl redacts, and there is deliberately no `Display`, no
/// `Serialize` and no `Deref`. `crate::http::run` logs failures as
/// `tracing::error!(error = ?e, …)`, and `dbus::reply` renders the whole
/// context chain into the error message: a `String` password caught up in
/// either would land in the journal or on the bus.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wrap a password the caller has already checked. Crate-private so the
    /// two validators ([`validate_password`] for a new password,
    /// [`crate::signin::validate_candidate`] for one being tried) stay the
    /// only ways to make one.
    pub(crate) fn new(password: &str) -> Secret {
        Secret(password.to_string())
    }

    /// The plaintext, for hashing it or putting it in a Basic header.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// Reject a user name that could be read as a flag, or that could not be an
/// account name.
pub fn validate_user(user: &str) -> Result<&str, String> {
    if user.is_empty() {
        return Err("user must not be empty".to_string());
    }
    if user.len() > 64 {
        return Err("user must be at most 64 characters".to_string());
    }
    if user.starts_with('-') {
        return Err(format!(
            "user must not start with '-' ({user:?} would be read as a flag)"
        ));
    }
    if !user
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'@'))
    {
        return Err(format!(
            "user may only contain letters, digits and . _ - @ (got {user:?})"
        ));
    }
    Ok(user)
}

/// Check a candidate password and wrap it.
///
/// No error here ever quotes the password: these messages travel to the
/// journal, to the D-Bus error name and to the SPA.
pub fn validate_password(password: &str) -> Result<Secret, String> {
    // Characters, not bytes, so a passphrase in a non-Latin script is measured
    // the way its owner would count it.
    let chars = password.chars().count();
    if chars < MIN_PASSWORD_CHARS {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_CHARS} characters (got {chars})"
        ));
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(format!(
            "password must be at most {MAX_PASSWORD_BYTES} bytes"
        ));
    }
    // A control character would end a header line early on the way to
    // LosOS Git, and none of them can be typed back reliably on a phone.
    if let Some(c) = password.chars().find(|c| c.is_control()) {
        return Err(format!(
            "password must not contain control characters (found U+{:04X})",
            c as u32
        ));
    }
    // The three composition rules, each with its own sentence so the wizard
    // can show the one that is missing. Unicode-aware on purpose: `Ž` is an
    // upper-case letter and `ß` a lower-case one, and an owner typing on a
    // Slovak or German keyboard should not be told otherwise.
    if !password.chars().any(char::is_lowercase) {
        return Err("password must contain a lower-case letter".to_string());
    }
    if !password.chars().any(char::is_uppercase) {
        return Err("password must contain an upper-case letter".to_string());
    }
    if !password.chars().any(char::is_numeric) {
        return Err("password must contain a digit".to_string());
    }
    if !password.chars().any(is_symbol) {
        return Err("password must contain a symbol (anything that is not a letter or a digit, such as - or !)".to_string());
    }
    Ok(Secret(password.to_string()))
}

/// A "special character" for the password rule: not a letter, not a digit,
/// not whitespace. Punctuation and symbols in any script, so `-`, `!`, `€`
/// and `…` all count, and a space does not — a space separates the words
/// the hint recommends and should not satisfy a rule about symbols.
pub fn is_symbol(c: char) -> bool {
    !c.is_alphanumeric() && !c.is_whitespace()
}

/// Something the request needs is not up yet: LosOS Cloud or LosOS Git still
/// starting, or sign-in not set up.
///
/// A distinct error type so the HTTP layer can answer 503 with the reason and
/// a `Retry-After`, rather than the opaque 500 every other command failure
/// gets: it is nobody's fault and resolves itself. Nothing was changed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not ready yet: {0}")]
pub struct NotReady(pub String);

/// A claim on a box that already has an owner. Typed so the HTTP layer can
/// answer 409 with the sentence, which the wizard shows, rather than the 500
/// "command failed; see the lososd journal" every untyped error becomes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("this box has already been set up")]
pub struct AlreadyClaimed;
