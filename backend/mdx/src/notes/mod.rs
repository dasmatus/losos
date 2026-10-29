//! Notes: a note the way Obsidian and GitHub read it, in either flavour.
//! `.note` files are MDX and `.md` files Markdown, and both carry
//! Obsidian's own syntax on top: [[wikilinks]] and ![[embeds]], #tags,
//! ==highlights==, %%comments%%, callouts, Obsidian's task marks, ^block
//! ids and single line breaks kept as breaks. `pass.rs` finds it in the
//! parsed tree and `draw.rs` writes it, through the same allowlist as
//! everything else.
//!
//! [`to_note`] turns a `.md` note into a `.note` that says the same thing.

mod convert;
mod draw;
mod pass;

use crate::{Error, Flavour, Place, Rendered, Resolver};

/// A note, written out as sanitized HTML. Heading and task lines count
/// from the top of the file, front matter included.
pub fn render_note(
    source: &str,
    flavour: Flavour,
    resolver: &dyn Resolver,
) -> Result<Rendered, Error> {
    let text = pass::strip_comments(source);
    let mut parsed = crate::parse(&text, flavour)?;
    pass::apply(&mut parsed.tree, &mut parsed.attributes);
    Ok(parsed.render_with(resolver, Some(&draw::Draw)))
}

/// A `.md` note as MDX for a `.note`: braces and angle brackets outside
/// code escaped, HTML comments as %%comments%%, other HTML as text unless
/// MDX writes it as itself, `<https://…>` as links and indented code
/// fenced. Refused, with the line, when the result would not read as
/// MDX without a problem.
pub fn to_note(source: &str) -> Result<String, Error> {
    if source.len() > crate::MAX_SOURCE {
        return Err(Error::TooLarge {
            bytes: source.len(),
        });
    }
    let refused = |line: usize, message: String| Error::Syntax {
        place: Place { line, column: 1 },
        message,
    };
    let note = convert::md_to_mdx(source).map_err(|r| refused(r.line, r.reason))?;
    // The same outline and the same links, or it is not the same note.
    let before = render_note(source, Flavour::Md, &crate::Plain)?;
    let after = render_note(&note, Flavour::Mdx, &crate::Plain)?;
    let moved = before
        .headings
        .iter()
        .enumerate()
        .find(|(i, h)| after.headings.get(*i) != Some(h));
    if let Some((_, h)) = moved {
        return Err(refused(
            h.line + 1,
            format!("the heading \"{}\" would change", h.text),
        ));
    }
    if before.headings.len() != after.headings.len() || before.links != after.links {
        let differs = before.links.iter().zip(&after.links).find(|(a, b)| a != b);
        let what = differs.map_or_else(|| "a link".to_owned(), |(a, _)| a.target.clone());
        let line = source
            .lines()
            .position(|l| l.contains(&what))
            .map_or(1, |n| n + 1);
        return Err(refused(
            line,
            format!("{what} would read differently as MDX"),
        ));
    }
    Ok(note)
}
