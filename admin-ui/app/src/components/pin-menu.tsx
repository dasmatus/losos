import { LinkSquare02Icon, PinIcon, PinOffIcon } from "@hugeicons/core-free-icons";
import type { MenuCommand } from "@/lib/context-menu";
import { useT } from "@/lib/i18n-react";
import { isPinned, toggled, type Pin } from "@/lib/pins";
import { usePins, useSavePins } from "./pins-dialog";

/* The items a right-click menu (components/context-menu.tsx) offers on
 * anything that can be pinned: a file, folder or repository in the command
 * palette, an app tile on the Overview, a pin in the sidebar. Pin to
 * sidebar or Unpin, and Open in new tab, since the custom menu takes the
 * place of the browser's own. Signed out there is no list to change, so
 * there is no pin item. */

export function usePinItems(): {
  pin: (pin: Pin | null) => MenuCommand;
  newTab: (open: () => void) => MenuCommand;
} {
  const t = useT();
  const state = usePins();
  const { busy, save } = useSavePins();
  return {
    pin: (pin) => {
      const ready = pin !== null && state.status === "ready";
      const pinned = ready && isPinned(state.pins, pin);
      const full = ready && !pinned && state.pins.length >= state.limit;
      return {
        id: "pin",
        label: t(pinned ? "pins.menu.unpin" : "pins.menu.pin"),
        icon: pinned ? PinOffIcon : PinIcon,
        hidden: !ready,
        disabled: busy || full,
        title: full ? t("pins.full", { limit: state.limit }) : undefined,
        onSelect: () => {
          if (pin !== null) void save(toggled(state.pins, pin));
        },
      };
    },
    newTab: (open) => ({ id: "new-tab", label: t("pins.menu.newTab"), icon: LinkSquare02Icon, onSelect: open }),
  };
}

/** Open an address of this box in a new tab, the way the palette's Ctrl+Enter does. */
export function openInNewTab(href: string): void {
  window.open(href, "_blank", "noopener");
}
