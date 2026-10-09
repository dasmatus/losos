/* Every braces in the tree must be the depth-limited copy in vendor/braces
 * (see its README): the packages that glob for Docusaurus and webpack all
 * reach it, and a pattern nested deeper than the limit is refused with a
 * SyntaxError instead of overflowing the stack. */
import { realpathSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const vendored = realpathSync(join(here, '..', 'vendor', 'braces', 'index.js'));
const require = createRequire(join(here, '..', 'package.json'));

let failed = false;
const fail = message => {
  console.error(message);
  failed = true;
};

for (const user of ['micromatch', 'chokidar', 'fast-glob']) {
  const found = realpathSync(createRequire(require.resolve(user)).resolve('braces'));
  if (found !== vendored) fail(`${user} loads braces from ${found}, not vendor/braces`);
}

const braces = require(vendored);
const nested = depth => '{'.repeat(depth) + '}'.repeat(depth);
for (const name of ['create', 'compile', 'expand', 'stringify']) {
  try {
    braces[name](nested(5000));
    fail(`braces.${name} accepted 5000 nested braces`);
  } catch (error) {
    if (!(error instanceof SyntaxError)) fail(`braces.${name} threw ${error}`);
  }
}

const got = JSON.stringify(braces.expand('a/{b,{c,d}}/{1..2}'));
const want = JSON.stringify(['a/b/1', 'a/b/2', 'a/c/1', 'a/c/2', 'a/d/1', 'a/d/2']);
if (got !== want) fail(`expand gave ${got}, wanted ${want}`);

if (failed) process.exit(1);
console.log('braces: every user loads vendor/braces; deep nesting is refused');
