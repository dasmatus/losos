import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { AlertSquareIcon, Tick02Icon } from "@hugeicons/core-free-icons";
import { PersonAvatar } from "@/components/person";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { useFaces } from "../store";
import { CATEGORY_ORDER, type Category, type Cycle, type Label, type Priority, type Project, type WorkflowState } from "./api";
import { useTracker } from "./store";

/* The small pieces every tracker page draws alike: a state's ring, a
 * priority's bars, a label's dot, and the menus that change each. */

export const CATEGORY_NAME: Record<Category, MessageKey> = {
  backlog: "tracker.category.backlog",
  unstarted: "tracker.category.unstarted",
  started: "tracker.category.started",
  done: "tracker.category.done",
  canceled: "tracker.category.canceled",
};

export const PRIORITY_NAME: Record<Priority, MessageKey> = {
  0: "tracker.priority.none",
  1: "tracker.priority.urgent",
  2: "tracker.priority.high",
  3: "tracker.priority.medium",
  4: "tracker.priority.low",
};

/** The order the priority menu lists them in. */
export const PRIORITIES: Priority[] = [1, 2, 3, 4, 0];

const CATEGORY_TONE: Record<Category, string> = {
  backlog: "text-faint",
  unstarted: "text-muted",
  started: "text-warn",
  done: "text-accent",
  canceled: "text-faint",
};

/** A state as a ring: dashed for the backlog, empty to do, half for under
 *  way, filled with a tick when done, crossed when dropped. */
export function StateIcon({ category, className, label }: { category: Category; className?: string; label?: string }) {
  const common = { fill: "none", stroke: "currentColor", strokeWidth: 1.6 } as const;
  return (
    <svg
      viewBox="0 0 16 16"
      className={cn("size-3.5 shrink-0", CATEGORY_TONE[category], className)}
      role={label === undefined ? undefined : "img"}
      aria-label={label}
      aria-hidden={label === undefined ? true : undefined}
      data-category={category}
    >
      {category === "backlog" && <circle cx="8" cy="8" r="6" {...common} strokeDasharray="2.2 2.2" />}
      {category === "unstarted" && <circle cx="8" cy="8" r="6" {...common} />}
      {category === "started" && (
        <>
          <circle cx="8" cy="8" r="6" {...common} />
          <path d="M8 4.5a3.5 3.5 0 0 1 0 7z" fill="currentColor" />
        </>
      )}
      {category === "done" && (
        <>
          <circle cx="8" cy="8" r="6.8" fill="currentColor" />
          <path d="M5.2 8.2l1.9 1.9 3.7-3.9" fill="none" stroke="var(--color-surface)" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </>
      )}
      {category === "canceled" && (
        <>
          <circle cx="8" cy="8" r="6.8" fill="currentColor" />
          <path d="M5.8 5.8l4.4 4.4M10.2 5.8l-4.4 4.4" stroke="var(--color-surface)" strokeWidth="1.6" strokeLinecap="round" />
        </>
      )}
    </svg>
  );
}

/** Urgent is a filled mark; high to low are three bars with fewer lit. */
export function PriorityIcon({ priority, className, label }: { priority: Priority; className?: string; label?: string }) {
  if (priority === 1) {
    return (
      <HugeiconsIcon
        icon={AlertSquareIcon}
        size={14}
        strokeWidth={1.8}
        color="currentColor"
        className={cn("shrink-0 text-crit", className)}
        aria-label={label}
        aria-hidden={label === undefined ? true : undefined}
      />
    );
  }
  const lit = priority === 2 ? 3 : priority === 3 ? 2 : priority === 4 ? 1 : 0;
  return (
    <svg
      viewBox="0 0 16 16"
      className={cn("size-3.5 shrink-0 text-muted", className)}
      role={label === undefined ? undefined : "img"}
      aria-label={label}
      aria-hidden={label === undefined ? true : undefined}
      data-priority={priority}
    >
      {priority === 0 ? (
        <path d="M3 8h2M7 8h2M11 8h2" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      ) : (
        [0, 1, 2].map((i) => (
          <rect
            key={i}
            x={2.5 + i * 4}
            y={10 - i * 3}
            width="3"
            height={4 + i * 3}
            rx="0.8"
            fill="currentColor"
            opacity={i < lit ? 1 : 0.25}
          />
        ))
      )}
    </svg>
  );
}

/** A label's colour is the person's own pick, so it is data, not a token. */
export function LabelDot({ color, className }: { color: string; className?: string }) {
  return (
    <span
      className={cn("size-2 shrink-0 rounded-full", color === "" && "bg-faint", className)}
      style={color !== "" ? { backgroundColor: color } : undefined}
      aria-hidden="true"
    />
  );
}

export function LabelChip({ label }: { label: Label }) {
  return (
    <span className="inline-flex max-w-32 items-center gap-1.5 rounded-full border border-line px-2 py-px text-[12px] text-muted" title={label.description || label.name}>
      <LabelDot color={label.color} />
      <span className="truncate">{label.name}</span>
    </span>
  );
}

