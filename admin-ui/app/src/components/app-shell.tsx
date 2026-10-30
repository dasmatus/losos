import * as React from "react";
import { AppBar, type AppBarPerson } from "@/components/app-bar";
import { PersonAvatar } from "@/components/person";
import { Badge } from "@/components/ui/badge";
import { SidebarProvider } from "@/components/ui/sidebar";
import { signOut, useMe } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { useSiteLink } from "@/lib/site";
import { cn } from "@/lib/utils";

/* The frame every screen of the box sits in: the admin pages at /, LosOS
 * Cloud at /cloud/ and the Office editor in both. The same top bar and the
 * same sidebar (components/app-sidebar.tsx) at the same width on every
 * address, so going from one app to the next, or opening a document, moves
 * nothing but the page itself.
 *
 * Before this the frame changed under people's hands: LosOS Cloud drew a
 * bar with its own name and search and a sidebar of its own places, the
 * bar was narrower on a page than on Diagrams, a document dropped the
 * sidebar, and a page long enough to scroll slid the whole column sideways
 * by half a scrollbar (index.css keeps the gutter now). What changes width
 * now is the page inside `main`: a pane of rows keeps to a readable column
 * in the middle of it, a canvas, a calendar or a document takes all of it.
 * nix/tests/cloud-shell.nix measures the bar and the sidebar on every one of
 * these addresses on an installed box. */

/** The bar and the sidebar's column, the same on every page. */
export const SHELL_COLUMN = "mx-auto w-full max-w-[96rem] px-4 sm:px-6";

/** The box's top bar: the logo home, "this box", and the person's menu. */
export function ShellBar({ signedIn }: { signedIn: boolean }) {
  const t = useT();
  const me = useMe();
  const link = useSiteLink();
  let person: AppBarPerson | null = null;
  if (signedIn && me !== null) {
    const name = me.spareKey ? t("shell.me.spareKey") : (me.displayName ?? me.name ?? "");
    const tip = me.spareKey
      ? t("shell.me.spareKeyTip")
      : `${t("shell.me.signedIn", { name: me.name ?? name })} ${t(me.admin ? "shell.me.admin" : "shell.me.limited")}`;
    person = { label: name, tip };
    if (!me.spareKey && me.name) {
      person.avatar = (
        <PersonAvatar
          size="xs"
          data-testid="signed-in-avatar"
          person={{ name: me.name, displayName: me.displayName ?? me.name, picture: me.picture, accent: me.accent }}
        />
      );
      person.accountLink = link("/settings/account");
    }
  }
  return (
    <AppBar
      homeLink={link("/")}
      badge={<Badge variant="outline">{t("shell.thisBox")}</Badge>}
      person={person}
      onSignOut={signedIn ? signOut : undefined}
      width="wide"
    />
  );
}

/** Under the bar: the sidebar and `main`. `fill` gives `main` the rest of
 *  the window's height, for an editor; `wide` its whole width. */
export function ShellBody({
  sidebar,
  wide = false,
  fill = false,
  className,
  children,
}: {
  sidebar: React.ReactNode;
  wide?: boolean;
  fill?: boolean;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <SidebarProvider
      data-shell-body=""
      className={cn(
        SHELL_COLUMN,
        "flex pt-5 max-md:flex-col max-md:gap-3 md:gap-6",
        fill ? "min-h-0 flex-1 pb-4" : "pb-12",
        className,
      )}
    >
      {sidebar}
      <main
        className={cn(
          "min-w-0 flex-1",
          !wide && !fill && "md:mx-auto md:max-w-[52.5rem]",
          fill && "flex min-h-0 flex-col",
        )}
      >
        {children}
      </main>
    </SidebarProvider>
  );
}

/** An editor in `main`: the whole of it, framed like a card. */
export function ShellDocument({ children }: { children: React.ReactNode }) {
  return (
    <div data-shell-document="" className="min-h-0 flex-1 overflow-hidden rounded-card border border-line bg-surface">
      {children}
    </div>
  );
}
