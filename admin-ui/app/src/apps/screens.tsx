/* The owner's own apps: an app's page, and the editor that makes one.
 *
 * /apps/<id> runs a saved app in its frame (app-frame.tsx), with its data
 * on the box. /apps/new and /apps/<id>/edit are the editor: the model on
 * the box writes the app from a description (widgets/builder-panel.tsx,
 * kind "app"), or the owner writes it in the Code tab, and the preview
 * beside it runs what the box compiled. Every change to the source is sent
 * to POST /api/user-apps/check, which compiles it the way a save would and
 * names what is wrong in plain sentences; those mistakes, and any the app
 * throws while it runs, can go back to the model as a request to fix them.
 * Nothing is kept until Save.
 */

import * as React from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, ArrowLeft01Icon, SparklesIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { CodeEditor } from "@/components/code-editor";
import { FieldError, Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { toast } from "@/components/ui/toast";
import {
  ApiError,
  checkUserApp,
  getUserApp,
  postBuilderTried,
  saveUserApp,
  type BuildView,
  type UserApp,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { BuilderPanel, builtApp } from "@/widgets/builder-panel";
import { AppFrame, type TryResult } from "./app-frame";

/** How long the source rests before the box checks it. */
const CHECK_MS = 500;
/** The box's own limits on a name and a summary (backend/src/user_apps.rs). */
const MAX_NAME_CHARS = 60;
const MAX_SUMMARY_CHARS = 600;

type Loaded = { state: "loading" } | { state: "missing" } | { state: "failed" } | { state: "ready"; app: UserApp };

function useUserApp(id: string | undefined): [Loaded, () => void] {
  const [loaded, setLoaded] = React.useState<Loaded>({ state: "loading" });
  const load = React.useCallback(() => {
    if (id === undefined) return;
    setLoaded({ state: "loading" });
    getUserApp(id)
      .then(({ app }) => setLoaded({ state: "ready", app }))
      .catch((error: unknown) =>
        setLoaded({ state: error instanceof ApiError && error.status === 404 ? "missing" : "failed" }),
      );
  }, [id]);
  React.useEffect(load, [load]);
  return [loaded, load];
}

function BackLink() {
  const t = useT();
  return (
    <Link
      to="/apps"
      className="inline-flex items-center gap-1 text-[13px] text-muted transition-colors hover:text-ink"
    >
      <HugeiconsIcon icon={ArrowLeft01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      {t("userApps.back")}
    </Link>
  );
}

function NotThere({ loaded, retry }: { loaded: Loaded; retry: () => void }) {
  const t = useT();
  if (loaded.state === "loading") return <Spinner label={t("userApps.loading")} className="text-muted" />;
  return (
    <div className="flex flex-col items-start gap-3 text-[13px] text-muted">
      <p>{t(loaded.state === "missing" ? "userApps.notFound" : "userApps.loadFailed")}</p>
      {loaded.state === "failed" && (
        <Button variant="secondary" size="sm" onClick={retry}>
          {t("look.builder.retry")}
        </Button>
      )}
    </div>
  );
}

// ── An app's page ─────────────────────────────────────────────────────────

export function AppScreen() {
  const t = useT();
  const navigate = useNavigate();
  const { id } = useParams();
  const [loaded, retry] = useUserApp(id);
  const [crash, setCrash] = React.useState<string | null>(null);

  if (loaded.state !== "ready") {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        <NotThere loaded={loaded} retry={retry} />
      </div>
    );
  }
  const app = loaded.app;
  return (
    <div className="flex flex-col gap-4" data-testid="user-app">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div className="min-w-0">
          <BackLink />
          <h1 className="mt-1.5 truncate text-[19px] leading-tight font-semibold text-ink">{app.name}</h1>
          {app.summary !== null && app.summary !== "" && (
            <p className="mt-0.5 text-[12.5px] text-muted">{app.summary}</p>
          )}
        </div>
        <Button size="sm" variant="secondary" onClick={() => navigate(`/apps/${app.id}/edit`)}>
          {t("userApps.change")}
        </Button>
      </div>
      {crash !== null && <CrashNote message={crash} />}
      <Card className="h-[max(32rem,calc(100dvh-13rem))] overflow-hidden">
        <AppFrame
          code={app.code}
          appId={app.id}
          name={app.name}
          onError={setCrash}
          onSaveError={(message) => toast.error(t("userApps.saveError", { message }), undefined, { help: "apps" })}
        />
      </Card>
    </div>
  );
}

function CrashNote({ message, onFix }: { message: string; onFix?: () => void }) {
  const t = useT();
  return (
    <Alert variant="warn" role="status" data-testid="user-app-crash">
      <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertDescription>
        <p>{t("userApps.crashed", { message })}</p>
        {onFix !== undefined && (
          <Button size="xs" variant="secondary" className="mt-2" onClick={onFix}>
            <HugeiconsIcon icon={SparklesIcon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("userApps.editor.askFix")}
          </Button>
        )}
      </AlertDescription>
    </Alert>
  );
}

// ── The editor ────────────────────────────────────────────────────────────

export function AppEditorScreen() {
  const { id } = useParams();
  const [loaded, retry] = useUserApp(id);
  if (id === undefined) return <AppEditor />;
  if (loaded.state !== "ready") {
    return (
      <div className="flex flex-col gap-4">
        <BackLink />
        <NotThere loaded={loaded} retry={retry} />
      </div>
    );
  }
  return <AppEditor key={loaded.app.id} app={loaded.app} />;
}

interface Checked {
  /** The source this answer is for. */
  source: string;
  code: string | null;
  problems: string[];
}

function AppEditor({ app }: { app?: UserApp }) {
  const t = useT();
  const navigate = useNavigate();
  const titleId = React.useId();
  const nameId = React.useId();
  const summaryId = React.useId();
  const problemId = React.useId();

  const [tab, setTab] = React.useState("ai");
  const [source, setSource] = React.useState(app?.source ?? "");
  const [name, setName] = React.useState(app?.name ?? "");
  const [summary, setSummary] = React.useState(app?.summary ?? "");
  const [prompt, setPrompt] = React.useState<string | null>(app?.prompt ?? null);
  const [checked, setChecked] = React.useState<Checked | null>(
    app === undefined ? null : { source: app.source, code: app.code, problems: [] },
  );
  /** The last script that compiled, so a slip in the Code tab does not
   *  blank the preview. */
  const [shown, setShown] = React.useState<string | null>(app?.code ?? null);
  const [crash, setCrash] = React.useState<string | null>(null);
  const [building, setBuilding] = React.useState<BuildView | null>(null);
  const [suggest, setSuggest] = React.useState<{ prompt: string; key: number } | null>(null);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  /* The box checks the source once it rests. An answer for an older source
   * is dropped. */
  React.useEffect(() => {
    if (source.trim() === "") {
      setChecked(null);
      return;
    }
    if (checked?.source === source) return;
    let stopped = false;
    const timer = setTimeout(() => {
      checkUserApp(source)
        .then((answer) => {
          if (stopped) return;
          setChecked({ source, ...answer });
          if (answer.code !== null) {
            setShown(answer.code);
            setCrash(null);
          }
        })
        .catch(() => {
          // The next change asks again; Save says what the box thinks.
        });
    }, CHECK_MS);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
    // `checked` is read only to skip a source the box has already answered for.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source]);

  const written = (view: BuildView): void => {
    const built = builtApp(view);
    if (built === null) return;
    setSource(built);
    if (view.code !== null) {
      setChecked({ source: built, code: view.code, problems: view.problems });
      setShown(view.code);
      setCrash(null);
    }
    if (view.name !== null && (name.trim() === "" || app === undefined)) setName(view.name.slice(0, MAX_NAME_CHARS));
    if (view.notes !== null) setSummary(view.notes.slice(0, MAX_SUMMARY_CHARS));
    setPrompt(view.prompt);
    setProblem(null);
  };

  const askFix = (mistake: string): void => {
    setSuggest({ prompt: t("userApps.editor.fixPrompt", { problem: mistake }), key: Date.now() });
    setTab("ai");
  };

  const save = async (): Promise<void> => {
    if (name.trim() === "") {
      setProblem(t("userApps.editor.needName"));
      return;
    }
    if (source.trim() === "") {
      setProblem(t("userApps.editor.needCode"));
      return;
    }
    setProblem(null);
    setBusy(true);
    try {
      const saved = await saveUserApp({
        ...(app !== undefined ? { id: app.id } : {}),
        name: name.trim(),
        summary: summary.trim() === "" ? null : summary.trim(),
        prompt,
        source,
      });
      toast.success(t("userApps.editor.saved", { name: saved.app.name }), t("userApps.editor.savedNote"));
      navigate(`/apps/${saved.app.id}`);
    } catch (error) {
      setProblem(error instanceof Error && error.message.length > 0 ? error.message : t("userApps.editor.notSaved"));
    } finally {
      setBusy(false);
    }
  };

  const running = building?.status === "running";
  /* While the AI works, the preview shows the version it is on, and tries
   * it when the build asks: the box runs no app code, so the page is where
   * an app is tried. */
  const preview = running ? (building?.code ?? null) : null;
  const trial = running ? (building?.trial ?? null) : null;
  const buildId = building?.id;
  const tried = React.useCallback(
    (id: string, result: TryResult) => {
      if (buildId === undefined) return;
      void postBuilderTried(buildId, { trial: id, ...result }).catch(() => {
        // The build carries on untried when the box does not take it.
      });
    },
    [buildId],
  );
  const problems = checked?.source === source ? checked.problems : [];
  const title = app === undefined ? t("userApps.editor.newTitle") : t("userApps.editor.editTitle", { name: app.name });

  return (
    <div className="flex flex-col gap-4" data-testid="app-editor">
      <div>
        <BackLink />
        <h1 id={titleId} className="mt-1.5 text-[19px] leading-tight font-semibold text-ink">
          {title}
        </h1>
      </div>

      <div className="grid items-start gap-4 lg:grid-cols-[minmax(0,25rem)_minmax(0,1fr)]">
        <Card>
          <CardContent className="flex flex-col gap-4 pt-4">
            <Tabs value={tab} onValueChange={setTab}>
              <TabsList>
                <TabsTrigger value="ai">{t("userApps.editor.tab.ai")}</TabsTrigger>
                <TabsTrigger value="code">{t("userApps.editor.tab.code")}</TabsTrigger>
              </TabsList>
              {/* Kept mounted so a build is still followed from the Code tab. */}
              <TabsContent value="ai" keepMounted className="pt-4">
                <BuilderPanel
                  kind="app"
                  source={source}
                  hasApp={source.trim() !== ""}
                  suggest={suggest}
                  onProgress={setBuilding}
                  onWritten={written}
                />
              </TabsContent>
              <TabsContent value="code" className="pt-4">
                <div className="flex flex-col gap-1.5">
                  <p className="text-[12.5px] leading-relaxed text-muted">{t("userApps.editor.codeHint")}</p>
                  <CodeEditor
                    value={source}
                    onChange={setSource}
                    language="jsx"
                    label={t("userApps.editor.codeLabel")}
                    disabled={busy || running}
                    invalid={problems.length > 0}
                    className="h-[28rem]"
                  />
                </div>
              </TabsContent>
            </Tabs>

            <Label htmlFor={nameId} className="flex flex-col gap-1.5">
              <span>{t("userApps.editor.field.name")}</span>
              <Input
                id={nameId}
                value={name}
                maxLength={MAX_NAME_CHARS}
                disabled={busy}
                onChange={(event) => {
                  setName(event.target.value);
                  setProblem(null);
                }}
              />
            </Label>
            <Label htmlFor={summaryId} className="flex flex-col gap-1.5">
              <span>{t("userApps.editor.field.summary")}</span>
              <Input
                id={summaryId}
                value={summary}
                maxLength={MAX_SUMMARY_CHARS}
                disabled={busy}
                onChange={(event) => setSummary(event.target.value)}
              />
            </Label>

            <div className="flex flex-wrap items-center gap-2">
              <Button disabled={busy || running} onClick={() => void save()} data-testid="app-save">
                {busy && <Spinner size={14} label={t("userApps.editor.save")} />}
                {t("userApps.editor.save")}
              </Button>
              <Button variant="ghost" disabled={busy} onClick={() => navigate(app === undefined ? "/apps" : `/apps/${app.id}`)}>
                {t("userApps.editor.cancel")}
              </Button>
            </div>
            <FieldError id={problemId}>{problem}</FieldError>
          </CardContent>
        </Card>

        <div className="flex flex-col gap-3">
          {problems.length > 0 && (
            <Alert variant="warn" role="status" data-testid="app-problems">
              <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              <AlertTitle>{t("userApps.editor.problems")}</AlertTitle>
              <AlertDescription>
                <ul className="list-disc pl-4">
                  {problems.map((p) => (
                    <li key={p}>{p}</li>
                  ))}
                </ul>
                <Button size="xs" variant="secondary" className="mt-2" disabled={running} onClick={() => askFix(problems[0] ?? "")}>
                  <HugeiconsIcon icon={SparklesIcon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  {t("userApps.editor.askFix")}
                </Button>
              </AlertDescription>
            </Alert>
          )}
          {crash !== null && problems.length === 0 && (
            <CrashNote message={crash} onFix={running ? undefined : () => askFix(crash)} />
          )}
          <Card className="overflow-hidden" data-testid="app-preview">
            <CardHeader className="flex-row items-center justify-between pb-3">
              <CardTitle>{t("userApps.editor.preview")}</CardTitle>
            </CardHeader>
            <div className={cn("h-[38rem] border-t border-hair", (preview ?? shown) === null && "grid place-items-center")}>
              {(preview ?? shown) === null ? (
                <div className="flex max-w-xs flex-col items-center gap-3 px-6 text-center text-[13px] text-muted">
                  {running ? (
                    <>
                      <Spinner label={t("look.builder.runningShort")} />
                      <p>
                        {building !== null && building.lines > 0
                          ? t("userApps.builder.writing", { lines: building.lines })
                          : t("look.builder.reading")}
                      </p>
                    </>
                  ) : (
                    <p>{t("userApps.editor.previewEmpty")}</p>
                  )}
                </div>
              ) : (
                <AppFrame
                  code={(preview ?? shown) as string}
                  name={name || t("userApps.editor.preview")}
                  onError={running ? undefined : setCrash}
                  trial={trial?.id ?? null}
                  onTried={tried}
                />
              )}
            </div>
            <p className="border-t border-hair px-5 py-2.5 text-[12px] text-muted">{t("userApps.editor.previewNote")}</p>
          </Card>
        </div>
      </div>
    </div>
  );
}
