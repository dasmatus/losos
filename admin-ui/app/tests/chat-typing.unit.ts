/* The typing indicator in chat (src/chat/typing.ts): who the page names,
 * how it words it in each language, and what the composer sends to the
 * homeserver, including the throttle and the stop.
 *
 *   bun tests/chat-typing.unit.ts
 */

import assert from './assert.ts';
import type { Room } from 'matrix-js-sdk';
import { interpolate, template } from '../src/lib/i18n.ts';
import type { Locale } from '../src/lib/i18n.ts';
import { TYPING_RESEND_MS, TYPING_TIMEOUT_MS, TypingNotifier, typingLabel, typingMembers } from '../src/chat/typing.ts';

const ME = '@matus:b';
const member = (userId: string, name: string, typing: boolean) => ({ userId, name, typing });

// Who counts: everyone but me, and only while the SDK says they are typing.
const room = (members: ReturnType<typeof member>[]): Room => ({ getMembers: () => members }) as unknown as Room;
assert.deepEqual(typingMembers(room([]), ME), []);
assert.deepEqual(
  typingMembers(room([member('@eva:b', 'Eva', true), member(ME, 'Matus', true), member('@jan:b', 'Jan', false)]), ME),
  ['Eva'],
);
assert.deepEqual(
  typingMembers(room([member('@eva:b', 'Eva', true), member('@jan:b', 'Jan', true)]), ME),
  ['Eva', 'Jan'],
);

// The line says the same thing in all three languages, for one and for many.
const translate = (locale: Locale) => (key: Parameters<typeof template>[0], vars: Record<string, string>) => interpolate(template(key, vars, locale), vars, locale);
assert.equal(typingLabel([], translate('en')), '');
assert.equal(typingLabel(['Eva'], translate('en')), 'Eva is typing…');
assert.equal(typingLabel(['Eva'], translate('sk')), 'Eva píše…');
assert.equal(typingLabel(['Eva'], translate('de')), 'Eva schreibt…');
assert.equal(typingLabel(['Eva', 'Jan'], translate('en')), 'Eva, Jan are typing…');
assert.equal(typingLabel(['Eva', 'Jan'], translate('sk')), 'Eva, Jan píšu…');
assert.equal(typingLabel(['Eva', 'Jan'], translate('de')), 'Eva, Jan schreiben…');

// What the composer sends: typing on once, renewed at most every 3 s while
// it keeps being true, cleared at once when it stops, and sent again
// immediately when it starts over after a stop.
type Sent = [roomId: string, typing: boolean, timeout: number | undefined];
const ROOM = '!room:b';
let clock = 0;
const sent: Sent[] = [];
const client = {
  sendTyping: (roomId: string, typing: boolean, timeout: number | undefined): Promise<unknown> => {
    sent.push([roomId, typing, timeout]);
    return Promise.resolve({});
  },
};
const wire = new TypingNotifier(client, ROOM, () => clock);
wire.update(true);
assert.deepEqual(sent, [[ROOM, true, TYPING_TIMEOUT_MS]]);
assert.equal(wire.isTyping, true);

clock = TYPING_RESEND_MS - 1;
wire.update(true);
assert.equal(sent.length, 1, 'renewed before the timeout ran out');

clock = TYPING_RESEND_MS;
wire.update(true);
assert.deepEqual(sent, [[ROOM, true, TYPING_TIMEOUT_MS], [ROOM, true, TYPING_TIMEOUT_MS]]);

wire.update(false);
assert.deepEqual(sent.at(-1), [ROOM, false, 0]);
assert.equal(wire.isTyping, false);
wire.update(false);
wire.stop();
assert.equal(sent.length, 3, 'nothing was said, so nothing is taken back');

// A stop resets the throttle: typing again right away must be sent.
clock = TYPING_RESEND_MS + 1;
wire.update(true);
assert.equal(sent.length, 4);
assert.equal(wire.isTyping, true);
wire.stop();
assert.deepEqual(sent.at(-1), [ROOM, false, 0]);
assert.equal(wire.isTyping, false);

// An unreachable homeserver is not a crash: the page keeps taking keys.
let attempts = 0;
const offline = new TypingNotifier(
  { sendTyping: () => { attempts += 1; return Promise.reject(new Error('offline')); } },
  ROOM,
  () => 0,
);
offline.update(true);
await new Promise((resolve) => setTimeout(resolve, 0));
assert.equal(attempts, 1);

console.log('chat typing: ok');
