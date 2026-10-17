import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon, InformationCircleIcon, LinkSquare02Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Spinner } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import type { CloudApp, CloudStoreApp } from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import type { CloudApps } from "./use-cloud-apps";
import { useCloudStore } from "./use-cloud-apps";

/* LosOS cloud's own apps, inside the Apps pane's LosOS cloud accordion: the
 * list with a switch per app, and the app store search under it.
 *
 * The list is long (Nextcloud ships some forty apps of its own), so the ones
 * built into LosOS cloud are folded behind a button until asked for or
 * filtered to; the apps LosOS bundles and the ones added from the store are
 * the ones an owner comes here for. */

const LOCKED_KEY = {
  nextcloud: "cloudApps.locked.nextcloud",
  signIn: "cloudApps.locked.signIn",
  sharing: "cloudApps.locked.sharing",
  look: "cloudApps.locked.look",
  wizard: "cloudApps.locked.wizard",
} as const satisfies Record<string, MessageKey>;

function lockedText(app: CloudApp): MessageKey | null {
  if (app.locked === null) return null;
  return app.locked.by === "nextcloud" ? LOCKED_KEY.nextcloud : LOCKED_KEY[app.locked.reason];
}

function matches(app: CloudApp, filter: string): boolean {
  const q = filter.trim().toLowerCase();
  if (q.length === 0) return true;
  return (
    app.name.toLowerCase().includes(q) ||
    app.id.includes(q) ||
    (app.summary?.toLowerCase().includes(q) ?? false)
  );
}

export function CloudAppsSection({ apps, disabled }: { apps: CloudApps; disabled: boolean }) {
  const t = useT();
  const filterId = React.useId();
  const [filter, setFilter] = React.useState("");
  const [showCore, setShowCore] = React.useState(false);
  const [removing, setRemoving] = React.useState<CloudApp | null>(null);
  const { state, busy, change } = apps;

  if (state.kind === "unsupported") return null;

  let body: React.ReactNode;
  if (state.kind === "loading") {
    body = (
      <StackRow last>
        <div className="flex items-center gap-2.5 text-[13px] text-muted">
          <Spinner size={16} label={t("cloudApps.apps.loading")} />
          {t("cloudApps.apps.loading")}
        </div>
      </StackRow>
    );
  } else if (state.kind === "unavailable" || state.kind === "failed") {
    body = (
      <StackRow last>
        <Alert variant={state.kind === "failed" ? "crit" : undefined}>
          <HugeiconsIcon
            icon={state.kind === "failed" ? Alert01Icon : InformationCircleIcon}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
          <AlertDescription>
            {state.kind === "failed"
              ? t("cloudApps.apps.failed", { message: state.message })
              : t("cloudApps.apps.unavailable", { reason: state.reason })}
          </AlertDescription>
        </Alert>
      </StackRow>
    );
  } else {
    const filtering = filter.trim().length > 0;
    const shown = state.apps.filter((app) => matches(app, filter));
    const added = shown.filter((app) => app.kind !== "core");
    const core = shown.filter((app) => app.kind === "core");
    const rows = filtering || showCore ? [...added, ...core] : added;
    body = (
      <>
        <StackRow>
          <label htmlFor={filterId} className="sr-only">
            {t("cloudApps.apps.filterLabel")}
          </label>
          <InputGroup>
            <InputGroupAddon>
              <HugeiconsIcon
                icon={Search01Icon}
                size={16}
                strokeWidth={1.5}
                color="currentColor"
                className="text-faint"
                aria-hidden="true"
              />
            </InputGroupAddon>
            <InputGroupInput
              id={filterId}
              type="search"
              value={filter}
              placeholder={t("cloudApps.apps.filterPlaceholder")}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setFilter(event.target.value)}
              className="[&::-webkit-search-cancel-button]:hidden"
            />
          </InputGroup>
        </StackRow>
        {rows.length === 0 && (
          <StackRow last={filtering}>
            <p className="text-[13px] text-muted">{t("cloudApps.apps.noMatch")}</p>
          </StackRow>
        )}
        {rows.map((app, index) => (
          <CloudAppRow
            key={app.id}
            app={app}
            busy={busy === app.id}
            disabled={disabled || busy !== null}
            last={filtering && index === rows.length - 1}
            onToggle={(on) => void change(on ? "enable" : "disable", app.id)}
            onRemove={() => setRemoving(app)}
          />
        ))}
        {!filtering && core.length > 0 && (
          <StackRow last className="py-2">
            <Button
              size="sm"
              variant="ghost"
              className="self-start"
              aria-expanded={showCore}
              onClick={() => setShowCore((open) => !open)}
            >
              {showCore ? t("cloudApps.apps.hideCore") : t("cloudApps.apps.showCore", { count: core.length })}
            </Button>
          </StackRow>
        )}
      </>
    );
  }

  return (
    <>
      <PaneSection data-testid="cloud-apps">
        <GroupTitle>{t("cloudApps.apps.title")}</GroupTitle>
        <Group>{body}</Group>
        <GroupCaption>{t("cloudApps.apps.caption")}</GroupCaption>
      </PaneSection>
      {state.kind === "ready" && state.store && (
        <CloudStoreSection installed={state.apps} busy={busy} disabled={disabled} onAdd={(id) => change("install", id)} />
      )}
      <RemoveCloudAppDialog
        app={removing}
        onClose={() => setRemoving(null)}
        onRemove={(id) => change("remove", id)}
      />
    </>
  );
}

