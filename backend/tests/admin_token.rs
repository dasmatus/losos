//! The admin token is root on this appliance: `POST /api/apply` writes Nix and
//! runs `nixos-rebuild switch`. These tests pin what `ensure_token` will and
//! will not accept as that secret.

use assert_fs::TempDir;
use losos_ctl::http::ensure_token;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn mode_of(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn is_minted_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Every one of these is something a bad write, a bad backup restore or a
/// hostile local process could leave in the file. The old code returned each of
/// them verbatim and then used it as the shared secret — a file holding `x`
/// authenticated `Authorization: Bearer x`.
#[test]
fn a_token_file_that_is_not_a_minted_token_is_replaced() {
    for junk in [
        "",                                                                  // truncated write
        "   \n",                                                             // whitespace only
        "deadbeef",                                                          // too short
        "x",                                                                 // planted secret
        "DEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEF",  // uppercase
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",  // not hex
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0", // too long
    ] {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, junk).unwrap();

        let token = ensure_token(&path).unwrap();

        assert!(is_minted_token(&token), "accepted {junk:?} as a token");
        assert_ne!(token, junk.trim(), "kept {junk:?} as the secret");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), token);
    }
}

#[test]
fn a_minted_token_survives_a_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("token");

    let first = ensure_token(&path).unwrap();
    assert!(is_minted_token(&first));
    // Rotating means deleting the file; a restart on its own must not.
    assert_eq!(ensure_token(&path).unwrap(), first);
    assert_eq!(ensure_token(&path).unwrap(), first);
}

#[test]
fn a_trailing_newline_is_tolerated_but_not_re_minted_over() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("token");
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    std::fs::write(&path, format!("{token}\n")).unwrap();

    assert_eq!(ensure_token(&path).unwrap(), token);
    // An operator's editor added the newline; that is not corruption, so the
    // file is left exactly as it was.
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!("{token}\n")
    );
}

#[test]
fn a_minted_token_is_never_left_world_readable() {
    let dir = TempDir::new().unwrap();
    let secrets = dir.path().join("secrets");
    let path = secrets.join("losos-admin-token");

    ensure_token(&path).unwrap();

    assert_eq!(mode_of(&path), 0o600, "the token is equivalent to root");
    assert_eq!(mode_of(&secrets), 0o700, "the directory holding it too");

    // The file is created 0600 rather than created wide and narrowed after, so
    // there is no window to read it in — and no temp file left holding a copy.
    let strays: Vec<_> = std::fs::read_dir(&secrets)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .filter(|name| name != "losos-admin-token")
        .collect();
    assert!(strays.is_empty(), "token copy left behind: {strays:?}");
}

#[test]
fn an_unreadable_token_file_is_an_error_not_a_fresh_token() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("token");
    // A directory in the token's place: any read failure that is not "absent"
    // means the daemon does not know whether a token already exists, and
    // minting a second one would silently lock out whoever holds the first.
    std::fs::create_dir(&path).unwrap();
    assert!(ensure_token(&path).is_err());
}
