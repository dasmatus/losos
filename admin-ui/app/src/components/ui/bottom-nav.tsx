import * as React from "react";
import { createPortal } from "react-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Menu01Icon } from "@hugeicons/core-free-icons";
import { useSidebar } from "@/components/ui/sidebar";
import { useT } from "@/lib/i18n-react";
import { useSiteLink } from "@/lib/site";
import { cn } from "@/lib/utils";

/* The phone's navigation: a bar along the bottom edge, where the thumb
 * already is, in place of the sidebar. It holds the few places a page is
 * used for and a More that raises the whole sidebar as a sheet from the
 * same edge (Sidebar's phone branch, components/ui/drawer.tsx), so every
 * destination stays one press away and nothing is listed twice at the top.
 *
 * It lives inside a SidebarProvider and draws only while that provider is
 * on a phone (below md, the same query that turns the sidebar into a
 * sheet). Fixed to the viewport, so it goes to <body> through a portal: an
 * ancestor with a transform or a filter would otherwise pin it to itself.
 * The one exception is a shell drawn as a picture of itself, the wizard's
 * desktop preview (App.tsx): that names itself as the host, and the bar
 * goes inside it, scaled with the rest and out of reach like the rest.
 * Its height reaches the page as --bottom-nav (index.css), which pads the
 * shell's foot and lifts anything else that floats at the bottom edge.
 *
 * The bar is the top bar's mirror: the same translucent surface, blur and
 * hairline, and the home indicator's safe area below the tabs. A tab's
 * choice is marked by a wash that widens out from under its icon; it is a
 * transition, so a quick second press turns it round mid-way. Presses sink
 * to 97 % like every Button. */

export interface BottomNavTab {
  key: string;
  /** An address on the box; lib/site.tsx makes the link. */
  to: string;
  label: string;
  icon: IconSvgElement;
  active: boolean;
}

const TAB = cn(
  "group/tab flex h-14 w-full min-w-0 flex-col items-center justify-center gap-1 rounded-control text-muted",
  "transition-[color,scale] duration-150 ease-snap active:scale-[0.97] data-active:text-ink",
  "focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent",
);

function Glyph({ icon }: { icon: IconSvgElement }) {
  return (
    <span className="relative flex h-7 w-14 items-center justify-center">
      <span
        aria-hidden="true"
        className={cn(
          "absolute inset-0 rounded-full bg-accent-wash opacity-0 scale-x-50",
          "transition-[opacity,scale] duration-200 ease-snap",
          "group-data-active/tab:opacity-100 group-data-active/tab:scale-x-100",
        )}
      />
      <HugeiconsIcon
        icon={icon}
        size={20}
        strokeWidth={1.5}
        color="currentColor"
        aria-hidden="true"
        className="relative transition-colors duration-150 group-data-active/tab:text-accent"
      />
    </span>
  );
}

function Label({ children }: { children: string }) {
  return <span className="max-w-full truncate px-1 text-[11px] leading-none font-medium">{children}</span>;
}

/** Where the bar goes instead of <body>, when a part of the page says so. */
export const BottomNavHost = React.createContext<HTMLElement | null>(null);

export function BottomNav({ label, tabs }: { label: string; tabs: readonly BottomNavTab[] }) {
  const t = useT();
  const host = React.useContext(BottomNavHost);
  const { isMobile, openMobile, setOpenMobile } = useSidebar();
  const link = useSiteLink();
  if (!isMobile) return null;
  // More is lit while its sheet is up, and while the page is one the tabs
  // do not name: it is where that page lives.
  const moreActive = openMobile || !tabs.some((tab) => tab.active);
  return createPortal(
    <nav
      data-slot="bottom-nav"
      aria-label={label}
      className="fixed inset-x-0 bottom-0 z-40 border-t border-line bg-surface/85 pb-[env(safe-area-inset-bottom,0px)] backdrop-blur-sm"
    >
      <ul className="mx-auto grid max-w-lg auto-cols-fr grid-flow-col px-1">
        {tabs.map((tab) => (
          <li key={tab.key} className="min-w-0">
            {React.cloneElement(
              link(tab.to, {
                "aria-current": tab.active ? "page" : undefined,
                "data-active": tab.active && !openMobile ? "" : undefined,
                className: TAB,
              }),
              undefined,
              <Glyph icon={tab.icon} />,
              <Label>{tab.label}</Label>,
            )}
          </li>
        ))}
        <li className="min-w-0">
          <button
            type="button"
            aria-haspopup="dialog"
            aria-expanded={openMobile}
            data-active={moreActive ? "" : undefined}
            onClick={() => setOpenMobile(!openMobile)}
            className={TAB}
          >
            <Glyph icon={Menu01Icon} />
            <Label>{t("ui.bottomNav.more")}</Label>
          </button>
        </li>
      </ul>
    </nav>,
    host ?? document.body,
  );
}
