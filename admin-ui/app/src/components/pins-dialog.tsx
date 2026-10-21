import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowDown01Icon,
  ArrowLeft01Icon,
  ArrowRight01Icon,
  ArrowUp01Icon,
  Folder01Icon,
  File01Icon,
  GitBranchIcon,
  PackageIcon,
  PinIcon,
  PinOffIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { toast } from "@/components/ui/toast";
import {
  getApps,
  getCloudFiles,
  getGitRepos,
  isAbort,
  type CloudFilesResponse,
  type GitReposResponse,
  type InstalledApp,
} from "@/lib/api";
import { CATALOGUE, formatBytes, recallAvailability } from "@/lib/apps";
import { useT } from "@/lib/i18n-react";
import {
  getPinsState,
  getServerPinsState,
  isPinned,
  moved,
  pinKey,
  savePins,
  subscribePins,
  toggled,
  viewPin,
  type Pin,
  type PinView,
} from "@/lib/pins";
import { cn } from "@/lib/utils";

/* Where the owner edits the sidebar's pins: the list in order, with up,
 * down and unpin on each, and below it the three places to pin from. The
 * Overview's app tiles and the Apps pane's installed apps can pin too; this
 * is the one place that does all of it, files and repositories included.
 *
 * Native <dialog> (components/ui/dialog.tsx) like every modal on this page:
 * the admin CSP refuses the scroll lock Radix would bring. */

export function usePins() {
  return React.useSyncExternalStore(subscribePins, getPinsState, getServerPinsState);
}

/** Write a change to the box; a refusal is a toast with the box's words. */
export function useSavePins(): { busy: boolean; save: (next: readonly Pin[]) => Promise<void> } {
  const t = useT();
  const [busy, setBusy] = React.useState(false);
  const save = React.useCallback(
    async (next: readonly Pin[]) => {
      setBusy(true);
      try {
        await savePins(next);
      } catch (error) {
        toast.error(t("pins.failed"), error instanceof Error ? error.message : undefined, { help: "index" });
      } finally {
        setBusy(false);
      }
    },
    [t],
  );
  return { busy, save };
}

