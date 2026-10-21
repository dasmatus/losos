import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { UserCircleIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { LanguagePicker } from "@/components/ui/language-picker";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { cn } from "@/lib/utils";

/* The bar across the top of every page the box serves: the admin UI and
 * each app (LosOS cloud, mail, chat, Office, ...). One component so the
 * height, the logo, the order of the controls and the phone layout are the
 * same everywhere, and a person going from one app to the next sees the same
 * frame with a different name in it.
 *
 * Left to right: the logo, which goes to the box's home; the app's name,
 * which goes to the app's first page; whatever the app puts in (a search, a
 * Compose button); then the language, the theme, who is signed in and Sign
 * out. On a phone the app's name and the person's name drop to icons and
 * tooltips first.
 *
 * `width="page"` keeps the bar on the content column (max-w-6xl) for apps
 * laid out as pages under a sidebar; `width="full"` runs it edge to edge for
 * apps laid out as panes (mail, chat, an editor). */

export interface AppBarPerson {
  /** What the bar shows: the person's name, or "Spare admin key". */
  label: string;
  /** The longer line in the tooltip. */
  tip: string;
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
  width?: "page" | "full";
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
  return (
    <header
      data-slot="app-bar"
      className={cn("sticky top-0 z-40 shrink-0 border-b border-line bg-surface/85 backdrop-blur-sm", className)}
    >
      <div
        className={cn(
          "flex w-full items-center gap-3 py-3",
          width === "page" ? "mx-auto max-w-6xl px-4 sm:px-6" : "px-4",
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
              <img src={logo.url} alt="" width={32} height={32} className="size-8" />
            </TooltipTrigger>
            <TooltipContent side="bottom" align="start" className="max-w-[22rem]">
              {t(logo.tip)}
            </TooltipContent>
          </Tooltip>
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
          <LanguagePicker />
          <ThemeToggle />
          {person != null && <SignedInAs person={person} />}
          {onSignOut !== undefined && (
            <Button variant="ghost" size="sm" onClick={onSignOut}>
              {t("shell.signOut")}
            </Button>
          )}
        </TooltipProvider>
      </div>
    </header>
  );
}

/** A search box for the bar: takes the free space, centred, at most max-w-md. */
export function AppBarSearch({ className, ...props }: React.ComponentPropsWithoutRef<"form">) {
  return <form role="search" className={cn("relative mx-auto w-full min-w-0 max-w-md flex-1", className)} {...props} />;
}

/** Who this tab is signed in as. The name hides on a phone, where the icon
 *  stays and the tooltip says the rest. */
function SignedInAs({ person }: { person: AppBarPerson }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            aria-label={person.tip}
            data-testid="signed-in-as"
            className="inline-flex min-w-0 shrink items-center gap-1.5 rounded-control px-2 py-1 text-sm text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
          />
        }
      >
        <HugeiconsIcon icon={UserCircleIcon} size={18} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <span className="hidden max-w-[12rem] truncate sm:inline">{person.label}</span>
      </TooltipTrigger>
      <TooltipContent side="bottom" align="end" className="max-w-[22rem]">
        {person.tip}
      </TooltipContent>
    </Tooltip>
  );
}
