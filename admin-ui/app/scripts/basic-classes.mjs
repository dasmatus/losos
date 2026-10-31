/* The classes the pages without JavaScript use.
 *
 * Those pages are drawn in Rust (backend/plugins/page and
 * backend/plugins/basic). The admin bundle is built from this folder
 * alone, so Tailwind cannot see the Rust sources. This writes them into
 * a Turso database — a SQLite file, the format libSQL stores — and into
 * the text file `@source` reads. Neither is committed: the database is a
 * Nix package (nix/packages.nix, `losos-basic-classes`) published to the
 * LosOS cache, and the text file is materialised from it at build time.
 *
 *   bun scripts/basic-classes.mjs [--repo ROOT] [--out DIR]
 *
 * ROOT is the checkout (it holds backend/plugins). DIR receives
 * basic-classes.db and basic-classes.txt. From Vite, both default to
 * this repo and admin-ui/app/src/styles.
 */

import { Database } from "bun:sqlite";
import { mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

/** Every `"…"` and `r#"…"#` in `src`, escapes left as they are.
 * Indexes are bytes, as in the Rust `str`, not UTF-16 code units. */
export function stringLiterals(src) {
  const b = Buffer.from(src, "utf8");
  const out = [];
  let i = 0;
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
      out.push(b.subarray(start, end).toString("utf8"));
      i = end + 2;
    } else if (b[i] === 0x22) {
      const start = i + 1;
      let j = start;
      while (j < b.length && b[j] !== 0x22) j += b[j] === 0x5c ? 2 : 1;
      out.push(b.subarray(start, Math.min(j, b.length)).toString("utf8"));
      i = j + 1;
    } else {
      i++;
    }
  }
  return out;
}

/** Class-like tokens in the page crates, in the order the database stores. */
export function classNames(repo) {
  const found = new Set();
  for (const dir of ["page/src", "basic/src"]) {
    const abs = join(repo, "backend/plugins", dir);
    for (const name of readdirSync(abs).sort()) {
      // The icons are paths, and the tests' strings are no page's.
      if (name === "icons.rs" || name === "tests.rs") continue;
      const path = join(abs, name);
      if (!statSync(path).isFile()) continue;
      for (const literal of stringLiterals(readFileSync(path, "utf8"))) {
        for (const token of literal.split(/\s+/)) {
          if (token.includes("-") || token.includes(":") || token.includes("[")) found.add(token);
        }
      }
    }
  }
  return [...found].sort();
}

/** Write the Turso database and the text Tailwind scans, from the database. */
export function writeBasicClasses(repo, out) {
  mkdirSync(out, { recursive: true });
  const dbPath = join(out, "basic-classes.db");
  rmSync(dbPath, { force: true });
  const names = classNames(repo);
  const db = new Database(dbPath, { create: true });
  try {
    db.exec("DROP TABLE IF EXISTS class");
    db.exec("CREATE TABLE class (name TEXT PRIMARY KEY NOT NULL)");
    const insert = db.prepare("INSERT INTO class (name) VALUES (?)");
    const tx = db.transaction((rows) => {
      for (const name of rows) insert.run(name);
    });
    tx(names);
    const stored = db.query("SELECT name FROM class ORDER BY name").all().map((row) => row.name);
    writeFileSync(join(out, "basic-classes.txt"), stored.join("\n") + (stored.length ? "\n" : ""));
  } finally {
    db.close();
  }
  return dbPath;
}

function arg(flag, fallback) {
  const i = process.argv.indexOf(flag);
  return i === -1 ? fallback : process.argv[i + 1];
}

export function basicClasses() {
  const repo = join(here, "../../..");
  const styles = join(here, "../src/styles");
  const sources = join(repo, "backend/plugins/basic/src");
  let present = false;
  try {
    present = statSync(sources).isDirectory();
  } catch {
    present = false;
  }
  // The Nix build has no plugin sources beside the UI. It copies the
  // text file out of the cached database before Vite starts.
  if (present) writeBasicClasses(repo, styles);
  return { name: "losos-basic-classes" };
}

if (import.meta.main) {
  const repo = arg("--repo", join(here, "../../.."));
  const out = arg("--out", join(here, "../src/styles"));
  writeBasicClasses(repo, out);
}
