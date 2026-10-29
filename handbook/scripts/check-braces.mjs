/* Every braces in the tree must be the depth-limited copy in vendor/braces
 * (see its README): the packages that glob for Docusaurus and webpack all
 * reach it, and a pattern nested deeper than the limit is refused with a
 * SyntaxError instead of overflowing the stack. */
const root = `${import.meta.dir}/..`;
const vendored = Bun.resolveSync('./vendor/braces/index.js', root);
const dirname = (file) => file.slice(0, file.lastIndexOf('/'));
// Bun installs a file: dependency as a copy, once beside each user, so a
// copy counts when its manifest names the vendored package.
const VENDORED = await Bun.file(`${root}/vendor/braces/package.json`).json();

let failed = false;
const fail = message => {
  console.error(message);
  failed = true;
};

for (const user of ['micromatch', 'chokidar', 'fast-glob']) {
  const found = Bun.resolveSync('braces', dirname(Bun.resolveSync(user, root)));
  const { name, version } = await Bun.file(`${dirname(found)}/package.json`).json();
  if (name !== VENDORED.name || version !== VENDORED.version) fail(`${user} loads braces ${name}@${version} from ${found}, not vendor/braces`);
}

const { default: braces } = await import(vendored);
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

// Thrown: an uncaught error is what makes Bun exit non-zero.
if (failed) throw new Error('braces: see above');
console.log('braces: every user loads vendor/braces; deep nesting is refused');
