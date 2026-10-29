/* Every entry in admin-ui/app/src/lib/handbook.ts must be a page of this
 * handbook: the admin UI's "What to do" links are built from that file, and
 * a page renamed here would otherwise 404 on every box. A page's address is
 * its `slug:` front matter (the troubleshooting pages and the manual pages
 * the UI links to declare one), else its path under docs/. */
const docs = `${import.meta.dir}/../docs`;
const source = `${import.meta.dir}/../../admin-ui/app/src/lib/handbook.ts`;

/* The addresses this handbook answers at, without the base and without a
 * trailing slash: a declared slug, or the file's path with index.md folded
 * into its directory. */
const pages = new Set();
for await (const file of new Bun.Glob('**/*.{md,mdx}').scan(docs)) {
  const text = await Bun.file(`${docs}/${file}`).text();
  const slug = /^---\n[\s\S]*?^slug:\s*(\S+)\s*$[\s\S]*?^---/m.exec(text)?.[1];
  let path = file.replace(/\.mdx?$/, '').replace(/\/index$/, '').replace(/^index$/, '');
  if (slug !== undefined) path = slug.replace(/^\//, '');
  pages.add(path.replace(/\/$/, ''));
}

const entries = [...(await Bun.file(source).text()).matchAll(/^\s+"?([\w-]+)"?:\s*"([^"]*)",?$/gm)];
if (entries.length === 0) throw new Error(`no entries found in ${source}`);
const missing = entries.filter(([, , target]) => !pages.has(target.replace(/\/$/, '')));
for (const [, entry, target] of missing) console.error(`handbook entry "${entry}" points at /${target}, which is not a page`);
if (missing.length > 0) throw new Error(`${missing.length} handbook links point at no page`);
console.log(`${entries.length} handbook links from the admin UI resolve to a page`);
