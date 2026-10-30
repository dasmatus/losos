# Pages as MDX

Every screen that can be changed is an MDX file here that places the
screen's blocks, and an administrator can write their own in its place
(handbook: Owner's manual, Changing the pages).

| File | What it is |
|---|---|
| `blocks.json` | Each page: its title, its path and its blocks, with the props a block takes (`text`, `choice`, `number`, `flag`, `url`). |
| `<id>.mdx` | The page as LosOS ships it: its blocks and nothing else, so it draws exactly what the screen drew before. |
| `page.tsx` | `<Page id blocks>`: the page's HTML read with DOMParser and drawn as React, each placeholder as the screen's own component. |
| `store.ts` | The changed pages and the stylesheet from `GET /api/pages`, kept in localStorage for the next visit, and the editor's drafts. |
| `editor.tsx` | The editor that docks beside the page. |
| `pages.css` | Type for text an administrator writes on a page. |

## How a page is read

One parser everywhere: losos-mdx (`backend/mdx`, its `page` module).

- At build time `vite.pages.ts` reads each default with the crate's
  WebAssembly package and the bundle carries the HTML as
  `virtual:losos-pages`. A default that does not read fails the build.
- On the box the `pages` plugin (`backend/plugins/pages`) reads an
  administrator's save the same way, against `dist/pages/blocks.json`, and
  refuses one with any problem.
- In the editor, `src/mdx/wasm.ts`'s `renderPage` reads each keystroke.

The crate writes each block as `<losos-block data-block data-props>` and
each run of text as `div.mdx-document.mdx-page-text`. A page places a block
on a line of its own; a block it leaves out is not drawn. ESM and
expressions are refused, and the only tags are the blocks, the document
components and `Button`.

## Adding a page

1. List it in `blocks.json`: an id of lowercase parts joined by dots
   (`settings.storage`), its title, its path and its blocks.
2. Write `<id>.mdx` with the blocks in the order the screen draws them.
3. Draw it with `<Page id="…" blocks={…} />`, a `BlockMap<"…">` that names
   exactly the blocks `blocks.json` lists (the type checks it).
4. Give it `pages.page.<id>` and each block `pages.block.<Name>` in
   `src/i18n/pages.ts`, and its blocks in the pages without a script
   (`backend/plugins/basic/src/custom.rs`'s callers).
