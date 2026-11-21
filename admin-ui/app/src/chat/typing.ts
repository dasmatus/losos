/* Who is typing in a room, and what the composer tells the homeserver.
 *
 * The page shows the same line in two places (the open room's timeline and
 * the room list), so both read it here: `typingMembers` picks the people who
 * are typing, `typingLabel` words it, and `TypingNotifier` is the sending
 * side. Kept apart from React so the throttle and stop rules can be tested
 * without a browser (tests/chat-typing.unit.ts). */

import type { Room } from "matrix-js-sdk";

/** What Synapse keeps a typing state for, and how often the composer may
 *  renew it while someone keeps writing. */
export const TYPING_TIMEOUT_MS = 5000;
export const TYPING_RESEND_MS = 3000;

/** The part of matrix-js-sdk's client the sender uses. */
export interface TypingSender {
  sendTyping(roomId: string, isTyping: boolean, timeoutMs?: number): Promise<unknown>;
}

/** The people whose typing line this page draws: everyone in the room who
 *  is typing except the person reading. */
export function typingMembers(room: Room, me: string): string[] {
  return room
    .getMembers()
    .filter((member) => member.typing && member.userId !== me)
    .map((member) => member.name);
}

/** "Eva is typing…", "Eva, Jan are typing…", or nothing when nobody is. */
export type TypingTranslator = (key: "chat.typing.one" | "chat.typing.many", vars: { name: string } | { names: string }) => string;

export function typingLabel(typers: readonly string[], t: TypingTranslator): string {
  if (typers.length === 0) return "";
  if (typers.length === 1) return t("chat.typing.one", { name: typers[0] ?? "" });
  return t("chat.typing.many", { names: typers.join(", ") });
}

/** The composer's half of a typing indicator.
 *
 *  Typing is not a state anyone can leave set: the homeserver forgets it
 *  after `TYPING_TIMEOUT_MS`, and the page renews it at most once every
 *  `TYPING_RESEND_MS` while the box has text in it. `stop` (and therefore
 *  `false`) is sent when the box empties, loses focus, the message is sent,
 *  the room is left or the page closes, so the other side does not wait out
 *  the timeout either. */
export class TypingNotifier {
  private sentAt = 0;
  private on = false;

  constructor(
    private readonly client: TypingSender,
    private readonly roomId: string,
    private readonly now: () => number = Date.now,
  ) {}

  get isTyping(): boolean {
    return this.on;
  }

  /** Tell the homeserver whether this person is typing, as of now. */
  update(typing: boolean): void {
    if (typing) {
      const at = this.now();
      // A renewal, not a new keystroke: the timeout has not run out yet.
      if (this.on && at - this.sentAt < TYPING_RESEND_MS) return;
      this.on = true;
      this.sentAt = at;
      void this.client.sendTyping(this.roomId, true, TYPING_TIMEOUT_MS).catch(() => undefined);
      return;
    }
    // Nothing was said, so nothing needs taking back.
    if (!this.on) return;
    this.on = false;
    this.sentAt = 0;
    void this.client.sendTyping(this.roomId, false, 0).catch(() => undefined);
  }

  /** Stop, once, whatever the reason (send, blur, unmount). */
  stop(): void {
    this.update(false);
  }
}
