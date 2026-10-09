import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon, Alert02Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { MonoInput } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import {
  getAppChart,
  isAbort,
  isUnauthorized,
  type AppChartResponse,
  type AppInstallRequest,
  type AppRunAs,
  type AppSource,
  type InstalledApp,
} from "@/lib/api";
import { t as translate } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import {
  changesToValues,
  pathKey,
  valuesToChanges,
  ValuesForm,
  type Changes,
  type Path,
} from "./values-form";

/* Installing an app the search found, or changing one already installed.
 *
 * Two steps in one native <dialog>. The first is the app's properties and
 * who it runs as; the second asks once more, plainly, whether to install
 * someone else's code on this box, and says what that means. Nothing is
 * sent until the second step's button. */

export interface InstallTarget {
  /** The name the search showed. */
  title: string;
  /** Who published it, from the search row. */
  publisher: string;
  source: AppSource;
  /** Set when this changes an app already installed. */
  existing: InstalledApp | null;
}

export interface InstallDialogProps {
  target: InstallTarget | null;
  sharedAvailable: boolean;
  onOpenChange: (open: boolean) => void;
  onInstall: (request: AppInstallRequest) => Promise<InstalledApp>;
}

type Loaded =
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; chart: AppChartResponse };

const RELEASE_PATTERN = /^[a-z]([-a-z0-9]{0,38}[a-z0-9])?$/;

function valueAt(values: unknown, path: Path): unknown {
  let at = values;
  for (const part of path) {
    if (typeof at !== "object" || at === null || Array.isArray(at)) return undefined;
    at = (at as Record<string, unknown>)[part];
  }
  return at;
}

