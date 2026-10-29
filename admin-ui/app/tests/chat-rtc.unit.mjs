/* How chat reads a room's call from MatrixRTC (src/chat/rtc.ts): which
 * memberships count, which transport a page may follow, and the notice
 * left for other Matrix apps.
 *
 *   bun tests/chat-rtc.unit.mjs
 */

import assert from './assert.mjs';

globalThis.location = new URL('https://mattbox.local/chat/');
const rtc = await import('../src/chat/rtc.ts');

const MEMBER = 'org.matrix.msc3401.call.member';
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
const membership = (sender, device, call, { ts = now, expires = 300_000, application = 'org.losos.call', slot = 'room' } = {}) =>
  event(MEMBER, sender, {
    application,
    call_id: slot,
    scope: 'm.room',
    device_id: device,
    expires,
    'm.call.intent': 'video',
    focus_active: { type: 'livekit', focus_selection: 'multi_sfu' },
    foci_preferred: [transport(call)],
  }, ts);
const room = (state, { joined = ['@matus:b', '@eva:b'], timeline = [] } = {}) => ({
  myUserId: '@matus:b',
  hasMembershipState: (user, m) => m === 'join' && joined.includes(user),
  findEventById: (id) => timeline.find((e) => e.getId() === id),
  currentState: {
    getStateEvents: (type, key) => {
      const all = state.filter((e) => e.getType() === type);
      return key === undefined ? all : all.find((e) => e.key === key);
    },
    maySendStateEvent: () => true,
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

// Members count only while the slot is open, oldest first; expired ones,
// other applications and people who left the room do not.
const first = membership('@eva:b', 'E', CALL, { ts: now - 60_000 });
const second = membership('@matus:b', 'M', CALL);
const members = [second, first, membership('@jan:b', 'J', OTHER), membership('@eva:b', 'E2', OTHER, { ts: now - 400_000 }),
  membership('@eva:b', 'E3', OTHER, { application: 'm.call', slot: '' })];
assert.deepEqual(rtc.rtcMembers(room(members)), []);
const open = room([slot(), ...members]);
assert.deepEqual(rtc.rtcMembers(open).map((m) => [m.user, m.device]), [['@eva:b', 'E'], ['@matus:b', 'M']]);
assert.deepEqual(rtc.liveCall(open), { call: CALL, video: true });
assert.ok(rtc.callLive(open, CALL));
assert.ok(!rtc.callLive(open, OTHER));
assert.ok(!rtc.slotOpen(room([slot({ status: 'closed', application: { type: 'org.losos.call' } })])));
assert.ok(!rtc.slotOpen(room([slot({ status: 'open', application: { type: 'm.call' } })])));

// A notification names its call through the membership it refers to,
// in the room state or, once that has moved on, in the timeline.
assert.deepEqual(rtc.callOfMembership(open, first.getId()), { call: CALL, video: true });
const gone = membership('@jan:b', 'J', OTHER);
assert.deepEqual(rtc.callOfMembership(room([slot()], { timeline: [gone] }), gone.getId()), { call: OTHER, video: true });
assert.equal(rtc.callOfMembership(open, '$missing'), null);

// New private rooms let every member send a membership and open the slot.
assert.equal(rtc.CALL_POWER_LEVELS.events[MEMBER], 0);
assert.equal(rtc.CALL_POWER_LEVELS.events['m.room.power_levels'], 100);
assert.deepEqual(rtc.slotState, { type: SLOT, state_key: 'org.losos.call#room', content: rtc.SLOT_CONTENT });
assert.equal(rtc.SLOT_CONTENT.encryption.type, 'org.matrix.msc4143.per_member');

// The notice for other apps is recognised and kept out of the timeline.
assert.ok(rtc.isStartNotice({ msgtype: 'm.notice', body: 'Matus started a call', 'org.losos.call': { call_id: CALL } }));
assert.ok(!rtc.isStartNotice({ msgtype: 'm.notice', body: 'org.losos.call' }));
assert.ok(!rtc.isStartNotice({ msgtype: 'm.text', 'org.losos.call': { call_id: CALL } }));

console.log('chat rtc: ok');
