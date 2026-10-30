import * as React from "react";
import { useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  CodeIcon,
  File01Icon,
  FileSearchIcon,
  Folder01Icon,
  PackageIcon,
  Search01Icon,
  SourceCodeIcon,
  TaskDone01Icon,
} from "@hugeicons/core-free-icons";
import { openInNewTab, usePinItems } from "@/components/pin-menu";
import { contextMenu } from "@/lib/context-menu";
import { Dialog } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import type { MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { getPinsState, getServerPinsState, subscribePins, viewPin } from "@/lib/pins";
import { getTheme, subscribeTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { actionItems, appItems, pageItems, pinItems, rank, type GroupId, type Item, type Target } from "./items";
import {
  cloudTextHref,
  fileHref,
  gitTextHref,
  issueHref,
  repoHref,
  searchApps,
  searchCloudText,
  searchFiles,
  searchGitText,
  searchIssues,
  searchRepos,
  useRemote,
  type Remote,
} from "./sources";
import { routeIn, useSiteBase } from "@/lib/site";
import { closePalette, paletteState, subscribePalette } from "./store";

/* The command palette: one box that finds a page, a setting, an app, a file
 * in LosOS Cloud, a repository in LosOS Git, one of the owner's pins or an
 * app to install, and opens it. Ctrl/Cmd+Shift+P opens it from anywhere on the page, a colon typed
 * outside a text field does too, and so does the search button at the top of
 * the sidebar.
 *
 * Text inside files is its own group: LosOS Cloud's files through the box's
 * index, LosOS Git's default branches through `git grep`, cloud first.
 *
 * Built on the native <dialog> (components/ui/dialog.tsx) and the ARIA
 * combobox pattern by hand: the input keeps focus, the arrow keys move a
 * highlighted option (aria-activedescendant), Enter opens it. cmdk and
 * Radix were not used because the admin page's `style-src 'self'` refuses
 * the <style> element their scroll lock injects.
 *
 * Results open where they live: a page here by the router, LosOS Cloud and
 * LosOS Git by a real navigation (Ctrl/Cmd+Enter or a Ctrl/Cmd-click opens
 * those in a new tab), and an app from the catalogue on the Apps pane with
 * its install dialog already open, so installing still goes through the two
 * steps that pane asks for. */

const GROUP_TITLE: Record<GroupId, MessageKey> = {
  pins: "pins.title",
  pages: "palette.group.pages",
  apps: "palette.group.apps",
  files: "palette.group.files",
  repos: "palette.group.repos",
  issues: "palette.group.issues",
  text: "palette.group.text",
  install: "palette.group.install",
  actions: "palette.group.actions",
};

/** How many rows a group shows. The palette is for jumping, not browsing. */
const LIMIT: Record<GroupId, number> = { pins: 8, pages: 8, apps: 6, files: 8, repos: 6, issues: 6, text: 8, install: 5, actions: 8 };

/** Letters typed before the box is asked what is inside the files: fewer
 *  match nearly every document. */
const TEXT_MIN = 3;

/** Before anything is typed: the pins, where to go, a few repositories,
 *  the open issues given to the person, what to do. */
const IDLE_GROUPS: readonly GroupId[] = ["pins", "pages", "repos", "issues", "actions"];

interface Group {
  id: GroupId;
  items: Item[];
  /** A line in place of, or under, the rows: still looking, or why not. */
  note: { text: string; busy: boolean } | null;
}

export default function CommandPalette() {
  const t = useT();
  const { open, query, serial } = React.useSyncExternalStore(subscribePalette, paletteState, paletteState);
  const titleId = React.useId();
  // The body stays until the dialog's way out (index.css) has finished:
  // unmounted at once, an empty box would fade. With reduced motion there
  // is nothing to wait for and it goes at once.
  const [kept, setKept] = React.useState(open);
  React.useEffect(() => {
    if (open) {
      setKept(true);
      return;
    }
    let live = true;
    const leaving = document.getElementById(titleId)?.closest("dialog")?.getAnimations() ?? [];
    void Promise.allSettled(leaving.map((animation) => animation.finished)).then(() => {
      if (live) setKept(false);
    });
    return () => {
      live = false;
    };
  }, [open, titleId]);

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) closePalette();
      }}
      labelledBy={titleId}
      dialogClassName="mt-[10vh] mb-auto w-[min(40rem,calc(100vw-2rem))] max-w-[min(40rem,calc(100vw-2rem))]"
      className="flex max-h-[min(34rem,calc(100dvh-14vh))] flex-col overflow-hidden"
    >
      <h2 id={titleId} className="sr-only">
        {t("palette.title")}
      </h2>
      {/* A fresh body per opening: the query starts from what was handed
          over and nothing lingers from the last search. */}
      {(open || kept) && <PaletteBody key={serial} initial={query} />}
    </Dialog>
  );
}

