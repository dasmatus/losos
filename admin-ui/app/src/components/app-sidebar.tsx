import * as React from "react";
import { Link, useLocation } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowRight01Icon,
  HardDriveIcon,
  Home01Icon,
  LayoutGridIcon,
  NetworkIcon,
  Search01Icon,
  Settings01Icon,
  Share08Icon,
  ShoppingCart01Icon,
  ServerStack01Icon,
} from "@hugeicons/core-free-icons";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuBadge,
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
import { openPalette, shortcutLabel } from "@/palette/store";
import { paneById, type SettingsPaneId } from "@/screens/settings/panes";

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
 *     Market   soon(TM)     greyed: planned, not open
 *       Disk sharing        greyed with it (its switch is on the Market pane)
 *     Machines              virtual machines on the mesh
 *   Lab                     (LosOS Lab at /lab/, a page of its own)
 *   Settings            ▾   (a fold only; /settings opens Network)
 *     Network, Hardware, Security, About, Reset
 *
 * A folded entry opens by itself when the page is on one of its
 * sub-entries. The search box at the top is the command palette's button
 * (src/palette/): it opens the palette, which finds every entry here and
 * also apps, files in LosOS cloud and repositories in LosOS Git. A planned
 * entry is a disabled button: greyed, skipped by Tab, and announced as
 * unavailable, with "soon(TM)" in its accessible name.
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
  children?: readonly Entry[];
}

interface PlannedEntry {
  kind: "planned";
  key: string;
  label: MessageKey;
  icon?: IconSvgElement;
  pane?: SettingsPaneId;
  /** Whether the entry carries the soon(TM) badge itself. */
  badge: boolean;
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

type Entry = LinkEntry | PlannedEntry | FoldEntry;

const settingsLeaf = (pane: SettingsPaneId): LinkEntry => {
  const meta = paneById(pane);
  return { kind: "link", key: pane, to: paneHref(pane), label: meta.labelKey, icon: meta.icon, pane };
};

/* The market hangs under Mesh: it sells the mesh's storage and compute.
 * While it is planned (panes.ts) it is a greyed entry with "soon(TM)" and
 * the disk-sharing entry greyed under it, since that switch lives on the
 * Market pane and opens with it. Opening the market is `planned: false` in
 * panes.ts and nothing here: the entry becomes a link to its pane, and the
 * pane carries the sharing switch. */
function marketEntry(): Entry {
  const market = paneById("market");
  if (!market.planned) return settingsLeaf("market");
  return {
    kind: "planned",
    key: "market",
    label: market.labelKey,
    icon: ShoppingCart01Icon,
    pane: "market",
    badge: true,
    children: [
      {
        kind: "planned",
        key: "disk-sharing",
        label: "shell.nav.diskSharing",
        icon: ServerStack01Icon,
        badge: false,
      },
    ],
  };
}

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
    children: [marketEntry(), settingsLeaf("machines")],
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
    kind: "fold",
    key: "settings",
    label: "shell.nav.settings",
    icon: Settings01Icon,
    railTo: "/settings",
    children: (
      ["network", "look", "hardware", "security", "access", "advanced", "history", "backup", "about", "reset"] as const
    ).map(
      settingsLeaf,
    ),
  },
];

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

  // Which folds are open. One that holds the current page opens by itself.
  const [openFolds, setOpenFolds] = React.useState<Record<string, boolean>>({});
  React.useEffect(() => {
    setOpenFolds((current) => {
      const next = { ...current };
      for (const entry of TREE) {
        // A fold that holds the page opens, and so does a link that has
        // entries of its own when it is the page (Mesh shows Market).
        const here = entry.kind === "link" && isCurrent(entry, pathname);
        if (here || containsCurrent(entry, pathname)) next[entry.key] = true;
      }
      return next;
    });
    // A phone's panel closes once a destination is chosen.
    setOpenMobile(false);
  }, [pathname, setOpenMobile]);

  const here = React.useMemo(() => currentLabel(TREE, pathname), [pathname]);
  const shortcut = shortcutLabel();
  const searchLabel = t("palette.open", { shortcut });

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
          {/* The command palette's button, drawn as the search box it
              replaced. On the rail it is the magnifier alone. */}
          <button
            type="button"
            onClick={() => {
              setOpenMobile(false);
              openPalette();
            }}
            aria-label={searchLabel}
            aria-keyshortcuts="Control+K Meta+K"
            aria-haspopup="dialog"
            title={rail ? searchLabel : undefined}
            className={cn(
              "flex h-8 min-w-0 flex-1 items-center gap-2 rounded-control border border-line bg-surface px-2 text-left",
              "text-[13px] text-faint transition-[color,border-color] duration-150 hover:border-accent/45 hover:text-muted",
              "focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/35 focus-visible:outline-none",
              "group-data-[collapsible=icon]/sidebar:size-8 group-data-[collapsible=icon]/sidebar:flex-none",
              "group-data-[collapsible=icon]/sidebar:justify-center group-data-[collapsible=icon]/sidebar:border-transparent",
              "group-data-[collapsible=icon]/sidebar:bg-transparent group-data-[collapsible=icon]/sidebar:px-0",
            )}
          >
            <HugeiconsIcon icon={Search01Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="min-w-0 flex-1 truncate group-data-[collapsible=icon]/sidebar:hidden">
              {t("settings.sidebar.searchPlaceholder")}
            </span>
            <kbd
              aria-hidden="true"
              className="rounded-[4px] border border-line bg-sunk px-1 font-sans text-[10.5px] leading-[16px] text-faint group-data-[collapsible=icon]/sidebar:hidden"
            >
              {shortcut}
            </kbd>
          </button>
        </SidebarHeader>

        <SidebarContent>
          <SidebarMenu>
            {TREE.map((entry) => (
              <TopEntry
                key={entry.key}
                entry={entry}
                pathname={pathname}
                rail={rail}
                open={isOpen(entry.key)}
                onOpenChange={(open) => setOpen(entry.key, open)}
                isOpen={isOpen}
                setOpen={setOpen}
              />
            ))}
          </SidebarMenu>
        </SidebarContent>
      </Sidebar>
    </>
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
  isOpen: (key: string) => boolean;
  setOpen: (key: string, open: boolean) => void;
}