export function InstallDialog({ target, sharedAvailable, onOpenChange, onInstall }: InstallDialogProps) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const nameId = React.useId();
  const yamlId = React.useId();

  const [loaded, setLoaded] = React.useState<Loaded>({ kind: "loading" });
  const [attempt, setAttempt] = React.useState(0);
  const [step, setStep] = React.useState<"form" | "confirm">("form");
  const [release, setRelease] = React.useState("");
  const [runAs, setRunAs] = React.useState<AppRunAs>("notshared");
  const [changes, setChanges] = React.useState<Changes>(new Map());
  const [yaml, setYaml] = React.useState("");
  const [tab, setTab] = React.useState("properties");
  const [sending, setSending] = React.useState(false);
  const [refusal, setRefusal] = React.useState<string | null>(null);

  const open = target !== null;
  const existing = target?.existing ?? null;

  // A new target starts the dialog over: its own fetch, its own defaults.
  React.useEffect(() => {
    if (target === null) return;
    setStep("form");
    setRefusal(null);
    setTab("properties");
    setRunAs(target.existing?.runAs ?? "notshared");
    setChanges(valuesToChanges(target.existing?.values ?? {}));
    setLoaded({ kind: "loading" });
    const controller = new AbortController();
    void (async () => {
      try {
        const chart = await getAppChart(target.source, { signal: controller.signal });
        if (controller.signal.aborted) return;
        setLoaded({ kind: "ready", chart });
        setRelease(target.existing?.release ?? chart.release);
        setYaml(target.existing?.valuesYaml ?? chart.valuesYaml);
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        const said = error instanceof Error ? error.message : "";
        setLoaded({ kind: "failed", message: said.length > 0 ? said : translate("install.failedFallback") });
      }
    })();
    return () => controller.abort();
  }, [target, attempt]);

  const chart = loaded.kind === "ready" ? loaded.chart : null;
  const releaseValid = RELEASE_PATTERN.test(release);
  const yamlEdited = chart !== null && yaml !== chart.valuesYaml;
  const changedCount = changes.size + (yamlEdited ? 1 : 0);

  const setValue = (path: Path, value: unknown) => {
    setChanges((old) => {
      const next = new Map(old);
      // Back to the default is no change at all.
      if (JSON.stringify(value) === JSON.stringify(valueAt(chart?.values, path))) next.delete(pathKey(path));
      else next.set(pathKey(path), value);
      return next;
    });
  };
  const resetValue = (path: Path) =>
    setChanges((old) => {
      const next = new Map(old);
      next.delete(pathKey(path));
      return next;
    });

  const submit = async () => {
    if (target === null || chart === null) return;
    setSending(true);
    setRefusal(null);
    try {
      await onInstall({
        release,
        chart: target.source,
        runAs,
        values: changesToValues(changes),
        valuesYaml: yamlEdited ? yaml : null,
      });
      onOpenChange(false);
      toast.success(
        translate(existing === null ? "install.startedTitle" : "install.changingTitle", { name: release }),
        translate("install.startedBody"),
      );
    } catch (error) {
      if (isUnauthorized(error) || isAbort(error)) return;
      const said = error instanceof Error ? error.message : "";
      setRefusal(said.length > 0 ? said : translate("install.failedFallback"));
    } finally {
      setSending(false);
    }
  };

  const heading = target === null ? "" : existing === null ? t("install.title", { name: target.title }) : t("install.changeTitle", { name: existing.release });

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      dismissible={!sending}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(40rem,calc(100vw-2rem))]"
    >
      {target !== null && step === "form" && (
        <>
          <DialogHeader>
            <DialogTitle id={titleId}>{heading}</DialogTitle>
            <DialogDescription id={bodyId}>
              {t("install.byline", { version: target.source.version, publisher: target.publisher })}
            </DialogDescription>
          </DialogHeader>
          <DialogBody className="flex flex-col gap-5">
            {loaded.kind === "loading" && (
              <div className="flex items-center gap-2.5 py-6 text-[13px] text-muted">
                <Spinner size={16} label={t("install.fetching")} />
                {t("install.fetching")}
              </div>
            )}

            {loaded.kind === "failed" && (
              <Alert variant="crit">
                <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                <AlertDescription>
                  <p>{t("install.fetchFailed", { message: loaded.message })}</p>
                  <Button size="sm" variant="secondary" className="mt-2" onClick={() => setAttempt((n) => n + 1)}>
                    {t("install.retry")}
                  </Button>
                </AlertDescription>
              </Alert>
            )}

            {chart !== null && (
              <>
                <Field>
                  <FieldLabel htmlFor={nameId}>{t("install.name")}</FieldLabel>
                  <MonoInput
                    id={nameId}
                    value={release}
                    maxLength={40}
                    disabled={existing !== null || sending}
                    aria-invalid={!releaseValid || undefined}
                    onChange={(event) => setRelease(event.target.value.toLowerCase())}
                  />
                  <FieldDescription>{t(releaseValid ? "install.nameHint" : "install.nameInvalid")}</FieldDescription>
                </Field>

                <fieldset className="flex flex-col gap-2">
                  <legend className="mb-2 text-sm font-medium text-ink">{t("install.runAs")}</legend>
                  <div className="grid gap-2 sm:grid-cols-2">
                    <RunAsChoice
                      value="notshared"
                      current={runAs}
                      onPick={setRunAs}
                      disabled={sending || (existing !== null && existing.runAs !== "notshared")}
                      title="notshared"
                      detail={t("install.notsharedDetail")}
                    />
                    <RunAsChoice
                      value="shared"
                      current={runAs}
                      onPick={setRunAs}
                      disabled={sending || !sharedAvailable || (existing !== null && existing.runAs !== "shared")}
                      title="shared"
                      detail={sharedAvailable ? t("install.sharedDetail") : t("install.sharedLocked")}
                    />
                  </div>
                  <p className="text-[12px] leading-snug text-muted">
                    {t("install.runAsCaption", { path: `/home/${runAs}/data/apps/${releaseValid ? release : "…"}` })}
                  </p>
                </fieldset>

                <Tabs value={tab} onValueChange={setTab} className="gap-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <TabsList>
                      <TabsTrigger value="properties">{t("install.properties")}</TabsTrigger>
                      <TabsTrigger value="text">{t("install.asText")}</TabsTrigger>
                    </TabsList>
                    {changedCount > 0 && (
                      <span className="text-[12px] text-muted">{t("install.changedCount", { count: changedCount })}</span>
                    )}
                  </div>
                  <TabsContent value="properties">
                    <ValuesForm
                      defaults={chart.values}
                      schema={chart.schema}
                      changes={changes}
                      onChange={setValue}
                      onReset={resetValue}
                      disabled={sending}
                    />
                  </TabsContent>
                  <TabsContent value="text" className="flex flex-col gap-2">
                    <label htmlFor={yamlId} className="sr-only">
                      {t("install.asText")}
                    </label>
                    <Textarea
                      id={yamlId}
                      rows={14}
                      className="font-mono text-[12px] leading-relaxed"
                      spellCheck={false}
                      value={yaml}
                      disabled={sending}
                      onChange={(event) => setYaml(event.target.value)}
                    />
                    <p className="text-[12px] leading-snug text-muted">{t("install.textCaption")}</p>
                    {yamlEdited && (
                      <Button size="xs" variant="ghost" className="self-start" onClick={() => setYaml(chart.valuesYaml)}>
                        {t("install.textReset")}
                      </Button>
                    )}
                  </TabsContent>
                </Tabs>
              </>
            )}
          </DialogBody>
          <DialogFooter>
            <Button variant="ghost" onClick={() => onOpenChange(false)}>
              {t("install.cancel")}
            </Button>
            <Button disabled={chart === null || !releaseValid} onClick={() => setStep("confirm")}>
              {existing === null ? t("install.continue") : t("install.continueChange")}
            </Button>
          </DialogFooter>
        </>
      )}

      {target !== null && step === "confirm" && (
        <>
          <DialogHeader>
            <DialogTitle id={titleId} className="flex items-center gap-2">
              <HugeiconsIcon
                icon={Alert02Icon}
                size={19}
                strokeWidth={1.5}
                color="currentColor"
                className="text-warn"
                aria-hidden="true"
              />
              {existing === null ? t("install.confirmTitle", { name: release }) : t("install.confirmChangeTitle", { name: release })}
            </DialogTitle>
            <DialogDescription id={bodyId}>{t("install.confirmBody", { publisher: target.publisher })}</DialogDescription>
          </DialogHeader>
          <DialogBody className="flex flex-col gap-3">
            <ul className="flex flex-col gap-1.5 text-[13px] leading-snug text-muted">
              <Point dot="bg-warn">{t("install.pointCode", { publisher: target.publisher })}</Point>
              <Point dot="bg-ok">{t("install.pointUser", { user: runAs, path: `/home/${runAs}/data/apps/${release}` })}</Point>
              <Point dot="bg-faint">
                {changedCount === 0 ? t("install.pointDefaults") : t("install.pointChanged", { count: changedCount })}
              </Point>
              <Point dot="bg-faint">{t("install.pointOpen")}</Point>
            </ul>
            {refusal !== null && (
              <Alert variant="crit">
                <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                <AlertDescription>{refusal}</AlertDescription>
              </Alert>
            )}
          </DialogBody>
          <DialogFooter>
            <Button variant="ghost" disabled={sending} onClick={() => setStep("form")}>
              {t("install.back")}
            </Button>
            <Button disabled={sending} onClick={() => void submit()}>
              {sending && <Spinner size={14} label={t("install.sending")} />}
              {existing === null ? t("install.confirm") : t("install.confirmChange")}
            </Button>
          </DialogFooter>
        </>
      )}
    </Dialog>
  );
}

