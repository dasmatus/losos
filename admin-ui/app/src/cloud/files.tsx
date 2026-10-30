import * as React from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowRight01Icon,
  CloudUploadIcon,
  FileAddIcon,
  FolderAddIcon,
  FolderUploadIcon,
  PresentationBarChart01Icon,
  Upload01Icon,
} from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Spinner } from "@/components/ui/progress";
import { contextMenu } from "@/lib/context-menu";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { cloud, isAbort, joinPath, parentOf, type Entry } from "./api";
import { NameDialog, explain, failed } from "./dialogs";
import { EntryTable, SelectionBar, folderHref, sortEntries, type SortKey } from "./entries";
import { createPaper, type Kind } from "./office/paper/new";
import { bump, enqueue, useRevision } from "./store";

type Loaded = { state: "loading" } | { state: "error"; message: string; missing: boolean } | { state: "ready"; entries: Entry[] };

/** Walk what was dropped: files, and folders with everything in them. */
async function dropped(items: DataTransferItemList): Promise<{ file: File; relative: string }[]> {
  const out: { file: File; relative: string }[] = [];
  const walk = async (entry: FileSystemEntry, prefix: string): Promise<void> => {
    if (entry.isFile) {
      const file = await new Promise<File>((resolve, reject) => (entry as FileSystemFileEntry).file(resolve, reject));
      out.push({ file, relative: prefix + entry.name });
    } else if (entry.isDirectory) {
      const reader = (entry as FileSystemDirectoryEntry).createReader();
      // readEntries hands them over in batches until an empty one.
      for (;;) {
        const batch = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
        if (batch.length === 0) break;
        for (const child of batch) await walk(child, `${prefix}${entry.name}/`);
      }
    }
  };
  const roots = [...items].map((i) => i.webkitGetAsEntry()).filter((e): e is FileSystemEntry => e !== null);
  for (const root of roots) await walk(root, "");
  return out;
}

export function Crumbs({ path }: { path: string }) {
  const t = useT();
  const parts = path === "" ? [] : path.split("/");
  return (
    <nav aria-label={t("cloud.crumbs")} className="flex min-w-0 flex-wrap items-center gap-1 text-[19px] leading-tight">
      <Link to="/files" className={cn("font-semibold tracking-tight", parts.length > 0 && "text-muted hover:text-ink")}>
        {t("cloud.nav.files")}
      </Link>
      {parts.map((part, i) => (
        <React.Fragment key={i}>
          <HugeiconsIcon icon={ArrowRight01Icon} size={14} strokeWidth={1.5} color="currentColor" className="text-faint" aria-hidden="true" />
          {i === parts.length - 1 ? (
            <span className="truncate font-semibold tracking-tight" aria-current="page">
              {part}
            </span>
          ) : (
            <Link to={folderHref(parts.slice(0, i + 1).join("/"))} className="truncate text-muted hover:text-ink">
              {part}
            </Link>
          )}
        </React.Fragment>
      ))}
    </nav>
  );
}

