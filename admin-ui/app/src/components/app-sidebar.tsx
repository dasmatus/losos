import * as React from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowRight01Icon,
  Cancel01Icon,
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
  SidebarInput,
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
import { t as translate, template, type MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { paneFromPath, paneHref } from "@/lib/routes";
import { cn } from "@/lib/utils";
import { paneById, paneMatches, type SettingsPaneId } from "@/screens/settings/panes";

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
 *   Lab                     (LosOS Lab at /lab/, a page of its own)
 *   Settings            ▾   (a fold only; /settings opens Network)
 *     Network, Hardware, Security, About, Reset
 *
 * A folded entry opens by itself when the page is on one of its
 * sub-entries, and while a search is typed. The search at the top filters
 * the whole tree by name, summary and keywords in the language on screen and
 * in English (paneMatches), and Enter opens the first match that can be
 * opened. A planned entry is a disabled button: greyed, skipped by Tab, and
 * announced as unavailable, with "soon(TM)" in its accessible name.
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
  /** Extra English search words, for an entry with no pane. */
  keywords?: readonly string[];
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
  keywords?: readonly string[];
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
        keywords: ["share", "sharing", "lend", "disk", "copies", "backup"],
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
    keywords: ["home", "start", "widgets", "board", "dashboard"],
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
    children: [marketEntry()],
  },
  {
    kind: "link",
    key: "lab",
    to: "/lab/",
    label: "shell.nav.lab",
    icon: NetworkIcon,
    keywords: ["lab", "simulator", "topology", "diagram", "network", "map", "packet", "visualize"],
    external: true,
  },
  {
    kind: "fold",
    key: "settings",
    label: "shell.nav.settings",
    icon: Settings01Icon,
    railTo: "/settings",
    children: (
      ["network", "look", "hardware", "security", "advanced", "history", "backup", "about", "reset"] as const
    ).map(
      settingsLeaf,
    ),
  },
];

/** Lower-case and strip diacritics: "Úložisko" -> "ulozisko". */
function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

/* Does this entry, by itself, match the search? A pane-backed entry matches
 * the way the old settings list did (paneMatches); the others on their name
 * in the language on screen and in English, plus their keywords. */
function selfMatches(entry: Entry, query: string): boolean {
  if (query.trim().length === 0) return true;
  if (entry.kind !== "fold" && entry.pane !== undefined) return paneMatches(paneById(entry.pane), query);
  const terms = fold(query).split(/\s+/).filter((term) => term.length > 0);
  const haystack = fold(
    [translate(entry.label), template(entry.label, undefined, "en"), ...(entry.kind === "fold" ? [] : (entry.keywords ?? []))].join(" "),
  );
  return terms.every((term) => haystack.includes(term));
}

/* The tree as the search leaves it: an entry stays if it matches or any
 * entry under it does; a fold whose own name matches keeps all its entries
 * ("settings" shows everything under Settings). */
function filterTree(entries: readonly Entry[], query: string): Entry[] {
  const out: Entry[] = [];
  for (const entry of entries) {
    const own = selfMatches(entry, query);
    const children = entry.children ?? [];
    const kept = own && entry.kind === "fold" ? [...children] : filterTree(children, query);
    if (own || kept.length > 0) out.push({ ...entry, children: kept } as Entry);
  }
  return out;
}

function firstOpenLink(entries: readonly Entry[]): LinkEntry | null {
  for (const entry of entries) {
    if (entry.kind === "link") return entry;
    if (entry.kind === "fold") {
      const inner = firstOpenLink(entry.children);
      if (inner !== null) return inner;
    }
  }
  return null;
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
  const navigate = useNavigate();
  const { state, isMobile, setOpenMobile } = useSidebar();
  const rail = !isMobile && state === "collapsed";
  const [search, setSearch] = React.useState("");
  const searchId = React.useId();
  const searching = search.trim().length > 0;

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

  const tree = React.useMemo(() => (searching ? filterTree(TREE, search) : TREE), [search, searching]);
  const here = React.useMemo(() => currentLabel(TREE, pathname), [pathname]);

  const submit = (event: React.FormEvent): void => {
    event.preventDefault();
    const first = firstOpenLink(tree);
    if (first !== null) {
      if (first.external === true) window.location.assign(first.to);
      else navigate(first.to);
      setSearch("");
    }
  };

  const isOpen = (key: string): boolean => searching || openFolds[key] === true;
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
          <form onSubmit={submit} role="search" className="relative min-w-0 flex-1 group-data-[collapsible=icon]/sidebar:hidden">
            <label htmlFor={searchId} className="sr-only">
              {t("settings.sidebar.searchLabel")}
            </label>
            <HugeiconsIcon
              icon={Search01Icon}
              size={15}
              strokeWidth={1.5}
              color="currentColor"
              className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-faint"
              aria-hidden="true"
            />
            <SidebarInput
              id={searchId}
              type="search"
              value={search}
              placeholder={t("settings.sidebar.searchPlaceholder")}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setSearch(event.target.value)}
            />
            {search.length > 0 && (
              <button
                type="button"
                aria-label={t("settings.sidebar.clear")}
                onClick={() => setSearch("")}
                className="absolute top-1/2 right-1 -translate-y-1/2 rounded-control p-1 text-faint transition-colors duration-150 hover:text-ink"
              >
                <HugeiconsIcon icon={Cancel01Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </button>
            )}
          </form>
        </SidebarHeader>

        <SidebarContent>
          {tree.length === 0 ? (
            <p className="animate-fade-in px-2 py-3 text-[12.5px] leading-snug text-muted">
              {t("settings.sidebar.noMatch", { query: search.trim() })}
            </p>
          ) : (
            <SidebarMenu>
              {tree.map((entry) => (
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
          )}
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
