//! Text edits that turn one version of a file into another.
//!
//! When the file changes outside the editor (a sync app uploads it, or
//! WebDAV writes it), the document has to follow without throwing away what
//! the people in it are typing. Replacing the whole text would do that: every
//! cursor jumps and a concurrent keystroke lands in deleted text. So the new
//! version arrives as a line diff, and only the lines that changed are
//! deleted and inserted.
//!
//! Positions are UTF-16 code units, the unit Yjs counts text in.

use std::time::Duration;

use similar::{DiffOp, TextDiff};

/// One edit, in the old text's positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Where, in UTF-16 code units.
    pub at: u32,
    /// How many code units to delete there.
    pub delete: u32,
    /// What to insert after deleting.
    pub insert: String,
}

/// A diff of two large files gives up after this long and settles for a
/// coarser answer, which is still correct.
const TIMEOUT: Duration = Duration::from_millis(200);

fn utf16(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// The edits that make `old` read `new`, in ascending order. Apply them
/// last first, so each one's position is still valid.
pub fn edits(old: &str, new: &str) -> Vec<Edit> {
    let diff = TextDiff::configure().timeout(TIMEOUT).diff_lines(old, new);
    let new_lines: Vec<&str> = diff.iter_new_slices().collect();
    let mut starts = Vec::with_capacity(diff.old_len() + 1);
    let mut at = 0u32;
    for line in diff.iter_old_slices() {
        starts.push(at);
        at += utf16(line);
    }
    starts.push(at);

    let mut out = Vec::new();
    for op in diff.ops() {
        let (old_range, new_range) = match *op {
            DiffOp::Equal { .. } => continue,
            _ => (op.old_range(), op.new_range()),
        };
        let from = starts[old_range.start];
        let to = starts[old_range.end];
        let insert: String = new_lines[new_range].concat();
        out.push(Edit {
            at: from,
            delete: to - from,
            insert,
        });
    }
    out
}

/// `old` with `edits` applied, for tests and for checking a diff.
pub fn apply(old: &str, edits: &[Edit]) -> String {
    let mut units: Vec<u16> = old.encode_utf16().collect();
    for e in edits.iter().rev() {
        let at = e.at as usize;
        let end = at + e.delete as usize;
        units.splice(at..end, e.insert.encode_utf16());
    }
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_text_has_no_edits() {
        assert!(edits("a\nb\n", "a\nb\n").is_empty());
    }

    #[test]
    fn only_the_changed_line_is_touched() {
        let old = "one\ntwo\nthree\n";
        let new = "one\n2\nthree\n";
        let e = edits(old, new);
        assert_eq!(
            e,
            vec![Edit {
                at: 4,
                delete: 4,
                insert: "2\n".into()
            }]
        );
        assert_eq!(apply(old, &e), new);
    }

    #[test]
    fn two_distant_changes_stay_two_edits() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "A\nb\nc\nd\nE\n";
        let e = edits(old, new);
        assert_eq!(e.len(), 2);
        assert_eq!(apply(old, &e), new);
    }

    #[test]
    fn positions_count_utf16_units() {
        // An emoji is two UTF-16 units and four UTF-8 bytes.
        let old = "🐟 fish\nline\n";
        let new = "🐟 fish\nnew line\n";
        let e = edits(old, new);
        assert_eq!(e[0].at, 8);
        assert_eq!(apply(old, &e), new);
    }

    #[test]
    fn inserts_deletes_and_missing_final_newline() {
        for (old, new) in [
            ("", "hello"),
            ("hello", ""),
            ("a\nb", "a\nb\nc"),
            ("x\ny\nz\n", "y\n"),
            ("same", "same\n"),
            ("č\nš\n", "š\nč\n"),
        ] {
            assert_eq!(apply(old, &edits(old, new)), new, "{old:?} -> {new:?}");
        }
    }
}
