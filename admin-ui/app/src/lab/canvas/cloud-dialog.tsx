/* Opening a diagram from LosOS Cloud, and saving one there: the person's
 * folders and .canvas files, one folder at a time, and a name for a save. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, File02Icon, Folder01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { actions } from "../actions";
import { useLab } from "../store";
import { CloudFail, type CloudEntry, DIAGRAM, joinPath, listFolder, parentOf } from "./cloud";

export function CloudDialog({ mode, onClose }: { mode: "open" | "save" | null; onClose: () => void }) {
  const s = useLab();
  const t = useT();
  const titleId = React.useId();
  const nameId = React.useId();
  const [folder, setFolder] = React.useState("");
  const [entries, setEntries] = React.useState<CloudEntry[] | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [name, setName] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  /** A save that would replace a file: the file, to ask first. */
  const [replace, setReplace] = React.useState<CloudEntry | null>(null);

  // Each opening starts in the folder of the diagram it came from.
  React.useEffect(() => {
    if (!mode) return;
    const at = s.cloudFile ? parentOf(s.cloudFile.path) : "";
    setFolder(at);
    setReplace(null);
    setProblem(null);
    setName(s.core.exportSetup().filename);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per opening
  }, [mode]);

  React.useEffect(() => {
    if (!mode) return;
    let gone = false;
    setEntries(null);
    listFolder(folder).then(
      (e) => !gone && setEntries(e),
      (e: unknown) => {
        if (gone) return;
        setEntries([]);
        setProblem(e instanceof CloudFail && e.code === "signIn" ? t("dg.cloud.signIn") : t("dg.cloud.failed"));
      },
    );
    return () => {
      gone = true;
    };
  }, [mode, folder, t]);

  const open = async (e: CloudEntry) => {
    setBusy(true);
    const ok = await actions().openCloud(e.path);
    setBusy(false);
    if (ok) onClose();
  };

  const save = async (over: CloudEntry | null) => {
    let file = name.trim().replace(/[/\\]/g, "-");
    if (!file) return;
    if (!DIAGRAM.test(file)) file += ".canvas";
    const path = joinPath(folder, file);
    const there = entries?.find((e) => !e.dir && e.path === path) ?? null;
    if (there && !over && s.cloudFile?.path !== path) {
      setReplace(there);
      return;
    }
    setBusy(true);
    const r = await actions().saveCloud(path, over ? over.etag : undefined);
    setBusy(false);
    if (r === "saved") onClose();
    else if (r === "changed") setReplace(there ?? { id: 0, path, name: file, dir: false, size: 0, mtime: 0, etag: "*" });
  };

  const crumbs = folder ? folder.split("/") : [];
  return (
    <Dialog open={mode !== null} onOpenChange={(o) => !o && onClose()} labelledBy={titleId} dialogClassName="w-[min(34rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id={titleId}>{mode === "save" ? t("dg.cloud.saveTitle") : t("dg.cloud.openTitle")}</DialogTitle>
        <DialogDescription>{mode === "save" ? t("dg.cloud.saveHint") : t("dg.cloud.openHint")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <nav aria-label={t("dg.cloud.where")} className="flex flex-wrap items-center gap-1 text-[13px]">
          <Button variant="ghost" size="sm" disabled={!folder} onClick={() => setFolder(parentOf(folder))} aria-label={t("dg.cloud.up")}>
            <HugeiconsIcon icon={ArrowLeft01Icon} size={14} strokeWidth={1.8} color="currentColor" />
          </Button>
          <button type="button" className="rounded-control px-1 text-accent hover:underline" onClick={() => setFolder("")}>
            {t("dg.cloud.home")}
          </button>
          {crumbs.map((c, i) => (
            <React.Fragment key={i}>
              <span className="text-faint">/</span>
              <button type="button" className="rounded-control px-1 text-accent hover:underline" onClick={() => setFolder(crumbs.slice(0, i + 1).join("/"))}>
                {c}
              </button>
            </React.Fragment>
          ))}
        </nav>
        <ul className="max-h-[300px] min-h-[120px] overflow-y-auto rounded-card border border-line" data-testid="cloud-list">
          {entries === null ? (
            <li className="flex items-center gap-2 px-3 py-2.5 text-[13px] text-muted">
              <Spinner size={14} /> {t("dg.cloud.loading")}
            </li>
          ) : entries.length === 0 ? (
            <li className="px-3 py-2.5 text-[13px] text-muted">{problem ?? t("dg.cloud.emptyFolder")}</li>
          ) : (
            entries.map((e) => (
              <li key={e.path} className="border-b border-hair last:border-b-0">
                <button
                  type="button"
                  disabled={busy}
                  className={cn(
                    "flex w-full items-center gap-2.5 px-3 py-2 text-left text-[13.5px] hover:bg-sunk",
                    !e.dir && mode === "save" && "text-muted",
                  )}
                  onClick={() => (e.dir ? setFolder(e.path) : mode === "open" ? void open(e) : setName(e.name))}
                >
                  <HugeiconsIcon icon={e.dir ? Folder01Icon : File02Icon} size={16} strokeWidth={1.6} color="currentColor" className="shrink-0 text-muted" />
                  <span className="min-w-0 truncate">{e.name}</span>
                </button>
              </li>
            ))
          )}
        </ul>
        {mode === "save" && (
          <label htmlFor={nameId} className="flex flex-col gap-1 text-[13px]">
            {t("dg.cloud.name")}
            <Input
              id={nameId}
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                setReplace(null);
              }}
              onKeyDown={(e) => e.key === "Enter" && void save(null)}
            />
          </label>
        )}
        {replace && (
          <p className="rounded-card border border-warn/50 bg-warn/10 px-3 py-2 text-[13px]" role="alert">
            {t("dg.cloud.replaceAsk", { name: replace.name })}
          </p>
        )}
      </DialogBody>
      <DialogFooter>
        <Button variant="secondary" size="sm" onClick={onClose}>
          {t("dg.cloud.cancel")}
        </Button>
        {mode === "save" &&
          (replace ? (
            <Button size="sm" disabled={busy} onClick={() => void save(replace)}>
              {t("dg.cloud.replace")}
            </Button>
          ) : (
            <Button size="sm" disabled={busy || !name.trim()} onClick={() => void save(null)} data-testid="cloud-save">
              {t("dg.cloud.save")}
            </Button>
          ))}
      </DialogFooter>
    </Dialog>
  );
}
