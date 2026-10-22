import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { failed } from "../parts";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { CATEGORY_ORDER, closed, tracker, type Category, type Issue, type Priority, type Project, type WorkflowState } from "./api";
import { Assignee, CATEGORY_NAME, DueDate, LabelChip, PRIORITY_NAME, PriorityIcon, StateIcon, orderedStates } from "./parts";
import { useTracker } from "./store";

/* The board: a column per state of the project's workflow (per kind of
 * state when the list spans projects), cards in the project's own order.
 * A card is dragged to another place or column; it moves at once and the
 * box is told after. The keyboard has the same through s (state) on the
 * open issue. */

interface Column {
  id: string;
  title: string;
  category: Category;
  /** The state a card dropped here takes; across projects, its kind. */
  state?: WorkflowState;
  issues: Issue[];
}

/** How many cards a column draws before it says how many more. */
const SHOWN = 60;

export function Board({
  issues,
  project,
  focus,
  open,
  onOpen,
  onMoved,
  onChanged,
  onCreate,
}: {
  issues: readonly Issue[];
  project: Project | undefined;
  focus: string | null;
  open: string | null;
  onOpen: (key: string) => void;
  onMoved: (next: Issue[]) => void;
  onChanged: () => void;
  onCreate: (state: number) => void;
}) {
  const t = useT();
  const { home } = useTracker();
  const [dragging, setDragging] = React.useState<string | null>(null);
  const [over, setOver] = React.useState<{ column: string; before: string | null } | null>(null);

  const columns: Column[] = React.useMemo(() => {
    const sorted = [...issues].sort((a, b) => a.sort - b.sort || b.id - a.id);
    if (project !== undefined) {
      return orderedStates(project.states).map((s) => ({
        id: String(s.id),
        title: s.name,
        category: s.category,
        state: s,
        issues: sorted.filter((i) => i.state.id === s.id),
      }));
    }
    return CATEGORY_ORDER.map((c) => ({
      id: c,
      title: t(CATEGORY_NAME[c]),
      category: c,
      issues: sorted.filter((i) => i.state.category === c),
    }));
  }, [issues, project, t]);

  const drop = async (column: Column, before: string | null) => {
    const key = dragging;
    setDragging(null);
    setOver(null);
    if (key === null) return;
    const moved = issues.find((i) => i.key === key);
    if (moved === undefined) return;
    const rest = column.issues.filter((i) => i.key !== key);
    const at = before === null ? rest.length : Math.max(0, rest.findIndex((i) => i.key === before));
    const prev = rest[at - 1];
    const next = rest[at];
    // Between its new neighbours; at either end, a step past the last one.
    const sort =
      prev !== undefined && next !== undefined
        ? (prev.sort + next.sort) / 2
        : prev !== undefined
          ? prev.sort + 1024
          : next !== undefined
            ? next.sort - 1024
            : moved.sort;
    const sameColumn = column.state !== undefined ? moved.state.id === column.state.id : moved.state.category === column.category;
    if (sameColumn && Math.abs(sort - moved.sort) < 1e-9) return;
    const target = column.state ?? home?.projects.find((p) => p.id === moved.projectId)?.states.find((s) => s.category === column.category);
    onMoved(issues.map((i) => (i.key === key ? { ...i, sort, state: target ?? i.state } : i)));
    try {
      await tracker.edit(key, sameColumn ? { sort } : column.state !== undefined ? { state: column.state.id, sort } : { category: column.category, sort });
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
    onChanged();
  };

  return (
    <div className="flex min-h-0 flex-1 gap-2 overflow-x-auto bg-sunk/60 p-2" data-board>
      {columns.map((col) => (
        <section
          key={col.id}
          aria-label={col.title}
          data-column={col.title}
          className={cn(
            "flex w-72 shrink-0 flex-col rounded-card border border-transparent transition-colors duration-150",
            over?.column === col.id && "border-accent/40 bg-accent-wash/40",
            closed(col.category) && "w-64",
          )}
          onDragOver={(e) => {
            if (dragging === null) return;
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
            if (over?.column !== col.id) setOver({ column: col.id, before: null });
          }}
          onDragLeave={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver((o) => (o?.column === col.id ? null : o));
          }}
          onDrop={(e) => {
            e.preventDefault();
            void drop(col, over?.column === col.id ? over.before : null);
          }}
        >
          <header className="flex items-center gap-2 px-2 py-1.5">
            <StateIcon category={col.category} />
            <h3 className="min-w-0 flex-1 truncate text-[13px] font-medium">{col.title}</h3>
            <span className="text-[12px] text-muted tabular-nums">{col.issues.length}</span>
            {col.state !== undefined && (
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => col.state !== undefined && onCreate(col.state.id)}
                aria-label={t("tracker.newIn", { state: col.title })}
                title={t("tracker.newIn", { state: col.title })}
              >
                <HugeiconsIcon icon={Add01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </Button>
            )}
          </header>
          <ol className="flex min-h-16 flex-1 flex-col gap-1.5 overflow-y-auto px-1 pb-2">
            {col.issues.slice(0, SHOWN).map((i) => (
              <li
                key={i.key}
                onDragOver={(e) => {
                  if (dragging === null || dragging === i.key) return;
                  e.preventDefault();
                  const box = e.currentTarget.getBoundingClientRect();
                  const below = e.clientY > box.top + box.height / 2;
                  const idx = col.issues.findIndex((x) => x.key === i.key);
                  const before = below ? (col.issues[idx + 1]?.key ?? null) : i.key;
                  if (over?.column !== col.id || over.before !== before) setOver({ column: col.id, before });
                }}
              >
                {over?.column === col.id && over.before === i.key && dragging !== i.key && <DropMark />}
                <Card
                  issue={i}
                  focused={focus === i.key}
                  current={open === i.key}
                  dragging={dragging === i.key}
                  onOpen={() => onOpen(i.key)}
                  onDragStart={() => setDragging(i.key)}
                  onDragEnd={() => {
                    setDragging(null);
                    setOver(null);
                  }}
                />
              </li>
            ))}
            {over?.column === col.id && over.before === null && dragging !== null && <DropMark />}
            {col.issues.length > SHOWN && (
              <li className="px-2 py-1 text-[12px] text-muted">{t("tracker.board.more", { count: col.issues.length - SHOWN })}</li>
            )}
          </ol>
        </section>
      ))}
    </div>
  );
}