export function PinsDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const listId = React.useId();
  const fromId = React.useId();
  const state = usePins();
  const { busy, save } = useSavePins();
  const [tab, setTab] = React.useState("apps");
  const full = state.pins.length >= state.limit;

  const toggle = (pin: Pin): void => {
    if (!isPinned(state.pins, pin) && full) return;
    void save(toggled(state.pins, pin));
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(34rem,calc(100vw-2rem))]"
    >
      <DialogHeader>
        <DialogTitle id={titleId}>{t("pins.dialog.title")}</DialogTitle>
        <DialogDescription id={bodyId}>{t("pins.dialog.body")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-5">
        <section aria-labelledby={listId} className="flex flex-col gap-2">
          <h3 id={listId} className="text-[12.5px] font-medium text-muted">
            {t("pins.title")}
          </h3>
          {state.pins.length === 0 ? (
            <p className="rounded-card border border-dashed border-line px-3 py-3 text-[13px] text-muted">
              {t("pins.dialog.none")}
            </p>
          ) : (
            <ol className="m-0 flex list-none flex-col divide-y divide-hair rounded-card border border-line p-0" data-pins-list>
              {state.pins.map((pin, index) => {
                const view = viewPin(pin, state.installed);
                if (view === null) return null;
                const name = view.labelKey !== null ? t(view.labelKey) : view.label;
                return (
                  <li key={view.key} className="flex items-center gap-2 px-2.5 py-1.5">
                    <Glyph icon={view.icon} />
                    <PinText name={name} detail={view.detail} />
                    <Button
                      size="icon-xs"
                      variant="ghost"
                      aria-label={t("pins.up", { name })}
                      disabled={busy || index === 0}
                      onClick={() => void save(moved(state.pins, view.key, -1))}
                    >
                      <Icon icon={ArrowUp01Icon} />
                    </Button>
                    <Button
                      size="icon-xs"
                      variant="ghost"
                      aria-label={t("pins.down", { name })}
                      disabled={busy || index === state.pins.length - 1}
                      onClick={() => void save(moved(state.pins, view.key, 1))}
                    >
                      <Icon icon={ArrowDown01Icon} />
                    </Button>
                    <Button
                      size="icon-xs"
                      variant="ghost"
                      aria-label={t("pins.unpin", { name })}
                      disabled={busy}
                      onClick={() => void save(state.pins.filter((p) => pinKey(p) !== view.key))}
                    >
                      <Icon icon={PinOffIcon} />
                    </Button>
                  </li>
                );
              })}
            </ol>
          )}
          {full && <p className="text-[12.5px] text-warn">{t("pins.full", { limit: state.limit })}</p>}
        </section>

        <section aria-labelledby={fromId} className="flex flex-col gap-2">
          <h3 id={fromId} className="text-[12.5px] font-medium text-muted">
            {t("pins.dialog.choose")}
          </h3>
          <Tabs value={tab} onValueChange={setTab} className="gap-3">
            <TabsList className="self-start">
              <TabsTrigger value="apps">{t("pins.tab.apps")}</TabsTrigger>
              <TabsTrigger value="files">{t("pins.tab.files")}</TabsTrigger>
              <TabsTrigger value="repos">{t("pins.tab.repos")}</TabsTrigger>
            </TabsList>
            <TabsContent value="apps">
              <AppsPicker open={open} pins={state.pins} busy={busy} full={full} onToggle={toggle} />
            </TabsContent>
            <TabsContent value="files">
              <FilesPicker open={open} pins={state.pins} busy={busy} full={full} onToggle={toggle} />
            </TabsContent>
            <TabsContent value="repos">
              <ReposPicker open={open} pins={state.pins} busy={busy} full={full} onToggle={toggle} />
            </TabsContent>
          </Tabs>
        </section>
      </DialogBody>
      <DialogFooter>
        <Button size="sm" onClick={() => onOpenChange(false)}>
          {t("pins.dialog.done")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

// ── Pieces ────────────────────────────────────────────────────────────────

function Icon({ icon, size = 14 }: { icon: IconSvgElement; size?: number }) {
  return <HugeiconsIcon icon={icon} size={size} strokeWidth={1.5} color="currentColor" aria-hidden="true" />;
}

function Glyph({ icon }: { icon: IconSvgElement }) {
  return (
    <span
      aria-hidden="true"
      className="flex size-7 shrink-0 items-center justify-center rounded-[7px] border border-hair bg-sunk text-muted"
    >
      <Icon icon={icon} size={15} />
    </span>
  );
}

function PinText({ name, detail }: { name: string; detail: string | null }) {
  return (
    <span className="flex min-w-0 flex-1 flex-col">
      <span className="truncate text-[13px] text-ink">{name}</span>
      {detail !== null && <span className="truncate text-[11.5px] text-faint">{detail}</span>}
    </span>
  );
}

interface PickerProps {
  open: boolean;
  pins: readonly Pin[];
  busy: boolean;
  full: boolean;
  onToggle: (pin: Pin) => void;
}

/** The pin toggle at the end of a picker row. */
export function PinToggle({
  pin,
  pins,
  name,
  busy,
  full,
  onToggle,
  className,
}: {
  pin: Pin;
  pins: readonly Pin[];
  name: string;
  busy: boolean;
  full: boolean;
  onToggle: (pin: Pin) => void;
  className?: string;
}) {
  const t = useT();
  const pinned = isPinned(pins, pin);
  return (
    <Button
      size="xs"
      variant={pinned ? "secondary" : "ghost"}
      aria-pressed={pinned}
      aria-label={t(pinned ? "pins.unpin" : "pins.pin", { name })}
      disabled={busy || (!pinned && full)}
      onClick={() => onToggle(pin)}
      className={cn("shrink-0", pinned && "text-accent", className)}
    >
      <Icon icon={pinned ? PinOffIcon : PinIcon} size={13} />
      {t(pinned ? "pins.unpinShort" : "pins.pinShort")}
    </Button>
  );
}

function Rows({ children, label }: { children: React.ReactNode; label?: string }) {
  return (
    <ul
      aria-label={label}
      className="m-0 flex max-h-64 list-none flex-col divide-y divide-hair overflow-y-auto rounded-card border border-line p-0"
    >
      {children}
    </ul>
  );
}

function Row({ children }: { children: React.ReactNode }) {
  return <li className="flex items-center gap-2 px-2.5 py-1.5">{children}</li>;
}

function Note({ children, tone = "muted" }: { children: React.ReactNode; tone?: "muted" | "warn" }) {
  return (
    <p className={cn("px-0.5 py-2 text-[13px] leading-snug", tone === "warn" ? "text-warn" : "text-muted")}>
      {children}
    </p>
  );
}

function Loading() {
  const t = useT();
  return (
    <div className="flex items-center gap-2 py-3 text-[13px] text-muted">
      <Spinner size={14} label={t("pins.loading")} />
      {t("pins.loading")}
    </div>
  );
}

function GroupLabel({ children }: { children: React.ReactNode }) {
  return <p className="px-0.5 pb-1 text-[11.5px] font-medium tracking-wide text-faint">{children}</p>;
}

// ── Apps ──────────────────────────────────────────────────────────────────

function AppsPicker({ open, pins, busy, full, onToggle }: PickerProps) {
  const t = useT();
  const [installed, setInstalled] = React.useState<InstalledApp[] | null>(null);
  React.useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const reply = await getApps({ signal: controller.signal });
        setInstalled(reply.available ? reply.apps : []);
      } catch (error) {
        if (!isAbort(error)) setInstalled([]);
      }
    })();
    return () => controller.abort();
  }, [open]);

  // The Overview's tiles, less the one that is only there when a probe
  // found it (lib/apps.ts, `optional`).
  const mail = recallAvailability().mail;
  const tiles = CATALOGUE.filter((app) => app.optional !== true || mail === "present");

  return (
    <div className="flex flex-col gap-3">
      <div>
        <GroupLabel>{t("pins.apps.installed")}</GroupLabel>
        {installed === null ? (
          <Loading />
        ) : installed.length === 0 ? (
          <Note>{t("pins.apps.noneInstalled")}</Note>
        ) : (
          <Rows label={t("pins.apps.installed")}>
            {installed.map((record) => (
              <Row key={record.release}>
                <Glyph icon={PackageIcon} />
                <PinText name={record.release} detail={record.chart?.name ?? record.title ?? ""} />
                <PinToggle
                  pin={{ kind: "app", release: record.release }}
                  pins={pins}
                  name={record.release}
                  busy={busy}
                  full={full}
                  onToggle={onToggle}
                />
              </Row>
            ))}
          </Rows>
        )}
      </div>
      <div>
        <GroupLabel>{t("pins.apps.overview")}</GroupLabel>
        <Rows label={t("pins.apps.overview")}>
          {tiles.map((app) => {
            const name = t(app.name);
            return (
              <Row key={app.id}>
                <Glyph icon={app.icon} />
                <PinText name={name} detail={null} />
                <PinToggle
                  pin={{ kind: "tile", app: app.id }}
                  pins={pins}
                  name={name}
                  busy={busy}
                  full={full}
                  onToggle={onToggle}
                />
              </Row>
            );
          })}
        </Rows>
      </div>
    </div>
  );
}

