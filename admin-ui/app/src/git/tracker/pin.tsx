import { HugeiconsIcon } from "@hugeicons/react";
import { Copy01Icon, Link01Icon, Mail01Icon, Message01Icon, PinIcon, PinOffIcon } from "@hugeicons/core-free-icons";
import { openInNewTab, usePinItems } from "@/components/pin-menu";
import { usePins, useSavePins } from "@/components/pins-dialog";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { copyText, menuItemsFor, type MenuBuilder } from "@/lib/context-menu";
import { useT } from "@/lib/i18n-react";
import { canHandTo, chatShareHref, composeMailHref } from "@/lib/intents";
import { isPinned, pinKey, toggled, type Pin } from "@/lib/pins";

/* An issue or a project on the admin page's sidebar (lib/pins.ts), and the
 * right-click menu of an issue wherever the tracker draws one: its row in
 * a list, its card on a board. The pins are the box's one list, which only
 * an administrator may change, so for anyone else the pin items are not
 * drawn at all. */

declare module "@/lib/context-menu" {
  interface MenuSubjects {
    /** An issue of LosOS Git's tracker, right-clicked in a list or on a board. */
    "tracker.issue": { key: string; title: string };
  }
}

/** The address of an issue on this box, for a link that leaves the page. */
export function issueLink(key: string): string {
  return `${window.location.origin}/git/issues/${encodeURIComponent(key)}`;
}

/** Mail's composer and Chat's share sheet with an issue in them. */
export function issueHandOvers(issue: { key: string; title: string }): { mail: string; chat: string } {
  const link = issueLink(issue.key);
  return {
    mail: composeMailHref({ subject: `${issue.key} ${issue.title}`, body: `${issue.title}\n${link}\n` }),
    chat: chatShareHref({ text: `${issue.key} ${issue.title} ${link}` }),
  };
}

/** The pin toggle of an issue's or a project's top bar. */
export function PinButton({ pin, name }: { pin: Pin; name: string }) {
  const t = useT();
  const state = usePins();
  const { busy, save } = useSavePins();
  if (state.status !== "ready") return null;
  const pinned = isPinned(state.pins, pin);
  const full = !pinned && state.pins.length >= state.limit;
  const label = t(pinned ? "pins.menu.unpin" : "pins.menu.pin");
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-pressed={pinned}
      aria-label={label}
      title={full ? t("pins.full", { limit: state.limit }) : label}
      disabled={busy || full}
      onClick={() => void save(toggled(state.pins, pin), new Map([[pinKey(pin), name]]))}
      className={pinned ? "text-accent" : undefined}
      data-pin={pinKey(pin)}
    >
      <HugeiconsIcon icon={pinned ? PinOffIcon : PinIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
    </Button>
  );
}

/** The right-click menu of one issue: open it, copy it, pin it, hand it to
 *  Mail or Chat, and whatever other parts add for "tracker.issue". */
export function useIssueMenu(): (issue: { key: string; title: string }, open: () => void) => MenuBuilder {
  const t = useT();
  const items = usePinItems();
  return (issue, open) => () => {
    const hand = issueHandOvers(issue);
    const copied = (text: string) =>
      void copyText(text).then((ok) => {
        if (ok) toast.done(t("tracker.done.copied"));
      });
    return {
      label: `${issue.key} ${issue.title}`,
      sections: [
        [
          { id: "open", label: t("tracker.menu.open"), onSelect: open },
          items.newTab(() => openInNewTab(`/git/issues/${encodeURIComponent(issue.key)}`)),
        ],
        [
          { id: "copy-link", label: t("tracker.reader.copyLink"), icon: Link01Icon, onSelect: () => copied(issueLink(issue.key)) },
          { id: "copy-key", label: t("tracker.reader.copyKey"), icon: Copy01Icon, onSelect: () => copied(`${issue.key} ${issue.title}`) },
        ],
        [items.pin({ kind: "issue", key: issue.key }, issue.title)],
        [
          {
            id: "mail",
            label: t("tracker.hand.mail"),
            icon: Mail01Icon,
            hidden: !canHandTo("mail"),
            onSelect: () => window.location.assign(hand.mail),
          },
          {
            id: "chat",
            label: t("tracker.hand.chat"),
            icon: Message01Icon,
            hidden: !canHandTo("chat"),
            onSelect: () => window.location.assign(hand.chat),
          },
        ],
        menuItemsFor("tracker.issue", issue),
      ],
    };
  };
}
