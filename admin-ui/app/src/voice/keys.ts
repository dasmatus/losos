/* Two sessions in one page, one device.
 *
 * The agent joins the call as a second session of the page that added it,
 * as the same user and device, so the keys the two send each other have no
 * Olm session to travel over: a device does not write to itself. They pass
 * through this hub instead, in memory, and everyone else's keys still go
 * over the real channel. Keys that arrive from other people reach both
 * sessions, each of which opens only the pids it was told about. */

import type { KeyChannel, KeyMessage, Sender } from "@/calls/session";

type Handler = (from: Sender, msg: KeyMessage) => void;

export class KeyHub {
  private ends = new Set<{ handlers: Set<Handler> }>();

  constructor(
    private base: KeyChannel,
    private self: { user: string; device: string },
  ) {}

  /* A channel for one more session of this page. */
  channel(): KeyChannel {
    const end = { handlers: new Set<Handler>() };
    this.ends.add(end);
    const here = (d: { user: string; device: string }): boolean => d.user === this.self.user && d.device === this.self.device;
    return {
      send: async (to, msg) => {
        const far = to.filter((d) => !here(d));
        if (to.some(here)) {
          for (const other of this.ends) {
            if (other === end) continue;
            for (const h of other.handlers) h({ user: this.self.user, device: this.self.device }, msg);
          }
        }
        // One message per device, however many members it carries.
        const seen = new Set<string>();
        const once = far.filter((d) => {
          const id = `${d.user}\n${d.device}`;
          if (seen.has(id)) return false;
          seen.add(id);
          return true;
        });
        if (once.length > 0) await this.base.send(once, msg);
      },
      subscribe: (handler) => {
        end.handlers.add(handler);
        const off = this.base.subscribe(handler);
        return () => {
          end.handlers.delete(handler);
          this.ends.delete(end);
          off();
        };
      },
    };
  }
}
