import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert01Icon,
  InformationCircleIcon,
  LinkSquare02Icon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
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
import { HelpLink } from "@/components/ui/help-link";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia } from "@/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { NativeSelect } from "@/components/ui/native-select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Spinner } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { t as translate, type MessageKey } from "@/lib/i18n";
import { Rich, useT } from "@/lib/i18n-react";
import type { InstalledApp, ServiceMode } from "@/lib/api";
import { useCatalogue, type CatalogueApp, type CatalogueState } from "./catalogue";
import { InstallDialog, type InstallTarget } from "./install-dialog";
import { useInstalledApps } from "./use-installed-apps";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import type { SettingsForm } from "./use-settings-form";

/* The apps this box runs, and where to look for more.
 *
 * Two rules hold over every string below. Nothing here names a container
 * runtime, an orchestrator or any of their nouns — the reader owns a box that
 * runs apps, and how those apps are packaged is this system's problem, not
 * theirs. And nothing here implies anyone checked the things the search
 * finds: the footer says so once, plainly, under the results rather than
 * buried in a tooltip.
 */

/** The wire spells these `native` and `container`; the reader never sees
 *  either word. The labels are what the choice actually costs them. */
const MODE_LABEL: Record<ServiceMode, MessageKey> = {
  native: "panes.apps.mode.native",
  container: "panes.apps.mode.container",
};

function isServiceMode(value: string): value is ServiceMode {
  return value === "native" || value === "container";
}

export function AppsPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const filesId = React.useId();
  const codeId = React.useId();
  const searchId = React.useId();

  const draft = form.draft;
  const disabled = form.locked || !form.ready;
  const catalogue = useCatalogue(!form.locked);
  const installed = useInstalledApps(!form.locked);
  const [target, setTarget] = React.useState<InstallTarget | null>(null);
  const [removing, setRemoving] = React.useState<InstalledApp | null>(null);

  const ready = installed.state.kind === "ready" ? installed.state : null;
  const records = ready?.apps ?? [];
  const install: Installer | null =
    ready === null || disabled
      ? null
      : {
          records,
          start: (app, source) =>
            setTarget({
              title: app.name,
              publisher: app.source,
              source,
              existing: records.find((r) => r.chart.name === source.name && r.chart.repo === source.repo) ?? null,
            }),
        };

  return (
    <>
      <PaneSection>
        <GroupTitle>{t("panes.apps.onThisBox")}</GroupTitle>
        <Group>
          <AppModeRow
            id={filesId}
            title={t("panes.apps.files.title")}
            detail={t("panes.apps.files.detail")}
            value={draft?.nextcloudMode ?? "container"}
            disabled={disabled}
            onChange={(mode) => form.set("nextcloudMode", mode)}
          />
          <AppModeRow
            id={codeId}
            title={t("panes.apps.code.title")}
            detail={t("panes.apps.code.detail")}
            value={draft?.forgejoMode ?? "container"}
            disabled={disabled}
            onChange={(mode) => form.set("forgejoMode", mode)}
            last
          />
        </Group>
        <GroupCaption>
          <Rich
            k="panes.apps.modeCaption"
            vars={{
              kept: (
                <strong className="font-medium text-ink">{t(MODE_LABEL.container)}</strong>
              ),
              native: <strong className="font-medium text-ink">{t(MODE_LABEL.native)}</strong>,
            }}
          />
        </GroupCaption>
      </PaneSection>

      {(records.length > 0 || installed.state.kind === "failed") && (
        <PaneSection>
          <GroupTitle>{t("install.installed")}</GroupTitle>
          <Group>
            {installed.state.kind === "failed" ? (
              <StackRow last>
                <Alert variant="crit">
                  <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  <AlertDescription>{t("install.listFailed", { message: installed.state.message })}</AlertDescription>
                </Alert>
              </StackRow>
            ) : (
              records.map((record, index) => (
                <InstalledRow
                  key={record.release}
                  record={record}
                  disabled={disabled}
                  last={index === records.length - 1}
                  onChange={() =>
                    setTarget({
                      title: record.chart.name,
                      publisher: record.chart.repo,
                      source: record.chart,
                      existing: record,
                    })
                  }
                  onRemove={() => setRemoving(record)}
                />
              ))
            )}
          </Group>
          <GroupCaption>{t("install.installedCaption")}</GroupCaption>
        </PaneSection>
      )}

      <PaneSection>
        <GroupTitle>{t("panes.apps.findMore")}</GroupTitle>
        <Group>
          <StackRow last={catalogue.state.kind === "idle"}>
            <label htmlFor={searchId} className="sr-only">
              {t("panes.apps.searchLabel")}
            </label>
            {/* A shadcn Input Group: the magnifier is an addon inside the
                field's border, not an icon floated over a plain input. */}
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
                value={catalogue.query}
                placeholder={t("panes.apps.searchPlaceholder")}
                autoComplete="off"
                spellCheck={false}
                disabled={disabled || catalogue.state.kind === "unsupported"}
                onChange={(event) => catalogue.setQuery(event.target.value)}
                className="[&::-webkit-search-cancel-button]:hidden"
              />
            </InputGroup>
          </StackRow>

          <CatalogueBody state={catalogue.state} install={install} />
        </Group>

        <GroupCaption>
          {footerFor(catalogue.state)}
          {installed.state.kind === "unavailable" && catalogue.state.kind === "results" && (
            <> {t("install.noCluster")}</>
          )}
        </GroupCaption>
      </PaneSection>

      <InstallDialog
        target={target}
        sharedAvailable={ready?.sharedAvailable ?? false}
        onOpenChange={(open) => {
          if (!open) setTarget(null);
        }}
        onInstall={installed.install}
      />
      <RemoveDialog
        record={removing}
        onClose={() => setRemoving(null)}
        onRemove={installed.remove}
      />
    </>
  );
}

