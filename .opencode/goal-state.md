# Goal: typing indicators in LosOS Chat

## Goal statement

While someone types in a room in LosOS Chat, every other member of that room
sees "<name> is typing…" (and "<names> are typing…" for several) both in the
open room's timeline and in the room list, and the line goes away when they
send, clear the box, blur it, switch rooms or close the page. Typing state is
sent to the homeserver no more than once every 3 s and expires on its own
after 5 s.

Measurable outcome:

1. `bun tests/chat-typing.unit.ts` passes, covering:
   - which members count as typing (not me, typing flag set, names resolved),
   - the one/many wording in all three locales,
   - the sender: one send per 3 s, immediate resend after a stop, stop on
     clear/blur/send, and the 5 s timeout,
   - an unreachable homeserver not breaking the composer.
2. `bun run typecheck` (both tsconfigs) passes with the new module wired in.
3. `bun build.ts` bundles the chat page with the new module.
4. Both the room list and the timeline use the same tested helper, so the
   line cannot disagree between them.

## Milestones

| # | Milestone | Status |
|---|-----------|--------|
| 1 | Audit the existing typing path; record what already works and what is missing | verified |
| 2 | `src/chat/typing.ts`: `TypingNotifier` (send/throttle/stop), `typingMembers`, `typingLabel` | verified |
| 3 | Composer uses `TypingNotifier`, including stop on unmount/room switch | verified |
| 4 | Room list shows the typing line (`RoomMemberEvent.Typing` per row) | verified |
| 5 | `tests/chat-typing.unit.ts` and its entry in `package.json`'s `test:browser` | verified |
| 6 | Verification: unit test, typecheck, production bundle, handbook copy | verified |
| 7 | End-to-end assertion in the box test (two browsers) | added, not run here (needs the VM) |

## Evidence per milestone

1. **Audit.** `composer.tsx` already called `client.sendTyping(room, on,
   5000)` behind a 3 s throttle, and `timeline.tsx` already drew
   `chat.typing.one/many`. `grep -rn "typing" admin-ui/app/src/chat/` showed
   no typing state in `room-list.tsx` or `thread-panel.tsx`, and no test
   anywhere. matrix-js-sdk 45 re-emits `RoomMemberEvent.Typing` from each
   member onto the client (`src/sync.ts` `reEmitter.reEmit(member, […])`,
   `src/models/room-member.ts` `setTypingEvent`), so a client listener is
   correct; `RoomState.setTypingEvent` updates every member in state.
2. **`typing.ts`.** New module with `typingMembers` (not me, `typing` flag,
   names), `typingLabel` (one/many), and `TypingNotifier` (injected clock;
   5 s timeout, 3 s resend, immediate stop/reset). Check:
   `cd admin-ui/app && bun tests/chat-typing.unit.ts` → `chat typing: ok`.
3. **Composer.** The ad-hoc `typingSent` ref is gone; the composer builds one
   `TypingNotifier` per mount and calls `update(on)` from the textarea. An
   unmount effect calls `stop()`, which covers leaving the room (`RoomView`
   is keyed by room ID) and closing the page. `send()` and blur still stop.
   Verified by the unit test's stop cases plus `tsc` and the bundle below.
4. **Room list.** `RoomRow` listens on `client` for
   `RoomMemberEvent.Typing`, recomputes `typingMembers`, and shows
   `typingLabel` in place of the preview (in the accent colour) while anyone
   is typing. Verified by `tsc`, by the production bundle containing the
   strings, and by the shared helper's unit test; a live visual check would
   need the box test below.
5. **Test + entry.** `tests/chat-typing.unit.ts` is in the `test:browser`
   chain right after `chat-rtc.unit.ts`; the suite's chat unit tests all pass.
6. **Final checks.** Recorded below after running them.
7. **Box test.** `nix/tests/chat-browser.ts` now waits, on Eva's page, for
   "Matus is typing…" to appear while Matus types and to disappear after the
   send. This runs in `nix build .#checks.x86_64-linux.losos-chat`; it was
   not run in this session (the VM test builds Synapse, nginx and the whole
   UI), so it is a CI gate rather than evidence gathered here.

## Checks run

```
$ cd admin-ui/app && bun tests/chat-typing.unit.ts
chat typing: ok
$ cd admin-ui/app && bun tests/chat-links.unit.ts && bun tests/chat-rtc.unit.ts && bun tests/chat-typing.unit.ts
chat links: ok
chat rtc: ok
chat typing: ok
$ cd admin-ui/app && bun run typecheck        # both tsconfigs
(no output; exit 0)
$ cd admin-ui/app && bun build.ts --outdir /tmp/opencode/losos-dist
(no output; exit 0; dist/assets/index-*.js carries "is typing")
```

## Key decisions

- Typing already shipped in the first chat commit (`7d6672de`): a sender in
  `Composer` and a line in `RoomTimeline`. The work was therefore to close
  the three real gaps — no room-list indicator, no stop on unmount, no tests
  — instead of writing a second implementation.
- Listen on the client, not the room: matrix-js-sdk re-emits
  `RoomMemberEvent.Typing` from members onto the client; the room does not
  re-emit it itself.
- Keep the sender in a tiny class with an injected clock, so the throttle and
  stop contract is testable without a browser or a homeserver.
- The room list shows the typing line instead of the preview, and only while
  someone types; the preview and its `Masked` treatment are untouched
  otherwise.
- `thread-panel.tsx` needs nothing: typing is room-scoped and the main
  timeline already shows it.
- Handbook copy (EN/SK/DE) now mentions the indicator; no screenshots were
  regenerated (the existing chat screenshot predates the room-list line).

## Blockers

None. The only unrun check is the two-browser VM test (milestone 7), which is
a CI gate; everything that can run in a checkout has run and passed.
