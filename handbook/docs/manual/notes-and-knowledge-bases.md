---
title: Notes and knowledge bases
sidebar_position: 4.42
slug: /manual/notes-and-knowledge-bases
---

# Notes and knowledge bases

**Notes** in [LosOS Cloud](losos-cloud) opens a folder of notes, `.note`
and `.md` files, the way Obsidian opens a vault: the notes on the left, the open note in the
middle, and beside it the notes that link to it. An Obsidian vault synced
into LosOS Cloud opens as it is, with its links, embeds, tags and canvases.
While the box shares and is in a mesh, a folder of notes can also be
published as a knowledge base: a site that people read in a browser.

## Open your notes

- **Notes** in the sidebar opens the folder `Notes` in your files. It is
  made the first time you open it.
- Right-click any folder in **Files** and choose **Open as notes**.
- Click a `.note` or `.md` file in **Files**. It opens in its vault: the nearest folder
  above it with an `.obsidian` folder inside, as Obsidian finds a vault, or
  else the folder the file is in.

## .note and .md

A new note is a `.note` file. A `.note` is MDX: Markdown that can also hold
components, written like HTML tags. These are the ones a note can use, the
same as in LosOS Office's documents:

| Component | What it draws |
| --- | --- |
| `<Callout type="tip" title="…">` | A box in the colour of its type: `note`, `tip`, `important`, `warning` or `caution` |
| `<Details summary="…">` | A part that opens when clicked |
| `<Tabs>` with `<Tab label="…">` | Parts read one after the other, each under its label |
| `<Columns>` with `<Column>` | Parts side by side, one under the other on a phone |
| `<Figure src="…" caption="…">` | A picture with a caption |
| `<TableOfContents />` | The note's headings, as links |
| `<Table>`, `<Row>`, `<Cell>` | A table with merged cells and a caption |
| `<Mention login="…" />`, `<Date value="2026-08-14" />` | A person and a date |
| `<Equation tex="…" />` | Maths on a line of its own |

`<kbd>`, `<sub>`, `<sup>`, `<mark>`, `<u>`, `<s>`, `<small>`, `<abbr>` and
`<br />` work as in HTML.

A note never runs code. MDX would let a file carry JavaScript, so Notes
refuses all of it: `{…}` expressions, `import` and `export` lines,
`<script>`, event handlers such as `onClick`, `javascript:` links, and any
tag or component not in the table. What a note asked for and did not get
is listed above it with its line, and the rest of the note is drawn. A
`.note` that is not valid MDX, such as one with a component left open, is
drawn as Markdown with the line that broke it.

`.md` notes stay Markdown and keep working as before. **Convert to .note**,
above a `.md` note, makes it a `.note` that reads the same: braces and
angle brackets that MDX would read as code are escaped, and HTML comments
become `%%comments%%`. If the note's headings or links would come out
differently, it stays `.md` and the message says which line. `[[Links]]`
name a note without its ending, so they keep working. Obsidian opens only
`.md` files, so keep notes you also edit in Obsidian as `.md`.

## Reading a note

Both kinds of Markdown people write are understood, in `.note` and `.md`
files alike:

- GitHub's: tables, task lists, ~~strikethrough~~, footnotes, links that
  start with `https://`, and alerts such as `> [!NOTE]`.
- Obsidian's: `[[links]]` to other notes, also `[[Note|shown text]]`,
  `[[Note#Heading]]` and `[[Note#^block]]`; `![[embeds]]` of a note, a part
  of one, or a picture; callouts, including ones that fold (`> [!tip]-`);
  `#tags` and `#nested/tags`; `==highlights==`; `%%comments%%`, which are
  not shown; maths between `$` and `$$`; and the properties at the top of a
  note.

Code blocks are coloured by language. Mermaid diagrams stay code, because
drawing them needs styles the box's pages do not allow.

A link to a note that does not exist yet is drawn fainter. Click it to make
the note. Tick a task box and the note is saved with the box ticked.

## Beside a note

- **Local graph**: the note and the notes it links to or from, one to three
  links away. Click a dot to open that note.
- **Linked here**: the notes that link to this one.
- **Links to** and **Not written yet**: what this note links.
- **Outline**: its headings. Click one to jump to it.

## Edit, mind map and graph

- **Edit** opens the note in the editor people type in together (see
  [Sharing and editing together](sharing-and-editing-together)), with the
  note drawn beside the text on a wide screen. Type `[[` to pick a note to
  link. Select words, in the editor or while reading, for
  [a bar of actions](admin-pages#the-bar-over-a-selection) above them.
- **Mind map** draws the note's headings and the lists under them as
  branches. Click a heading's branch to read that part.
- **Graph of all notes**, above the notes, shows every note in the folder
  and every link between them. Switches add tags, attachments, links to notes
  not written yet, and notes nothing links to. The field filters by name.

The search field above the notes finds words in names and in the text.
**Tags** lists every tag; click one to see its notes.

## Publish a folder as a knowledge base

Knowledge bases need the box to share and to be in a mesh, as chat and mail
do. Without both, the menu item is not there and the sites are off.

1. In **Files**, right-click the folder and choose **Publish as knowledge
   base**.
2. Give it a title, and choose who can read it:
   - **Anyone with the link** reads without signing in, wherever the box
     can be reached: on its network, and through its edge when it has one.
   - **People on this box** sign in with LosOS Login first.
3. Press **Publish**. The window shows the site's address, for example
   `https://<name>.local/kb/kitchen-garden/`.

The site follows the folder. A note you change or add shows there at once.
Each note gets its own address, such as `/kb/kitchen-garden/Tomatoes`.
Readers get the same links, embeds, search, graph and mind map, and nothing
to change the notes with.

Left out of the site:

- notes whose properties say `publish: false` or `private: true`;
- folders whose names start with a dot, `.obsidian` among them;
- anything outside the folder.

To change the title or who can read it, right-click the folder again and
choose **Knowledge base**. **Stop publishing** takes the site down; the
notes stay in your files.

If the box stops sharing or leaves the mesh, every site stops answering.
They come back as they were when the box is sharing in a mesh again.