export function Assignee({ login, size = "xs", className }: { login: string | null; size?: "xs" | "sm"; className?: string }) {
  const t = useT();
  const faces = useFaces();
  if (login === null) {
    return (
      <span
        className={cn("inline-flex shrink-0 items-center justify-center rounded-full border border-dashed border-line text-faint", size === "xs" ? "size-6" : "size-8", className)}
        aria-label={t("tracker.unassigned")}
        title={t("tracker.unassigned")}
      />
    );
  }
  const face = faces(login);
  return <PersonAvatar person={face} size={size} className={className} title={face.displayName || login} />;
}

/** "Due 3 Nov", red once it is past and the issue still open. */
export function DueDate({ due, today, open }: { due: string; today: string; open: boolean }) {
  const t = useT();
  const late = open && due < today;
  const soon = open && !late && due <= addDays(today, 2);
  return (
    <span
      className={cn("shrink-0 text-[12px] tabular-nums", late ? "text-crit" : soon ? "text-warn" : "text-muted")}
      title={t("tracker.due.full", { date: due })}
      data-due={late ? "late" : soon ? "soon" : "later"}
    >
      {shortDay(due)}
    </span>
  );
}

export function addDays(day: string, n: number): string {
  const d = new Date(`${day}T00:00:00`);
  d.setDate(d.getDate() + n);
  return isoDay(d);
}

