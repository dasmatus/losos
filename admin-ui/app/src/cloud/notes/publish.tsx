import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Copy01Icon, LinkSquare02Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import { toast } from "@/components/ui/toast";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useT } from "@/lib/i18n-react";
import type { FolderDialogProps } from "../openers";
import { explain } from "../dialogs";
import { kb, siteUrl, type Audience } from "./api";
import { reloadSites, siteFor, useSites } from "./sites";

/* A folder as a knowledge base: the right click's dialog. Unpublished, it
 * asks for a title and who may read it and publishes; published, it shows
 * the address, lets the owner change both, and stops the site. */

export function PublishDialog({ folder, open, onOpenChange }: FolderDialogProps) {
  const t = useT();
  const id = React.useId();
  const { available } = useSites();
  const site = siteFor(folder.id);
  const [title, setTitle] = React.useState(site?.title ?? folder.name);
  const [audience, setAudience] = React.useState<Audience>(site?.audience ?? "anyone");
  const [busy, setBusy] = React.useState<"save" | "stop" | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!open) return;
    setTitle(site?.title ?? folder.name);
    setAudience(site?.audience ?? "anyone");
    setError(null);
    setBusy(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, folder.id]);

  const save = async () => {
    setBusy("save");
    setError(null);
    try {
      if (site === undefined) {
        const made = await kb.create(folder.path, title.trim(), audience);
        await reloadSites();
        toast.success(t("notes.publish.done", { title: made.title }), siteUrl(made.slug));
      } else {
        await kb.update(site.id, { title: title.trim(), audience });
        await reloadSites();
        onOpenChange(false);
      }
    } catch (err) {
      setError(explain(err));
    } finally {
      setBusy(null);
    }
  };

  const stop = async () => {
    if (site === undefined) return;
    setBusy("stop");
    try {
      await kb.remove(site.id);
      await reloadSites();
      toast.done(t("notes.publish.stopped", { title: site.title }));
      onOpenChange(false);
    } catch (err) {
      setError(explain(err));
      setBusy(null);
    }
  };

  const url = site === undefined ? null : siteUrl(site.slug);

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={`${id}-title`} dialogClassName="w-[min(32rem,calc(100vw-2rem))]" data-publish-dialog>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (busy === null && available) void save();
        }}
      >
        <DialogHeader>
          <DialogTitle id={`${id}-title`}>{site === undefined ? t("notes.publish.title", { name: folder.name }) : site.title}</DialogTitle>
          <DialogDescription>{site === undefined ? t("notes.publish.what") : t("notes.publish.live")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          {!available && (
            <Alert variant="warn">
              <AlertDescription>{t("notes.publish.mesh")}</AlertDescription>
            </Alert>
          )}
          {url !== null && (
            <div className="flex items-center gap-2 rounded-control border border-line bg-sunk px-3 py-2" data-site-url>
              <a href={url} target="_blank" rel="noopener" className="min-w-0 flex-1 truncate text-[13px] text-accent underline-offset-2 hover:underline">
                {url}
              </a>
              <Button
                type="button"
                size="icon-sm"
                variant="ghost"
                aria-label={t("notes.publish.copy")}
                title={t("notes.publish.copy")}
                onClick={() => {
                  void navigator.clipboard?.writeText(url).then(() => toast.done(t("notes.publish.copied")));
                }}
              >
                <HugeiconsIcon icon={Copy01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </Button>
              <Button type="button" size="icon-sm" variant="ghost" aria-label={t("notes.publish.open")} title={t("notes.publish.open")} onClick={() => window.open(url, "_blank", "noopener")}>
                <HugeiconsIcon icon={LinkSquare02Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </Button>
            </div>
          )}
          <Field>
            <FieldLabel htmlFor={`${id}-name`}>{t("notes.publish.name")}</FieldLabel>
            <Input id={`${id}-name`} value={title} maxLength={120} onChange={(event) => setTitle(event.target.value)} autoComplete="off" />
          </Field>
          <Field>
            <FieldLabel id={`${id}-who`}>{t("notes.publish.who")}</FieldLabel>
            <ToggleGroup variant="outline" value={audience} onValueChange={(v) => setAudience(v as Audience)} aria-labelledby={`${id}-who`} className="w-fit">
              <ToggleGroupItem value="anyone" data-audience="anyone">
                {t("notes.publish.anyone")}
              </ToggleGroupItem>
              <ToggleGroupItem value="box" data-audience="box">
                {t("notes.publish.box")}
              </ToggleGroupItem>
            </ToggleGroup>
            <FieldDescription>{audience === "anyone" ? t("notes.publish.anyoneHint") : t("notes.publish.boxHint")}</FieldDescription>
          </Field>
          <p className="text-[12.5px] text-muted">{t("notes.publish.private")}</p>
          {error !== null && (
            <Alert variant="crit">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
        </DialogBody>
        <DialogFooter className="flex-wrap">
          {site !== undefined && (
            <Button type="button" variant="ghost" className="mr-auto text-crit hover:text-crit" disabled={busy !== null} onClick={() => void stop()} data-stop>
              {busy === "stop" && <Spinner size={14} />}
              {t("notes.publish.stop")}
            </Button>
          )}
          <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
            {site === undefined ? t("cloud.cancel") : t("cloud.close")}
          </Button>
          <Button type="submit" disabled={busy !== null || !available || title.trim() === ""} data-publish>
            {busy === "save" && <Spinner size={14} />}
            {site === undefined ? t("notes.publish.submit") : t("notes.publish.save")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
