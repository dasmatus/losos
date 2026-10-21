import * as React from "react";
import { useNavigate } from "react-router-dom";
import {
  Copy01Icon,
  Delete02Icon,
  Download01Icon,
  FolderOpenIcon,
  FolderTransferIcon,
  HistoryIcon,
  Link01Icon,
  LinkSquare02Icon,
  PencilEdit02Icon,
  Share08Icon,
  StarIcon,
  StarOffIcon,
} from "@hugeicons/core-free-icons";
import { toast } from "@/components/ui/toast";
import {
  copyText,
  menuItemsFor,
  onMac,
  takesText,
  type MenuBuilder,
  type MenuSection,
} from "@/lib/context-menu";
import { useT } from "@/lib/i18n-react";
import { cloud, contentUrl, joinPath, parentOf, type Entry } from "./api";
import { MoveDialog, NameDialog, VersionsDialog, failed } from "./dialogs";
import { entryHref } from "./entries";
import { MeshShareDialog } from "./mesh/share-dialog";
import { folderActionsFor, openersFor, type FolderAction } from "./openers";
import { bump } from "./store";

/* The right-click menu of a row in LosOS Cloud's lists (components/
 * context-menu.tsx draws it). A right click on a row that is not picked
 * picks it alone, as a file manager does, and the menu then acts on what
 * is picked: one file or folder gets everything, several get what works on
 * many. The apps that open files (openers.ts), the folder actions (Notes,
 * knowledge bases) and other parts' items ("cloud.entry") come in between.
 *
 * F2 renames the one picked row and Delete moves the picked rows to the
 * trash (⌘⌫ on a Mac), here as in the menu, while no dialog is open and
 * the focus is not in a field. */

declare module "@/lib/context-menu" {
  interface MenuSubjects {
    /** The picked rows of LosOS Cloud's lists. */
    "cloud.entry": readonly Entry[];
  }
}

const RENAME_KEYS = ["F2"] as const;
const deleteKeys = (): readonly string[] => (onMac() ? ["Mod", "Delete"] : ["Delete"]);