// ── Rows ──────────────────────────────────────────────────────────────────

interface AppModeRowProps {
  id: string;
  title: string;
  detail: string;
  value: ServiceMode;
  disabled: boolean;
  onChange: (mode: ServiceMode) => void;
  last?: boolean;
}

function AppModeRow({ id, title, detail, value, disabled, onChange, last }: AppModeRowProps) {
  const t = useT();
  return (
    <Row last={last}>
      <RowText htmlFor={id} title={title} detail={detail} />
      {/* No badge beside the picker. `local` and `mesh` are the app's only
          two badge variants and they mean "on this box" versus "on the mesh";
          both choices here run on this box, so borrowing that pair would
          teach the reader something untrue about the texture. */}
      <NativeSelect
        id={id}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          const next = event.target.value;
          if (isServiceMode(next)) onChange(next);
        }}
      >
        <option value="container">{t(MODE_LABEL.container)}</option>
        <option value="native">{t(MODE_LABEL.native)}</option>
      </NativeSelect>
    </Row>
  );
}

// ── The catalogue ─────────────────────────────────────────────────────────

/** What a result row needs to offer an install; null while the box cannot
 *  take one (no cluster, still loading, the form locked). */
interface Installer {
  records: readonly InstalledApp[];
  start: (app: CatalogueApp, source: NonNullable<CatalogueApp["chart"]>) => void;
}

