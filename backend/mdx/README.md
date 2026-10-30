# losos-mdx

MDX and Markdown to sanitized HTML, for LosOS Office (`.paper`, `.slides`)
and Notes (`.note`, `.md`). The same crate runs in three places: on the box,
inside lososd's document plugin, which hands the HTML to Pandoc for an
export; in the browser, as the `losos-mdx-wasm` package
(`nix build .#losos-mdx-wasm`), for previews and the editor; and in tests.
A preview and an export therefore come from the same code.

## Nothing in a document runs

The usual MDX tools compile a document to JavaScript and run it, so a
document can do anything a script can. This crate never compiles or runs
anything. It parses the document with markdown-rs and writes the HTML
itself:

- **JSX elements** are one of LosOS's components or a few plain HTML
  elements (the table below). Anything else is refused.
- **Props** are strings in quotes, checked against the component's entry:
  a choice from a list, a number, a date, a link or plain text. A prop
  written as an expression (`title={x}`), a spread (`{...x}`), or a prop the
  component does not have is refused.
- **Expressions** in braces, and `import` and `export` lines, are refused.
  A brace in text is written `\{`.
- **Links** may use http, https and mailto, or a path. Any other scheme
  (`javascript:`, `data:`) is refused and dropped.
- **The output** goes through an allowlist of tags, attributes, classes
  and URL schemes (`src/sanitize.rs`), whatever wrote it: render.rs, raw
  HTML in a `.md` file, or a Notes pass. No script, style, frame or form
  can get through, nor any form field, event handler or `style` attribute.
  MathML is allowed for equations, with its layout attributes only.
  Classes are limited to the document's own (`mdx-*`, `callout-*`,
  `language-*` and a few more), so a document cannot borrow the page's
  layout classes to cover the page.
- **Size and depth** are bounded: documents over 1 MiB, and nesting deeper
  than 100 levels, are errors. markdown-rs recurses, and its time grows
  faster than the document does.

## Finished with no script

The HTML is complete as written: a page shows it with JavaScript off, and
an export carries none.

- **Equations** (`$…$`, `$$…$$`, `<Equation>`) are MathML Core, made by
  math-core (`src/math.rs`), which browsers lay out themselves. The TeX
  rides along as an `application/x-tex` annotation, which Pandoc reads back.
  An equation that does not read, is longer than 4 KiB or nests deeper than
  32 levels stays TeX, as text; an unknown command is marked in place.
- **Dates** (`<Date>`) are words (`src/date.rs`): in the document's
  language (`lang` in its front matter), else the reader's
  (`Parsed::lang`), else English. English, Slovak and German.
- **Charts** (`<Chart>`) are a figure with the data's address for now;
  they become inline SVG with the sheet editor.

The browser package exports `mathml` and `date`, so the editor shows the
same as the box.

A refusal is a `Problem` with a line and column, not a failure. The page
still draws the document, with the refused part as plain text, and lists
what was refused. An export calls `Rendered::strict()`, which fails on any
problem. `tests/hostile.rs` covers each rule with documents written to
break it.

## Flavours

| Files | Flavour | `<` and `{` | Raw HTML |
|---|---|---|---|
| `.paper`, `.slides`, `.note`, `.mdx` | `Flavour::Mdx` | syntax | no; components instead |
| `.md`, `.markdown` | `Flavour::Md` | text | through the allowlist |

Both read CommonMark, GFM (tables, task lists, strikethrough, footnotes,
autolinks), YAML front matter and `$maths$`.

## Components

| Element | Props | HTML |
|---|---|---|
| `Callout` | `type` (note, tip, important, warning, caution), `title` | `aside.callout.callout-<type>` |
| `Details` | `summary`, `open` | `details.mdx-details` |
| `Columns`, `Column` | | `div.mdx-columns`, `div.mdx-column` |
| `Tabs`, `Tab` | `label` (required on Tab) | `div.mdx-tabs`, `section.mdx-tab` |
| `Figure` | `src` (required), `alt`, `caption` | `figure.mdx-figure` |
| `Embed` | `href` (required), `title` | `a.mdx-embed` |
| `Button` | `href` (required), `variant` (primary, secondary, outline, ghost, link), `size` (sm, md, lg) | `a.mdx-button`, its text as the label |
| `Slide` | `layout` (title, content, two-columns, picture) | `section.mdx-slide` |
| `Notes` | | `aside.mdx-speaker-notes` |
| `PageBreak` | | `div.page-break` |
| `TableOfContents` | `title` | `nav.mdx-toc`, the headings as nested lists |
| `Header`, `Footer` | | `header.mdx-page-header`, `footer.mdx-page-footer` |
| `PageNumber` | | `span.mdx-page-number` |
| `Table`, `Row`, `Cell` | `caption`; `header`; `colSpan`, `rowSpan`, `align` | `table.mdx-table`, `tr`, `th` or `td` |
| `Comment` | `text` (required), `author`, `date` | `span.mdx-comment` around the text it is about |
| `Equation` | `tex` (required), `inline` | `math` (MathML), `display="block"` unless inline |
| `Chart` | `src` (required, a `.sheet`), `range`, `type` (bar, line, area, pie), `title` | `figure.mdx-chart` with `data-*` |
| `Mention` | `login` (required) | `span.mdx-mention` |
| `Date` | `value` (required, YYYY-MM-DD), `format` (short, long) | `time.mdx-date`, the date in words |