function PaletteBody({ initial }: { initial: string }) {
  const t = useT();
  useLocale();
  React.useSyncExternalStore(subscribeTheme, getTheme, getTheme);
  const navigate = useNavigate();
  const base = useSiteBase();
  const [query, setQuery] = React.useState(initial);
  const [activeId, setActiveId] = React.useState<string | null>(null);
  const listId = React.useId();
  const listRef = React.useRef<HTMLDivElement>(null);
  const typed = query.trim().length > 0;

  const files = useRemote(true, query, 2, searchFiles);
  const repos = useRemote(true, query, 0, searchRepos);
  const issues = useRemote(true, query, 0, searchIssues);
  const install = useRemote(true, query, 2, searchApps);
  const cloudText = useRemote(true, query, TEXT_MIN, searchCloudText);
  const gitText = useRemote(true, query, TEXT_MIN, searchGitText);
  const pins = React.useSyncExternalStore(subscribePins, getPinsState, getServerPinsState);
  const pinned = pinItems(
    pins.pins.flatMap((pin) => {
      const view = viewPin(pin, pins.installed, pins.names);
      return view === null ? [] : [{ pin, view }];
    }),
  );

  const groups: Group[] = [];
  const push = (id: GroupId, items: Item[], note: Group["note"] = null): void => {
    if (typed || IDLE_GROUPS.includes(id)) {
      const shown = items.slice(0, LIMIT[id]);
      if (shown.length > 0 || note !== null) groups.push({ id, items: shown, note });
    }
  };

  push("pins", typed ? rank(pinned, query) : pinned);
  // Before anything is typed the pages are a plain list of where to go;
  // the line under each name is for telling matches apart.
  push("pages", typed ? rank(pageItems(), query) : pageItems().map((item) => ({ ...item, detail: null })));
  push("apps", rank(appItems(), query));
  push(
    "files",
    remoteItems(files, query, (file) => ({
      id: `file-${file.id}`,
      group: "files",
      title: file.name,
      detail: parentOf(file.path),
      icon: file.folder ? Folder01Icon : File01Icon,
      target: { kind: "href", href: fileHref(file) },
      pin: { kind: "file", id: file.id, path: file.path, folder: file.folder },
    })),
    remoteNote(files, {
      busy: "palette.files.looking",
      signIn: "palette.files.signIn",
      notUp: "palette.files.notUp",
      failed: "palette.files.failed",
    }, t),
  );
  push(
    "repos",
    remoteItems(repos, query, (repo) => ({
      id: `repo-${repo.owner}-${repo.name}`,
      group: "repos",
      title: `${repo.owner}/${repo.name}`,
      detail: repo.description,
      icon: SourceCodeIcon,
      target: { kind: "href", href: repoHref(repo) },
      pin: { kind: "repo", owner: repo.owner, name: repo.name },
    })),
    remoteNote(repos, {
      busy: "palette.repos.looking",
      notUp: "palette.repos.notUp",
      failed: "palette.repos.failed",
    }, t),
  );
  push(
    "issues",
    remoteItems(issues, query, (issue) => ({
      id: `issue-${issue.key}`,
      group: "issues",
      title: issue.title,
      detail: issue.state === "" ? issue.key : `${issue.key} · ${issue.state}`,
      words: issue.key,
      icon: TaskDone01Icon,
      target: { kind: "href", href: issueHref(issue) },
      pin: { kind: "issue", key: issue.key },
    })),
    remoteNote(issues, { busy: "palette.issues.looking", failed: "palette.issues.failed" }, t),
  );
  push(
    "text",
    [
      ...remoteItems(cloudText, query, (hit) => {
        const cut = hit.path.lastIndexOf("/");
        return {
          id: `text-cloud-${hit.path}`,
          group: "text",
          title: hit.path.slice(cut + 1),
          location: cut < 0 ? t("palette.inCloud") : `${t("palette.inCloud")} · ${hit.path.slice(0, cut)}`,
          snippet: hit.snippet,
          words: hit.snippet.join(""),
          icon: FileSearchIcon,
          target: { kind: "href", href: cloudTextHref(hit) },
        };
      }),
      ...remoteItems(gitText, query, (hit) => ({
        id: `text-git-${hit.owner}-${hit.repo}-${hit.path}-${hit.line}`,
        group: "text",
        title: hit.path.slice(hit.path.lastIndexOf("/") + 1),
        location: `${hit.owner}/${hit.repo} · ${t("palette.text.line", { line: String(hit.line) })}`,
        snippet: hit.snippet,
        words: hit.snippet.join(""),
        icon: CodeIcon,
        target: { kind: "href", href: gitTextHref(hit) },
      })),
    ],
    textNote(cloudText, gitText, t),
  );
  push(
    "install",
    // A hit with nowhere to fetch it from has no Install button on the pane
    // either (install-dialog.tsx), so it is not offered here.
    remoteItems(install, query, (app) => ({
      id: `install-${app.id}`,
      group: "install",
      title: app.name,
      detail: [app.source, app.version].filter((part) => part !== null).join(" · "),
      icon: PackageIcon,
      target: { kind: "route", to: "/apps", state: { install: app, query: query.trim() } },
    }), (app) => app.chart !== null),
    remoteNote(install, { busy: "palette.install.looking", failed: "palette.install.failed" }, t),
  );
  push("actions", typed ? rank(actionItems(), query) : actionItems());

  const options = groups.flatMap((group) => group.items);
  const active = options.find((item) => item.id === activeId) ?? options[0] ?? null;
  const optionId = (item: Item): string => `${listId}-${item.id}`;
  const stillLooking = groups.some((group) => group.note?.busy === true);

  React.useEffect(() => {
    if (active === null) return;
    document.getElementById(`${listId}-${active.id}`)?.scrollIntoView({ block: "nearest" });
  }, [active, listId]);

  const choose = (item: Item, newTab = false): void => {
    closePalette();
    go(item.target, newTab, navigate, base);
  };

  const move = (by: number): void => {
    if (options.length === 0) return;
    const at = active === null ? -1 : options.indexOf(active);
    const next = options[(at + by + options.length) % options.length];
    if (next !== undefined) setActiveId(next.id);
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>): void => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        event.preventDefault();
        move(-1);
        break;
      case "Home":
      case "End":
        if (!event.ctrlKey) return;
        event.preventDefault();
        setActiveId((event.key === "Home" ? options[0] : options[options.length - 1])?.id ?? null);
        break;
      case "Enter":
        event.preventDefault();
        if (active !== null) choose(active, event.ctrlKey || event.metaKey);
        break;
    }
  };

  return (
    <>
      <div className="flex items-center gap-2.5 border-b border-hair px-4">
        <HugeiconsIcon
          icon={Search01Icon}
          size={17}
          strokeWidth={1.5}
          color="currentColor"
          className="shrink-0 text-faint"
          aria-hidden="true"
        />
        <input
          type="text"
          role="combobox"
          aria-label={t("palette.label")}
          aria-expanded="true"
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={active === null ? undefined : optionId(active)}
          autoFocus
          autoComplete="off"
          spellCheck={false}
          value={query}
          placeholder={t("palette.placeholder")}
          onChange={(event) => {
            setQuery(event.target.value);
            setActiveId(null);
          }}
          onKeyDown={onKeyDown}
          className="h-12 min-w-0 flex-1 bg-transparent text-[15px] text-ink outline-none placeholder:text-faint"
        />
        {stillLooking && <Spinner size={15} label={t("palette.looking")} className="text-faint" />}
      </div>

      <div ref={listRef} id={listId} role="listbox" aria-label={t("palette.results")} className="min-h-0 flex-1 overflow-y-auto p-1.5">
        {groups.length === 0 && (
          <p className="animate-fade-in px-3 py-6 text-center text-[13px] text-muted">
            {t("palette.noMatch", { query: query.trim() })}
          </p>
        )}
        {groups.map((group) => (
          <div key={group.id} role="group" aria-labelledby={`${listId}-g-${group.id}`} className="pb-1">
            <div
              id={`${listId}-g-${group.id}`}
              role="presentation"
              className="px-2.5 pt-2 pb-1 text-[11.5px] font-medium tracking-wide text-faint"
            >
              {t(GROUP_TITLE[group.id])}
            </div>
            {group.items.map((item) => (
              <Option
                key={item.id}
                id={optionId(item)}
                item={item}
                selected={item === active}
                onHover={() => setActiveId(item.id)}
                onChoose={(newTab) => choose(item, newTab)}
              />
            ))}
            {group.note !== null && (
              <p role="presentation" className="px-2.5 py-1.5 text-[12.5px] leading-snug text-muted">
                {group.note.text}
              </p>
            )}
          </div>
        ))}
      </div>

      <div
        aria-hidden="true"
        className="hidden items-center gap-4 border-t border-hair px-4 py-2 text-[11.5px] text-faint sm:flex"
      >
        <span>
          <Kbd>↑</Kbd> <Kbd>↓</Kbd> {t("palette.hint.move")}
        </span>
        <span>
          <Kbd>Enter</Kbd> {t("palette.hint.open")}
        </span>
        <span>
          <Kbd>Esc</Kbd> {t("palette.hint.close")}
        </span>
      </div>
    </>
  );
}

