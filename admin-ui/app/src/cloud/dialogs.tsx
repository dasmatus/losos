import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowRight01Icon, Download01Icon, Folder01Icon, HistoryIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { CloudError, cloud, isAbort, joinPath, nameOf, versionUrl, type Entry, type Version } from "./api";
import { formatBytes, formatFull } from "./format";
import { bump } from "./store";

/** The service's refusal in the reader's language. */
export function explain(err: unknown): string {
  if (err instanceof CloudError) {
    switch (err.code) {
      case "exists":
        return t("cloud.error.exists");
      case "conflict":
        return t("cloud.error.conflict");
      case "badName":
        return t("cloud.error.badName");
      case "loop":
        return t("cloud.error.loop");
      case "full":
        return t("cloud.error.full");
      case "notFound":
        return t("cloud.error.notFound");
      case "network":
        return t("cloud.error.network");
      default:
        return t("cloud.error.other", { message: err.message });
    }
  }
  return t("cloud.error.other", { message: err instanceof Error ? err.message : String(err) });
}

export function failed(title: string, err: unknown): void {
  toast.error(title, explain(err), { help: "index" });
}

// ── A name: a new folder, a rename ──────────────────────────────────────

export function NameDialog({
  open,
  onOpenChange,
  title,
  submit,
  initial,
  onSubmit,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  submit: string;
  initial: string;
  /** Resolves when done; a throw is shown under the field. */
  onSubmit: (name: string) => Promise<void>;
}) {
  const t = useT();
  const id = React.useId();
  const [name, setName] = React.useState(initial);
  const [error, setError] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    setName(initial);
    setError(null);
    setBusy(false);
    // Select the name without its extension, as a file manager does.
    requestAnimationFrame(() => {
      const el = document.getElementById(`${id}-name`) as HTMLInputElement | null;
      if (el === null) return;
      el.focus();
      const dot = initial.lastIndexOf(".");
      el.setSelectionRange(0, dot > 0 ? dot : initial.length);
    });
  }, [open, initial, id]);

  const trimmed = name.trim();
  const bad = trimmed.includes("/") || trimmed === "." || trimmed === "..";

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={`${id}-title`}>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (trimmed === "" || bad || busy) return;
          setBusy(true);
          onSubmit(trimmed).then(
            () => onOpenChange(false),
            (err: unknown) => {
              setBusy(false);
              setError(explain(err));
            },
          );
        }}
      >
        <DialogHeader>
          <DialogTitle id={`${id}-title`}>{title}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <Field invalid={error !== null || bad}>
            <FieldLabel htmlFor={`${id}-name`}>{t("cloud.name.label")}</FieldLabel>
            <Input
              id={`${id}-name`}
              value={name}
              autoComplete="off"
              spellCheck={false}
              aria-invalid={error !== null || bad}
              onChange={(event) => {
                setName(event.target.value);
                setError(null);
              }}
            />
            <FieldError>{bad ? t("cloud.error.badName") : error}</FieldError>
          </Field>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t("cloud.cancel")}
          </Button>
          <Button type="submit" disabled={trimmed === "" || bad || busy || trimmed === initial}>
            {busy && <Spinner size={14} />}
            {submit}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

// ── Move or copy: pick a folder ─────────────────────────────────────────