export function FilesView() {
  const t = useT();
  const navigate = useNavigate();
  const params = useParams();
  const [search] = useSearchParams();
  const path = (params["*"] ?? "").replace(/^\/+|\/+$/g, "");
  const revision = useRevision();
  const [loaded, setLoaded] = React.useState<Loaded>({ state: "loading" });
  const [selected, setSelected] = React.useState<Set<string>>(new Set());
  const [sort, setSort] = React.useState<{ key: SortKey; up: boolean }>({ key: "name", up: true });
  const [creating, setCreating] = React.useState(false);
  const [over, setOver] = React.useState(false);
  const [highlight, setHighlight] = React.useState<number | null>(null);
  const fileInput = React.useRef<HTMLInputElement>(null);
  const folderInput = React.useRef<HTMLInputElement>(null);

  // /cloud/files?fileid=N, the address the apps and the palette link: the
  // file's folder, with the file marked.
  const fileId = search.get("fileid");
  React.useEffect(() => {
    if (fileId === null) return;
    const id = Number(fileId);
    const ctl = new AbortController();
    cloud.byId(id, ctl.signal).then(
      (entry) => {
        setHighlight(entry.id);
        navigate(entry.dir ? folderHref(entry.path) : folderHref(parentOf(entry.path)), { replace: true });
      },
      (err: unknown) => {
        if (!isAbort(err)) navigate("/files", { replace: true });
      },
    );
    return () => ctl.abort();
  }, [fileId, navigate]);

  React.useEffect(() => {
    setSelected(new Set());
  }, [path]);

  React.useEffect(() => {
    if (fileId !== null) return;
    const ctl = new AbortController();
    setLoaded((current) => (current.state === "ready" ? current : { state: "loading" }));
    cloud.list(path, ctl.signal).then(
      (r) => setLoaded({ state: "ready", entries: r.entries }),
      (err: unknown) => {
        if (isAbort(err)) return;
        setLoaded({ state: "error", message: explain(err), missing: (err as { code?: string }).code === "notFound" });
      },
    );
    return () => ctl.abort();
  }, [path, revision, fileId]);

  // A path change shows the spinner rather than the last folder's rows.
  React.useEffect(() => setLoaded({ state: "loading" }), [path]);

  const entries = loaded.state === "ready" ? sortEntries(loaded.entries, sort.key, sort.up) : [];
  const picked = entries.filter((e) => selected.has(e.path));

  // A new document or presentation opens in LosOS Office at once.
  const [making, setMaking] = React.useState(false);
  const newDocument = (kind: Kind) => {
    setMaking(true);
    const starter = {
      name: kind === "paper" ? t("office.md.untitledDoc") : t("office.md.untitledDeck"),
      title: kind === "paper" ? t("office.md.untitledDoc") : t("office.md.untitledDeck"),
      subtitle: t("office.md.starter.subtitle"),
      next: t("office.md.starter.next"),
      point: t("office.md.starter.point"),
    };
    createPaper(path, kind, starter).then(
      (entry) => {
        bump();
        navigate(`/office/${entry.id}`);
      },
      (err: unknown) => {
        setMaking(false);
        failed(kind === "paper" ? t("office.md.newDocFailed") : t("office.md.newDeckFailed"), err);
      },
    );
  };

  const onSort = (key: SortKey) => setSort((s) => (s.key === key ? { key, up: !s.up } : { key, up: key === "name" }));

  return (
    <div
      ref={contextMenu(() => ({
        label: path === "" ? t("cloud.nav.files") : path,
        sections: [
          [
            { id: "new-folder", label: `${t("cloud.folder.new")}…`, icon: FolderAddIcon, onSelect: () => setCreating(true) },
            { id: "upload", label: `${t("cloud.upload.files")}…`, icon: Upload01Icon, onSelect: () => fileInput.current?.click() },
            { id: "upload-folder", label: `${t("cloud.upload.folder")}…`, icon: FolderUploadIcon, onSelect: () => folderInput.current?.click() },
          ],
        ],
      }))}
      // Tall enough that the blank space under a short list is still this
      // folder's, for its right-click menu and for dropped files.
      className="relative flex min-h-[70dvh] flex-col gap-4"
      onDragOver={(event) => {
        if (![...event.dataTransfer.types].includes("Files")) return;
        event.preventDefault();
        setOver(true);
      }}
      onDragLeave={(event) => {
        if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
        setOver(false);
      }}
      onDrop={(event) => {
        event.preventDefault();
        setOver(false);
        void dropped(event.dataTransfer.items).then((files) => {
          if (files.length > 0) enqueue(path, files);
        });
      }}
    >
      <div className="flex min-h-9 flex-wrap items-center gap-3">
        <Crumbs path={path} />
        <div className="ml-auto flex items-center gap-2">
          <Button size="sm" variant="secondary" onClick={() => newDocument("paper")} disabled={making} data-new="paper">
            <HugeiconsIcon icon={FileAddIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t("office.md.newDoc")}</span>
          </Button>
          <Button size="sm" variant="secondary" onClick={() => newDocument("slides")} disabled={making} data-new="slides">
            <HugeiconsIcon icon={PresentationBarChart01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t("office.md.newDeck")}</span>
          </Button>
          <Button size="sm" variant="secondary" onClick={() => setCreating(true)}>
            <HugeiconsIcon icon={FolderAddIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t("cloud.folder.new")}</span>
          </Button>
          <Button size="sm" variant="secondary" onClick={() => folderInput.current?.click()} className="max-sm:hidden">
            <HugeiconsIcon icon={FolderUploadIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("cloud.upload.folder")}
          </Button>
          <Button size="sm" onClick={() => fileInput.current?.click()}>
            <HugeiconsIcon icon={Upload01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("cloud.upload.files")}
          </Button>
        </div>
        <input
          ref={fileInput}
          type="file"
          multiple
          hidden
          data-upload
          onChange={(event) => {
            const files = [...(event.target.files ?? [])].map((file) => ({ file }));
            if (files.length > 0) enqueue(path, files);
            event.target.value = "";
          }}
        />
        <input
          ref={folderInput}
          type="file"
          multiple
          hidden
          // @ts-expect-error: not in React's types, in every browser.
          webkitdirectory=""
          onChange={(event) => {
            const files = [...(event.target.files ?? [])].map((file) => ({ file, relative: file.webkitRelativePath || file.name }));
            if (files.length > 0) enqueue(path, files);
            event.target.value = "";
          }}
        />
      </div>

      {picked.length > 0 && <SelectionBar entries={picked} folder={path} onClear={() => setSelected(new Set())} />}

      {loaded.state === "loading" ? (
        <div className="grid min-h-60 place-items-center">
          <Spinner />
        </div>
      ) : loaded.state === "error" ? (
        <Alert variant="crit">
          <AlertDescription>
            {loaded.missing ? t("cloud.folder.missing") : loaded.message}{" "}
            <Link to="/files" className="underline">
              {t("cloud.folder.home")}
            </Link>
          </AlertDescription>
        </Alert>
      ) : entries.length === 0 ? (
        <Empty className="min-h-72">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={CloudUploadIcon} size={22} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("cloud.folder.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("cloud.folder.emptyBody")}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button size="sm" onClick={() => fileInput.current?.click()}>
              {t("cloud.upload.files")}
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <div className="rounded-card border border-line bg-surface">
          <EntryTable
            entries={entries}
            selected={selected}
            onSelectedChange={setSelected}
            sort={sort}
            onSort={onSort}
            highlight={highlight}
          />
        </div>
      )}

      {over && (
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 z-10 grid place-items-center rounded-card border-2 border-dashed border-accent bg-accent-wash/80"
        >
          <p className="flex items-center gap-2 text-[15px] font-medium text-accent">
            <HugeiconsIcon icon={CloudUploadIcon} size={20} strokeWidth={1.5} color="currentColor" />
            {t("cloud.upload.drop", { folder: path === "" ? t("cloud.nav.files") : path })}
          </p>
        </div>
      )}

      <NameDialog
        open={creating}
        onOpenChange={setCreating}
        title={t("cloud.folder.newTitle")}
        submit={t("cloud.folder.create")}
        initial=""
        onSubmit={async (name) => {
          await cloud.mkdir(joinPath(path, name));
          bump();
        }}
      />
    </div>
  );
}
