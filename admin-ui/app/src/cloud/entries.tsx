import * as React from "react";
import { Link, useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Cancel01Icon,
  Copy01Icon,
  Delete02Icon,
  Download01Icon,
  FolderTransferIcon,
  HistoryIcon,
  PencilEdit02Icon,
  Share08Icon,
  StarIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { toast } from "@/components/ui/toast";
import { contextMenu } from "@/lib/context-menu";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { cloud, contentUrl, hasPreview, joinPath, parentOf, previewUrl, type Entry } from "./api";
import { MoveDialog, NameDialog, VersionsDialog, failed } from "./dialogs";
import { useEntryMenu } from "./entry-menu";
import { formatBytes, formatFull, formatWhen, iconFor } from "./format";
import { MeshShareDialog } from "./mesh/share-dialog";
import { folderActionsFor, openersFor, type FolderAction } from "./openers";
import { bump } from "./store";

/** The address of a folder in the web app. */
export function folderHref(path: string): string {
  return path === "" ? "/files" : `/files/${path.split("/").map(encodeURIComponent).join("/")}`;
}

/** Where a row's name goes: a folder opens here, a file in its app or tab. */
export function entryHref(entry: Entry): string {
  if (entry.dir) return folderHref(entry.path);
  const opener = openersFor(entry)[0];
  return opener !== undefined ? opener.href(entry) : contentUrl(entry.path);
}

export type SortKey = "name" | "size" | "mtime";

export function sortEntries(entries: readonly Entry[], key: SortKey, up: boolean): Entry[] {
  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
  const dir = up ? 1 : -1;
  return [...entries].sort((a, b) => {
    if (a.dir !== b.dir) return a.dir ? -1 : 1;
    if (key === "size" && a.size !== b.size) return (a.size - b.size) * dir;
    if (key === "mtime" && a.mtime !== b.mtime) return (a.mtime - b.mtime) * dir;
    return collator.compare(a.name, b.name) * (key === "name" ? dir : 1);
  });
}

export function Thumb({ entry, size = 32 }: { entry: Entry; size?: number }) {
  const [broken, setBroken] = React.useState(false);
  if (hasPreview(entry) && !broken) {
    return (
      <img
        src={previewUrl(entry, 64)}
        alt=""
        loading="lazy"
        onError={() => setBroken(true)}
        className={cn("shrink-0 rounded-[6px] border border-hair object-cover", size === 32 ? "size-8" : "size-10")}
      />
    );
  }
  return (
    <span className={cn("grid shrink-0 place-items-center", size === 32 ? "size-8" : "size-10")}>
      <HugeiconsIcon
        icon={iconFor(entry)}
        size={22}
        strokeWidth={1.5}
        color="currentColor"
        className={entry.dir ? "text-accent" : "text-muted"}
        aria-hidden="true"
      />
    </span>
  );
}

function FavoriteButton({ entry }: { entry: Entry }) {
  const t = useT();
  const label = t(entry.favorite ? "cloud.favorite.remove" : "cloud.favorite.add");
  return (
    <button
      type="button"
      aria-pressed={entry.favorite}
      aria-label={label}
      title={label}
      onClick={(event) => {
        event.stopPropagation();
        cloud.favorite(entry.path, !entry.favorite).then(bump, (err: unknown) => failed(label, err));
      }}
      className={cn(
        "grid size-7 place-items-center rounded-control transition-colors hover:bg-sunk",
        entry.favorite ? "text-warn" : "text-faint opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100",
      )}
    >
      <HugeiconsIcon
        icon={StarIcon}
        size={16}
        strokeWidth={1.5}
        color="currentColor"
        fill={entry.favorite ? "currentColor" : "none"}
        aria-hidden="true"
      />
    </button>
  );
}

/**
 * The list of files: one row each, a checkbox to pick it, its picture,
 * name, star, size and date. `showFolder` adds the folder a row is in, for
 * lists that cut across folders (Recent, Favorites, a search).
 */
export function EntryTable({
  entries,
  selected,
  onSelectedChange,
  showFolder = false,
  sort,
  onSort,
  highlight,
}: {
  entries: readonly Entry[];
  selected: ReadonlySet<string>;
  onSelectedChange: (next: Set<string>) => void;
  showFolder?: boolean;
  sort?: { key: SortKey; up: boolean };
  onSort?: (key: SortKey) => void;
  highlight?: number | null;
}) {
  const t = useT();
  const navigate = useNavigate();
  const all = entries.length > 0 && entries.every((e) => selected.has(e.path));
  const last = React.useRef<number | null>(null);
  const menu = useEntryMenu({ entries, selected, onSelectedChange });

  const pick = (index: number, entry: Entry, shift: boolean) => {
    const next = new Set(selected);
    if (shift && last.current !== null) {
      const [a, b] = [Math.min(last.current, index), Math.max(last.current, index)];
      for (const e of entries.slice(a, b + 1)) next.add(e.path);
    } else if (next.has(entry.path)) next.delete(entry.path);
    else next.add(entry.path);
    last.current = index;
    onSelectedChange(next);
  };

  const head = (key: SortKey, label: string, className?: string) => (
    <TableHead className={className} aria-sort={sort?.key === key ? (sort.up ? "ascending" : "descending") : undefined}>
      {onSort !== undefined ? (
        <button type="button" className="inline-flex items-center gap-1 hover:text-ink" onClick={() => onSort(key)}>
          {label}
          {sort?.key === key && <span aria-hidden="true">{sort.up ? "↑" : "↓"}</span>}
        </button>
      ) : (
        label
      )}
    </TableHead>
  );

  return (
    <>
      <Table data-files>
        <TableHeader>
          <TableRow className="hover:bg-transparent">
            <TableHead className="w-10">
              <input
                type="checkbox"
                role="checkbox"
                aria-label={t("cloud.selectAll")}
                checked={all}
                onChange={() => onSelectedChange(all ? new Set() : new Set(entries.map((e) => e.path)))}
                className="size-4 accent-accent"
              />
            </TableHead>
            {head("name", t("cloud.col.name"))}
            {showFolder && <TableHead className="max-md:hidden">{t("cloud.col.folder")}</TableHead>}
            {head("size", t("cloud.col.size"), "w-28 text-right max-sm:hidden")}
            {head("mtime", t("cloud.col.modified"), "w-40 max-sm:hidden")}
          </TableRow>
        </TableHeader>
        <TableBody>
          {entries.map((entry, index) => {
            const isSel = selected.has(entry.path);
            const href = entryHref(entry);
            const inApp = entry.dir;
            return (
              <TableRow
                key={entry.id}
                ref={contextMenu(menu.row(entry))}
                data-state={isSel ? "selected" : undefined}
                data-entry={entry.name}
                data-entry-id={entry.id}
                className={cn("group/row", highlight === entry.id && "bg-accent-wash")}
                onClick={(event) => {
                  // A click on the row's empty space picks it, as in a file manager.
                  if ((event.target as HTMLElement).closest("a,button,input") === null) pick(index, entry, event.shiftKey);
                }}
                onDoubleClick={(event) => {
                  if ((event.target as HTMLElement).closest("a,button,input") !== null) return;
                  if (inApp) navigate(href);
                  else window.open(href, "_blank", "noopener");
                }}
              >
                <TableCell>
                  <input
                    type="checkbox"
                    role="checkbox"
                    aria-label={t("cloud.select", { name: entry.name })}
                    checked={isSel}
                    onChange={(event) => pick(index, entry, (event.nativeEvent as MouseEvent).shiftKey)}
                    className="size-4 accent-accent"
                  />
                </TableCell>
                <TableCell className="max-w-0 w-full">
                  <div className="flex min-w-0 items-center gap-3">
                    <Thumb entry={entry} />
                    {inApp ? (
                      <Link to={href} className="min-w-0 truncate font-medium hover:underline">
                        {entry.name}
                      </Link>
                    ) : (
                      <a href={href} target="_blank" rel="noopener" className="min-w-0 truncate font-medium hover:underline">
                        {entry.name}
                      </a>
                    )}
                    <FavoriteButton entry={entry} />
                  </div>
                </TableCell>
                {showFolder && (
                  <TableCell className="max-md:hidden text-muted">
                    <Link to={folderHref(parentOf(entry.path))} className="hover:text-ink hover:underline">
                      {parentOf(entry.path) === "" ? t("cloud.nav.files") : parentOf(entry.path)}
                    </Link>
                  </TableCell>
                )}
                <TableCell className="numeric text-right text-muted max-sm:hidden">
                  {entry.dir ? "" : formatBytes(entry.size)}
                </TableCell>
                <TableCell className="text-muted max-sm:hidden" title={formatFull(entry.mtime)}>
                  {formatWhen(entry.mtime)}
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
      {menu.dialogs}
    </>
  );
}

/**
 * What can be done with the picked rows: shown above the list in place of
 * the folder's own buttons while anything is picked.
 */
export function SelectionBar({
  entries,
  folder,
  onClear,
}: {
  /** The picked rows. */
  entries: readonly Entry[];
  /** The folder on screen, where Move and Copy start. */
  folder: string;
  onClear: () => void;
}) {
  const t = useT();
  const [renaming, setRenaming] = React.useState(false);
  const [sharing, setSharing] = React.useState(false);
  const [moving, setMoving] = React.useState<"move" | "copy" | null>(null);
  const [versions, setVersions] = React.useState<Entry | null>(null);
  const one = entries.length === 1 ? entries[0] : undefined;
  const oneFile = one !== undefined && !one.dir ? one : undefined;
  const oneFolder = one !== undefined && one.dir ? one : undefined;
  const navigate = useNavigate();
  const [folderDialog, setFolderDialog] = React.useState<FolderAction | null>(null);

  const remove = async () => {
    const paths = entries.map((e) => e.path);
    try {
      await cloud.remove(paths);
      toast.done(t("cloud.delete.done", { count: paths.length }), t("cloud.delete.where"));
      onClear();
      bump();
    } catch (err) {
      failed(t("cloud.delete.failed"), err);
      bump();
    }
  };

  const tool = (label: string, icon: typeof Copy01Icon, onClick: () => void, extra?: string) => (
    <Button size="sm" variant="ghost" onClick={onClick} className={cn("text-ink", extra)}>
      <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <span className="max-sm:sr-only">{label}</span>
    </Button>
  );

  return (
    <div
      role="toolbar"
      aria-label={t("cloud.selection.label")}
      data-selection
      className="flex flex-wrap items-center gap-1 rounded-card border border-accent/30 bg-accent-wash px-2 py-1.5"
    >
      <Button size="icon-sm" variant="ghost" onClick={onClear} aria-label={t("cloud.selection.clear")} title={t("cloud.selection.clear")}>
        <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </Button>
      <span className="mr-2 text-[13px] font-medium">{t("cloud.selection.count", { count: entries.length })}</span>
      {oneFile !== undefined &&
        openersFor(oneFile).map((o) => (
          <Button key={o.id} size="sm" variant="ghost" className="text-ink" onClick={() => window.open(o.href(oneFile), "_blank", "noopener")}>
            <HugeiconsIcon icon={o.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t(o.label)}</span>
          </Button>
        ))}
      {oneFolder !== undefined &&
        folderActionsFor(oneFolder).map((a) => (
          <Button
            key={a.id}
            size="sm"
            variant="ghost"
            className="text-ink"
            data-action={a.id}
            onClick={() => {
              if (a.href !== undefined) navigate(a.href(oneFolder));
              else setFolderDialog(a);
            }}
          >
            <HugeiconsIcon icon={a.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t(a.label(oneFolder))}</span>
          </Button>
        ))}
      {oneFolder !== undefined && folderDialog?.Dialog !== undefined && (
        <folderDialog.Dialog folder={oneFolder} open onOpenChange={(open) => !open && setFolderDialog(null)} />
      )}
      {oneFile !== undefined &&
        tool(t("cloud.download"), Download01Icon, () => {
          window.location.href = contentUrl(oneFile.path, true);
        })}
      {one !== undefined && tool(t("cloud.rename.action"), PencilEdit02Icon, () => setRenaming(true))}
      {tool(t("cloud.move.action"), FolderTransferIcon, () => setMoving("move"))}
      {tool(t("cloud.copy.action"), Copy01Icon, () => setMoving("copy"))}
      {oneFile !== undefined && tool(t("cloud.versions.action"), HistoryIcon, () => setVersions(oneFile))}
      {one !== undefined && tool(t("collab.share"), Share08Icon, () => setSharing(true))}
      {tool(t("cloud.delete.action"), Delete02Icon, () => void remove(), "hover:text-crit")}

      {one !== undefined && (
        <NameDialog
          open={renaming}
          onOpenChange={setRenaming}
          title={t("cloud.rename.title", { name: one.name })}
          submit={t("cloud.rename.submit")}
          initial={one.name}
          onSubmit={async (name) => {
            await cloud.move(one.path, joinPath(parentOf(one.path), name));
            onClear();
            bump();
          }}
        />
      )}
      <MoveDialog
        mode={moving}
        entries={entries}
        start={folder}
        onOpenChange={(open) => {
          if (!open) setMoving(null);
        }}
      />
      <VersionsDialog entry={versions} onOpenChange={(open) => !open && setVersions(null)} />
      {one !== undefined && <MeshShareDialog open={sharing} onOpenChange={setSharing} fileId={one.id} name={one.name} />}
    </div>
  );
}