function Point({ dot, children }: { dot: string; children: React.ReactNode }) {
  return (
    <li className="flex gap-2">
      <span aria-hidden="true" className={cn("mt-2 size-1.5 shrink-0 rounded-full", dot)} />
      <span>{children}</span>
    </li>
  );
}

function RunAsChoice({
  value,
  current,
  onPick,
  disabled,
  title,
  detail,
}: {
  value: AppRunAs;
  current: AppRunAs;
  onPick: (value: AppRunAs) => void;
  disabled: boolean;
  title: string;
  detail: string;
}) {
  const id = React.useId();
  const checked = current === value;
  return (
    <label
      htmlFor={id}
      className={cn(
        "flex cursor-pointer gap-2.5 rounded-control border p-3 transition-colors duration-150",
        checked ? "border-accent bg-accent-wash" : "border-line hover:bg-sunk",
        disabled && "cursor-not-allowed opacity-55 hover:bg-transparent",
      )}
    >
      <input
        id={id}
        type="radio"
        name="losos-run-as"
        value={value}
        checked={checked}
        disabled={disabled}
        onChange={() => onPick(value)}
        className="mt-0.5 size-4 shrink-0 accent-[var(--accent)]"
      />
      <span className="flex flex-col gap-0.5">
        <span className="font-mono text-[13px] text-ink">{title}</span>
        <span className="text-[12px] leading-snug text-muted">{detail}</span>
      </span>
    </label>
  );
}