function CloudAppRow({
  app,
  busy,
  disabled,
  last,
  onToggle,
  onRemove,
}: {
  app: CloudApp;
  busy: boolean;
  disabled: boolean;
  last: boolean;
  onToggle: (on: boolean) => void;
  onRemove: () => void;
}) {
  const t = useT();
  const id = React.useId();
  const locked = lockedText(app);
  const detail = locked !== null ? t(locked) : app.summary;
  return (
    <Row last={last} data-app={app.id} className="max-sm:flex-wrap">
      <RowText
        htmlFor={locked === null ? id : undefined}
        title={
          <span className="inline-flex flex-wrap items-center gap-2">
            {app.name}
            {app.kind === "store" && <Badge variant="local">{t("cloudApps.apps.fromStore")}</Badge>}
          </span>
        }
        detail={detail ?? undefined}
      />
      <div className="flex shrink-0 items-center gap-2">
        {busy && <Spinner size={14} label={t("cloudApps.apps.changing")} />}
        {app.kind === "store" && (
          <Button
            size="sm"
            variant="ghost"
            disabled={disabled}
            aria-label={t("cloudApps.apps.removeLabel", { name: app.name })}
            onClick={onRemove}
          >
            {t("cloudApps.apps.remove")}
          </Button>
        )}
        <Switch
          id={id}
          aria-label={t("cloudApps.apps.switchLabel", { name: app.name })}
          isSelected={app.enabled}
          isDisabled={disabled || locked !== null}
          onChange={onToggle}
        />
      </div>
    </Row>
  );
}

