/* LosOS Calendar's reminders: wherever the person is in the admin UI, an
 * event's reminder (its VALARM, minutes before the start) shows as a
 * notice with the event and, for a call, the button that joins it. Once a
 * minute the page asks for today's and tomorrow's events; a reminder that
 * came due since the last look is shown once per page. With the browser's
 * permission it is a system notification too, so a box open in a
 * background tab still says it.
 *
 * Mounted by the shell only while Calendar is listed for the person, so a
 * box without LosOS Cloud asks nothing. */

import * as React from "react";
import { getCalendarEvents, sharedApp, useMe, type CalendarEvent } from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { useLocale, useT } from "@/lib/i18n-react";
import { chatWithHref } from "@/lib/intents";
import { addDays, dayOf, span } from "./time";
import { myAddress, othersIn } from "./event";

const LOOK_MS = 60_000;

/** The reminders of `events` that come due in `(after, upTo]`. */
export function dueReminders(events: CalendarEvent[], after: number, upTo: number): { event: CalendarEvent; at: number; key: string }[] {
  const out: { event: CalendarEvent; at: number; key: string }[] = [];
  for (const e of events) {
    const [start] = span(e);
    for (const m of e.alarms) {
      const at = start - Math.max(m, 0) * 60_000;
      if (at > after && at <= upTo) out.push({ event: e, at, key: `${e.uid}|${e.occurrence ?? e.start}|${m}` });
    }
  }
  return out.sort((a, b) => a.at - b.at);
}

export default function Reminders() {
  const t = useT();
  const locale = useLocale();
  const me = useMe();
  const on = me !== null && !me.spareKey && sharedApp("calendar");
  const shown = React.useRef(new Set<string>());

  React.useEffect(() => {
    if (!on) return;
    // A reminder due while the page was closed is not news; from now on.
    let last = Date.now() - LOOK_MS;
    let stopped = false;
    const look = async () => {
      const now = Date.now();
      const today = dayOf(new Date(now));
      try {
        const events = await getCalendarEvents(today, addDays(today, 2));
        if (stopped) return;
        const mine = await myAddress();
        for (const r of dueReminders(events, last, now)) {
          if (shown.current.has(r.key)) continue;
          shown.current.add(r.key);
          const [start] = span(r.event);
          const when =
            r.event.allDay
              ? t("calendar.allDay")
              : new Intl.DateTimeFormat(locale, { hour: "numeric", minute: "2-digit" }).format(new Date(start));
          const title = r.event.summary || t("calendar.untitled");
          const other = othersIn(r.event, mine).find((p) => p.matrix !== undefined && p.matrix !== "");
          const isCall = (r.event.call === "video" || r.event.call === "voice") && other?.matrix !== undefined && sharedApp("chat");
          toast.done(t("calendar.reminder.title", { title }), t("calendar.reminder.at", { when }), {
            duration: 120_000,
            action: isCall
              ? {
                  label: t("calendar.call.join"),
                  onClick: () => window.location.assign(chatWithHref(other?.matrix ?? "", { call: r.event.call === "voice" ? "voice" : "video" })),
                }
              : { label: t("calendar.reminder.open"), onClick: () => window.location.assign(`/calendar?event=${encodeURIComponent(r.event.id)}`) },
          });
          if (document.hidden && "Notification" in window && Notification.permission === "granted") {
            try {
              new Notification(t("calendar.reminder.title", { title }), { body: t("calendar.reminder.at", { when }), tag: r.key });
            } catch {
              /* the notice in the page is enough */
            }
          }
        }
        last = now;
      } catch {
        /* Cloud is down for a moment: the next look tries again */
      }
    };
    void look();
    const timer = window.setInterval(() => void look(), LOOK_MS);
    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [on, t, locale]);

  return null;
}
