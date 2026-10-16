import * as React from "react";
import { ContextMenu } from "@base-ui/react/context-menu";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { LinkSquare02Icon, PinIcon, PinOffIcon } from "@hugeicons/core-free-icons";
import { useT } from "@/lib/i18n-react";
import { isPinned, toggled, type Pin } from "@/lib/pins";
import { cn } from "@/lib/utils";
import { usePins, useSavePins } from "./pins-dialog";

/* The right-click menu on anything that can be pinned: a file, folder or
 * repository in the command palette, an app tile on the Overview, a pin in
 * the sidebar. It pins to the sidebar or unpins, and opens the item in a new
 * tab, since a custom menu takes the place of the browser's own. A long
 * press opens it on a touch screen.
 *
 * shadcn's base-registry Context Menu on Base UI, like the Sidebar: its
 * scroll lock and positioner write CSSOM, which the admin CSP allows. Inside
 * a native <dialog> (the palette) the popup has to be portalled into that
 * dialog, because the dialog sits in the top layer above anything in <body>.
 *
 * With no list to change (signed out) and nothing to open, there is no menu
 * and the browser's own one stays. */

export function PinMenu({
  pin,
  name,
  newTab,
  children,
}: {
  /** What "Pin to sidebar" adds; null when the item cannot be pinned. */
  pin: Pin | null;
  /** The item's name, for the menu's accessible label. */
  name: string;
  /** Opens the item in a new tab; absent when it has nowhere to open. */
  newTab?: () => void;
  /** The element right-clicked: one element, which takes the trigger's props. */
  children: React.ReactElement;
}) {
  const t = useT();
  const state = usePins();
  const { busy, save } = useSavePins();
  const [container, setContainer] = React.useState<HTMLElement | undefined>(undefined);

  const canPin = pin !== null && state.status === "ready";
  if (!canPin && newTab === undefined) return children;

  const pinned = canPin && isPinned(state.pins, pin);
  const full = canPin && !pinned && state.pins.length >= state.limit;

  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger
        render={children}
        onContextMenu={(event: React.MouseEvent<HTMLElement>) => {
          const dialog = event.currentTarget.closest("dialog");
          setContainer(dialog ?? undefined);
        }}
      />
      <ContextMenu.Portal container={container}>
        <ContextMenu.Positioner className="isolate z-50 outline-none">
          <ContextMenu.Popup
            data-slot="pin-menu"
            aria-label={name}
            className={cn(
              "min-w-48 rounded-card border border-line bg-surface p-1 text-ink shadow-md outline-none",
              "origin-(--transform-origin) transition-[opacity,scale] duration-100 ease-out",
              "data-starting-style:scale-95 data-starting-style:opacity-0",
              "data-ending-style:scale-95 data-ending-style:opacity-0",
            )}
          >
            {canPin && (
              <MenuItem
                icon={pinned ? PinOffIcon : PinIcon}
                disabled={busy || full}
                title={full ? t("pins.full", { limit: state.limit }) : undefined}
                onClick={() => void save(toggled(state.pins, pin))}
              >
                {t(pinned ? "pins.menu.unpin" : "pins.menu.pin")}
              </MenuItem>
            )}
            {newTab !== undefined && (
              <MenuItem icon={LinkSquare02Icon} onClick={newTab}>
                {t("pins.menu.newTab")}
              </MenuItem>
            )}
          </ContextMenu.Popup>
        </ContextMenu.Positioner>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

function MenuItem({
  icon,
  disabled,
  title,
  onClick,
  children,
}: {
  icon: IconSvgElement;
  disabled?: boolean;
  title?: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <ContextMenu.Item
      disabled={disabled}
      title={title}
      onClick={onClick}
      className={cn(
        "flex cursor-default items-center gap-2 rounded-control px-2 py-1.5 text-[13px] outline-none select-none",
        "data-highlighted:bg-accent-wash data-highlighted:text-ink",
        "data-disabled:cursor-not-allowed data-disabled:text-faint",
      )}
    >
      <HugeiconsIcon icon={icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      {children}
    </ContextMenu.Item>
  );
}

/** Open an address of this box in a new tab, the way the palette's Ctrl+Enter does. */
export function openInNewTab(href: string): void {
  window.open(href, "_blank", "noopener");
}
