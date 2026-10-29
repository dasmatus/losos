import * as React from "react";
import { Menu } from "@base-ui/react/menu";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { ArrowDown01Icon, Logout01Icon, UserCircleIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { KbdGroup } from "@/components/ui/kbd";
import { popupMotion } from "@/components/ui/motion";
import { DisplayMenu, menuItemClass, PreferenceItems } from "@/components/preferences-menu";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { isSignOutShortcut, signOutKeys } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";

/* The bar across the top of every page the box serves: the admin UI and
 * each app (LosOS Cloud, Mail, Chat, Office, ...). One component so the
 * height, the logo, the order of the controls and the phone layout are the
 * same everywhere, and a person going from one app to the next sees the same
 * frame with a different name in it.
 *
 * Left to right: the logo, which goes to the box's home; the app's name,
 * which goes to the app's first page; whatever the app puts in (a search, a
 * Compose button); then the person's menu, which holds the appearance, the
 * language, Your account and Sign out. With nobody signed in, the appearance
 * and the language sit in a menu of their own (components/preferences-menu.tsx).
 * On a phone the app's name and the person's name drop out first.
 *
 * Sign out is also Alt+Shift+Q (lib/shortcuts.ts) on every page that draws
 * this bar with `onSignOut`; the menu shows the keys.
 *
 * `width="page"` keeps the bar on the content column (max-w-6xl) for apps
 * laid out as pages under a sidebar, `width="wide"` on the wider column
 * (max-w-[96rem]) of a canvas such as Diagrams; `width="full"` runs it edge
 * to edge for apps laid out as panes (Chat, an editor). */

export interface AppBarPerson {
  /** What the bar shows: the person's name, or "Spare admin key". */
  label: string;
  /** The longer line at the top of the menu. */
  tip: string;
  /** Their picture (PersonAvatar size xs); a person icon without one. */
  avatar?: React.ReactNode;
  /** Your account, as a link element: `<a href="/settings/account" />` or a
   *  router `<Link to="/settings/account" />`. Left out, no such entry. */
  accountLink?: React.ReactElement | undefined;
}

export interface AppBarProps {
  /** The app's name beside the logo. Left out on the box's own pages. */
  name?: string | undefined;
  /** Where the name goes, as a link element: `<a href="/cloud/" />` or a
   *  router `<Link to="/files" />`. */
  nameLink?: React.ReactElement<{ className?: string; children?: React.ReactNode }> | undefined;
  /** Where the logo goes. The box's home unless the page routes it itself. */
  homeLink?: React.ReactElement | undefined;
  /** Shown beside the name on wide screens, such as "this box". */
  badge?: React.ReactNode;
  width?: "page" | "wide" | "full";
  /** The app's own controls, between the name and the settings. A search
   *  takes the free space; anything else sits at the right. */
  children?: React.ReactNode;
  person?: AppBarPerson | null | undefined;
  onSignOut?: (() => void) | undefined;
  className?: string;
}

export function AppBar({
  name,
  nameLink,
  homeLink,
  badge,
  width = "page",
  children,
  person,
  onSignOut,
  className,
}: AppBarProps) {
  const t = useT();
  const logo = logoFor();

  React.useEffect(() => {
    if (onSignOut === undefined) return;
    const onKey = (event: KeyboardEvent) => {
      if (!isSignOutShortcut(event)) return;
      event.preventDefault();
      onSignOut();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onSignOut]);

  return (
    <header
      data-slot="app-bar"
      className={cn("sticky top-0 z-40 shrink-0 border-b border-line bg-surface/85 backdrop-blur-sm", className)}
    >
      <div
        className={cn(
          "flex w-full items-center gap-3 py-3",
          width === "page" && "mx-auto max-w-6xl px-4 sm:px-6",
          width === "wide" && "mx-auto max-w-[96rem] px-4 sm:px-6",
          width === "full" && "px-4",
        )}
      >
        <TooltipProvider>
          {/* The logo is a live salmon (a plate of it on Halloween; lib/logo.ts
              picks). A photo needs a word of explanation, so the link's
              accessible name is that sentence and its tooltip says it again
              (Base UI shows no tooltip on touch). */}
          <Tooltip>
            <TooltipTrigger
              render={homeLink ?? <a href="/" />}
              aria-label={t(logo.tip)}
              className="inline-flex shrink-0 rounded-full focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
            >
              <img src={logo.url} alt="LosOS" width={32} height={32} className="size-8" />
            </TooltipTrigger>
            <TooltipContent side="bottom" align="start" className="max-w-[22rem]">
              {t(logo.tip)}
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
        {name !== undefined &&
          React.cloneElement(
            nameLink ?? <a href="./" />,
            {
              className: cn(
                "min-w-0 shrink truncate rounded-control text-[15px] font-semibold tracking-tight text-ink",
                "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
                // A search in the bar needs the room more than the name does.
                children !== undefined && "max-sm:sr-only",
              ),
            },
            name,
          )}
        {badge !== undefined && <span className="hidden shrink-0 sm:inline-flex">{badge}</span>}
        {children ?? <div className="flex-1" />}
        {person != null ? (
          <PersonMenu person={person} onSignOut={onSignOut} />
        ) : (
          <>
            <DisplayMenu />
            {onSignOut !== undefined && (
              <Button variant="ghost" size="sm" onClick={onSignOut}>
                {t("shell.signOut")}
              </Button>
            )}
          </>
        )}
      </div>
    </header>
  );
}

/** A search box for the bar: takes the free space, centred, at most max-w-md. */
export function AppBarSearch({ className, ...props }: React.ComponentPropsWithoutRef<"form">) {
  return <form role="search" className={cn("relative mx-auto w-full min-w-0 max-w-md flex-1", className)} {...props} />;
}

/* Who this tab is signed in as, and the menu under them: who they are in
 * full, the appearance and the language, Your account, Sign out with its
 * keys. shadcn's base-registry
 * Dropdown Menu on Base UI, like the pin menu: its positioner and scroll lock
 * write CSSOM, which the admin CSP allows. The name hides on a phone, where
 * the picture stays. */
export function PersonMenu({
  person,
  onSignOut,
  className,
}: {
  person: AppBarPerson;
  onSignOut: (() => void) | undefined;
  className?: string;
}) {
  const t = useT();
  return (
    <Menu.Root>
      <Menu.Trigger
        data-testid="signed-in-as"
        aria-label={`${person.label}: ${person.tip}`}
        className={cn(
          "group/person inline-flex h-[34px] min-w-0 shrink items-center gap-1.5 rounded-control px-2 text-sm text-ink",
          "hover:bg-sunk data-popup-open:bg-sunk",
          "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
          className,
        )}
      >
        {person.avatar ?? (
          <HugeiconsIcon icon={UserCircleIcon} size={18} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        )}
        <span data-testid="signed-in-name" className="hidden max-w-[12rem] truncate sm:inline">
          {person.label}
        </span>
        <HugeiconsIcon
          icon={ArrowDown01Icon}
          size={14}
          strokeWidth={1.5}
          color="currentColor"
          className="text-faint transition-[rotate] duration-150 ease-snap group-data-popup-open/person:rotate-180 max-sm:hidden"
          aria-hidden="true"
        />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner side="bottom" align="end" sideOffset={6} className="isolate z-50 outline-none">
          <Menu.Popup
            data-slot="person-menu"
            className={cn(
              "w-64 max-w-[calc(100vw-2rem)] rounded-card border border-line bg-surface p-1 text-ink shadow-pop outline-none",
              popupMotion,
            )}
          >
            <div className="flex flex-col gap-0.5 px-2 pt-1.5 pb-2">
              <span className="truncate text-[13.5px] font-medium">{person.label}</span>
              <span className="text-[12px] leading-snug text-muted">{person.tip}</span>
            </div>
            <Menu.Separator className="my-1 h-px bg-hair" />
            <PreferenceItems />
            <Menu.Separator className="my-1 h-px bg-hair" />
            {person.accountLink !== undefined && (
              <Menu.LinkItem render={person.accountLink} className={menuItemClass}>
                <ItemIcon icon={UserCircleIcon} />
                <span className="flex-1">{t("settings.panes.account.label")}</span>
              </Menu.LinkItem>
            )}
            {onSignOut !== undefined && (
              <Menu.Item onClick={onSignOut} className={menuItemClass} data-testid="sign-out">
                <ItemIcon icon={Logout01Icon} />
                <span className="flex-1">{t("shell.signOut")}</span>
                <KbdGroup keys={signOutKeys()} />
              </Menu.Item>
            )}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

function ItemIcon({ icon }: { icon: IconSvgElement }) {
  return <HugeiconsIcon icon={icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />;
}
