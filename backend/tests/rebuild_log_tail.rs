//! The one line of the rebuild log the admin UI ever sees.
//!
//! `log_tail` is what makes the progress line move while a rebuild runs, and
//! what puts the Nix error next to "rebuild failed (exit 1)" when one doesn't.
//! It reads only the last [`LOG_WINDOW`] bytes of a file that grows to many
//! megabytes, so the seek is the part worth holding to a standard — the pure
//! line picking is covered next to the function itself.

use assert_fs::TempDir;
use losos_ctl::supervisor::{log_tail, LOG_LINE_CAP, LOG_WINDOW};
use std::io::Write;
use std::path::PathBuf;

/// Write `body` to `rebuild.log` in `dir` and hand back its path.
fn log_of(dir: &TempDir, body: &str) -> PathBuf {
    let path = dir.path().join("rebuild.log");
    std::fs::write(&path, body).unwrap();
    path
}

/// Append to an already-written log, as the transient unit does.
fn append(path: &PathBuf, more: &str) {
    let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    f.write_all(more.as_bytes()).unwrap();
}

#[test]
fn a_log_that_cannot_be_read_is_no_message_rather_than_an_error() {
    let dir = TempDir::new().unwrap();
    // Never started a rebuild: there is no log yet, and "" makes the UI fall
    // back to the stored message.
    assert_eq!(log_tail(&dir.path().join("rebuild.log")), "");

    // A directory where the file should be stands in for any read error that
    // is not "absent". It must not take the status endpoint down with it.
    let blocked = dir.path().join("blocked.log");
    std::fs::create_dir(&blocked).unwrap();
    assert_eq!(log_tail(&blocked), "");
}

#[test]
fn an_empty_log_is_no_message() {
    let dir = TempDir::new().unwrap();
    assert_eq!(log_tail(&log_of(&dir, "")), "");
    assert_eq!(log_tail(&log_of(&dir, "\n\n   \n")), "");
}

#[test]
fn a_log_shorter_than_the_window_gives_its_last_line() {
    let dir = TempDir::new().unwrap();
    let path = log_of(
        &dir,
        "building '/nix/store/aaaa-etc.drv'\n  copying path '/nix/store/bbbb-bash'  \n\n",
    );
    assert!(std::fs::metadata(&path).unwrap().len() < LOG_WINDOW);
    // Trailing blank lines are skipped and the line is trimmed: systemd's
    // append leaves a newline behind after every write.
    assert_eq!(log_tail(&path), "copying path '/nix/store/bbbb-bash'");
}

#[test]
fn a_log_without_a_trailing_newline_still_gives_its_last_line() {
    // A rebuild killed mid-write leaves exactly this.
    let dir = TempDir::new().unwrap();
    let path = log_of(&dir, "copying path '/nix/store/aaaa'\nerror: build failed");
    assert_eq!(log_tail(&path), "error: build failed");
}

#[test]
fn a_log_longer_than_the_window_still_gives_its_last_line() {
    let dir = TempDir::new().unwrap();
    let noise = "copying path '/nix/store/0000000000000000000000000000000-x'\n".repeat(400);
    let path = log_of(
        &dir,
        &format!("{noise}error: builder for '/nix/store/z.drv' failed"),
    );

    assert!(std::fs::metadata(&path).unwrap().len() > LOG_WINDOW);
    assert_eq!(
        log_tail(&path),
        "error: builder for '/nix/store/z.drv' failed"
    );
}

#[test]
fn only_the_tail_window_is_read() {
    // The counterpart of the test above: a message older than the window is
    // gone, not merely outranked. This is the whole reason the seek exists —
    // without it the reader would pull a multi-megabyte log into memory on
    // every two-second poll from the admin UI.
    let dir = TempDir::new().unwrap();
    let quiet = "\n".repeat(LOG_WINDOW as usize + 100);
    let path = log_of(
        &dir,
        &format!("error: this scrolled out of the window\n{quiet}"),
    );
    assert_eq!(log_tail(&path), "");
}

#[test]
fn a_window_boundary_inside_a_character_leaves_the_last_line_intact() {
    let dir = TempDir::new().unwrap();
    // A padding line of three-byte characters, sized so the window boundary
    // lands mid-character and the lossy decode substitutes U+FFFD there. What
    // decides that is the length of everything *after* the padding, so the
    // assertion below is not decoration — it caught this being wrong once.
    let pad = "…".repeat(2000);
    let path = log_of(&dir, &format!("{pad}\nerror: unexpected token '}}'\n"));

    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > LOG_WINDOW);
    assert_ne!(
        (size - LOG_WINDOW) % 3,
        0,
        "the boundary fell on a character boundary; this test proved nothing"
    );

    // Only the first line in the window can be damaged, and that one is
    // discarded, so nothing corrupt ever reaches the UI.
    assert_eq!(log_tail(&path), "error: unexpected token '}'");
}

#[test]
fn an_over_long_line_is_capped_before_it_reaches_the_ui() {
    let dir = TempDir::new().unwrap();
    // Nix prints single lines of thousands of characters; the settings page
    // has one row for this.
    let path = log_of(&dir, &format!("{}\n", "x".repeat(2000)));
    assert_eq!(log_tail(&path).chars().count(), LOG_LINE_CAP);
}

#[test]
fn a_log_being_appended_to_is_re_read_from_its_current_end() {
    // The admin UI polls status roughly every two seconds while the transient
    // unit appends to this same file. Each call has to seek from the size it
    // sees now, not from one it cached.
    let dir = TempDir::new().unwrap();
    let path = log_of(&dir, "copying path '/nix/store/aaaa'\n");
    assert_eq!(log_tail(&path), "copying path '/nix/store/aaaa'");

    append(&path, "copying path '/nix/store/bbbb'\n");
    assert_eq!(log_tail(&path), "copying path '/nix/store/bbbb'");

    // And once the log grows past the window, so the seek starts doing work.
    append(&path, &"copying path '/nix/store/cccc'\n".repeat(300));
    append(&path, "error: build of '/nix/store/z.drv' failed\n");
    assert!(std::fs::metadata(&path).unwrap().len() > LOG_WINDOW);
    assert_eq!(log_tail(&path), "error: build of '/nix/store/z.drv' failed");
}