function Option({
  id,
  item,
  selected,
  onHover,
  onChoose,
}: {
  id: string;
  item: Item;
  selected: boolean;
  onHover: () => void;
  onChoose: (newTab: boolean) => void;
}) {
  const target = item.target;
  const newTab =
    target.kind === "href" ? () => onChoose(true) : target.kind === "route" ? () => openInNewTab(target.to) : undefined;
  const t = useT();
  const items = usePinItems();
  return (
      <div
        ref={contextMenu(() => ({
          label: item.title,
          sections: [
            [
              { id: "open", label: t("menu.open"), keys: ["Enter"], onSelect: () => onChoose(false) },
              { ...items.newTab(() => newTab?.()), keys: ["Mod", "Enter"], hidden: newTab === undefined },
            ],
            [items.pin(item.pin ?? null, item.group === "issues" ? item.title : undefined)],
          ],
        }))}
        id={id}
        role="option"
        aria-selected={selected}
        data-group={item.group}
        // The input keeps focus; a press here must not take it away first.
        onMouseDown={(event) => event.preventDefault()}
        onMouseMove={selected ? undefined : onHover}
        onClick={(event) => onChoose(event.ctrlKey || event.metaKey)}
        className={cn(
          "flex cursor-default items-center gap-3 rounded-control px-2.5 py-2",
          selected ? "bg-accent-wash text-ink" : "text-ink",
        )}
      >
        <span
          className={cn(
            "flex size-7 shrink-0 items-center justify-center rounded-control border border-line bg-sunk",
            selected ? "text-accent" : "text-muted",
          )}
        >
          <HugeiconsIcon icon={item.icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex min-w-0 items-baseline gap-2">
            <span className="truncate text-[13.5px] leading-tight font-medium">{item.title}</span>
            {item.location !== undefined && (
              <span className="min-w-0 shrink truncate text-[12px] leading-tight text-faint">{item.location}</span>
            )}
          </span>
          {item.snippet !== undefined ? (
            <span className="block truncate text-[12px] leading-tight text-muted">
              {item.snippet.map((run, index) =>
                index % 2 === 1 ? (
                  <mark key={index} className="bg-transparent font-semibold text-ink">
                    {run}
                  </mark>
                ) : (
                  <React.Fragment key={index}>{run}</React.Fragment>
                ),
              )}
            </span>
          ) : (
            item.detail !== undefined &&
            item.detail !== null &&
            item.detail.length > 0 && (
              <span className="block truncate text-[12px] leading-tight text-muted">{item.detail}</span>
            )
          )}
        </span>
        {item.keys !== undefined && <KbdGroup keys={item.keys} className="shrink-0" />}
      </div>
  );
}

/* `base` is the router's basename (lib/site.tsx): a route is the admin
 * page's, so from LosOS Cloud it is a page load; an address in LosOS Cloud
 * is a router move there and a page load from anywhere else. */
function go(target: Target, newTab: boolean, navigate: ReturnType<typeof useNavigate>, base: string): void {
  switch (target.kind) {
    case "route":
      if (base !== "") window.location.assign(target.to);
      else navigate(target.to, target.state === undefined ? undefined : { state: target.state });
      return;
    case "href": {
      if (newTab || target.newTab === true) {
        window.open(target.href, "_blank", "noopener");
        return;
      }
      const route = base === "" ? null : routeIn(base, target.href);
      if (route === null) window.location.assign(target.href);
      else navigate(route);
      return;
    }
    case "run":
      target.run();
      return;
  }
}

/* A remote group's rows. An answer to an earlier query stays on screen
 * while the next is on its way, narrowed to what is typed now, so the list
 * does not blink at every key and Enter never opens a row that no longer
 * matches. */
function remoteItems<T>(
  remote: Remote<T>,
  query: string,
  toItem: (value: T) => Item,
  keep: (value: T) => boolean = () => true,
): Item[] {
  if (remote.kind !== "ready") return [];
  const items = remote.items.filter(keep).map(toItem);
  return remote.query === query.trim() ? items : rank(items, query);
}

interface NoteKeys {
  busy: MessageKey;
  signIn?: MessageKey;
  notUp?: MessageKey;
  failed: MessageKey;
}

/* What a remote group says instead of rows. Loading says so only while
 * there is nothing yet to show; an empty answer says nothing at all, since
 * "no files match" next to three pages that do would be noise. */
function remoteNote<T>(remote: Remote<T>, keys: NoteKeys, t: ReturnType<typeof useT>): Group["note"] {
  switch (remote.kind) {
    case "loading":
      return { text: t(keys.busy), busy: true };
    case "blocked": {
      // A box without LosOS Git simply has no repositories to list.
      if (remote.reason === "noGit") return null;
      const key = remote.reason === "signIn" ? keys.signIn : keys.notUp;
      return key === undefined ? null : { text: t(key), busy: false };
    }
    case "failed":
      return { text: t(keys.failed), busy: false };
    default:
      return null;
  }
}

/* The text group's one line for both searches: looking while neither has
 * anything to show yet, then that the cloud index is still being built, then
 * a failure. A search the box does not run (off, no LosOS Git, not up yet)
 * says nothing; the other one still lists its hits. */
function textNote<A, B>(cloud: Remote<A>, git: Remote<B>, t: ReturnType<typeof useT>): Group["note"] {
  const shown = (r: Remote<unknown>): boolean => r.kind === "ready" && r.items.length > 0;
  if ((cloud.kind === "loading" || git.kind === "loading") && !shown(cloud) && !shown(git)) {
    return { text: t("palette.text.looking"), busy: true };
  }
  if (cloud.kind === "ready" && cloud.partial) return { text: t("palette.text.partial"), busy: false };
  if (cloud.kind === "failed" || git.kind === "failed") return { text: t("palette.text.failed"), busy: false };
  return null;
}

/** "/Documents/Taxes/x.pdf" -> "/Documents/Taxes". */
function parentOf(path: string): string {
  const cut = path.replace(/\/+$/, "").lastIndexOf("/");
  return cut <= 0 ? "/" : path.slice(0, cut);
}