// ── Files ─────────────────────────────────────────────────────────────────

type Load<T> = { kind: "loading" } | { kind: "ready"; reply: T } | { kind: "failed" };

function FilesPicker({ open, pins, busy, full, onToggle }: PickerProps) {
  const t = useT();
  const [path, setPath] = React.useState("/");
  const [load, setLoad] = React.useState<Load<CloudFilesResponse>>({ kind: "loading" });

  React.useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    setLoad({ kind: "loading" });
    void (async () => {
      try {
        const reply = await getCloudFiles({ path }, { signal: controller.signal });
        setLoad({ kind: "ready", reply });
      } catch (error) {
        if (!isAbort(error)) setLoad({ kind: "failed" });
      }
    })();
    return () => controller.abort();
  }, [open, path]);

  const parts = path.split("/").filter((part) => part.length > 0);
  const up = parts.length === 0 ? null : `/${parts.slice(0, -1).join("/")}`;

  return (
    <div className="flex flex-col gap-2">
      <nav aria-label={t("pins.tab.files")} className="flex min-w-0 flex-wrap items-center gap-1 text-[12.5px]">
        {up !== null && (
          <Button size="icon-xs" variant="ghost" aria-label={t("pins.files.up")} onClick={() => setPath(up)}>
            <Icon icon={ArrowLeft01Icon} />
          </Button>
        )}
        <button
          type="button"
          onClick={() => setPath("/")}
          className="rounded-control px-1 text-muted hover:text-ink"
          aria-current={parts.length === 0 ? "location" : undefined}
        >
          {t("pins.files.top")}
        </button>
        {parts.map((part, index) => {
          const to = `/${parts.slice(0, index + 1).join("/")}`;
          const last = index === parts.length - 1;
          return (
            <React.Fragment key={to}>
              <span aria-hidden="true" className="text-faint">
                /
              </span>
              <button
                type="button"
                onClick={() => setPath(to)}
                aria-current={last ? "location" : undefined}
                className={cn("max-w-40 truncate rounded-control px-1", last ? "font-medium text-ink" : "text-muted hover:text-ink")}
              >
                {part}
              </button>
            </React.Fragment>
          );
        })}
      </nav>

      {load.kind === "loading" && <Loading />}
      {load.kind === "failed" && <Note tone="warn">{t("pins.noAnswer")}</Note>}
      {load.kind === "ready" && !load.reply.available && (
        <Note tone={load.reply.reason === "signIn" ? "muted" : "warn"}>
          {t(load.reply.reason === "signIn" ? "pins.files.signIn" : "pins.files.notUp")}
        </Note>
      )}
      {load.kind === "ready" && load.reply.available && load.reply.entries.length === 0 && (
        <Note>{t("pins.files.empty")}</Note>
      )}
      {load.kind === "ready" && load.reply.available && load.reply.entries.length > 0 && (
        <>
          <Rows label={parts.at(-1) ?? t("pins.files.top")}>
            {load.reply.entries.map((entry) => (
              <Row key={entry.id}>
                <Glyph icon={entry.folder ? Folder01Icon : File01Icon} />
                {entry.folder ? (
                  <button
                    type="button"
                    onClick={() => setPath(entry.path)}
                    aria-label={t("pins.files.open", { name: entry.name })}
                    className="group flex min-w-0 flex-1 items-center gap-1 text-left"
                  >
                    <span className="truncate text-[13px] text-ink group-hover:text-accent">{entry.name}</span>
                    <span className="text-faint">
                      <Icon icon={ArrowRight01Icon} size={12} />
                    </span>
                  </button>
                ) : (
                  <PinText name={entry.name} detail={entry.size === null ? null : formatBytes(entry.size)} />
                )}
                <PinToggle
                  pin={{ kind: "file", id: entry.id, path: entry.path, folder: entry.folder }}
                  pins={pins}
                  name={entry.name}
                  busy={busy}
                  full={full}
                  onToggle={onToggle}
                />
              </Row>
            ))}
          </Rows>
          {load.reply.more && <Note>{t("pins.files.more")}</Note>}
        </>
      )}
    </div>
  );
}