function Glyph({ icon, active = false, planned = false }: { icon: IconSvgElement; active?: boolean; planned?: boolean }) {
  return (
    <SidebarMenuIcon active={active} planned={planned}>
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

function TopEntry({ entry, pathname, rail, open, onOpenChange, isOpen, setOpen }: TopEntryProps) {
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
            <SubEntries entries={entry.children} pathname={pathname} isOpen={isOpen} setOpen={setOpen} />
          </CollapsibleContent>
        </Collapsible>
      </SidebarMenuItem>
    );
  }

  if (entry.kind === "planned") {
    return (
      <SidebarMenuItem>
        <SidebarMenuButton planned disabled aria-disabled="true">
          {entry.icon !== undefined && <Glyph icon={entry.icon} planned />}
          <span className="group-data-[collapsible=icon]/sidebar:sr-only">{label}</span>
          {entry.badge && <SidebarMenuBadge>{t("settings.sidebar.soon")}</SidebarMenuBadge>}
        </SidebarMenuButton>
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
          <SubEntries entries={children} pathname={pathname} isOpen={isOpen} setOpen={setOpen} />
        </CollapsibleContent>
      </Collapsible>
    </SidebarMenuItem>
  );
}

function SubEntries({
  entries,
  pathname,
  isOpen,
  setOpen,
}: {
  entries: readonly Entry[];
  pathname: string;
  isOpen: (key: string) => boolean;
  setOpen: (key: string, open: boolean) => void;
}) {
  const t = useT();
  return (
    <SidebarMenuSub>
      {entries.map((entry) => {
        const label = t(entry.label);
        if (entry.kind === "planned") {
          return (
            <SidebarMenuSubItem key={entry.key}>
              <SidebarMenuSubButton planned disabled aria-disabled="true">
                {entry.icon !== undefined && <Glyph icon={entry.icon} planned />}
                <span className="truncate">{label}</span>
                {entry.badge && <SidebarMenuBadge>{t("settings.sidebar.soon")}</SidebarMenuBadge>}
              </SidebarMenuSubButton>
              {/* A planned entry's own sub-entries are planned with it and
                  always shown: there is nothing to fold open on a row that
                  cannot be used. */}
              {(entry.children ?? []).length > 0 && (
                <SubEntries entries={entry.children ?? []} pathname={pathname} isOpen={isOpen} setOpen={setOpen} />
              )}
            </SidebarMenuSubItem>
          );
        }
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
