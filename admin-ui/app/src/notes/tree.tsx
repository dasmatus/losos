import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowDown01Icon, ArrowRight01Icon, DashboardSquare01Icon, Folder01Icon, NoteIcon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";
import { baseName, isNote } from "./links";
import { displayName, type Vault, type VaultNote } from "./vault";

/* A vault's notes as folders, the way Obsidian's file explorer lists them:
 * folders first, then notes and canvases, by name. Only the folders on the
 * way to the open note start unfolded. Shared by Notes in LosOS Cloud and
 * a knowledge base's pages. */

interface Folder {
  name: string;
  path: string;
  folders: Folder[];
  notes: VaultNote[];
}

function build(vault: Vault): Folder {
  const root: Folder = { name: "", path: "", folders: [], notes: [] };
  const byPath = new Map<string, Folder>([["", root]]);
  const folder = (path: string): Folder => {
    const have = byPath.get(path);
    if (have !== undefined) return have;
    const cut = path.lastIndexOf("/");
    const parent = folder(cut < 0 ? "" : path.slice(0, cut));
    const made: Folder = { name: path.slice(cut + 1), path, folders: [], notes: [] };
    parent.folders.push(made);
    byPath.set(path, made);
    return made;
  };
  for (const f of vault.index.folders) folder(f);
  for (const n of [...vault.index.notes, ...vault.index.canvases]) {
    const cut = n.path.lastIndexOf("/");
    folder(cut < 0 ? "" : n.path.slice(0, cut)).notes.push(n);
  }
  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
  const sort = (f: Folder) => {
    f.folders.sort((a, b) => collator.compare(a.name, b.name));
    f.notes.sort((a, b) => collator.compare(baseName(a.path), baseName(b.path)));
    f.folders.forEach(sort);
  };
  sort(root);
  return root;
}

export interface VaultTreeProps {
  vault: Vault;
  current: string | null;
  /** A note's address, for the link's href (opening in a new tab). */
  href: (path: string) => string;
  onOpen: (path: string) => void;
  label: string;
}

export function VaultTree({ vault, current, href, onOpen, label }: VaultTreeProps) {
  const tree = React.useMemo(() => build(vault), [vault]);
  const [open, setOpen] = React.useState<Set<string>>(() => new Set());

  // Unfold the way to the open note whenever it changes.
  React.useEffect(() => {
    if (current === null) return;
    const parts = current.split("/").slice(0, -1);
    if (parts.length === 0) return;
    setOpen((was) => {
      const next = new Set(was);
      for (let i = 1; i <= parts.length; i++) next.add(parts.slice(0, i).join("/"));
      return next.size === was.size ? was : next;
    });
  }, [current]);

  const toggle = (path: string) =>
    setOpen((was) => {
      const next = new Set(was);
      if (!next.delete(path)) next.add(path);
      return next;
    });

  const list = (folder: Folder, depth: number): React.ReactNode => (
    <ul className={cn("flex flex-col", depth > 0 && "ml-2.5 border-l border-hair pl-1.5")}>
      {folder.folders.map((f) => {
        const unfolded = open.has(f.path);
        return (
          <li key={`d:${f.path}`}>
            <button
              type="button"
              aria-expanded={unfolded}
              onClick={() => toggle(f.path)}
              className="flex w-full items-center gap-1.5 rounded-control px-1.5 py-1 text-left text-[13px] text-muted hover:bg-sunk hover:text-ink focus-visible:outline-2 focus-visible:outline-accent"
              data-tree-folder={f.path}
            >
              <HugeiconsIcon icon={unfolded ? ArrowDown01Icon : ArrowRight01Icon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              <HugeiconsIcon icon={Folder01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              <span className="min-w-0 truncate">{f.name}</span>
            </button>
            {unfolded && list(f, depth + 1)}
          </li>
        );
      })}
      {folder.notes.map((n) => {
        const active = n.path === current;
        return (
          <li key={`n:${n.path}`}>
            <a
              href={href(n.path)}
              onClick={(event) => {
                if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey) return;
                event.preventDefault();
                onOpen(n.path);
              }}
              aria-current={active ? "page" : undefined}
              className={cn(
                "flex items-center gap-1.5 rounded-control px-1.5 py-1 text-[13px] focus-visible:outline-2 focus-visible:outline-accent",
                active ? "bg-accent-wash font-medium text-accent" : "text-ink hover:bg-sunk",
              )}
              data-tree-note={n.path}
            >
              <HugeiconsIcon
                icon={isNote(n.path) ? NoteIcon : DashboardSquare01Icon}
                size={14}
                strokeWidth={1.5}
                color="currentColor"
                className="shrink-0 text-faint"
                aria-hidden="true"
              />
              <span className="min-w-0 truncate">{displayName(n)}</span>
            </a>
          </li>
        );
      })}
    </ul>
  );

  return (
    <nav aria-label={label} data-vault-tree>
      {list(tree, 0)}
    </nav>
  );
}