export function MoveDialog({
  mode,
  entries,
  start,
  onOpenChange,
}: {
  mode: "move" | "copy" | null;
  entries: readonly Entry[];
  start: string;
  onOpenChange: (open: boolean) => void;
}) {
  const t = useT();
  const id = React.useId();
  const open = mode !== null;
  const [folder, setFolder] = React.useState(start);
  const [folders, setFolders] = React.useState<Entry[] | null>(null);
  const [busy, setBusy] = React.useState(false);

  React.useEffect(() => {
    if (open) setFolder(start);
  }, [open, start]);

  React.useEffect(() => {
    if (!open) return;
    const ctl = new AbortController();
    setFolders(null);
    cloud.list(folder, ctl.signal).then(
      (r) => setFolders(r.entries.filter((e) => e.dir)),
      (err: unknown) => {
        if (!isAbort(err)) setFolders([]);
      },
    );
    return () => ctl.abort();
  }, [open, folder]);

  // A folder cannot go into itself or below itself.
  const inside = (path: string) => entries.some((e) => e.dir && (path === e.path || path.startsWith(`${e.path}/`)));
  const here = entries.every((e) => (e.path.includes("/") ? e.path.slice(0, e.path.lastIndexOf("/")) : "") === folder);
  const crumbs = folder === "" ? [] : folder.split("/");

  const run = async () => {
    if (mode === null) return;
    setBusy(true);
    let done = 0;
    for (const e of entries) {
      const to = joinPath(folder, e.name);
      try {
        if (mode === "move") await cloud.move(e.path, to);
        else await cloud.copy(e.path, to);
        done++;
      } catch (err) {
        failed(t(mode === "move" ? "cloud.move.failed" : "cloud.copy.failed", { name: e.name }), err);
      }
    }
    setBusy(false);
    bump();
    if (done > 0) {
      toast.done(
        t(mode === "move" ? "cloud.move.done" : "cloud.copy.done", {
          count: done,
          folder: folder === "" ? t("cloud.nav.files") : nameOf(folder),
        }),
      );
    }
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={`${id}-title`} dialogClassName="w-[min(32rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>
          {t(mode === "copy" ? "cloud.copy.title" : "cloud.move.title", { count: entries.length })}
        </DialogTitle>
        <nav aria-label={t("cloud.crumbs")} className="flex flex-wrap items-center gap-1 text-[13px] text-muted">
          <button type="button" className="hover:text-ink" onClick={() => setFolder("")}>
            {t("cloud.nav.files")}
          </button>
          {crumbs.map((part, i) => (
            <React.Fragment key={i}>
              <HugeiconsIcon icon={ArrowRight01Icon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              <button
                type="button"
                className={cn("hover:text-ink", i === crumbs.length - 1 && "font-medium text-ink")}
                onClick={() => setFolder(crumbs.slice(0, i + 1).join("/"))}
              >
                {part}
              </button>
            </React.Fragment>
          ))}
        </nav>
      </DialogHeader>
      <DialogBody className="max-h-80 min-h-40 overflow-y-auto">
        {folders === null ? (
          <Spinner />
        ) : folders.length === 0 ? (
          <p className="py-6 text-center text-[13px] text-muted">{t("cloud.move.noFolders")}</p>
        ) : (
          <ul className="flex flex-col">
            {folders.map((f) => {
              const blocked = inside(f.path);
              return (
                <li key={f.id}>
                  <button
                    type="button"
                    disabled={blocked}
                    onClick={() => setFolder(f.path)}
                    className="flex w-full items-center gap-2.5 rounded-control px-2 py-2 text-left text-sm hover:bg-sunk disabled:opacity-45"
                  >
                    <HugeiconsIcon icon={Folder01Icon} size={18} strokeWidth={1.5} color="currentColor" className="text-accent" aria-hidden="true" />
                    <span className="min-w-0 flex-1 truncate">{f.name}</span>
                    <HugeiconsIcon icon={ArrowRight01Icon} size={14} strokeWidth={1.5} color="currentColor" className="text-faint" aria-hidden="true" />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>
          {t("cloud.cancel")}
        </Button>
        <Button onClick={() => void run()} disabled={busy || inside(folder) || (mode === "move" && here)}>
          {busy && <Spinner size={14} />}
          {t(mode === "copy" ? "cloud.copy.here" : "cloud.move.here")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

// ── Earlier versions of a file ──────────────────────────────────────────

export function VersionsDialog({ entry, onOpenChange }: { entry: Entry | null; onOpenChange: (open: boolean) => void }) {
  const t = useT();
  const id = React.useId();
  const [versions, setVersions] = React.useState<Version[] | null>(null);
  const [busy, setBusy] = React.useState<number | null>(null);

  React.useEffect(() => {
    if (entry === null) return;
    const ctl = new AbortController();
    setVersions(null);
    cloud.versions(entry.path, ctl.signal).then(
      (r) => setVersions([...r.versions].sort((a, b) => b.mtime - a.mtime)),
      (err: unknown) => {
        if (!isAbort(err)) setVersions([]);
      },
    );
    return () => ctl.abort();
  }, [entry]);

  const restore = async (v: Version) => {
    if (entry === null) return;
    setBusy(v.mtime);
    try {
      await cloud.restoreVersion(entry.id, v.mtime);
      toast.done(t("cloud.versions.restored", { when: formatFull(v.mtime) }));
      bump();
      onOpenChange(false);
    } catch (err) {
      failed(t("cloud.versions.failed"), err);
    }
    setBusy(null);
  };

  return (
    <Dialog open={entry !== null} onOpenChange={onOpenChange} labelledBy={`${id}-title`} describedBy={`${id}-body`} dialogClassName="w-[min(32rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>{t("cloud.versions.title", { name: entry?.name ?? "" })}</DialogTitle>
        <DialogDescription id={`${id}-body`}>{t("cloud.versions.summary")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="max-h-96 overflow-y-auto">
        {entry !== null && (
          <div className="flex items-center gap-3 border-b border-hair py-2.5">
            <HugeiconsIcon icon={HistoryIcon} size={18} strokeWidth={1.5} color="currentColor" className="text-accent" aria-hidden="true" />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">{t("cloud.versions.current")}</p>
              <p className="text-[12.5px] text-muted">
                {formatFull(entry.mtime)} · {formatBytes(entry.size)}
              </p>
            </div>
          </div>
        )}
        {versions === null ? (
          <div className="py-4">
            <Spinner />
          </div>
        ) : versions.length === 0 ? (
          <p className="py-6 text-center text-[13px] text-muted">{t("cloud.versions.none")}</p>
        ) : (
          <ul>
            {versions.map((v) => (
              <li key={v.mtime} className="flex items-center gap-3 border-b border-hair py-2.5 last:border-b-0">
                <div className="min-w-0 flex-1">
                  <p className="text-sm">{formatFull(v.mtime)}</p>
                  <p className="text-[12.5px] text-muted">{formatBytes(v.size)}</p>
                </div>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t("cloud.download")}
                  title={t("cloud.download")}
                  onClick={() => window.open(versionUrl(v.file, v.mtime), "_blank", "noopener")}
                >
                  <HugeiconsIcon icon={Download01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                </Button>
                <Button size="sm" variant="secondary" disabled={busy !== null} onClick={() => void restore(v)}>
                  {busy === v.mtime && <Spinner size={14} />}
                  {t("cloud.versions.restore")}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>
          {t("cloud.close")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

// ── Are you sure ────────────────────────────────────────────────────────

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  body,
  confirm,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  body: string;
  confirm: string;
  onConfirm: () => void;
}) {
  const t = useT();
  const id = React.useId();
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={`${id}-title`} describedBy={`${id}-body`}>
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>{title}</DialogTitle>
        <DialogDescription id={`${id}-body`}>{body}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>
          {t("cloud.cancel")}
        </Button>
        <Button
          variant="destructive"
          onClick={() => {
            onOpenChange(false);
            onConfirm();
          }}
        >
          {confirm}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
