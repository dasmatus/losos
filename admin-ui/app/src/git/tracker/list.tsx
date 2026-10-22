import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, ArrowDown01Icon, Comment01Icon, GitPullRequestIcon } from "@hugeicons/core-free-icons";
import { staggerClass } from "@/components/AppTile";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { closed, priorityRank, type GroupBy, type Home, type Issue, type Priority, type WorkflowState } from "./api";
import { Assignee, CATEGORY_NAME, DueDate, LabelChip, PRIORITY_NAME, PriorityIcon, StateIcon, cycleName } from "./parts";
import { useTracker } from "./store";

/* Issues in groups, one row each: priority, key, state, title, labels, what
 * is under it, the due day and who has it. A row is a button that opens the
 * issue beside the list; the box at its start picks it for a change to
 * many at once. Groups fold. */

export interface Group {
  id: string;
  title: string;
  /** What the header draws before the title. */
  icon?: React.ReactNode;
  /** A state a new issue made from the header lands in. */
  state?: WorkflowState;
  issues: Issue[];
}

export function grouped(issues: readonly Issue[], by: GroupBy, home: Home | null, t?: ReturnType<typeof useT>): Group[] {
  const label = (k: Parameters<NonNullable<typeof t>>[0]) => (t === undefined ? String(k) : t(k));
  const map = new Map<string, Group>();
  const add = (id: string, make: () => Omit<Group, "issues">, issue: Issue) => {
    let g = map.get(id);
    if (g === undefined) {
      g = { ...make(), issues: [] };
      map.set(id, g);
    }
    g.issues.push(issue);
  };
  for (const i of issues) {
    switch (by) {
      case "state": {
        // States of one name across projects are one group.
        add(`${i.state.category}:${i.state.name.toLowerCase()}`, () => ({ id: "", title: i.state.name, state: i.state, icon: <StateIcon category={i.state.category} /> }), i);
        break;
      }
      case "assignee":
        add(i.assignee ?? "", () => ({ id: "", title: i.assignee === null ? label("tracker.unassigned") : (home?.people.find((p) => p.login === i.assignee)?.fullName || i.assignee), icon: <Assignee login={i.assignee} /> }), i);
        break;
      case "priority":
        add(String(i.priority), () => ({ id: "", title: label(PRIORITY_NAME[i.priority]), icon: <PriorityIcon priority={i.priority} /> }), i);
        break;
      case "project": {
        const p = home?.projects.find((x) => x.id === i.projectId);
        add(String(i.projectId), () => ({ id: "", title: p?.name ?? String(i.projectId), icon: <span className="font-mono text-[11.5px] text-muted">{p?.key}</span> }), i);
        break;
      }
      case "cycle": {
        const p = home?.projects.find((x) => x.id === i.projectId);
        const c = p?.cycles.find((x) => x.id === i.cycleId);
        add(c === undefined ? "" : String(c.id), () => ({ id: "", title: c === undefined ? label("tracker.cycle.none") : t === undefined ? c.name : cycleName(c, t) }), i);
        break;
      }
      case "none":
        add("all", () => ({ id: "", title: "" }), i);
        break;
    }
  }
  const groups = [...map.entries()].map(([id, g]) => ({ ...g, id }));
  const rank = (g: Group): number => {
    switch (by) {
      case "state": {
        const s = g.issues[0]?.state;
        if (s === undefined) return 0;
        const order = ["started", "unstarted", "backlog", "done", "canceled"].indexOf(s.category);
        return order * 1000 + s.position;
      }
      case "priority":
        return priorityRank(g.issues[0]?.priority ?? 0);
      case "assignee":
        return g.id === "" ? 1e9 : g.id === home?.me ? -1 : 0;
      case "cycle":
        return g.id === "" ? 1e9 : -Number(g.id);
      default:
        return 0;
    }
  };
  return groups.sort((a, b) => rank(a) - rank(b) || a.title.localeCompare(b.title));
}

