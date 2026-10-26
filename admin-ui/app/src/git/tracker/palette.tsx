import * as React from "react";
import { useNavigate } from "react-router-dom";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { Add01Icon, Bookmark01Icon, Folder01Icon, InboxIcon, Search01Icon, Task01Icon, UserCircleIcon } from "@hugeicons/core-free-icons";
import { Dialog } from "@/components/ui/dialog";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { tracker, type Issue } from "./api";
import { StateIcon } from "./parts";
import { useTracker } from "./store";

/* Ctrl+K (Cmd+K): find any issue by its key or words, jump to a project or
 * a saved view, or make an issue, all without leaving the keyboard. Arrows
 * move, Enter goes, Escape closes. */

interface Item {
  id: string;
  label: string;
  hint?: string;
  icon?: IconSvgElement;
  issue?: Issue;
  run: () => void;
}

export function Palette({ open, onClose, onOpenIssue, onCreate }: { open: boolean; onClose: () => void; onOpenIssue: (key: string) => void; onCreate: () => void }) {
  const t = useT();
  const navigate = useNavigate();
  const { home } = useTracker();
  const [q, setQ] = React.useState("");
  const [found, setFound] = React.useState<Issue[]>([]);
  const [active, setActive] = React.useState(0);
  const listId = React.useId();

  React.useEffect(() => {
    if (!open) {
      setQ("");
      setFound([]);
      setActive(0);
    }
  }, [open]);

  React.useEffect(() => {
    if (q.trim() === "") {
      setFound([]);
      return;
    }
    const ctl = new AbortController();
    const timer = window.setTimeout(() => {
      tracker.issues({ q: q.trim(), sort: "updated" }, ctl.signal).then(
        (list) => setFound(list.slice(0, 8)),
        () => {},
      );
    }, 120);
    return () => {
      ctl.abort();
      window.clearTimeout(timer);
    };
  }, [q]);

  const go = (to: string) => () => {
    onClose();
    navigate(to);
  };
  const words = q.trim().toLowerCase();
  const matches = (s: string) => words === "" || s.toLowerCase().includes(words);
  const items: Item[] = [
    ...found.map((i) => ({ id: `i${i.key}`, label: i.title, hint: i.key, issue: i, run: () => onOpenIssue(i.key) })),
    ...[
      { id: "new", label: t("tracker.new"), hint: "C", icon: Add01Icon, run: onCreate },
      { id: "inbox", label: t("tracker.nav.inbox"), icon: InboxIcon, run: go("/issues/inbox") },
      { id: "mine", label: t("tracker.nav.mine"), icon: UserCircleIcon, run: go("/issues") },
      { id: "all", label: t("tracker.nav.all"), icon: Task01Icon, run: go("/issues/all") },
      ...(home?.projects ?? []).map((p) => ({ id: `p${p.key}`, label: p.name, hint: p.key, icon: Folder01Icon, run: go(`/issues/p/${p.key}`) })),
      ...(home?.views ?? []).map((v) => ({ id: `v${v.id}`, label: v.name, icon: Bookmark01Icon, run: go(`/issues/views/${v.id}`) })),
    ].filter((it) => matches(it.label) || matches(it.hint ?? "")),
  ];
  const current = Math.min(active, Math.max(0, items.length - 1));

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()} labelledBy={`${listId}-label`} dialogClassName="w-[min(36rem,calc(100vw-2rem))] self-start mt-[12vh]">
      <div className="flex flex-col" data-palette>
        <div className="flex items-center gap-2 border-b border-line px-3">
          <HugeiconsIcon icon={Search01Icon} size={16} strokeWidth={1.5} color="currentColor" className="text-muted" aria-hidden="true" />
          <span id={`${listId}-label`} className="sr-only">
            {t("tracker.palette.label")}
          </span>
          <input
            autoFocus
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setActive(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setActive((a) => Math.min(items.length - 1, a + 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActive((a) => Math.max(0, a - 1));
              } else if (e.key === "Enter") {
                e.preventDefault();
                items[current]?.run();
              }
            }}
            placeholder={t("tracker.palette.placeholder")}
            className="h-12 min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-faint"
            role="combobox"
            aria-expanded="true"
            aria-controls={listId}
            aria-activedescendant={items[current] !== undefined ? `${listId}-${items[current].id}` : undefined}
          />
        </div>
        <ul id={listId} role="listbox" className="max-h-[50vh] overflow-y-auto p-1.5">
          {items.map((it, i) => (
            <li
              key={it.id}
              id={`${listId}-${it.id}`}
              role="option"
              aria-selected={i === current}
              onMouseMove={() => setActive(i)}
              onClick={it.run}
              className={cn("flex cursor-pointer items-center gap-2.5 rounded-control px-2.5 py-2 text-[13.5px]", i === current && "bg-sunk")}
            >
              {it.issue !== undefined ? (
                <StateIcon category={it.issue.state.category} />
              ) : it.icon !== undefined ? (
                <HugeiconsIcon icon={it.icon} size={16} strokeWidth={1.5} color="currentColor" className="text-muted" aria-hidden="true" />
              ) : null}
              <span className="min-w-0 flex-1 truncate">{it.label}</span>
              {it.hint !== undefined && <span className="font-mono text-[12px] text-muted">{it.hint}</span>}
            </li>
          ))}
          {items.length === 0 && <li className="px-2.5 py-6 text-center text-[13px] text-muted">{t("tracker.palette.nothing")}</li>}
        </ul>
      </div>
    </Dialog>
  );
}