function DropMark() {
  return <div className="mx-1 my-0.5 h-0.5 animate-pop rounded-full bg-accent" aria-hidden="true" />;
}

function Card({
  issue: i,
  focused,
  current,
  dragging,
  onOpen,
  onDragStart,
  onDragEnd,
}: {
  issue: Issue;
  focused: boolean;
  current: boolean;
  dragging: boolean;
  onOpen: () => void;
  onDragStart: () => void;
  onDragEnd: () => void;
}) {
  const t = useT();
  const { home } = useTracker();
  return (
    <button
      type="button"
      draggable
      data-issue-key={i.key}
      data-card
      onClick={onOpen}
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", i.key);
        onDragStart();
      }}
      onDragEnd={onDragEnd}
      aria-current={current ? "true" : undefined}
      className={cn(
        "flex w-full flex-col gap-1.5 rounded-control border bg-surface p-2.5 text-left shadow-card",
        "transition-[opacity,scale,border-color] duration-150 active:scale-[0.98]",
        "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
        current ? "border-accent" : focused ? "border-accent/50" : "border-line hover:border-faint",
        dragging && "scale-[0.98] opacity-40",
      )}
    >
      <span className="flex items-center gap-2">
        <span className="font-mono text-[11.5px] text-muted">{i.key}</span>
        <span className="flex-1" />
        <Assignee login={i.assignee} />
      </span>
      <span className={cn("line-clamp-3 text-[13.5px] leading-snug", closed(i.state.category) ? "text-muted" : "text-ink")}>{i.title}</span>
      <span className="flex flex-wrap items-center gap-1.5">
        <PriorityIcon priority={i.priority as Priority} label={t(PRIORITY_NAME[i.priority as Priority])} />
        {i.labels.slice(0, 3).map((l) => (
          <LabelChip key={l.id} label={l} />
        ))}
        {i.children > 0 && (
          <span className="text-[12px] text-muted tabular-nums">
            {i.childrenClosed}/{i.children}
          </span>
        )}
        {i.due !== null && <DueDate due={i.due} today={home?.today ?? ""} open={!closed(i.state.category)} />}
      </span>
    </button>
  );
}
