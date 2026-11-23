#!/usr/bin/env bun
/* The virtual modules the SPA imports, written for Metro's web build where
 * Bun's bundler would have answered them from its own plugin (bundler.ts,
 * the checkout's `bun build.ts` still reads the import itself):
 *
 *   bun run virtuals   # scripts/gen-virtuals.ts
 *
 *   losos:pages — src/pages/pages-defaults.gen.ts, the calls.html of the
 *   defaults pages/ (pages.ts, over src/mdx/pkg's losos-mdx: run `bun run
 *   mdx` after a fresh clone), what the pages' editor starts from when a
 *   box serves none an administrator made yet;
 *
 *   losos:build — src/lib/build.gen.ts, the settings losos:build exports
 *   (bundler.ts's settings(), read from environment now so a checkout runs
 *   the same values the box's build writes: BASE — the SPA is the document
 *   root, LAB_HOSTED — the hosted Diagrams copy only, LAB_MEME only with
 *   no.jpg/yes.jpg named beside it before the build runs).
 *
 * Both files are gitignored (this folder .gitignore); nix/packages.nix's
 * losos-admin-ui preBuild runs this script the same way before `bunx expo
 * export`, and metro.config.cjs answers the two imports at these paths.
 * `losos:pages` typechecks through src/pages/virtual.d.ts's ambient
 * declare — the generated module is compatible with it (typed DEFAULTS of
 * the same page→HTML map).
 */

const HERE = import.meta.dir;
const app = `${HERE}/..`;

/* ── losos:pages ───────────────────────────────────────────────────────── */
const { compilePages } = await import(`${app}/pages.ts`);
const compiled = await compilePages(app);
await Bun.write(
  `${app}/src/pages/pages-defaults.gen.ts`,
  `/* GENERATED FILE (scripts/gen-virtuals.ts, \`bun run virtuals\`): losos:pages' module for the Metro web build — the HTML pages.ts compiles the defaults into; src/pages/virtual.d.ts is the shape's record. Bun's bundler answers this import itself (bundler.ts), and never reads the file. */
export const DEFAULTS = ${JSON.stringify(compiled.html) as string};
`,
);

/* ── losos:build ───────────────────────────────────────────────────────── */
const settings = {
  BASE: Bun.env.LOSOS_BASE ?? "/",
  LAB_HOSTED: Bun.env.LOSOS_LAB_HOSTED === "1",
  LAB_MEME: await readDirMeme(),
};
await Bun.write(
  `${app}/src/lib/build.gen.ts`,
  `/* GENERATED FILE (scripts/gen-virtuals.ts, \`bun run virtuals\`): losos:build's module for the Metro web build — the settings bundler.ts's settings() reads from environment; src/env.d.ts is the shape's record. Bun's bundler answers this import itself (bundler.ts), and never reads the file. */
export const BASE = ${JSON.stringify(settings.BASE)};
export const LAB_HOSTED = ${String(settings.LAB_HOSTED)};
export const LAB_MEME = ${String(settings.LAB_MEME)};
`,
);

/** The meme pictures the Lab splash asks for. LOSOS_LAB_MEME is a folder;
 * both non-pictures live beside the pages. */
async function readDirMeme(): Promise<boolean> {
  const dir = Bun.env.LOSOS_LAB_MEME;
  if (dir === undefined || dir === "") return false;
  try {
    void (await Bun.file(`${dir}/no.jpg`).exists());
    void (await Bun.file(`${dir}/yes.jpg`).exists());
    return true;
  } catch {
    return false;
  }
}

console.log(`virtuals: src/pages/pages-defaults.gen.ts (${Object.keys(compiled.html).length} pages), src/lib/build.gen.ts`);