export function useEntryMenu({
  entries,
  selected,
  onSelectedChange,
}: {
  entries: readonly Entry[];
  selected: ReadonlySet<string>;
  onSelectedChange: (next: Set<string>) => void;
}): { row: (entry: Entry) => MenuBuilder; dialogs: React.ReactNode } {
  const t = useT();
  const navigate = useNavigate();
  const [renaming, setRenaming] = React.useState<Entry | null>(null);
  const [moving, setMoving] = React.useState<{ mode: "move" | "copy"; entries: readonly Entry[] } | null>(null);
  const [versions, setVersions] = React.useState<Entry | null>(null);
  const [sharing, setSharing] = React.useState<Entry | null>(null);
  const [folderDialog, setFolderDialog] = React.useState<{ action: FolderAction; folder: Entry } | null>(null);

  const remove = React.useCallback(
    async (targets: readonly Entry[]) => {
      const paths = targets.map((e) => e.path);
      try {
        await cloud.remove(paths);
        toast.done(t("cloud.delete.done", { count: paths.length }), t("cloud.delete.where"));
        onSelectedChange(new Set());
      } catch (err) {
        failed(t("cloud.delete.failed"), err);
      }
      bump();
    },
    [t, onSelectedChange],
  );

  const picked = React.useMemo(() => entries.filter((e) => selected.has(e.path)), [entries, selected]);

  // The keys the menu shows beside Rename and Delete.
  React.useEffect(() => {
    if (picked.length === 0) return;
    const onKey = (event: KeyboardEvent): void => {
      if (event.defaultPrevented || event.altKey || document.querySelector("dialog[open]") !== null) return;
      if (event.target instanceof Element && takesText(event.target)) return;
      const one = picked.length === 1 ? picked[0] : undefined;
      if (event.key === "F2" && one !== undefined && !event.ctrlKey && !event.metaKey) {
        event.preventDefault();
        setRenaming(one);
      } else if (
        (event.key === "Delete" && !event.ctrlKey && !event.metaKey) ||
        (onMac() && event.key === "Backspace" && event.metaKey)
      ) {
        event.preventDefault();
        void remove(picked);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [picked, remove]);

  const row = (entry: Entry): MenuBuilder => () => {
    // A row outside the picked ones becomes the only one picked.
    const targets = selected.has(entry.path) ? picked : [entry];
    if (!selected.has(entry.path)) onSelectedChange(new Set([entry.path]));
    const one = targets.length === 1 ? targets[0] : undefined;
    const file = one !== undefined && !one.dir ? one : undefined;
    const folder = one !== undefined && one.dir ? one : undefined;
    const href = one === undefined ? "" : entryHref(one);

    const open: MenuSection =
      one === undefined
        ? []
        : [
            {
              id: "open",
              label: t("menu.open"),
              icon: one.dir ? FolderOpenIcon : LinkSquare02Icon,
              onSelect: () => (one.dir ? navigate(href) : window.open(href, "_blank", "noopener")),
            },
            ...(file === undefined ? [] : openersFor(file).slice(1)).map((o) => ({
              id: `opener-${o.id}`,
              label: t(o.label),
              icon: o.icon,
              onSelect: () => window.open(o.href(file as Entry), "_blank", "noopener"),
            })),
            {
              id: "new-tab",
              label: t("menu.newTab"),
              icon: LinkSquare02Icon,
              hidden: !one.dir,
              onSelect: () => window.open(`/cloud${href}`, "_blank", "noopener"),
            },
            ...(folder === undefined ? [] : folderActionsFor(folder)).map((a) => ({
              id: `folder-${a.id}`,
              label: t(a.label(folder as Entry)),
              icon: a.icon,
              onSelect: () => {
                if (a.href !== undefined) navigate(a.href(folder as Entry));
                else setFolderDialog({ action: a, folder: folder as Entry });
              },
            })),
            {
              id: "download",
              label: t("cloud.download"),
              icon: Download01Icon,
              hidden: file === undefined,
              onSelect: () => {
                if (file !== undefined) window.location.href = contentUrl(file.path, true);
              },
            },
          ];

    const change: MenuSection = [
      {
        id: "rename",
        label: `${t("cloud.rename.action")}…`,
        icon: PencilEdit02Icon,
        keys: RENAME_KEYS,
        hidden: one === undefined,
        onSelect: () => setRenaming(one ?? null),
      },
      { id: "move", label: `${t("cloud.move.action")}…`, icon: FolderTransferIcon, onSelect: () => setMoving({ mode: "move", entries: targets }) },
      { id: "copy", label: `${t("cloud.copy.action")}…`, icon: Copy01Icon, onSelect: () => setMoving({ mode: "copy", entries: targets }) },
      {
        id: "versions",
        label: `${t("cloud.versions.action")}…`,
        icon: HistoryIcon,
        hidden: file === undefined,
        onSelect: () => setVersions(file ?? null),
      },
    ];

    const keep: MenuSection =
      one === undefined
        ? []
        : [
            {
              id: "favorite",
              label: t(one.favorite ? "cloud.favorite.remove" : "cloud.favorite.add"),
              icon: one.favorite ? StarOffIcon : StarIcon,
              onSelect: () =>
                void cloud.favorite(one.path, !one.favorite).then(bump, (err: unknown) => failed(t("cloud.favorite.add"), err)),
            },
            {
              id: "share",
              label: `${t("collab.share")}…`,
              icon: Share08Icon,
              onSelect: () => setSharing(one),
            },
            {
              id: "copy-link",
              label: t("menu.copyLink"),
              icon: Link01Icon,
              onSelect: () =>
                void copyText(`${window.location.origin}/cloud/files?fileid=${one.id}`).then((ok) =>
                  ok ? toast.done(t("menu.copied")) : toast.error(t("menu.copyFailed"), undefined, { help: "index" }),
                ),
            },
          ];

    return {
      label: one?.name ?? t("cloud.selection.count", { count: targets.length }),
      sections: [
        open,
        change,
        keep,
        menuItemsFor("cloud.entry", targets),
        [
          {
            id: "delete",
            label: t("cloud.delete.action"),
            icon: Delete02Icon,
            keys: deleteKeys(),
            danger: true,
            onSelect: () => void remove(targets),
          },
        ],
      ],
    };
  };

  const dialogs = (
    <>
      {renaming !== null && (
        <NameDialog
          open
          onOpenChange={(open) => !open && setRenaming(null)}
          title={t("cloud.rename.title", { name: renaming.name })}
          submit={t("cloud.rename.submit")}
          initial={renaming.name}
          onSubmit={async (name) => {
            await cloud.move(renaming.path, joinPath(parentOf(renaming.path), name));
            onSelectedChange(new Set());
            bump();
          }}
        />
      )}
      <MoveDialog
        mode={moving?.mode ?? null}
        entries={moving?.entries ?? []}
        start={moving?.entries[0] !== undefined ? parentOf(moving.entries[0].path) : ""}
        onOpenChange={(open) => !open && setMoving(null)}
      />
      <VersionsDialog entry={versions} onOpenChange={(open) => !open && setVersions(null)} />
      {sharing !== null && (
        <MeshShareDialog open onOpenChange={(open) => !open && setSharing(null)} fileId={sharing.id} name={sharing.name} />
      )}
      {folderDialog?.action.Dialog !== undefined && (
        <folderDialog.action.Dialog folder={folderDialog.folder} open onOpenChange={(open) => !open && setFolderDialog(null)} />
      )}
    </>
  );

  return { row, dialogs };
}
