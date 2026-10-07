/* Every entry in admin-ui/app/src/lib/handbook.ts must be a page of this
 * handbook: the admin UI's "What to do" links are built from that file, and
 * a page renamed here would otherwise 404 on every box. A page's address is
 * its `slug:` front matter (the troubleshooting pages and the manual pages
 * the UI links to declare one), else its path under docs/. */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const docs = join(here, '..', 'docs');
const source = join(here, '..', '..', 'admin-ui', 'app', 'src', 'lib', 'handbook.ts');

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const file = join(dir, name);
    if (statSync(file).isDirectory()) yield* walk(file);
    else if (name.endsWith('.md') || name.endsWith('.mdx')) yield file;
  }
}

/* The addresses this handbook answers at, without the base and without a
 * trailing slash: a declared slug, or the file's path with index.md folded
 * into its directory. */
const pages = new Set();
for (const file of walk(docs)) {
  const text = readFileSync(file, 'utf8');
  const slug = /^---\n[\s\S]*?^slug:\s*(\S+)\s*$[\s\S]*?^---/m.exec(text)?.[1];
  let path = relative(docs, file).replace(/\.mdx?$/, '').replace(/\/index$/, '').replace(/^index$/, '');
  if (slug !== undefined) path = slug.replace(/^\//, '');
  pages.add(path.replace(/\/$/, ''));
}

const entries = [...readFileSync(source, 'utf8').matchAll(/^\s+"?([\w-]+)"?:\s*"([^"]*)",?$/gm)];
if (entries.length === 0) throw new Error(`no entries found in ${source}`);
const missing = entries.filter(([, , target]) => !pages.has(target.replace(/\/$/, '')));
for (const [, entry, target] of missing) console.error(`handbook entry "${entry}" points at /${target}, which is not a page`);
if (missing.length > 0) process.exit(1);
console.log(`${entries.length} handbook links from the admin UI resolve to a page`);
