/* src/css/tokens.css must be the admin UI's palette file byte for byte, the
 * way the Nextcloud and Forgejo themes carry it: one colour changed there
 * changes here too, or the handbook stops reading as the same product. The
 * copy exists because the box's build of this site (flake/packages.nix) sees
 * handbook/ alone, never admin-ui/. Skipped where admin-ui/ is absent. */
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const copy = join(here, '..', 'src', 'css', 'tokens.css');
const source = join(here, '..', '..', 'admin-ui', 'app', 'src', 'styles', 'tokens.css');

if (!existsSync(source)) {
  console.log('admin-ui/ is not beside handbook/; the tokens copy is not checked');
  process.exit(0);
}
if (readFileSync(copy, 'utf8') !== readFileSync(source, 'utf8')) {
  console.error(`handbook/src/css/tokens.css differs from ${source}; copy it over: cp ${source} ${copy}`);
  process.exit(1);
}
console.log('handbook/src/css/tokens.css is the admin UI tokens.css, byte for byte');
