import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Delete02Icon, RestoreBinIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Spinner } from "@/components/ui/progress";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { cloud, isAbort, parentOf, type Trashed } from "./api";
import { ConfirmDialog, explain, failed } from "./dialogs";
import { formatBytes, formatFull, formatWhen, iconFor } from "./format";
import { bump, useRevision } from "./store";

export function TrashView() {
  const t = useT();
  const revision = useRevision();
  const [items, setItems] = React.useState<Trashed[] | null>(null);
  const [days, setDays] = React.useState(30);
  const [error, setError] = React.useState<string | null>(null);
  const [purging, setPurging] = React.useState<Trashed | null>(null);
  const [emptying, setEmptying] = React.useState(false);

  React.useEffect(() => {
    const ctl = new AbortController();
    cloud.trash(ctl.signal).then(
      (r) => {
        setItems([...r.entries].sort((a, b) => b.deleted - a.deleted));
        setDays(r.days);
      },
      (err: unknown) => {
        if (!isAbort(err)) setError(explain(err));
      },
    );
    return () => ctl.abort();
  }, [revision]);

  const restore = async (item: Trashed) => {
    try {
      const entry = await cloud.restore(item.id);
      toast.done(t("cloud.trash.restored", { name: entry.name }), parentOf(entry.path) || t("cloud.nav.files"));
      bump();
    } catch (err) {
      failed(t("cloud.trash.restoreFailed", { name: item.name }), err);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex min-h-9 flex-wrap items-center gap-3">
        <h1 className="text-[19px] leading-tight font-semibold tracking-tight">{t("cloud.nav.trash")}</h1>
        {items !== null && items.length > 0 && (
          <Button size="sm" variant="secondary" className="ml-auto" onClick={() => setEmptying(true)}>
            {t("cloud.trash.empty")}
          </Button>
        )}
      </div>
      <p className="text-[13px] text-muted">{t("cloud.trash.summary", { days })}</p>
      {error !== null ? (
        <Alert variant="crit">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : items === null ? (
        <div className="grid min-h-60 place-items-center">
          <Spinner />
        </div>
      ) : items.length === 0 ? (
        <Empty className="min-h-72">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={Delete02Icon} size={22} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("cloud.trash.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("cloud.trash.emptyBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <div className="rounded-card border border-line bg-surface">
          <Table data-trash>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead>{t("cloud.col.name")}</TableHead>
                <TableHead className="max-md:hidden">{t("cloud.trash.from")}</TableHead>
                <TableHead className="max-sm:hidden">{t("cloud.trash.deleted")}</TableHead>
                <TableHead className="w-0">
                  <span className="sr-only">{t("cloud.trash.actions")}</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {items.map((item) => (
                <TableRow key={item.id} data-entry={item.name}>
                  <TableCell className="w-full max-w-0">
                    <div className="flex min-w-0 items-center gap-3">
                      <HugeiconsIcon
                        icon={iconFor({ dir: item.dir, mime: "", name: item.name })}
                        size={22}
                        strokeWidth={1.5}
                        color="currentColor"
                        className={item.dir ? "text-accent" : "text-muted"}
                        aria-hidden="true"
                      />
                      <span className="min-w-0 truncate font-medium">{item.name}</span>
                      {!item.dir && <span className="numeric text-[12px] text-faint">{formatBytes(item.size)}</span>}
                    </div>
                  </TableCell>
                  <TableCell className="text-muted max-md:hidden">{parentOf(item.original) || t("cloud.nav.files")}</TableCell>
                  <TableCell className="text-muted max-sm:hidden" title={formatFull(item.deleted)}>
                    {formatWhen(item.deleted)}
                  </TableCell>
                  <TableCell>
                    <div className="flex justify-end gap-1">
                      <Button size="sm" variant="ghost" className="text-ink" onClick={() => void restore(item)}>
                        <HugeiconsIcon icon={RestoreBinIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                        {t("cloud.trash.restore")}
                      </Button>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        className="hover:text-crit"
                        aria-label={t("cloud.trash.purge", { name: item.name })}
                        title={t("cloud.trash.purge", { name: item.name })}
                        onClick={() => setPurging(item)}
                      >
                        <HugeiconsIcon icon={Delete02Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}

      <ConfirmDialog
        open={purging !== null}
        onOpenChange={(o) => !o && setPurging(null)}
        title={t("cloud.trash.purgeTitle", { name: purging?.name ?? "" })}
        body={t("cloud.trash.purgeBody")}
        confirm={t("cloud.trash.purgeConfirm")}
        onConfirm={() => {
          const item = purging;
          if (item === null) return;
          cloud.purge(item.id).then(bump, (err: unknown) => failed(t("cloud.trash.purgeFailed"), err));
        }}
      />
      <ConfirmDialog
        open={emptying}
        onOpenChange={setEmptying}
        title={t("cloud.trash.emptyConfirmTitle", { count: items?.length ?? 0 })}
        body={t("cloud.trash.purgeBody")}
        confirm={t("cloud.trash.empty")}
        onConfirm={() => {
          cloud.emptyTrash().then(bump, (err: unknown) => failed(t("cloud.trash.purgeFailed"), err));
        }}
      />
    </div>
  );
}