Plain elements, as themselves: `abbr` (with `title`), `br`, `kbd`, `mark`,
`s`, `small`, `sub`, `sup`, `u`.

Charts, the table of contents and page numbers are filled in by whoever
shows the page: the editor draws a chart from its sheet, and an export
lays the pages out.

## Pages

`page::render_page(source, spec)` reads a page of the admin UI or an app
(`admin-ui/app/src/pages/README.md`). `spec` is the page's entry in
`blocks.json`, read by `page::Registry::from_json`: the blocks the page
has, each with typed props (`text`, `choice` with `values`, `number`,
`flag`, `url`), which are required, and whether it takes MDX inside it.

On top of the MDX rules above:

- A JSX element named like one of the page's blocks becomes
  `<losos-block data-block="Name" data-props="{...}">`, an empty
  placeholder the page swaps for its component. Its props are checked
  against the entry and carried as JSON; a bad one is dropped with a
  problem, and a block missing a required prop is not drawn.
- A block goes on a line of its own; one inside a sentence is refused.
- Any other name that is neither a block of this page nor a component is
  refused. An author cannot write the placeholder: `losos-block` is a
  name no table knows, so it is refused like any other, and only the page
  allowlist (`sanitize::clean_page`) keeps the element at all.
- Each run of the author's own text and components at the top of a page is
  wrapped in `div.mdx-document.mdx-page-text`, so the document styles
  reach it and never the blocks beside it. A page that is only blocks is
  only placeholders, so the default pages draw exactly what they did.

`tests/page.rs` covers each rule.

## API

```rust
use losos_mdx::{render, Flavour, Plain};

let rendered = render(source, Flavour::Mdx, &Plain)?;
rendered.html;          // sanitized HTML
rendered.properties;    // front matter as JSON
rendered.headings;      // level, text, id, line
rendered.links;         // what the document points at
rendered.problems;      // what was refused, with places
rendered.strict()?;     // an export: fails on any problem
```

A `Resolver` decides where links go: `note` (a note by name to its path),
`file` (a picture's address) and `href` (a path plus `#heading`). The crate
never decides; Notes, the knowledge-base pages and Office each pass their
own.

Notes' own syntax (wikilinks, tags, callouts) is a pass between `parse` and
`Parsed::render_with`:

- The pass expresses it as JSX elements in the reserved `losos:` namespace.
  `parse` refuses that namespace in what an author wrote, so only a pass
  can put such an element in the tree.
- A `Hook` draws the reserved elements, with the `Writer`'s `raw`, `text`,
  `children` and `resolver`.
- `Parsed::attributes` adds `id`, `class` and `data-*` to the element
  written for a node, keyed by the node's start offset and the tag
  written for it (`(12, "li")`), since a list and its first item start at
  the same offset.

The allowlist runs over all of it.

### Notes

`notes::render_note(source, flavour, resolver)` is that pass, in this
crate (`src/notes/`), for both flavours. On top of CommonMark and GFM it
reads Obsidian's syntax:

| Syntax | HTML |
|---|---|
| `[[Note#Heading\|shown]]` | `a.wikilink` with `data-note-path` and `data-sub`; `.is-unresolved` with `data-note-target` when the vault has no such note, `.is-file` for an attachment, `.is-internal` for `[[#Heading]]` |
| `![[picture.png\|300]]` | `img.embed-image` with `data-width`; audio and video as `span.embed-media` with `data-kind` and `data-src`; a note as an empty `span.note-embed` with `data-embed-path` |
| `#tag`, `#nested/tag` | `a.tag` with `data-tag` |
| `==highlight==` | `mark` |
| `%%comment%%` | nothing; its lines stay, as blank |
| `> [!tip]- Title` (and GitHub's `> [!NOTE]`) | `aside.callout.callout-tip`, or `details` when it folds, with `data-callout` |
| `- [ ]`, `- [x]`, `- [/]` and Obsidian's other marks | `li.task-list-item` with `data-task`, and `span.task-box` with `data-line` |
| `^block-id` | `id="^block-id"` on the block |
| one line break | `br` |

The page turns a `span.task-box` into a checkbox and a `span.embed-media`
into a player, since no document may write a form field or a media
element.

`notes::to_note(source)` turns a `.md` note into a `.note` that says the
same: braces and `<` outside code escaped, HTML comments as `%%comments%%`,
`<br>` and the other plain elements kept, any other HTML as text,
`<https://…>` as a link, indented code fenced. It is refused, with the
line, unless the result reads without a problem and keeps the note's
headings and links.

In the browser, `render(source, "mdx" | "md", resolver?)` returns the same
result as JSON, `page(source, registry, page)` a page's, `tree(source, flavour)` the mdast (positions kept) the
WYSIWYG editor builds its document from, and `renderNote` and `toNote` the
two above.