function CatalogueBody({ state, install }: { state: CatalogueState; install: Installer | null }) {
  const t = useT();
  switch (state.kind) {
    case "idle":
      return null;

    case "searching":
      return (
        <StackRow last>
          <div className="flex items-center gap-2.5 text-[13px] text-muted">
            <Spinner size={16} label={t("panes.apps.searching")} />
            {t("panes.apps.looking")}
          </div>
        </StackRow>
      );

    case "unsupported":
      return (
        <StackRow last>
          <Alert>
            <HugeiconsIcon icon={InformationCircleIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <AlertDescription>{t("panes.apps.unsupported")}</AlertDescription>
          </Alert>
        </StackRow>
      );

    case "failed":
      return (
        <StackRow last>
          <Alert variant="crit">
            <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <AlertDescription>
              <p>{t("panes.apps.failed", { message: state.message })}</p>
              {/* The catalogue is fetched from the internet: the usual cause
                  is a box with only the LAN. */}
              <HelpLink entry="only-the-lan-works" />
            </AlertDescription>
          </Alert>
        </StackRow>
      );

    case "results": {
      const { apps } = state.results;
      if (apps.length === 0) {
        return (
          <StackRow last>
            <Empty className="border-0 p-4 md:p-6">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <HugeiconsIcon icon={Search01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                </EmptyMedia>
                <EmptyDescription>{t("panes.apps.empty")}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          </StackRow>
        );
      }
      /* A shadcn Table: every hit has the same three facts, and the one that
         matters most — who published it — gets a column of its own, never
         abbreviated, rather than a footnote under the name. */
      return (
        <Table className="animate-fade-in">
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead>{t("panes.apps.col.app")}</TableHead>
              <TableHead>{t("panes.apps.col.source")}</TableHead>
              <TableHead>
                <span className="sr-only">{t("panes.apps.lookAt")}</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {apps.map((app) => (
              <ResultRow key={`${app.source}:${app.id}`} app={app} install={install} />
            ))}
          </TableBody>
        </Table>
      );
    }

    default: {
      const exhaustive: never = state;
      return exhaustive;
    }
  }
}

function ResultRow({ app, install }: { app: CatalogueApp; install: Installer | null }) {
  const t = useT();
  const chart = app.chart;
  const record =
    chart === null
      ? undefined
      : install?.records.find((r) => r.chart.name === chart.name && r.chart.repo === chart.repo);
  return (
    <TableRow>
      <TableCell className="whitespace-normal align-top">
        <p className="flex flex-wrap items-baseline gap-x-2 text-sm leading-snug text-ink">
          {app.name}
          {app.version !== null && <span className="numeric text-[12px] text-faint">{app.version}</span>}
        </p>
        {app.summary !== null && (
          <p className="mt-0.5 text-[12.5px] leading-snug text-muted">{app.summary}</p>
        )}
      </TableCell>
      {/* The source is on every row and never abbreviated. It is the only
          thing here that says who wrote this and who you would be trusting. */}
      <TableCell className="align-top text-[12.5px] text-muted">{app.source}</TableCell>
      <TableCell className="text-right align-top">
        <div className="flex flex-wrap items-center justify-end gap-1.5">
          {app.homepage !== null && (
            <a
              href={app.homepage}
              target="_blank"
              rel="noreferrer noopener external"
              className={cn(
                "inline-flex shrink-0 items-center gap-1.5 rounded-control px-2 py-1",
                "text-[12.5px] text-accent transition-colors duration-150 hover:bg-accent-wash",
              )}
            >
              {t("panes.apps.lookAt")}
              <HugeiconsIcon
                icon={LinkSquare02Icon}
                size={13}
                strokeWidth={1.5}
                color="currentColor"
                aria-hidden="true"
              />
            </a>
          )}
          {/* Only a row the box can fetch an app from, on a box that can run
              one. A second install of the same app is a change to the first. */}
          {chart !== null && install !== null && (
            <Button
              size="sm"
              variant={record === undefined ? "primary" : "secondary"}
              aria-label={record === undefined ? t("install.buttonLabel", { name: app.name }) : undefined}
              onClick={() => install.start(app, chart)}
            >
              {record === undefined ? t("install.button") : t("install.change")}
            </Button>
          )}
        </div>
      </TableCell>
    </TableRow>
  );
}

// ── Installed apps ────────────────────────────────────────────────────────

const PHASE_KEY = {
  installing: "install.phase.installing",
  removing: "install.phase.removing",
  running: "install.phase.running",
  failed: "install.phase.failed",
} as const satisfies Record<InstalledApp["phase"], MessageKey>;

function InstalledRow({
  record,
  disabled,
  last,
  onChange,
  onRemove,
}: {
  record: InstalledApp;
  disabled: boolean;
  last: boolean;
  onChange: () => void;
  onRemove: () => void;
}) {
  const t = useT();
  const busy = record.phase === "installing" || record.phase === "removing";
  // The forward listens on every address of the box, so the page's own host
  // reaches it: the .local name, or the IP address the owner typed.
  const href =
    record.phase === "running" && record.appPort !== null
      ? `http://${window.location.hostname}:${record.frontPort}/`
      : null;
  return (
    <StackRow last={last}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="flex flex-wrap items-center gap-2 text-sm leading-snug text-ink">
            <span className="font-mono">{record.release}</span>
            <Badge variant={record.phase === "running" ? "ok" : record.phase === "failed" ? "crit" : "neutral"}>
              {busy && <Spinner size={12} label={t(PHASE_KEY[record.phase])} />}
              {t(PHASE_KEY[record.phase])}
            </Badge>
          </p>
          <p className="mt-0.5 text-[12.5px] leading-snug text-muted">
            {t("install.runsAs", { chart: record.chart.name, version: record.chart.version, user: record.runAs })}
          </p>
          {record.phase === "failed" && record.message !== null && (
            <p className="mt-1 text-[12.5px] leading-snug text-crit">{record.message}</p>
          )}
          {record.phase === "running" && record.appPort === null && (
            <p className="mt-1 text-[12.5px] leading-snug text-muted">{t("install.noPort")}</p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {href !== null && (
            <a
              href={href}
              target="_blank"
              rel="noreferrer noopener"
              className={cn(
                "inline-flex items-center gap-1.5 rounded-control px-2 py-1",
                "text-[12.5px] text-accent transition-colors duration-150 hover:bg-accent-wash",
              )}
            >
              {t("install.open")}
              <HugeiconsIcon icon={LinkSquare02Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </a>
          )}
          <Button size="sm" variant="secondary" disabled={disabled || busy} onClick={onChange}>
            {t("install.change")}
          </Button>
          <Button size="sm" variant="ghost" disabled={disabled || busy} onClick={onRemove}>
            {t("install.remove")}
          </Button>
        </div>
      </div>
    </StackRow>
  );
}

function RemoveDialog({
  record,
  onClose,
  onRemove,
}: {
  record: InstalledApp | null;
  onClose: () => void;
  onRemove: (release: string) => Promise<boolean>;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const [sending, setSending] = React.useState(false);
  // Kept while the dialog closes, so its text does not blank mid-animation.
  const [shown, setShown] = React.useState<InstalledApp | null>(record);
  React.useEffect(() => {
    if (record !== null) setShown(record);
  }, [record]);

  const confirm = async () => {
    if (record === null) return;
    setSending(true);
    try {
      if (await onRemove(record.release)) onClose();
    } finally {
      setSending(false);
    }
  };

  return (
    <Dialog
      open={record !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      dismissible={!sending}
      labelledBy={titleId}
      describedBy={bodyId}
    >
      <DialogHeader>
        <DialogTitle id={titleId}>{t("install.removeTitle", { name: shown?.release ?? "" })}</DialogTitle>
      </DialogHeader>
      <DialogBody>
        <DialogDescription id={bodyId}>
          {t("install.removeBody", {
            path: shown === null ? "" : `/home/${shown.runAs}/data/apps/${shown.release}`,
          })}
        </DialogDescription>
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" disabled={sending} onClick={onClose}>
          {t("install.cancel")}
        </Button>
        <Button disabled={sending} onClick={() => void confirm()}>
          {sending && <Spinner size={14} label={t("install.sending")} />}
          {t("install.removeConfirm")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

/* The disclaimer, and it has to be a disclaimer rather than a hedge.
 *
 * Anything found here was written by a stranger and published to a public
 * index. Nobody on the LosOS side looked at it, and this box has no shell to
 * undo an install from. Naming the indexes searched is part of the same
 * honesty: the reader can go and judge the source for themselves. */
function footerFor(state: CatalogueState): string {
  const base = translate("panes.apps.footer");
  if (state.kind !== "results" || state.results.sources.length === 0) return base;
  const { sources } = state.results;
  return translate("panes.apps.footerSearched", {
    count: sources.length,
    sources: sources.join(", "),
    footer: base,
  });
}