function CloudStoreSection({
  installed,
  busy,
  disabled,
  onAdd,
}: {
  installed: readonly CloudApp[];
  busy: string | null;
  disabled: boolean;
  onAdd: (id: string) => Promise<boolean>;
}) {
  const t = useT();
  const searchId = React.useId();
  const store = useCloudStore(!disabled);
  const have = new Set(installed.map((app) => app.id));
  const { state } = store;

  return (
    <PaneSection data-testid="cloud-store">
      <GroupTitle>{t("cloudApps.store.title")}</GroupTitle>
      <Group>
        <StackRow last={state.kind === "idle"}>
          <label htmlFor={searchId} className="sr-only">
            {t("cloudApps.store.searchLabel")}
          </label>
          <InputGroup>
            <InputGroupAddon>
              <HugeiconsIcon
                icon={Search01Icon}
                size={16}
                strokeWidth={1.5}
                color="currentColor"
                className="text-faint"
                aria-hidden="true"
              />
            </InputGroupAddon>
            <InputGroupInput
              id={searchId}
              type="search"
              value={store.query}
              placeholder={t("cloudApps.store.searchPlaceholder")}
              autoComplete="off"
              spellCheck={false}
              disabled={disabled}
              onChange={(event) => store.setQuery(event.target.value)}
              className="[&::-webkit-search-cancel-button]:hidden"
            />
          </InputGroup>
        </StackRow>
        {state.kind === "searching" && (
          <StackRow last>
            <div className="flex items-center gap-2.5 text-[13px] text-muted">
              <Spinner size={16} label={t("cloudApps.store.looking")} />
              {t("cloudApps.store.looking")}
            </div>
          </StackRow>
        )}
        {state.kind === "failed" && (
          <StackRow last>
            <Alert variant="crit">
              <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              <AlertDescription>{t("cloudApps.store.failed", { message: state.message })}</AlertDescription>
            </Alert>
          </StackRow>
        )}
        {state.kind === "results" && state.apps.length === 0 && (
          <StackRow last>
            <p className="text-[13px] text-muted">{t("cloudApps.store.empty")}</p>
          </StackRow>
        )}
        {state.kind === "results" &&
          state.apps.map((app, index) => (
            <StoreRow
              key={app.id}
              app={app}
              have={have.has(app.id)}
              busy={busy === app.id}
              disabled={disabled || busy !== null}
              last={index === state.apps.length - 1}
              onAdd={() => void onAdd(app.id)}
            />
          ))}
      </Group>
      <GroupCaption>{t("cloudApps.store.caption")}</GroupCaption>
    </PaneSection>
  );
}

function StoreRow({
  app,
  have,
  busy,
  disabled,
  last,
  onAdd,
}: {
  app: CloudStoreApp;
  have: boolean;
  busy: boolean;
  disabled: boolean;
  last: boolean;
  onAdd: () => void;
}) {
  const t = useT();
  return (
    <Row last={last} data-store-app={app.id} className="max-sm:flex-wrap">
      <RowText
        title={
          <span className="inline-flex flex-wrap items-baseline gap-x-2">
            {app.name}
            {app.version !== null && <span className="numeric text-[12px] text-faint">{app.version}</span>}
          </span>
        }
        detail={app.summary ?? undefined}
      />
      <div className="flex shrink-0 items-center gap-1.5">
        <a
          href={app.homepage}
          target="_blank"
          rel="noreferrer noopener external"
          className={cn(
            "inline-flex shrink-0 items-center gap-1.5 rounded-control px-2 py-1",
            "text-[12.5px] text-accent transition-colors duration-150 hover:bg-accent-wash",
          )}
        >
          {t("cloudApps.store.lookAt")}
          <HugeiconsIcon icon={LinkSquare02Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </a>
        {have ? (
          <Badge variant="ok">{t("cloudApps.store.added")}</Badge>
        ) : (
          <Button
            size="sm"
            disabled={disabled}
            aria-label={t("cloudApps.store.addLabel", { name: app.name })}
            onClick={onAdd}
          >
            {busy && <Spinner size={14} label={t("cloudApps.store.adding")} />}
            {t("cloudApps.store.add")}
          </Button>
        )}
      </div>
    </Row>
  );
}

function RemoveCloudAppDialog({
  app,
  onClose,
  onRemove,
}: {
  app: CloudApp | null;
  onClose: () => void;
  onRemove: (id: string) => Promise<boolean>;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const [sending, setSending] = React.useState(false);
  // Kept while the dialog closes, so its text does not blank mid-animation.
  const [shown, setShown] = React.useState<CloudApp | null>(app);
  React.useEffect(() => {
    if (app !== null) setShown(app);
  }, [app]);

  const confirm = async () => {
    if (app === null) return;
    setSending(true);
    try {
      if (await onRemove(app.id)) onClose();
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
        <DialogTitle id={titleId}>{t("cloudApps.remove.title", { name: shown?.name ?? "" })}</DialogTitle>
      </DialogHeader>
      <DialogBody>
        <DialogDescription id={bodyId}>{t("cloudApps.remove.body")}</DialogDescription>
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" disabled={sending} onClick={onClose}>
          {t("cloudApps.remove.cancel")}
        </Button>
        <Button disabled={sending} onClick={() => void confirm()}>
          {sending && <Spinner size={14} label={t("cloudApps.apps.changing")} />}
          {t("cloudApps.remove.confirm")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