export function IssueList({
  issues,
  groupBy,
  focus,
  open,
  selected,
  onOpen,
  onSelect,
  onCreate,
}: {
  issues: readonly Issue[];
  groupBy: GroupBy;
  focus: string | null;
  open: string | null;
  selected: ReadonlySet<string>;
  onOpen: (key: string) => void;
  onSelect: (key: string) => void;
  onCreate: ((state: number) => void) | undefined;
}) {
  const t = useT();
  const { home } = useTracker();
  const groups = grouped(issues, groupBy, home, t);
  const [folded, setFolded] = React.useState<ReadonlySet<string>>(() => new Set(groups.filter((g) => g.state !== undefined && closed(g.state.category)).map((g) => g.id)));
  let row = 0;
  return (
    <div className="min-h-0 flex-1 overflow-y-auto" role="list" aria-label={t("tracker.list.label")} data-issue-list>
      {groups.map((g) => {
        const isFolded = folded.has(g.id);
        return (
          <section key={g.id} aria-label={g.title || t("tracker.list.label")}>
            {groupBy !== "none" && (
              <div className="sticky top-0 z-10 flex items-center gap-2 border-b border-hair bg-sunk px-3 py-1.5">
                <button
                  type="button"
                  onClick={() => setFolded((f) => (f.has(g.id) ? new Set([...f].filter((x) => x !== g.id)) : new Set([...f, g.id])))}
                  className="flex min-w-0 flex-1 items-center gap-2 text-left text-[13px] font-medium text-ink"
                  aria-expanded={!isFolded}
                >
                  <HugeiconsIcon
                    icon={ArrowDown01Icon}
                    size={13}
                    strokeWidth={1.8}
                    color="currentColor"
                    className={cn("shrink-0 text-muted transition-transform duration-150", isFolded && "-rotate-90")}
                    aria-hidden="true"
                  />
                  {g.icon}
                  <span className="truncate">{g.title}</span>
                  <span className="text-[12px] font-normal text-muted tabular-nums">{g.issues.length}</span>
                </button>
                {onCreate !== undefined && g.state !== undefined && (
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    onClick={() => g.state !== undefined && onCreate(g.state.id)}
                    aria-label={t("tracker.newIn", { state: g.title })}
                    title={t("tracker.newIn", { state: g.title })}
                  >
                    <HugeiconsIcon icon={Add01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  </Button>
                )}
              </div>
            )}
            {!isFolded &&
              g.issues.map((i) => (
                <IssueRow
                  key={i.key}
                  issue={i}
                  index={row++}
                  focused={focus === i.key}
                  current={open === i.key}
                  selected={selected.has(i.key)}
                  onOpen={() => onOpen(i.key)}
                  onSelect={() => onSelect(i.key)}
                />
              ))}
          </section>
        );
      })}
    </div>
  );
}

function IssueRow({
  issue: i,
  index,
  focused,
  current,
  selected,
  onOpen,
  onSelect,
}: {
  issue: Issue;
  index: number;
  focused: boolean;
  current: boolean;
  selected: boolean;
  onOpen: () => void;
  onSelect: () => void;
}) {
  const t = useT();
  const { home } = useTracker();
  const today = home?.today ?? "";
  return (
    <div
      role="listitem"
      data-issue-key={i.key}
      data-focused={focused || undefined}
      className={cn(
        "group relative flex animate-rise items-center gap-2.5 border-b border-hair pr-3 pl-1.5",
        staggerClass(Math.min(index, 10)),
        current ? "bg-accent-wash" : selected ? "bg-sunk" : "hover:bg-sunk",
        focused && !current && "shadow-[inset_2px_0_0_var(--color-accent)]",
      )}
    >
      <label className="grid size-7 shrink-0 cursor-pointer place-items-center">
        <input
          type="checkbox"
          checked={selected}
          onChange={onSelect}
          className={cn("size-3.5 accent-[var(--color-accent)]", !selected && "opacity-0 group-hover:opacity-100 focus-visible:opacity-100")}
          aria-label={t("tracker.list.pick", { key: i.key })}
        />
      </label>
      <button
        type="button"
        onClick={onOpen}
        aria-current={current ? "true" : undefined}
        className="flex min-w-0 flex-1 items-center gap-2.5 py-2 text-left outline-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-accent"
      >
        <PriorityIcon priority={i.priority as Priority} label={t(PRIORITY_NAME[i.priority as Priority])} />
        <span className="w-16 shrink-0 truncate font-mono text-[12px] text-muted">{i.key}</span>
        <StateIcon category={i.state.category} label={i.state.name || t(CATEGORY_NAME[i.state.category])} />
        <span className={cn("min-w-0 flex-1 truncate text-[13.5px]", closed(i.state.category) ? "text-muted" : "text-ink")}>{i.title}</span>
        <span className="flex shrink-0 items-center gap-1.5 max-sm:hidden">
          {i.labels.slice(0, 2).map((l) => (
            <LabelChip key={l.id} label={l} />
          ))}
          {i.labels.length > 2 && <span className="text-[12px] text-muted">+{i.labels.length - 2}</span>}
        </span>
        {i.children > 0 && (
          <span className="shrink-0 text-[12px] text-muted tabular-nums" title={t("tracker.list.children", { done: i.childrenClosed, all: i.children })}>
            {i.childrenClosed}/{i.children}
          </span>
        )}
        {i.comments > 0 && (
          <span className="flex shrink-0 items-center gap-0.5 text-[12px] text-muted tabular-nums max-md:hidden">
            <HugeiconsIcon icon={Comment01Icon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {i.comments}
          </span>
        )}
        {i.repo !== null && i.repo.number !== i.number && (
          <span className="flex shrink-0 items-center gap-0.5 text-[12px] text-muted max-lg:hidden">
            <HugeiconsIcon icon={GitPullRequestIcon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />#{i.repo.number}
          </span>
        )}
        {i.due !== null && <DueDate due={i.due} today={today} open={!closed(i.state.category)} />}
        <Assignee login={i.assignee} />
      </button>
    </div>
  );
}
