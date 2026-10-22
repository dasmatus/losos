import * as React from "react";
import { Navigate, Route, Routes, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  BookmarkAdd01Icon,
  Cancel01Icon,
  Delete02Icon,
  FilterIcon,
  KeyboardIcon,
  LayoutGridIcon,
  Menu01Icon,
  Search01Icon,
  Task01Icon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Dialog, DialogBody, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { SwitchRow } from "@/screens/settings/rows";
import { toast } from "@/components/ui/toast";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { sharedApp, loadMe } from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { ErrorAlert, failed, Loading, PageTitle } from "../parts";
import { useLoad, useMe, useVisitor } from "../store";
import { tracker, type Category, type Filter, type GroupBy, type Issue, type Layout, type Priority, type Project, type View } from "./api";
import { Board } from "./board";
import { CreateIssue, type Draft } from "./create";
import { InboxScreen } from "./inbox";
import { IssueList, grouped } from "./list";
import { Palette } from "./palette";
import { AssigneeMenu, CATEGORY_NAME, LabelMenu, PRIORITIES, PRIORITY_NAME, PriorityIcon, StateIcon, StateMenu, currentCycle } from "./parts";
import { CyclesView, NewProject, ProjectTabs } from "./projects";
import { Reader } from "./reader";
import { Shortcuts } from "./shortcuts";
import { TrackerContext, typingIn, useTracker } from "./store";

/* LosOS Git's issues at /git/issues: one tracker for every repository and
 * project on the box. A list (or a board) of issues and the open issue
 * beside it in one card, as LosOS Mail lays out its folder and message;
 * on a phone one at a time. Everything is a keystroke away: c makes an
 * issue, j and k move, Enter opens, x selects, s, p, a and l change the
 * state, priority, assignee and labels of the open or selected ones,
 * / searches and Ctrl+K (or Cmd+K) finds anything.
 *
 * What a page shows lives in its address (?layout=board&assignee=me&open=
 * LOS-12), so a reload, Back and a shared link land on the same thing. */

export function TrackerProvider({ children }: { children: React.ReactNode }) {
  const me = useMe();
  const visitor = useVisitor();
  const home = useLoad((signal) => (me === null ? Promise.resolve(null) : tracker.home(signal)), [me?.login]);
  // Which of Mail, Chat and Contacts this person has, for the hand-overs.
  React.useEffect(() => {
    if (me !== null && !visitor) void loadMe().catch(() => null);
  }, [me, visitor]);
  const value = React.useMemo(() => ({ home: home.data, reloadHome: home.reload }), [home.data, home.reload]);
  return <TrackerContext.Provider value={value}>{children}</TrackerContext.Provider>;
}

export function IssuesRoot() {
  const t = useT();
  const visitor = useVisitor();
  if (visitor) {
    return (
      <Empty className="mt-10">
        <EmptyHeader>
          <EmptyTitle>{t("tracker.visitor.title")}</EmptyTitle>
          <EmptyDescription>{t("tracker.visitor.body")}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }
  return (
    <Routes>
      <Route index element={<ScopeScreen scope={{ kind: "mine" }} />} />
      <Route path="inbox" element={<InboxScreen />} />
      <Route path="all" element={<ScopeScreen scope={{ kind: "all" }} />} />
      <Route path="views/:id" element={<ViewRoute />} />
      <Route path="p/:key" element={<ProjectRoute tab="issues" />} />
      <Route path="p/:key/backlog" element={<ProjectRoute tab="backlog" />} />
      <Route path="p/:key/cycles" element={<ProjectRoute tab="cycles" />} />
      <Route path="new-project" element={<NewProject />} />
      <Route path=":key" element={<IssueRoute />} />
    </Routes>
  );
}

/** What a screen lists before the person narrows it. */
export type Scope =
  | { kind: "mine" }
  | { kind: "all" }
  | { kind: "project"; project: Project; backlog: boolean }
  | { kind: "view"; view: View };

function scopeFilter(scope: Scope): Filter {
  switch (scope.kind) {
    case "mine":
      return { assignee: ["me"], category: ["active"] };
    case "all":
      return { category: ["active"] };
    case "project":
      return scope.backlog ? { project: [scope.project.key], category: ["backlog"] } : { project: [scope.project.key] };
    case "view":
      return scope.view.query;
  }
}

function ProjectRoute({ tab }: { tab: "issues" | "backlog" | "cycles" }) {
  const t = useT();
  const { key = "" } = useParams();
  const { home } = useTracker();
  const project = home?.projects.find((p) => p.key === key.toUpperCase());
  if (home === null) return <Loading rows={6} />;
  if (project === undefined) {
    return (
      <Empty className="mt-10">
        <EmptyHeader>
          <EmptyTitle>{t("tracker.project.missing")}</EmptyTitle>
        </EmptyHeader>
      </Empty>
    );
  }
  if (tab === "cycles") return <CyclesView project={project} />;
  return <ScopeScreen key={`${project.key}-${tab}`} scope={{ kind: "project", project, backlog: tab === "backlog" }} />;
}

function ViewRoute() {
  const { id = "" } = useParams();
  const { home } = useTracker();
  const view = home?.views.find((v) => String(v.id) === id);
  if (home === null) return <Loading rows={6} />;
  if (view === undefined) return <Navigate to="/issues" replace />;
  return <ScopeScreen key={view.id} scope={{ kind: "view", view }} />;
}

/** /issues/LOS-12: its project's list with it open. */
function IssueRoute() {
  const { key = "" } = useParams();
  const { home } = useTracker();
  const prefix = key.split("-")[0]?.toUpperCase() ?? "";
  if (home === null) return <Loading rows={6} />;
  const project = home.projects.find((p) => p.key === prefix);
  if (project === undefined) return <Navigate to="/issues" replace />;
  return <Navigate to={`/issues/p/${project.key}?open=${encodeURIComponent(key.toUpperCase())}`} replace />;
}

// ── The list or board of one scope ──────────────────────────────────────────

/** The filter the address adds to the scope's. */
function fromParams(params: URLSearchParams): Filter {
  const list = (k: string) => (params.get(k) ?? "").split(",").filter((v) => v !== "");
  const f: Filter = {};
  for (const k of ["category", "assignee", "label", "cycle", "project"] as const) {
    const v = list(k);
    if (v.length > 0) f[k] = v;
  }
  const prio = list("priority").map(Number).filter((n) => Number.isInteger(n));
  if (prio.length > 0) f.priority = prio;
  const q = params.get("q");
  if (q !== null && q !== "") f.q = q;
  const sort = params.get("sort");
  if (sort === "priority" || sort === "updated" || sort === "created" || sort === "due") f.sort = sort;
  return f;
}

function merge(base: Filter, extra: Filter): Filter {
  return { ...base, ...Object.fromEntries(Object.entries(extra).filter(([, v]) => v !== undefined)) };
}

function ScopeScreen({ scope }: { scope: Scope }) {
  const t = useT();
  const navigate = useNavigate();
  const { home, reloadHome } = useTracker();
  const [params, setParams] = useSearchParams();
  const extra = fromParams(params);
  const filter = merge(scopeFilter(scope), extra);
  const project = scope.kind === "project" ? scope.project : undefined;
  const viewLayout = scope.kind === "view" ? scope.view.layout : undefined;
  const viewGroup = scope.kind === "view" ? scope.view.groupBy : undefined;
  const layout: Layout = (params.get("layout") as Layout | null) ?? viewLayout ?? "list";
  const groupBy: GroupBy = (params.get("group") as GroupBy | null) ?? viewGroup ?? (project === undefined ? "project" : "state");
  // A board shows every state; a list of a project hides what is over
  // unless asked.
  const listed = layout === "board" && project !== undefined && extra.category === undefined ? { ...filter, category: undefined } : filter;
  const key = JSON.stringify(listed);
  const issues = useLoad((signal) => tracker.issues(listed, signal), [key]);
  const [shown, setShown] = React.useState<Issue[]>([]);
  React.useEffect(() => {
    if (issues.data !== null) setShown(issues.data);
  }, [issues.data]);
  const open = params.get("open");
  const [focus, setFocus] = React.useState<string | null>(open);
  const [selected, setSelected] = React.useState<ReadonlySet<string>>(new Set());
  const [creating, setCreating] = React.useState<Draft | null>(null);
  const [palette, setPalette] = React.useState(false);
  const [help, setHelp] = React.useState(false);
  const [menu, setMenu] = React.useState<"state" | "priority" | "assignee" | "labels" | null>(null);
  const [saving, setSaving] = React.useState(false);
  const search = React.useRef<HTMLInputElement>(null);
  const [query, setQuery] = React.useState(params.get("q") ?? "");

  const set = React.useCallback(
    (changes: Record<string, string | null>) => {
      setParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          for (const [k, v] of Object.entries(changes)) {
            if (v === null || v === "") next.delete(k);
            else next.set(k, v);
          }
          return next;
        },
        { replace: !("open" in changes) },
      );
    },
    [setParams],
  );

  const order = React.useMemo(() => grouped(shown, groupBy, home).flatMap((g) => g.issues.map((i) => i.key)), [shown, groupBy, home]);
  const openIssue = (k: string | null) => {
    set({ open: k });
    if (k !== null) setFocus(k);
  };
  const changed = React.useCallback(() => {
    issues.reload();
    reloadHome();
  }, [issues, reloadHome]);

  /** Apply one change to the open issue, or to every selected one. */
  const targets = (): string[] => (selected.size > 0 ? [...selected] : open !== null ? [open] : focus !== null ? [focus] : []);
  const change = async (fields: Parameters<typeof tracker.bulk>[1], done: MessageKey) => {
    const keys = targets();
    if (keys.length === 0) return;
    // At once on the screen; the answer settles it.
    setShown((list) =>
      list.map((i) => (keys.includes(i.key) ? optimistic(i, fields, home?.projects.find((p) => p.id === i.projectId)) : i)),
    );
    try {
      if (keys.length === 1 && keys[0] !== undefined) await tracker.edit(keys[0], fields);
      else await tracker.bulk(keys, fields);
      toast.done(t(done, { count: keys.length }));
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
    changed();
    setRound((r) => r + 1);
  };
  // Tells the reader to read its issue again after a change from here.
  const [round, setRound] = React.useState(0);

  React.useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPalette(true);
        return;
      }
      if (event.metaKey || event.ctrlKey || event.altKey || typingIn(event.target)) return;
      const at = focus === null ? -1 : order.indexOf(focus);
      const move = (by: number) => {
        event.preventDefault();
        const next = order[Math.max(0, Math.min(order.length - 1, at + by))];
        if (next !== undefined) {
          setFocus(next);
          document.querySelector(`[data-issue-key="${next}"]`)?.scrollIntoView({ block: "nearest" });
          if (open !== null) set({ open: next });
        }
      };
      switch (event.key) {
        case "j":
        case "ArrowDown":
          move(1);
          break;
        case "k":
        case "ArrowUp":
          move(-1);
          break;
        case "Enter":
        case "o":
          if (focus !== null) {
            event.preventDefault();
            openIssue(focus);
          }
          break;
        case "x":
          if (focus !== null) {
            event.preventDefault();
            setSelected((s) => toggle(s, focus));
          }
          break;
        case "Escape":
          if (selected.size > 0) setSelected(new Set());
          else if (open !== null) set({ open: null });
          break;
        case "c":
          event.preventDefault();
          setCreating({ project: project?.key ?? home?.projects[0]?.key ?? "" });
          break;
        case "/":
          event.preventDefault();
          search.current?.focus();
          break;
        case "b":
          set({ layout: layout === "board" ? "list" : "board" });
          break;
        case "?":
          setHelp(true);
          break;
        case "s":
        case "p":
        case "a":
        case "l":
          if (targets().length > 0) {
            event.preventDefault();
            setMenu(({ s: "state", p: "priority", a: "assignee", l: "labels" } as const)[event.key]);
          }
          break;
        default:
          // 1 to 4 and 0 set the priority of what is open or picked.
          if (/^[0-4]$/.test(event.key) && targets().length > 0) {
            void change({ priority: Number(event.key) }, "tracker.done.priority");
          }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const title =
    scope.kind === "mine"
      ? t("tracker.nav.mine")
      : scope.kind === "all"
        ? t("tracker.nav.all")
        : scope.kind === "view"
          ? scope.view.name
          : scope.backlog
            ? t("tracker.project.backlogOf", { name: scope.project.name })
            : scope.project.name;
  const cycle = project !== undefined && home !== null ? currentCycle(project, home.today) : undefined;
  const selectedProject = (() => {
    const keys = targets();
    const ids = new Set(shown.filter((i) => keys.includes(i.key)).map((i) => i.projectId));
    return ids.size === 1 ? home?.projects.find((p) => ids.has(p.id)) : undefined;
  })();
  const selectedIssues = shown.filter((i) => targets().includes(i.key));

  return (
    <div className="flex flex-col gap-3" data-tracker-scope={scope.kind}>
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <PageTitle className="flex items-center gap-2">
            {project !== undefined && (
              <span className="rounded-control bg-sunk px-1.5 py-0.5 font-mono text-[12px] font-medium text-muted">{project.key}</span>
            )}
            <span className="truncate">{title}</span>
          </PageTitle>
          {project !== undefined && (
            <ProjectTabs project={project} tab={scope.kind === "project" && scope.backlog ? "backlog" : "issues"} cycle={cycle?.id} />
          )}
        </div>
        <Button variant="ghost" size="icon-sm" onClick={() => setHelp(true)} aria-label={t("tracker.shortcuts.title")} title={t("tracker.shortcuts.title")}>
          <HugeiconsIcon icon={KeyboardIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <Button
          onClick={() => setCreating({ project: project?.key ?? home?.projects[0]?.key ?? "" })}
          disabled={home !== null && home.projects.length === 0}
          data-new-issue
        >
          <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("tracker.new")}
          <kbd className="rounded-sm bg-surface/20 px-1 text-[11px]">C</kbd>
        </Button>
      </div>

      <Toolbar
        filter={filter}
        extra={extra}
        project={project}
        layout={layout}
        groupBy={groupBy}
        query={query}
        searchRef={search}
        onQuery={(q) => {
          setQuery(q);
          if (q === "") set({ q: null });
        }}
        onSearch={() => set({ q: query })}
        onSet={set}
        onSave={() => setSaving(true)}
        canSave={scope.kind !== "view" && Object.keys(extra).length > 0 || layout !== "list" && scope.kind !== "view"}
        onForget={
          scope.kind === "view"
            ? async () => {
                try {
                  await tracker.removeView(scope.view.id);
                  reloadHome();
                  navigate("/issues");
                } catch (e) {
                  failed(t("tracker.error.change"), e);
                }
              }
            : undefined
        }
      />

      <Card className="flex h-[min(52rem,calc(100dvh-14rem))] min-h-[28rem] overflow-hidden">
        <section
          aria-label={title}
          className={cn(
            "min-w-0 flex-1 flex-col",
            open !== null ? "max-md:hidden md:flex" : "flex",
            open !== null && layout === "list" && "md:max-w-[min(40rem,55%)] md:border-r md:border-line",
            open !== null && layout === "board" && "md:border-r md:border-line",
          )}
        >
          {issues.error !== null && issues.data === null ? (
            <div className="m-3 flex flex-col gap-2">
              <ErrorAlert error={issues.error} />
              <Button variant="secondary" size="sm" className="self-start" onClick={issues.reload}>
                {t("tracker.retry")}
              </Button>
            </div>
          ) : issues.data === null ? (
            <div className="p-3">
              <Loading rows={8} />
            </div>
          ) : shown.length === 0 && layout === "list" ? (
            <Nothing scope={scope} filtered={Object.keys(extra).length > 0} onCreate={() => setCreating({ project: project?.key ?? home?.projects[0]?.key ?? "" })} />
          ) : layout === "board" ? (
            <Board
              issues={shown}
              project={project}
              focus={focus}
              open={open}
              onOpen={openIssue}
              onMoved={(next) => setShown(next)}
              onChanged={changed}
              onCreate={(state) => setCreating({ project: project?.key ?? "", state })}
            />
          ) : (
            <IssueList
              issues={shown}
              groupBy={groupBy}
              focus={focus}
              open={open}
              selected={selected}
              onOpen={openIssue}
              onSelect={(k) => setSelected((s) => toggle(s, k))}
              onCreate={project !== undefined ? (state) => setCreating({ project: project.key, state }) : undefined}
            />
          )}
        </section>
        {open !== null && (
          <article className={cn("min-w-0 flex-1 flex-col", "flex", layout === "board" && "md:max-w-[34rem]")} aria-label={open}>
            <Reader
              key={open}
              issueKey={open}
              round={round}
              onBack={() => set({ open: null })}
              onChanged={changed}
              onOpen={(k) => openIssue(k)}
              onCreateChild={(parent, proj) => setCreating({ project: proj, parent })}
              menu={selected.size === 0 ? menu : null}
              onMenu={setMenu}
            />
          </article>
        )}
      </Card>

      {/* The picked issues, or the one in focus while S, P, A or L has a
          menu open on it with nothing beside the list to anchor it to. */}
      {(selected.size > 0 || (open === null && focus !== null && menu !== null)) && (
        <SelectionBar
          count={selectedIssues.length}
          project={selectedProject}
          issues={selectedIssues}
          menu={menu}
          onMenu={setMenu}
          onChange={(fields, done) => void change(fields, done)}
          onClear={() => {
            setSelected(new Set());
            setMenu(null);
          }}
        />
      )}

      {creating !== null && (
        <CreateIssue
          draft={creating}
          onClose={() => setCreating(null)}
          onCreated={(issue, more) => {
            changed();
            if (!more) {
              setCreating(null);
              openIssue(issue.key);
            }
          }}
        />
      )}
      <Palette
        open={palette}
        onClose={() => setPalette(false)}
        onOpenIssue={(k) => {
          setPalette(false);
          navigate(`/issues/${k}`);
        }}
        onCreate={() => {
          setPalette(false);
          setCreating({ project: project?.key ?? home?.projects[0]?.key ?? "" });
        }}
      />
      <Shortcuts open={help} onClose={() => setHelp(false)} />
      {saving && (
        <SaveView
          filter={filter}
          layout={layout}
          groupBy={groupBy}
          onClose={() => setSaving(false)}
          onSaved={(v) => {
            setSaving(false);
            reloadHome();
            navigate(`/issues/views/${v.id}`);
          }}
        />
      )}
    </div>
  );
}

function toggle(set: ReadonlySet<string>, key: string): ReadonlySet<string> {
  const next = new Set(set);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  return next;
}

/** What a change will look like, before the box says so. */
function optimistic(i: Issue, f: Parameters<typeof tracker.bulk>[1], project: Project | undefined): Issue {
  const next = { ...i };
  if (f.priority !== undefined) next.priority = f.priority as Priority;
  if (f.assignee !== undefined) next.assignee = f.assignee;
  if (f.state !== undefined) {
    const s = project?.states.find((x) => x.id === f.state);
    if (s !== undefined) next.state = s;
  }
  if (f.category !== undefined) {
    const s = project?.states.find((x) => x.category === f.category);
    if (s !== undefined) next.state = s;
  }
  return next;
}

function Nothing({ scope, filtered, onCreate }: { scope: Scope; filtered: boolean; onCreate: () => void }) {
  const t = useT();
  const { home } = useTracker();
  const noProjects = home !== null && home.projects.length === 0;
  return (
    <Empty className="m-4 border-0">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <HugeiconsIcon icon={Task01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </EmptyMedia>
        <EmptyTitle>
          {noProjects
            ? t("tracker.empty.noProjects")
            : filtered
              ? t("tracker.empty.filtered")
              : scope.kind === "mine"
                ? t("tracker.empty.mine")
                : t("tracker.empty.none")}
        </EmptyTitle>
        <EmptyDescription>{noProjects ? t("tracker.empty.noProjectsBody") : t("tracker.empty.body")}</EmptyDescription>
      </EmptyHeader>
      {!noProjects && (
        <Button variant="secondary" onClick={onCreate}>
          {t("tracker.new")}
        </Button>
      )}
    </Empty>
  );
}

// ── Filters and display ───────────────────────────────────────────────────

const CATEGORIES: Category[] = ["backlog", "unstarted", "started", "done", "canceled"];

function Toolbar({
  filter,
  extra,
  project,
  layout,
  groupBy,
  query,
  searchRef,
  onQuery,
  onSearch,
  onSet,
  onSave,
  canSave,
  onForget,
}: {
  filter: Filter;
  extra: Filter;
  project: Project | undefined;
  layout: Layout;
  groupBy: GroupBy;
  query: string;
  searchRef: React.RefObject<HTMLInputElement | null>;
  onQuery: (q: string) => void;
  onSearch: () => void;
  onSet: (changes: Record<string, string | null>) => void;
  onSave: () => void;
  canSave: boolean;
  onForget: (() => void) | undefined;
}) {
  const t = useT();
  const { home } = useTracker();
  const faces = useFacesSafe();
  const labels = project?.labels ?? dedupe((home?.projects ?? []).flatMap((p) => p.labels.map((l) => l.name))).map((name, id) => ({ id, name, color: "", description: "" }));
  const togglesIn = (k: "category" | "assignee" | "label" | "priority" | "project", v: string) => {
    const now = (extra[k] ?? []).map(String);
    const next = now.includes(v) ? now.filter((x) => x !== v) : [...now, v];
    onSet({ [k]: next.length === 0 ? null : next.join(",") });
  };
  const chips: { key: string; label: string; clear: () => void }[] = [];
  for (const c of extra.category ?? []) chips.push({ key: `c${c}`, label: c === "active" ? t("tracker.filter.active") : t(CATEGORY_NAME[c as Category]), clear: () => togglesIn("category", c) });
  for (const a of extra.assignee ?? []) chips.push({ key: `a${a}`, label: a === "me" ? t("tracker.me") : a === "none" ? t("tracker.unassigned") : faces(a), clear: () => togglesIn("assignee", a) });
  for (const p of extra.priority ?? []) chips.push({ key: `p${p}`, label: t(PRIORITY_NAME[p as Priority]), clear: () => togglesIn("priority", String(p)) });
  for (const l of extra.label ?? []) chips.push({ key: `l${l}`, label: l, clear: () => togglesIn("label", l) });
  for (const p of extra.project ?? []) chips.push({ key: `j${p}`, label: p, clear: () => togglesIn("project", p) });
  if (extra.cycle !== undefined) chips.push({ key: "cy", label: t("tracker.field.cycle"), clear: () => onSet({ cycle: null }) });

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button variant="secondary" size="sm" data-filter-menu>
                <HugeiconsIcon icon={FilterIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("tracker.filter.label")}
              </Button>
            }
          />
          <DropdownMenuContent className="max-h-96 overflow-y-auto">
            <DropdownMenuLabel>{t("tracker.field.state")}</DropdownMenuLabel>
            <DropdownMenuItem closeOnClick={false} onClick={() => togglesIn("category", "active")}>
              <StateIcon category="started" />
              {t("tracker.filter.active")}
              <On on={(extra.category ?? []).includes("active")} />
            </DropdownMenuItem>
            {CATEGORIES.map((c) => (
              <DropdownMenuItem key={c} closeOnClick={false} onClick={() => togglesIn("category", c)}>
                <StateIcon category={c} />
                {t(CATEGORY_NAME[c])}
                <On on={(extra.category ?? []).includes(c)} />
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
            <DropdownMenuLabel>{t("tracker.field.assignee")}</DropdownMenuLabel>
            {["me", "none", ...(home?.people ?? []).map((p) => p.login).filter((l) => l !== home?.me)].map((a) => (
              <DropdownMenuItem key={a} closeOnClick={false} onClick={() => togglesIn("assignee", a)}>
                {a === "me" ? t("tracker.me") : a === "none" ? t("tracker.unassigned") : faces(a)}
                <On on={(extra.assignee ?? []).includes(a)} />
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
            <DropdownMenuLabel>{t("tracker.field.priority")}</DropdownMenuLabel>
            {PRIORITIES.map((p) => (
              <DropdownMenuItem key={p} closeOnClick={false} onClick={() => togglesIn("priority", String(p))}>
                <PriorityIcon priority={p} />
                {t(PRIORITY_NAME[p])}
                <On on={(extra.priority ?? []).includes(p)} />
              </DropdownMenuItem>
            ))}
            {labels.length > 0 && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuLabel>{t("tracker.field.labels")}</DropdownMenuLabel>
                {labels.map((l) => (
                  <DropdownMenuItem key={l.name} closeOnClick={false} onClick={() => togglesIn("label", l.name)}>
                    {l.name}
                    <On on={(extra.label ?? []).includes(l.name)} />
                  </DropdownMenuItem>
                ))}
              </>
            )}
            {project === undefined && (home?.projects.length ?? 0) > 1 && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuLabel>{t("tracker.field.project")}</DropdownMenuLabel>
                {(home?.projects ?? []).map((p) => (
                  <DropdownMenuItem key={p.key} closeOnClick={false} onClick={() => togglesIn("project", p.key)}>
                    <span className="font-mono text-[12px] text-muted">{p.key}</span>
                    {p.name}
                    <On on={(extra.project ?? []).includes(p.key)} />
                  </DropdownMenuItem>
                ))}
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>

        <ToggleGroup
          variant="outline"
          size="sm"
          value={layout}
          onValueChange={(v) => {
            if (v === "list" || v === "board") onSet({ layout: v === "list" ? null : v });
          }}
          aria-label={t("tracker.layout.label")}
        >
          <ToggleGroupItem value="list" aria-label={t("tracker.layout.list")} title={t("tracker.layout.list")}>
            <HugeiconsIcon icon={Menu01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t("tracker.layout.list")}</span>
          </ToggleGroupItem>
          <ToggleGroupItem value="board" aria-label={t("tracker.layout.board")} title={t("tracker.layout.board")} data-layout-board>
            <HugeiconsIcon icon={LayoutGridIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t("tracker.layout.board")}</span>
          </ToggleGroupItem>
        </ToggleGroup>

        {layout === "list" && (
          <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="ghost" size="sm">{t("tracker.display.label")}</Button>} />
            <DropdownMenuContent>
              <DropdownMenuLabel>{t("tracker.display.group")}</DropdownMenuLabel>
              {(["state", "assignee", "priority", "project", "cycle", "none"] as const).map((g) => (
                <DropdownMenuItem key={g} onClick={() => onSet({ group: g })}>
                  {t(GROUP_NAME[g])}
                  <On on={groupBy === g} />
                </DropdownMenuItem>
              ))}
              <DropdownMenuSeparator />
              <DropdownMenuLabel>{t("tracker.display.sort")}</DropdownMenuLabel>
              {(["manual", "priority", "updated", "created", "due"] as const).map((s) => (
                <DropdownMenuItem key={s} onClick={() => onSet({ sort: s === "manual" ? null : s })}>
                  {t(SORT_NAME[s])}
                  <On on={(filter.sort ?? "manual") === s} />
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}

        <div className="flex-1" />
        {canSave && (
          <Button variant="ghost" size="sm" onClick={onSave} data-save-view>
            <HugeiconsIcon icon={BookmarkAdd01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("tracker.view.save")}
          </Button>
        )}
        {onForget !== undefined && (
          <Button variant="ghost" size="sm" onClick={onForget}>
            <HugeiconsIcon icon={Delete02Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("tracker.view.forget")}
          </Button>
        )}
        <form
          role="search"
          className="w-full sm:w-60"
          onSubmit={(e) => {
            e.preventDefault();
            onSearch();
          }}
        >
          <InputGroup>
            <InputGroupAddon>
              <HugeiconsIcon icon={Search01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </InputGroupAddon>
            <InputGroupInput
              ref={searchRef}
              type="search"
              value={query}
              onChange={(e) => onQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") e.currentTarget.blur();
              }}
              placeholder={t("tracker.search")}
              aria-label={t("tracker.search")}
            />
          </InputGroup>
        </form>
      </div>
      {chips.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5" data-filter-chips>
          {chips.map((c) => (
            <span key={c.key} className="inline-flex animate-pop items-center gap-1 rounded-full bg-accent-wash py-0.5 pr-1 pl-2.5 text-[12px] font-medium text-accent">
              {c.label}
              <button
                type="button"
                onClick={c.clear}
                className="grid size-4 place-items-center rounded-full hover:bg-accent/15 active:scale-[0.97]"
                aria-label={t("tracker.filter.remove", { what: c.label })}
              >
                <HugeiconsIcon icon={Cancel01Icon} size={11} strokeWidth={2} color="currentColor" aria-hidden="true" />
              </button>
            </span>
          ))}
          <Button
            variant="link"
            size="xs"
            onClick={() => onSet({ category: null, assignee: null, priority: null, label: null, project: null, cycle: null })}
          >
            {t("tracker.filter.clear")}
          </Button>
        </div>
      )}
    </div>
  );
}

const GROUP_NAME: Record<GroupBy, MessageKey> = {
  state: "tracker.field.state",
  assignee: "tracker.field.assignee",
  priority: "tracker.field.priority",
  project: "tracker.field.project",
  cycle: "tracker.field.cycle",
  none: "tracker.display.noGroups",
};

const SORT_NAME: Record<NonNullable<Filter["sort"]>, MessageKey> = {
  manual: "tracker.sort.manual",
  priority: "tracker.field.priority",
  updated: "tracker.sort.updated",
  created: "tracker.sort.created",
  due: "tracker.field.due",
};

function On({ on }: { on: boolean }) {
  return <span className={cn("ml-auto size-1.5 rounded-full bg-accent", !on && "invisible")} aria-hidden="true" />;
}

function dedupe(names: string[]): string[] {
  return [...new Set(names.map((n) => n.toLowerCase()))].map((l) => names.find((n) => n.toLowerCase() === l) ?? l);
}

/** A login's name as the faces know it. */
function useFacesSafe(): (login: string) => string {
  const { home } = useTracker();
  return (login) => home?.people.find((p) => p.login === login)?.fullName || login;
}

// ── Many at once ──────────────────────────────────────────────────────────

function SelectionBar({
  count,
  project,
  issues,
  menu,
  onMenu,
  onChange,
  onClear,
}: {
  count: number;
  project: Project | undefined;
  issues: Issue[];
  menu: string | null;
  onMenu: (m: "state" | "priority" | "assignee" | "labels" | null) => void;
  onChange: (fields: Parameters<typeof tracker.bulk>[1], done: MessageKey) => void;
  onClear: () => void;
}) {
  const t = useT();
  const menuProps = (m: "state" | "priority" | "assignee" | "labels") => ({
    open: menu === m,
    onOpenChange: (o: boolean) => onMenu(o ? m : null),
  });
  const common = issues.length > 0 ? issues.flatMap((i) => i.labels.map((l) => l.name)).filter((n, _, all) => all.filter((x) => x === n).length === issues.length) : [];
  return (
    <div
      className="fixed inset-x-0 bottom-4 z-30 mx-auto flex w-fit max-w-[calc(100vw-2rem)] animate-rise flex-wrap items-center gap-1.5 rounded-card border border-line bg-surface p-1.5 pl-3 shadow-pop"
      role="toolbar"
      aria-label={t("tracker.selection.label", { count })}
      data-selection-bar
    >
      <span className="text-[13px] font-medium tabular-nums">{t("tracker.selection.count", { count })}</span>
      {project !== undefined ? (
        <StateMenu
          {...menuProps("state")}
          states={project.states}
          onPick={(s) => onChange({ state: s.id }, "tracker.done.state")}
          trigger={<Button variant="ghost" size="sm">{t("tracker.field.state")}</Button>}
        />
      ) : (
        <CategoryMenu {...menuProps("state")} onPick={(c) => onChange({ category: c }, "tracker.done.state")} />
      )}
      <PriorityMenuButton {...menuProps("priority")} onPick={(p) => onChange({ priority: p }, "tracker.done.priority")} />
      <AssigneeMenu
        {...menuProps("assignee")}
        onPick={(login) => onChange({ assignee: login }, "tracker.done.assignee")}
        trigger={<Button variant="ghost" size="sm">{t("tracker.field.assignee")}</Button>}
      />
      {project !== undefined && (
        <LabelMenu
          {...menuProps("labels")}
          labels={project.labels}
          current={common}
          onToggle={(name, on) => onChange(on ? { addLabels: [name] } : { removeLabels: [name] }, "tracker.done.labels")}
          trigger={<Button variant="ghost" size="sm">{t("tracker.field.labels")}</Button>}
        />
      )}
      <Button variant="ghost" size="icon-sm" onClick={onClear} aria-label={t("tracker.selection.clear")} title={t("tracker.selection.clear")}>
        <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </Button>
    </div>
  );
}

/** Across projects a state is named by its kind: each project has its own. */
function CategoryMenu({ open, onOpenChange, onPick }: { open: boolean; onOpenChange: (o: boolean) => void; onPick: (c: Category) => void }) {
  const t = useT();
  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger render={<Button variant="ghost" size="sm">{t("tracker.field.state")}</Button>} />
      <DropdownMenuContent>
        {CATEGORIES.map((c) => (
          <DropdownMenuItem key={c} onClick={() => onPick(c)}>
            <StateIcon category={c} />
            {t(CATEGORY_NAME[c])}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function PriorityMenuButton({ open, onOpenChange, onPick }: { open: boolean; onOpenChange: (o: boolean) => void; onPick: (p: Priority) => void }) {
  const t = useT();
  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger render={<Button variant="ghost" size="sm">{t("tracker.field.priority")}</Button>} />
      <DropdownMenuContent>
        {PRIORITIES.map((p) => (
          <DropdownMenuItem key={p} onClick={() => onPick(p)}>
            <PriorityIcon priority={p} />
            {t(PRIORITY_NAME[p])}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

// ── Saving a view ─────────────────────────────────────────────────────────

function SaveView({
  filter,
  layout,
  groupBy,
  onClose,
  onSaved,
}: {
  filter: Filter;
  layout: Layout;
  groupBy: GroupBy;
  onClose: () => void;
  onSaved: (v: View) => void;
}) {
  const t = useT();
  const id = React.useId();
  const [name, setName] = React.useState("");
  const [shared, setShared] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const save = async () => {
    if (name.trim() === "") return;
    setBusy(true);
    try {
      onSaved(await tracker.saveView({ name: name.trim(), query: filter, layout, groupBy, shared }));
      toast.done(t("tracker.view.saved", { name: name.trim() }));
    } catch (e) {
      failed(t("tracker.error.change"), e);
      setBusy(false);
    }
  };
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()} labelledBy={`${id}-title`}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <DialogHeader>
          <DialogTitle id={`${id}-title`}>{t("tracker.view.save")}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          <Field>
            <FieldLabel htmlFor={`${id}-name`}>{t("tracker.view.name")}</FieldLabel>
            <Input id={`${id}-name`} value={name} onChange={(e) => setName(e.target.value)} autoFocus maxLength={40} />
          </Field>
          <SwitchRow id={`${id}-shared`} title={t("tracker.view.shared")} description={t("tracker.view.sharedHint")} checked={shared} onCheckedChange={setShared} last />
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="secondary" onClick={onClose}>
            {t("git.cancel")}
          </Button>
          <Button type="submit" disabled={busy || name.trim() === ""}>
            {t("tracker.view.save")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

/** Whether Mail, Chat or Contacts may be handed something: those run only
 *  where the box shares, and a link that leads to a 404 is not drawn. */
export function canHandTo(app: "mail" | "chat" | "contacts"): boolean {
  return sharedApp(app);
}
