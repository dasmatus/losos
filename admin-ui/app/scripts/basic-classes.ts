/* The classes the pages without JavaScript use.
 *
 * Those pages are drawn in Rust (backend/plugins/page and
 * backend/plugins/basic). The admin bundle is built from this folder
 * alone, so Tailwind cannot see the Rust sources. This writes them into
 * a Turso database — a SQLite file, the format libSQL stores — and into the
 * text file `@source` reads. Neither is committed: the database is a
 * Nix package (nix/packages.nix, `losos-basic-classes`) published to the
 * LosOS cache, and the text file is materialised from it at build time.
 *
 *   bun scripts/basic-classes.ts [--repo ROOT] [--out DIR]
 *
 * ROOT is the checkout (it holds backend/plugins). DIR receives
 * basic-classes.db and basic-classes.txt. From build.ts and dev.ts, both
 * default to this repo and admin-ui/app/src/styles.
 */

import { Database } from 'bun:sqlite';

const here = import.meta.dirname;

/** Every `"…"` and `r#"…"#` in `src`, escapes left as they are.
 * Indexes are bytes, as in the Rust `str`, not UTF-16 code units. */
export function stringLiterals(src: string): string[] {
  const b = new TextEncoder().encode(src);
  const out: string[] = [];
  let i = 0;
  const decoder = new TextDecoder();
  while (i < b.length) {
    if (b[i] === 0x2f && b[i + 1] === 0x2f) {
      while (i < b.length && b[i] !== 0x0a) i++;
    } else if (b[i] === 0x27) {
      // A char literal ('"', '\'') or a lifetime.
      if (b[i + 2] === 0x27) i += 3;
      else if (b[i + 1] === 0x5c && b[i + 3] === 0x27) i += 4;
      else i += 1;
    } else if (b[i] === 0x72 && b[i + 1] === 0x23 && b[i + 2] === 0x22) {
      const start = i + 3;
      let end = b.length;
      for (let k = start; k + 1 < b.length; k++) {
        if (b[k] === 0x22 && b[k + 1] === 0x23) {
          end = k;
          break;
        }
      }
      out.push(decoder.decode(b.subarray(start, end)));
      i = end + 2;
    } else if (b[i] === 0x22) {
      const start = i + 1;
      let j = start;
      while (j < b.length && b[j] !== 0x22) j += b[j] === 0x5c ? 2 : 1;
      out.push(decoder.decode(b.subarray(start, Math.min(j, b.length))));
      i = j + 1;
    } else {
      i++;
    }
  }
  return out;
}

/** Class-like tokens in the page crates, in the order the database stores. */
export async function classNames(repo: string): Promise<string[]> {
  const found = new Set<string>();
  for (const dir of ['page/src', 'basic/src']) {
    const abs = `${repo}/backend/plugins/${dir}`;
    const names = [...new Bun.Glob('*.rs').scanSync({ cwd: abs, onlyFiles: true })].sort();
    for (const name of names) {
      // The icons are paths, and the tests' strings are no page's.
      if (name === 'icons.rs' || name === 'tests.rs') continue;
      const path = `${abs}/${name}`;
      for (const literal of stringLiterals(await Bun.file(path).text())) {
        for (const token of literal.split(/\s+/)) {
          if (token.includes('-') || token.includes(':') || token.includes('[')) found.add(token);
        }
      }
    }
  }
  return [...found].sort();
}

/** Write the Turso database and the text Tailwind scans, from the database. */
export async function writeBasicClasses(repo: string, out: string): Promise<string> {
  const dbPath = `${out}/basic-classes.db`;
  if (await Bun.file(dbPath).exists()) await Bun.file(dbPath).delete();
  const names = await classNames(repo);
  await Bun.write(dbPath, '');
  const db = new Database(dbPath, { create: true });
  try {
    db.exec('DROP TABLE IF EXISTS class');
    db.exec('CREATE TABLE class (name TEXT PRIMARY KEY NOT NULL)');
    const insert = db.prepare('INSERT INTO class (name) VALUES (?)');
    const tx = db.transaction((rows: string[]) => {
      for (const name of rows) insert.run(name);
    });
    tx(names);
    const stored = (db.query('SELECT name FROM class ORDER BY name').all() as { name: string }[]).map((row) => row.name);
    await Bun.write(`${out}/basic-classes.txt`, stored.join('\n') + (stored.length ? '\n' : ''));
  } finally {
    db.close();
  }
  return dbPath;
}

function arg(flag, fallback) {
  const i = process.argv.indexOf(flag);
  return i === -1 ? fallback : process.argv[i + 1];
}

/** Write the gitignored class list before Tailwind scans, where the plugin
 * sources are beside the UI (a checkout). */
export async function basicClasses() {
  const repo = `${here}/../../..`;
  const styles = `${here}/../src/styles`;
  const sources = `${repo}/backend/plugins/basic/src`;
  // The Nix build has no plugin sources beside the UI. It copies the
  // text file out of the cached database before the build starts.
  try {
    new Bun.Glob("*").scanSync({ cwd: sources }).next();
  } catch (error) {
    if (error.code === "ENOENT") return;
    throw error;
  }
  await writeBasicClasses(repo, styles);
}

/* The build (scripts/build.ts) runs writeBasicClasses the same way, so a
 * checkout compiles its sheet from these sources and the Nix build — which
 * carries no plugin sources beside the UI — copies the text file out of the
 * cached database before Tailwind starts. */
if (import.meta.main) {
  const repo = arg('--repo', `${here}/../../..`);
  const out = arg('--out', `${here}/../src/styles`);
  await writeBasicClasses(repo, out);
}
