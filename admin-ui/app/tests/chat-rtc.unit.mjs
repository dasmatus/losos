/* How chat reads a room's call from MatrixRTC (src/chat/rtc.ts): which
 * memberships count, which transport a page may follow, and the notice
 * left for other Matrix apps.
 *
 *   bun tests/chat-rtc.unit.mjs
 */

import assert from './assert.mjs';

globalThis.location = new URL('https://mattbox.local/chat/');
const rtc = await import('../src/chat/rtc.ts');

const MEMBER = 'org.matrix.msc4143.rtc.member';
const SLOT = 'org.matrix.msc4143.rtc.slot';
const CALL = 'a'.repeat(32);
const OTHER = 'b'.repeat(32);
const now = Date.now();

let n = 0;
const event = (type, sender, content, ts = now) => {
  const id = `$e${n++}`;
  return { getId: () => id, getType: () => type, getSender: () => sender, getContent: () => content, getTs: () => ts };
};
const transport = (call, url = 'wss://mattbox.local/calls/api/ws') => ({ type: 'org.losos.sfu', url, call });
// A sticky membership as matrix-js-sdk's StickyEventMembershipManager sends it.
const membership = (sender, device, call, { ts = now, application = 'org.losos.call', slot = 'org.losos.call#room', user = sender } = {}) =>
  event(MEMBER, sender, {
    application: { type: application, 'm.call.intent': 'video' },
    slot_id: slot,
    transports: { published: [transport(call)], can_subscribe: ['livekit'] },
    member: { user_id: user, device_id: device, id: `${user}:${device}` },
    versions: [],
    msc4354_sticky_key: `${user}:${device}`,
  }, ts);
const withdrawn = (sender, device) => event(MEMBER, sender, { msc4354_sticky_key: `${sender}:${device}` });
const room = (state, { sticky = [], joined = ['@matus:b', '@eva:b'], timeline = [] } = {}) => ({
  myUserId: '@matus:b',
  hasMembershipState: (user, m) => m === 'join' && joined.includes(user),
  findEventById: (id) => timeline.find((e) => e.getId() === id),
  _unstable_getStickyEvents: () => sticky.values(),
  currentState: {
    getStateEvents: (type, key) => {
      const all = state.filter((e) => e.getType() === type);
      return key === undefined ? all : all.find((e) => e.key === key);
    },
    maySendStateEvent: () => true,
    maySendEvent: () => true,
  },
});
const slot = (content = rtc.SLOT_CONTENT) => Object.assign(event(SLOT, '@matus:b', content), { key: rtc.RTC_SLOT_ID });

assert.equal(rtc.RTC_SLOT_ID, 'org.losos.call#room');

// A transport is followed only to the SFU's websocket path.
assert.deepEqual(rtc.sfuTransport(transport(CALL)), transport(CALL));
assert.deepEqual(rtc.sfuTransport(transport(CALL, 'wss://other.example/calls/api/ws')), transport(CALL, 'wss://other.example/calls/api/ws'));
assert.equal(rtc.sfuTransport(transport(CALL, 'https://mattbox.local/calls/api/ws')), null);
assert.equal(rtc.sfuTransport(transport(CALL, 'wss://mattbox.local/elsewhere')), null);
assert.equal(rtc.sfuTransport(transport('../etc')), null);
assert.equal(rtc.sfuTransport({ type: 'livekit', livekit_service_url: 'https://x' }), null);
// Whatever the membership says, this page connects through its own origin.
assert.equal(rtc.sfuUrl(), 'wss://mattbox.local/calls/api/ws');

// Members count only while the slot is open, oldest first; withdrawn
// ones, other applications and slots, someone else's member, and people
// who left the room do not.
const first = membership('@eva:b', 'E', CALL, { ts: now - 60_000 });
const second = membership('@matus:b', 'M', CALL);
const sticky = [second, first, membership('@jan:b', 'J', OTHER), withdrawn('@eva:b', 'E2'),
  membership('@eva:b', 'E3', OTHER, { application: 'm.call', slot: 'm.call#' }),
  membership('@eva:b', 'E4', OTHER, { slot: 'org.losos.call#other' }),
  membership('@eva:b', 'E5', OTHER, { user: '@matus:b' })];
assert.deepEqual(rtc.rtcMembers(room([], { sticky })), []);
const open = room([slot()], { sticky });
assert.deepEqual(rtc.rtcMembers(open).map((m) => [m.user, m.device]), [['@eva:b', 'E'], ['@matus:b', 'M']]);
assert.deepEqual(rtc.liveCall(open), { call: CALL, video: true });
assert.ok(rtc.callLive(open, CALL));
assert.ok(!rtc.callLive(open, OTHER));
assert.ok(!rtc.slotOpen(room([slot({ status: 'closed', application: { type: 'org.losos.call' } })])));
assert.ok(!rtc.slotOpen(room([slot({ status: 'open', application: { type: 'm.call' } })])));

// A notification names its call through the membership it refers to,
// among the sticky events or, once that has moved on, in the timeline.
assert.deepEqual(rtc.callOfMembership(open, first.getId()), { call: CALL, video: true });
const gone = membership('@jan:b', 'J', OTHER);
assert.deepEqual(rtc.callOfMembership(room([slot()], { timeline: [gone] }), gone.getId()), { call: OTHER, video: true });
assert.equal(rtc.callOfMembership(open, '$missing'), null);

// New private rooms open the slot; any member may then send a membership.
assert.deepEqual(rtc.slotState, { type: SLOT, state_key: 'org.losos.call#room', content: rtc.SLOT_CONTENT });
assert.equal(rtc.SLOT_CONTENT.encryption.type, 'org.matrix.msc4143.per_member');
assert.ok(rtc.mayCall(open, '@eva:b'));
assert.ok(!rtc.mayCall(room([]), '@eva:b'));

// The notice for other apps is recognised and kept out of the timeline.
assert.ok(rtc.isStartNotice({ msgtype: 'm.notice', body: 'Matus started a call', 'org.losos.call': { call_id: CALL } }));
assert.ok(!rtc.isStartNotice({ msgtype: 'm.notice', body: 'org.losos.call' }));
assert.ok(!rtc.isStartNotice({ msgtype: 'm.text', 'org.losos.call': { call_id: CALL } }));

console.log('chat rtc: ok');
