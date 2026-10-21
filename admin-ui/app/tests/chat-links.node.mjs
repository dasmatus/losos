/* The links other screens make into chat (src/chat/links.ts): a whole
 * Matrix ID or a bare sign-in name in `?with=`, and what is refused.
 *
 *   node tests/chat-links.node.mjs
 */

import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import * as esbuild from 'esbuild';

const built = await esbuild.build({
  entryPoints: [fileURLToPath(new URL('../src/chat/links.ts', import.meta.url))],
  bundle: true,
  format: 'esm',
  platform: 'node',
  write: false,
});
const { withUserId, withoutWith } = await import(`data:text/javascript;base64,${Buffer.from(built.outputFiles[0].text).toString('base64')}`);

const server = 'mattbox.local';
assert.equal(withUserId('eva', server), '@eva:mattbox.local');
assert.equal(withUserId(' eva ', server), '@eva:mattbox.local');
assert.equal(withUserId('@eva:mattbox.local', server), '@eva:mattbox.local');
assert.equal(withUserId('@eva:other.example', server), '@eva:other.example');
assert.equal(withUserId(null, server), null);
assert.equal(withUserId('', server), null);
assert.equal(withUserId('Eva', server), null);
assert.equal(withUserId('eva smith', server), null);
assert.equal(withUserId('@eva', server), null);
assert.equal(withUserId('@:mattbox.local', server), null);
assert.equal(withUserId('@eva:', server), null);
assert.equal(withUserId('eva:mattbox.local', server), null);

assert.equal(withoutWith('https://mattbox.local/chat/?with=eva'), '/chat/');
assert.equal(withoutWith('https://mattbox.local/chat/?with=eva&call=video#/room/x'), '/chat/?call=video#/room/x');

console.log('chat links: ok');