export function isoDay(d: Date): string {
  const pad = (v: number) => String(v).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function shortDay(day: string): string {
  const d = new Date(`${day}T00:00:00`);
  if (Number.isNaN(d.getTime())) return day;
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return new Intl.DateTimeFormat(document.documentElement.lang || undefined, {
    day: "numeric",
    month: "short",
    ...(sameYear ? {} : { year: "numeric" }),
  }).format(d);
}

/** The workflow's states in the order the board shows them. */
export function orderedStates(states: readonly WorkflowState[]): WorkflowState[] {
  return [...states].sort(
    (a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category) || a.position - b.position,
  );
}

// ── Menus ─────────────────────────────────────────────────────────────────

/** A menu a shortcut may open as well as a click: `open` and `onOpenChange`
 *  are the screen's when given. */
interface MenuProps {
  trigger: React.ReactElement;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

function Check({ on }: { on: boolean }) {
  return (
    <HugeiconsIcon
      icon={Tick02Icon}
      size={14}
      strokeWidth={1.8}
      color="currentColor"
      className={cn("ml-auto shrink-0 text-accent", !on && "invisible")}
      aria-hidden="true"
    />
  );
}

export function StateMenu({
  states,
  current,
  onPick,
  ...menu
}: MenuProps & { states: readonly WorkflowState[]; current?: number; onPick: (s: WorkflowState) => void }) {
  const t = useT();
  return (
    <DropdownMenu open={menu.open} onOpenChange={menu.onOpenChange}>
      <DropdownMenuTrigger render={menu.trigger} />
      <DropdownMenuContent data-menu="state">
        <DropdownMenuLabel>{t("tracker.field.state")}</DropdownMenuLabel>
        {orderedStates(states).map((s, i) => (
          <DropdownMenuItem key={s.id} onClick={() => onPick(s)} data-state-id={s.id}>
            <StateIcon category={s.category} />
            <span className="min-w-0 flex-1 truncate">{s.name}</span>
            {i < 9 && <kbd className="text-[11px] text-faint">{i + 1}</kbd>}
            <Check on={s.id === current} />
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function PriorityMenu({ current, onPick, ...menu }: MenuProps & { current?: Priority; onPick: (p: Priority) => void }) {
  const t = useT();
  return (
    <DropdownMenu open={menu.open} onOpenChange={menu.onOpenChange}>
      <DropdownMenuTrigger render={menu.trigger} />
      <DropdownMenuContent data-menu="priority">
        <DropdownMenuLabel>{t("tracker.field.priority")}</DropdownMenuLabel>
        {PRIORITIES.map((p) => (
          <DropdownMenuItem key={p} onClick={() => onPick(p)} data-priority={p}>
            <PriorityIcon priority={p} />
            <span className="flex-1">{t(PRIORITY_NAME[p])}</span>
            <kbd className="text-[11px] text-faint">{p}</kbd>
            <Check on={p === current} />
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function AssigneeMenu({ current, onPick, ...menu }: MenuProps & { current?: string | null; onPick: (login: string | null) => void }) {
  const t = useT();
  const { home } = useTracker();
  const faces = useFaces();
  const people = home?.people ?? [];
  const me = home?.me ?? null;
  // Me first, then everyone else by name.
  const sorted = [...people].sort((a, b) => (a.login === me ? -1 : b.login === me ? 1 : (a.fullName || a.login).localeCompare(b.fullName || b.login)));
  return (
    <DropdownMenu open={menu.open} onOpenChange={menu.onOpenChange}>
      <DropdownMenuTrigger render={menu.trigger} />
      <DropdownMenuContent data-menu="assignee" className="max-h-80 overflow-y-auto">
        <DropdownMenuLabel>{t("tracker.field.assignee")}</DropdownMenuLabel>
        <DropdownMenuItem onClick={() => onPick(null)}>
          <Assignee login={null} />
          <span className="flex-1">{t("tracker.unassigned")}</span>
          <Check on={current === null} />
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {sorted.map((p) => (
          <DropdownMenuItem key={p.login} onClick={() => onPick(p.login)} data-login={p.login}>
            <PersonAvatar person={faces(p.login)} size="xs" />
            <span className="min-w-0 flex-1 truncate">
              {faces(p.login).displayName || p.fullName || p.login}
              {p.login === me && <span className="text-muted"> · {t("tracker.me")}</span>}
            </span>
            <Check on={current === p.login} />
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** Labels toggle one at a time; the menu stays open between them. A name
 *  that is not there yet is made. */
export function LabelMenu({
  labels,
  current,
  onToggle,
  ...menu
}: MenuProps & { labels: readonly Label[]; current: readonly string[]; onToggle: (name: string, on: boolean) => void }) {
  const t = useT();
  const [name, setName] = React.useState("");
  const fresh = name.trim() !== "" && !labels.some((l) => l.name.toLowerCase() === name.trim().toLowerCase());
  return (
    <DropdownMenu open={menu.open} onOpenChange={menu.onOpenChange}>
      <DropdownMenuTrigger render={menu.trigger} />
      <DropdownMenuContent data-menu="labels" className="max-h-80 overflow-y-auto">
        <div className="p-1">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter" && fresh) {
                onToggle(name.trim(), true);
                setName("");
              }
            }}
            placeholder={t("tracker.labels.find")}
            aria-label={t("tracker.labels.find")}
            className="h-8 w-full rounded-control border border-line bg-surface px-2 text-[13px] outline-none focus-visible:border-accent"
          />
        </div>
        {labels
          .filter((l) => name.trim() === "" || l.name.toLowerCase().includes(name.trim().toLowerCase()))
          .map((l) => {
            const on = current.some((c) => c.toLowerCase() === l.name.toLowerCase());
            return (
              <DropdownMenuItem key={l.id} closeOnClick={false} onClick={() => onToggle(l.name, !on)} data-label={l.name}>
                <LabelDot color={l.color} />
                <span className="min-w-0 flex-1 truncate">{l.name}</span>
                <Check on={on} />
              </DropdownMenuItem>
            );
          })}
        {fresh && (
          <DropdownMenuItem
            onClick={() => {
              onToggle(name.trim(), true);
              setName("");
            }}
          >
            {t("tracker.labels.make", { name: name.trim() })}
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function CycleMenu({
  cycles,
  current,
  today,
  onPick,
  ...menu
}: MenuProps & { cycles: readonly Cycle[]; current: number | null; today: string; onPick: (id: number | null) => void }) {
  const t = useT();
  const open = cycles.filter((c) => c.ends >= today).sort((a, b) => a.starts.localeCompare(b.starts));
  return (
    <DropdownMenu open={menu.open} onOpenChange={menu.onOpenChange}>
      <DropdownMenuTrigger render={menu.trigger} />
      <DropdownMenuContent data-menu="cycle">
        <DropdownMenuLabel>{t("tracker.field.cycle")}</DropdownMenuLabel>
        <DropdownMenuItem onClick={() => onPick(null)}>
          <span className="flex-1">{t("tracker.cycle.none")}</span>
          <Check on={current === null} />
        </DropdownMenuItem>
        {open.map((c) => (
          <DropdownMenuItem key={c.id} onClick={() => onPick(c.id)}>
            <span className="min-w-0 flex-1 truncate">{cycleName(c, t)}</span>
            <span className="text-[12px] text-muted tabular-nums">
              {shortDay(c.starts)} – {shortDay(c.ends)}
            </span>
            <Check on={current === c.id} />
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export const ESTIMATES = [1, 2, 3, 5, 8, 13, 21] as const;

export function EstimateMenu({ current, onPick, ...menu }: MenuProps & { current: number | null; onPick: (points: number | null) => void }) {
  const t = useT();
  return (
    <DropdownMenu open={menu.open} onOpenChange={menu.onOpenChange}>
      <DropdownMenuTrigger render={menu.trigger} />
      <DropdownMenuContent data-menu="estimate">
        <DropdownMenuLabel>{t("tracker.field.estimate")}</DropdownMenuLabel>
        <DropdownMenuItem onClick={() => onPick(null)}>
          <span className="flex-1">{t("tracker.estimate.none")}</span>
          <Check on={current === null} />
        </DropdownMenuItem>
        {ESTIMATES.map((n) => (
          <DropdownMenuItem key={n} onClick={() => onPick(n)}>
            <span className="flex-1 tabular-nums">{t("tracker.estimate.points", { count: n })}</span>
            <Check on={current === n} />
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function cycleName(c: Cycle, t: ReturnType<typeof useT>): string {
  return c.name !== "" ? c.name : t("tracker.cycle.numbered", { n: c.number });
}

/** The cycle that holds `today`, if any. */
export function currentCycle(project: Project, today: string): Cycle | undefined {
  return project.cycles.find((c) => c.starts <= today && today <= c.ends);
}