// ── Repositories ──────────────────────────────────────────────────────────

function ReposPicker({ open, pins, busy, full, onToggle }: PickerProps) {
  const t = useT();
  const [load, setLoad] = React.useState<Load<GitReposResponse>>({ kind: "loading" });
  React.useEffect(() => {
    if (!open) return;
    const controller = new AbortController();
    setLoad({ kind: "loading" });
    void (async () => {
      try {
        const reply = await getGitRepos(null, { signal: controller.signal });
        setLoad({ kind: "ready", reply });
      } catch (error) {
        if (!isAbort(error)) setLoad({ kind: "failed" });
      }
    })();
    return () => controller.abort();
  }, [open]);

  if (load.kind === "loading") return <Loading />;
  if (load.kind === "failed") return <Note tone="warn">{t("pins.noAnswer")}</Note>;
  const { reply } = load;
  if (!reply.available) {
    return <Note tone="warn">{t(reply.reason === "noGit" ? "pins.repos.noGit" : "pins.repos.notUp")}</Note>;
  }
  if (reply.repos.length === 0) return <Note>{t("pins.repos.empty")}</Note>;
  return (
    <Rows label={t("pins.tab.repos")}>
      {reply.repos.map((repo) => {
        const detail = [repo.owner, repo.private ? t("pins.repos.private") : null, repo.description || null]
          .filter((part): part is string => part !== null)
          .join(" · ");
        return (
          <Row key={`${repo.owner}/${repo.name}`}>
            <Glyph icon={GitBranchIcon} />
            <PinText name={repo.name} detail={detail} />
            <PinToggle
              pin={{ kind: "repo", owner: repo.owner, name: repo.name }}
              pins={pins}
              name={repo.name}
              busy={busy}
              full={full}
              onToggle={onToggle}
            />
          </Row>
        );
      })}
    </Rows>
  );
}

export type { PinView };
