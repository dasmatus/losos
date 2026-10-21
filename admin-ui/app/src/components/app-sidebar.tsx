import * as React from "react";
import { Link, useLocation } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowRight01Icon,
  BubbleChatIcon,
  HardDriveIcon,
  Home01Icon,
  LayoutGridIcon,
  NetworkIcon,
  PencilEdit02Icon,
  PinOffIcon,
  Search01Icon,
  Settings01Icon,
  Share08Icon,
} from "@hugeicons/core-free-icons";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuIcon,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarTrigger,
  useSidebar,
} from "@/components/ui/sidebar";
import type { MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { paneFromPath, paneHref } from "@/lib/routes";
import { cn } from "@/lib/utils";
import { SHORTCUT_ARIA, openPalette, shortcutLabel } from "@/palette/store";
import { paneById, paneOpen, type SettingsPaneId } from "@/screens/settings/panes";
import { canUse, heldBy, sharedApp, useMe } from "@/lib/api";
import { openInNewTab, PinMenu } from "@/components/pin-menu";
import { PinsDialog, usePins, useSavePins } from "@/components/pins-dialog";
import { pinKey, viewPin, type PinView } from "@/lib/pins";

/* The one panel: every destination on this page in a single sidebar, in the
 * shape of shadcn's Sidebar ("collapses to icons"), inside the recessed box.
 *
 * Before this the page had two: the shell's five sections on the left and,
 * on any settings address, a second list of nine panes beside it. Now the
 * entries that have entries of their own fold them away under a chevron:
 *
 *   Overview
 *   Apps
 *   Storage
 *   Mesh                ▾   (the link opens Mesh; the chevron folds)
 *     Market                selling storage and compute, and disk sharing
 *     Machines              virtual machines on the mesh
 *   Lab                     (LosOS Lab at /lab/, a page of its own)
 *   Settings            ▾   (a fold only; /settings opens Network)
 *     Network, Hardware, Security, About, Reset
 *
 *   Pinned              ✎   (the owner's own: apps, files, repositories)
 *     Calendar
 *     plan.pdf
 *
 * The pins are the owner's, kept on the box (lib/pins.ts); each opens in
 * its own app, and the pencil opens the dialog that adds, orders and
 * removes them (components/pins-dialog.tsx). The palette finds them too.
 *
 * A folded entry opens by itself when the page is on one of its
 * sub-entries. The magnifier at the top opens the command palette
 * (src/palette/), as Ctrl/Cmd+Shift+P does, which finds every entry here and
 * also the pins, apps, files in LosOS cloud and repositories in LosOS Git.
 *
 * On a wide screen the trigger (or Ctrl/Cmd+B) narrows the panel to a rail of
 * icons, each naming itself in a tooltip; on a phone a row naming where you
 * are opens the panel as a Sheet from the left. The primitives are shadcn's
 * base registry (components/ui/sidebar.tsx, on Base UI). */

interface LinkEntry {
  kind: "link";
  key: string;
  to: string;
  label: MessageKey;
  icon: IconSvgElement;
  /** The settings pane behind it, for matching and for "is this current". */
  pane?: SettingsPaneId;
  /** A page outside this app (the lab at /lab/): a plain link that loads it,
   * not a router link that would draw this app's 404 at that address. */
  external?: boolean;
  /** An app that runs as the shared user (GET /api/me's `shared`): drawn
   * only while the box shares its storage and the app is up. */
  shared?: string;
  children?: readonly Entry[];
}

interface FoldEntry {
  kind: "fold";
  key: string;
  label: MessageKey;
  icon: IconSvgElement;
  /** Where the entry goes on the icon rail, where nothing unfolds. */
  railTo: string;
  children: readonly Entry[];
}

type Entry = LinkEntry | FoldEntry;

const settingsLeaf = (pane: SettingsPaneId): LinkEntry => {
  const meta = paneById(pane);
  return { kind: "link", key: pane, to: paneHref(pane), label: meta.labelKey, icon: meta.icon, pane };
};

const TREE: readonly Entry[] = [
  {
    kind: "link",
    key: "overview",
    to: "/",
    label: "shell.nav.overview",
    icon: Home01Icon,
  },
  { kind: "link", key: "apps", to: "/apps", label: "shell.nav.apps", icon: LayoutGridIcon, pane: "apps" },
  { kind: "link", key: "storage", to: "/storage", label: "shell.nav.storage", icon: HardDriveIcon, pane: "storage" },
  {
    kind: "link",
    key: "mesh",
    to: "/mesh",
    label: "shell.nav.mesh",
    icon: Share08Icon,
    pane: "mesh",
    children: [settingsLeaf("market"), settingsLeaf("machines")],
  },
  {
    kind: "link",
    key: "lab",
    to: "/lab/",
    label: "shell.nav.lab",
    icon: NetworkIcon,
    external: true,
  },
  {
    kind: "link",
    key: "chat",
    to: "/chat/",
    label: "shell.nav.chat",
    icon: BubbleChatIcon,
    external: true,
    shared: "chat",
  },
  {
    kind: "fold",
    key: "settings",
    label: "shell.nav.settings",
    icon: Settings01Icon,
    railTo: "/settings",
    children: (
      ["network", "look", "hardware", "security", "account", "users", "access", "advanced", "history", "backup", "about", "reset"] as const
    ).map(
      settingsLeaf,
    ),
  },
];

/* The entries this person may open: an administrator sees all of them,
 * anyone else the panes of their sections (GET /api/me). A hidden link's
 * own children that stay visible move up a level, so someone in the market
 * section sees Market without Mesh. A fold left empty goes. */
function allowed(entry: LinkEntry): boolean {
  if (entry.pane !== undefined) return paneOpen(entry.pane);
  if (entry.key === "lab") return canUse("lab");
  if (entry.shared !== undefined) return sharedApp(entry.shared);
  return true;
}

function visible(entries: readonly Entry[]): Entry[] {
  const out: Entry[] = [];
  for (const entry of entries) {
    const children = visible(entry.children ?? []);
    if (entry.kind === "fold") {
      if (children.length > 0) out.push({ ...entry, children });
    } else if (allowed(entry)) {
      out.push(entry.children === undefined ? entry : { ...entry, children });
    } else {
      out.push(...children);
    }
  }
  return out;
}

function isCurrent(entry: LinkEntry, pathname: string): boolean {
  if (entry.pane !== undefined) return paneFromPath(pathname) === entry.pane;
  return pathname === entry.to;
}

function containsCurrent(entry: Entry, pathname: string): boolean {
  return (entry.children ?? []).some(
    (child) => (child.kind === "link" && isCurrent(child, pathname)) || containsCurrent(child, pathname),
  );
}

export function AppSidebar() {
  const t = useT();
  useLocale(); // the tree's text is read during render
  const { pathname } = useLocation();
  const { state, isMobile, setOpenMobile } = useSidebar();
  const rail = !isMobile && state === "collapsed";
  const me = useMe();
  // Someone a rule of LosOS Login holds gets their account, at the top.
  const tree = React.useMemo(
    () => ((me?.locked?.length ?? 0) > 0 ? [settingsLeaf("account")] : visible(TREE)),
    [me],
  );

  // Which folds are open. One that holds the current page opens by itself.
  const [openFolds, setOpenFolds] = React.useState<Record<string, boolean>>({});
  React.useEffect(() => {
    setOpenFolds((current) => {
      const next = { ...current };
      for (const entry of tree) {
        // A fold that holds the page opens, and so does a link that has
        // entries of its own when it is the page (Mesh shows Market).
        const here = entry.kind === "link" && isCurrent(entry, pathname);
        if (here || containsCurrent(entry, pathname)) next[entry.key] = true;
      }
      return next;
    });
    // A phone's panel closes once a destination is chosen.
    setOpenMobile(false);
  }, [pathname, setOpenMobile, tree]);

  const here = React.useMemo(() => currentLabel(tree, pathname), [pathname, tree]);
  const shortcut = shortcutLabel();
  const searchLabel = t("palette.open", { shortcut });

  const pins = usePins();
  const [editing, setEditing] = React.useState(false);
  const views = React.useMemo(
    () => pins.pins.map((pin) => viewPin(pin, pins.installed)).filter((view): view is PinView => view !== null),
    [pins],
  );
  const openEditor = (): void => {
    // A phone's panel is a modal sheet of its own; it steps aside first.
    setOpenMobile(false);
    setEditing(true);
  };

  const isOpen = (key: string): boolean => openFolds[key] === true;
  const setOpen = (key: string, open: boolean): void => setOpenFolds((current) => ({ ...current, [key]: open }));

  return (
    <>
      {isMobile && (
        // A phone: one row naming where you are; it, or the trigger, slides
        // the panel in from the left as a Sheet.
        <div className="flex w-full items-center gap-1.5 rounded-card border border-line bg-sunk p-2">
          <SidebarTrigger label={t("shell.nav.expand")} />
          <button
            type="button"
            onClick={() => setOpenMobile(true)}
            className="min-w-0 flex-1 truncate text-left text-[13.5px] font-medium text-ink"
          >
            {here !== null ? t(here) : t("shell.nav.label")}
          </button>
        </div>
      )}
      <Sidebar aria-label={t("shell.nav.label")} mobileTitle={t("shell.nav.label")} closeLabel={t("ui.dismiss")}>
        <SidebarHeader className={cn(isMobile && "pr-10")}>
          {!isMobile && <SidebarTrigger label={t(rail ? "shell.nav.expand" : "shell.nav.collapse")} />}
          {/* The command palette's button: a magnifier at the end of the
              header, under the trigger on the rail. */}
          <button
            type="button"
            onClick={() => {
              setOpenMobile(false);
              openPalette();
            }}
            aria-label={searchLabel}
            aria-keyshortcuts={SHORTCUT_ARIA}
            aria-haspopup="dialog"
            title={searchLabel}
            className={cn(
              "ml-auto flex size-8 shrink-0 items-center justify-center rounded-control text-muted",
              "transition-colors duration-150 hover:bg-surface hover:text-ink",
              "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
              "group-data-[collapsible=icon]/sidebar:ml-0",
            )}
          >
            <HugeiconsIcon icon={Search01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          </button>
        </SidebarHeader>

        <SidebarContent>
          <SidebarMenu>
            {tree.map((entry) => (
              <TopEntry
                key={entry.key}
                entry={entry}
                pathname={pathname}
                rail={rail}
                open={isOpen(entry.key)}
                onOpenChange={(open) => setOpen(entry.key, open)}
              />
            ))}
          </SidebarMenu>
          {/* Signed out there is nothing to show; the box keeps the list,
              and a box that did not send one (an older build) gets none. */}
          {(pins.status === "ready" || pins.pins.length > 0) && heldBy().length === 0 && (
            <PinnedSection views={views} pathname={pathname} rail={rail} onEdit={openEditor} />
          )}
        </SidebarContent>
      </Sidebar>
      <PinsDialog open={editing} onOpenChange={setEditing} />
    </>
  );
}

function PinnedSection({
  views,
  pathname,
  rail,
  onEdit,
}: {
  views: readonly PinView[];
  pathname: string;
  rail: boolean;
  onEdit: () => void;
}) {
  const t = useT();
  const { pins } = usePins();
  const { busy, save } = useSavePins();
  // On the rail an empty section is nothing to point at.
  if (views.length === 0 && rail) return null;

  return (
    <SidebarGroup role="group" className="mt-2" aria-label={t("pins.title")} data-pins>
      {/* On the rail the label is gone; a hairline keeps the pins apart
          from the sections above them. */}
      <div aria-hidden="true" className="mx-2 mb-1.5 hidden h-px bg-line group-data-[collapsible=icon]/sidebar:block" />
      <div className="flex items-center group-data-[collapsible=icon]/sidebar:hidden">
        <SidebarGroupLabel className="flex-1 pb-1">{t("pins.title")}</SidebarGroupLabel>
        <button
          type="button"
          onClick={onEdit}
          aria-label={t("pins.edit")}
          title={t("pins.edit")}
          className={cn(
            "mr-1 flex size-6 items-center justify-center rounded-[6px] text-faint",
            "transition-[background-color,color] duration-150 hover:bg-surface hover:text-ink",
            "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
          )}
        >
          <HugeiconsIcon icon={PencilEdit02Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </button>
      </div>
      {views.length === 0 ? (
        <button
          type="button"
          onClick={onEdit}
          className={cn(
            "mx-1 rounded-control border border-dashed border-line px-2.5 py-2 text-left",
            "text-[12.5px] leading-snug text-muted transition-colors duration-150 hover:border-accent/45 hover:text-ink",
          )}
        >
          {t("pins.empty")}
        </button>
      ) : (
        <SidebarMenu>
          {views.map((view) => {
            const name = view.labelKey !== null ? t(view.labelKey) : view.label;
            const active = view.inApp && pathname === view.href;
            const pin = pins.find((candidate) => pinKey(candidate) === view.key) ?? null;
            return (
              <SidebarMenuItem key={view.key}>
                <PinMenu pin={pin} name={name} newTab={() => openInNewTab(view.href)}>
                  <SidebarMenuButton
                    isActive={active}
                    tooltip={name}
                    render={
                      view.inApp ? (
                        <Link to={view.href} aria-current={active ? "page" : undefined} />
                      ) : (
                        <a
                          href={view.href}
                          title={view.detail === null ? undefined : `${name} · ${view.detail}`}
                          {...(view.newTab ? { target: "_blank", rel: "noreferrer noopener" } : {})}
                        />
                      )
                    }
                  >
                    <Glyph icon={view.icon} active={active} />
                    <span className="group-data-[collapsible=icon]/sidebar:sr-only">{name}</span>
                  </SidebarMenuButton>
                </PinMenu>
                <SidebarMenuAction
                  aria-label={t("pins.unpin", { name })}
                  disabled={busy}
                  onClick={() => void save(pins.filter((pin) => pinKey(pin) !== view.key))}
                  className="opacity-0 group-focus-within/menu-item:opacity-100 group-hover/menu-item:opacity-100 max-md:opacity-100"
                >
                  <HugeiconsIcon icon={PinOffIcon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                </SidebarMenuAction>
              </SidebarMenuItem>
            );
          })}
        </SidebarMenu>
      )}
    </SidebarGroup>
  );
}

function currentLabel(entries: readonly Entry[], pathname: string): MessageKey | null {
  for (const entry of entries) {
    if (entry.kind === "link" && isCurrent(entry, pathname)) return entry.label;
    const inner = currentLabel(entry.children ?? [], pathname);
    if (inner !== null) return inner;
  }
  return null;
}

interface TopEntryProps {
  entry: Entry;
  pathname: string;
  rail: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function Glyph({ icon, active = false }: { icon: IconSvgElement; active?: boolean }) {
  return (
    <SidebarMenuIcon active={active}>
      <HugeiconsIcon icon={icon} size={13} strokeWidth={1.5} color="currentColor" />
    </SidebarMenuIcon>
  );
}

function Chevron() {
  return (
    <HugeiconsIcon
      icon={ArrowRight01Icon}
      size={14}
      strokeWidth={1.5}
      color="currentColor"
      aria-hidden="true"
      className="transition-transform duration-200 in-data-[panel-open]:rotate-90"
    />
  );
}

function TopEntry({ entry, pathname, rail, open, onOpenChange }: TopEntryProps) {
  const t = useT();
  const label = t(entry.label);

  if (entry.kind === "fold") {
    const holdsCurrent = containsCurrent(entry, pathname);
    if (rail) {
      // On the rail there is nothing to unfold: the icon goes to the fold's
      // first page, says where it is with the accent wash, and names itself
      // in a tooltip.
      return (
        <SidebarMenuItem>
          <SidebarMenuButton
            tooltip={label}
            render={<Link to={entry.railTo} aria-current={holdsCurrent ? "page" : undefined} />}
          >
            <Glyph icon={entry.icon} active={holdsCurrent} />
            <span className="sr-only">{label}</span>
          </SidebarMenuButton>
        </SidebarMenuItem>
      );
    }
    return (
      <SidebarMenuItem>
        <Collapsible open={open} onOpenChange={(next) => onOpenChange(next)}>
          <CollapsibleTrigger render={<SidebarMenuButton className={cn(holdsCurrent && "font-medium")} />}>
            <Glyph icon={entry.icon} active={holdsCurrent} />
            <span>{label}</span>
            <span className="ml-auto flex text-muted">
              <Chevron />
            </span>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <SubEntries entries={entry.children} pathname={pathname} />
          </CollapsibleContent>
        </Collapsible>
      </SidebarMenuItem>
    );
  }

  const active = isCurrent(entry, pathname);
  const link = (
    <SidebarMenuButton
      isActive={active}
      tooltip={label}
      render={
        entry.external === true ? (
          <a href={entry.to} />
        ) : (
          <Link to={entry.to} aria-current={active ? "page" : undefined} />
        )
      }
    >
      <Glyph icon={entry.icon} active={active} />
      <span className="group-data-[collapsible=icon]/sidebar:sr-only">{label}</span>
    </SidebarMenuButton>
  );

  const children = entry.children ?? [];
  if (children.length === 0 || rail) return <SidebarMenuItem>{link}</SidebarMenuItem>;

  return (
    <SidebarMenuItem>
      <Collapsible open={open} onOpenChange={(next) => onOpenChange(next)}>
        {link}
        <CollapsibleTrigger render={<SidebarMenuAction aria-label={t("shell.nav.subEntries", { name: label })} />}>
          <Chevron />
        </CollapsibleTrigger>
        <CollapsibleContent>
          <SubEntries entries={children} pathname={pathname} />
        </CollapsibleContent>
      </Collapsible>
    </SidebarMenuItem>
  );
}

function SubEntries({ entries, pathname }: { entries: readonly Entry[]; pathname: string }) {
  const t = useT();
  return (
    <SidebarMenuSub>
      {entries.map((entry) => {
        const label = t(entry.label);
        if (entry.kind === "link") {
          const active = isCurrent(entry, pathname);
          return (
            <SidebarMenuSubItem key={entry.key}>
              <SidebarMenuSubButton
                isActive={active}
                render={<Link to={entry.to} aria-current={active ? "page" : undefined} />}
              >
                <Glyph icon={entry.icon} active={active} />
                <span className="truncate">{label}</span>
              </SidebarMenuSubButton>
            </SidebarMenuSubItem>
          );
        }
        return null;
      })}
    </SidebarMenuSub>
  );
}
