/* The links other screens make into chat (src/chat/links.ts): a whole
 * Matrix ID or a bare sign-in name in `?with=`, and what is refused.
 *
 *   bun tests/chat-links.unit.mjs
 */

import assert from './assert.mjs';

const { withUserId, withoutLink, linkedCall, withoutCall } = await import('../src/chat/links.ts');

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

assert.equal(withoutLink('https://mattbox.local/chat/?with=eva'), '/chat/');
assert.equal(withoutLink('https://mattbox.local/chat/?with=eva&call=video#/room/x'), '/chat/?call=video#/room/x');
assert.equal(withoutLink('https://mattbox.local/chat/?share=Docs%2Fa.pdf&share=b.png&text=hi'), '/chat/');

assert.equal(linkedCall('?with=eva&call=video'), true);
assert.equal(linkedCall('?with=eva&call=voice'), false);
assert.equal(linkedCall('?with=eva'), null);
assert.equal(linkedCall('?with=eva&call=fax'), null);
assert.equal(withoutCall('/chat/?call=video#/room/x'), '/chat/#/room/x');
assert.equal(withoutCall('/chat/?x=1&call=voice'), '/chat/?x=1');

console.log('chat links: ok');
