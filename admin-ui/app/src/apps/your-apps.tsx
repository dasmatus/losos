/* Your apps: the owner's own apps, on the Apps pane.
 *
 * Each row opens the app on its own page (/apps/<id>), or the editor
 * (/apps/<id>/edit); "Make an app" opens a new one (/apps/new). Deleting
 * one deletes its data with it, so it asks first. The box holds the list
 * (GET /api/user-apps); nothing here is cached in the browser.
 */

import * as React from "react";
import { Link, useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon, PlusSignIcon, SparklesIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { deleteUserApp, getUserApps, type UserAppSummary, type UserAppsResponse } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Group, GroupCaption, GroupTitle, PaneSection, StackRow } from "@/screens/settings/rows";

type Listed = { state: "loading" } | { state: "failed" } | { state: "ready"; list: UserAppsResponse };

export function YourApps({ active, disabled }: { active: boolean; disabled: boolean }) {
  const t = useT();
  const navigate = useNavigate();
  const [listed, setListed] = React.useState<Listed>({ state: "loading" });
  const [deleting, setDeleting] = React.useState<UserAppSummary | null>(null);

  const load = React.useCallback(async (): Promise<void> => {
    try {
      const list = await getUserApps();
      // An older lososd answers this route with nothing it knows.
      if (!Array.isArray(list.apps)) throw new Error("no list");
      setListed({ state: "ready", list });
    } catch {
      setListed({ state: "failed" });
    }
  }, []);

  React.useEffect(() => {
    if (active) void load();
  }, [active, load]);

  const apps = listed.state === "ready" ? listed.list.apps : [];
  const full = listed.state === "ready" && apps.length >= listed.list.limits.apps;

  return (
    <PaneSection data-testid="your-apps">
      <div className="flex items-end justify-between gap-3 pb-1.5">
        <GroupTitle className="pb-0">{t("userApps.title")}</GroupTitle>
        <Button
          size="sm"
          disabled={disabled || full || listed.state !== "ready"}
          onClick={() => navigate("/apps/new")}
          data-testid="your-apps-make"
        >
          <HugeiconsIcon icon={PlusSignIcon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("userApps.make")}
        </Button>
      </div>
      {listed.state === "ready" && apps.length === 0 && (
        <Empty className="bg-surface" data-testid="your-apps-empty">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={SparklesIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("userApps.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("userApps.empty")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}
      {!(listed.state === "ready" && apps.length === 0) && (
        <Group>
          {listed.state === "loading" && (
            <StackRow last>
              <Spinner label={t("userApps.loading")} className="text-muted" />
            </StackRow>
          )}
          {listed.state === "failed" && (
            <StackRow last>
              <Alert variant="crit">
                <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                <AlertDescription>
                  <p>{t("userApps.listFailed")}</p>
                  <Button
                    size="xs"
                    variant="secondary"
                    className="mt-2"
                    onClick={() => {
                      setListed({ state: "loading" });
                      void load();
                    }}
                  >
                    {t("userApps.retry")}
                  </Button>
                </AlertDescription>
              </Alert>
            </StackRow>
          )}
          {apps.map((app, index) => (
            <StackRow key={app.id} last={index === apps.length - 1} data-testid="your-app">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="min-w-0">
                  <Link to={`/apps/${app.id}`} className="text-sm font-medium leading-snug text-ink hover:text-accent">
                    {app.name}
                  </Link>
                  {app.summary !== null && app.summary !== "" && (
                    <p className="mt-0.5 text-[12.5px] leading-snug text-muted">{app.summary}</p>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-1.5">
                  <Button size="sm" variant="secondary" onClick={() => navigate(`/apps/${app.id}`)}>
                    {t("userApps.open")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={disabled}
                    onClick={() => navigate(`/apps/${app.id}/edit`)}
                  >
                    {t("userApps.change")}
                  </Button>
                  <Button size="sm" variant="ghost" disabled={disabled} onClick={() => setDeleting(app)}>
                    {t("userApps.delete")}
                  </Button>
                </div>
              </div>
            </StackRow>
          ))}
        </Group>
      )}
      <GroupCaption className={cn(full && "text-warn")}>
        {full && listed.state === "ready"
          ? t("userApps.full", { max: listed.list.limits.apps })
          : t("userApps.caption")}
      </GroupCaption>
      <DeleteDialog
        app={deleting}
        onClose={() => setDeleting(null)}
        onDeleted={() => {
          setDeleting(null);
          void load();
        }}
      />
    </PaneSection>
  );
}

function DeleteDialog({
  app,
  onClose,
  onDeleted,
}: {
  app: UserAppSummary | null;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const [sending, setSending] = React.useState(false);
  // Kept while the dialog closes, so its text does not blank mid-animation.
  const [shown, setShown] = React.useState<UserAppSummary | null>(app);
  React.useEffect(() => {
    if (app !== null) setShown(app);
  }, [app]);

  const confirm = async (): Promise<void> => {
    if (app === null) return;
    setSending(true);
    try {
      await deleteUserApp(app.id);
      toast.success(t("userApps.deleted", { name: app.name }));
      onDeleted();
    } catch (error) {
      toast.error(t("userApps.deleteFailed"), error instanceof Error ? error.message : undefined, { help: "apps" });
    } finally {
      setSending(false);
    }
  };

  return (
    <Dialog
      open={app !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      dismissible={!sending}
      labelledBy={titleId}
      describedBy={bodyId}
    >
      <DialogHeader>
        <DialogTitle id={titleId}>{t("userApps.deleteTitle", { name: shown?.name ?? "" })}</DialogTitle>
      </DialogHeader>
      <DialogBody>
        <DialogDescription id={bodyId}>{t("userApps.deleteBody")}</DialogDescription>
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" disabled={sending} onClick={onClose}>
          {t("userApps.cancel")}
        </Button>
        <Button disabled={sending} onClick={() => void confirm()}>
          {sending && <Spinner size={14} label={t("userApps.delete")} />}
          {t("userApps.delete")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
