import * as React from "react";
import { BrowserRouter, Link, Navigate, Route, Routes, useParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon, ArrowLeft01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { MonoInput } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import { Toaster } from "@/components/ui/toast";
import {
  ApiError,
  dropToken,
  getEdgeStatistics,
  getManagedBoxes,
  getToken,
  isAbort,
  isWellFormedToken,
  routeThrough,
  saveToken,
  subscribeAuth,
  type ManagedBox,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { cn } from "@/lib/utils";
import { ApplyBar } from "@/screens/settings/apply-bar";
import { BoxApps } from "@/screens/settings/pane-apps";
import {
  Group,
  GroupCaption,
  GroupTitle,
  PaneHeader,
  PaneSection,
  Row,
  RowText,
  StackRow,
} from "@/screens/settings/rows";
import { SiteSettings } from "@/screens/settings/site-settings";
import { useStatistics } from "@/statistics/use-statistics";
import { StatisticsView } from "@/statistics/view";
import { StatisticsWidget } from "@/statistics/widget";
import { useSiteForm } from "./use-site-form";

/* A company edge's page for the boxes behind it (served by the edge at
 * /manage/, edge/registrar/src/server/manage.rs). Signed in with the edge's
 * manage key, it lists the boxes the edge routes and opens one whose owner
 * switched on "Let this edge manage the box" on its Mesh page. A box's page
 * is the Apps pane's own parts, with every /api/ call relayed into that box
 * (lib/api.ts routeThrough). The box decides what the edge may call: LosOS
 * cloud, LosOS Git and the apps, nothing of the owner's. Above the boxes, the
 * Statistics card opens /manage/statistics: what the edge's Traefik counted
 * (edge/registrar/src/stats.rs). */

export function ManageRoot() {
  return (
    <BrowserRouter basename="/manage">
      <Shell />
      <Toaster />
    </BrowserRouter>
  );
}

function useSignedIn(): boolean {
  return React.useSyncExternalStore(subscribeAuth, () => getToken() !== null);
}

function Shell() {
  const t = useT();
  const signedIn = useSignedIn();
  const logo = logoFor();
  return (
    <div className="losos-shell min-h-dvh text-ink">
      <header className="border-b border-line bg-surface/80">
        <div className="mx-auto flex max-w-3xl items-center gap-3 px-4 py-3 sm:px-6">
          <img src={logo.url} alt={t(logo.tip)} className="size-7" />
          <p className="flex-1 text-[15px] font-semibold tracking-tight">{t("manage.title")}</p>
          {signedIn && (
            <Button size="sm" variant="ghost" onClick={dropToken}>
              {t("manage.signOut")}
            </Button>
          )}
        </div>
      </header>
      <main className="mx-auto max-w-3xl px-4 pt-6 pb-12 sm:px-6">
        {signedIn ? (
          <Routes>
            <Route path="/" element={<BoxList />} />
            <Route path="/boxes/:id" element={<BoxView />} />
            <Route path="/statistics" element={<EdgeStatistics />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Routes>
        ) : (
          <KeyForm />
        )}
      </main>
    </div>
  );
}

// ── The key ───────────────────────────────────────────────────────────────

function KeyForm() {
  const t = useT();
  const id = React.useId();
  const [key, setKey] = React.useState("");
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const candidate = key.trim();
    if (!isWellFormedToken(candidate)) {
      setProblem(t("manage.key.shape"));
      return;
    }
    setBusy(true);
    try {
      await getManagedBoxes({ token: candidate });
      saveToken(candidate);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) setProblem(t("manage.key.wrong"));
      else if (error instanceof ApiError && error.status === 404) setProblem(t("manage.key.off"));
      else setProblem(t("manage.key.failed", { message: error instanceof Error ? error.message : "" }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={(event) => void submit(event)} noValidate>
      <PaneHeader title={t("manage.key.title")} summary={t("manage.key.summary")} />
      <Group>
        <StackRow last>
          <label htmlFor={id} className="mb-1.5 block text-[13px] font-medium">
            {t("manage.key.label")}
          </label>
          <div className="flex flex-wrap gap-2">
            <MonoInput
              id={id}
              value={key}
              autoComplete="off"
              spellCheck={false}
              aria-invalid={problem !== null}
              onChange={(event) => {
                setKey(event.target.value);
                setProblem(null);
              }}
              className="min-w-0 flex-1"
            />
            <Button type="submit" disabled={busy || key.trim() === ""}>
              {busy && <Spinner size={14} label={t("manage.key.submit")} />}
              {t("manage.key.submit")}
            </Button>
          </div>
          {problem !== null && (
            <p role="alert" className="mt-2 text-[12.5px] leading-snug text-crit">
              {problem}
            </p>
          )}
        </StackRow>
      </Group>
    </form>
  );
}

// ── The boxes ─────────────────────────────────────────────────────────────

type ListState =
  | { kind: "loading" }
  | { kind: "failed"; message: string }
  | { kind: "ready"; boxes: ManagedBox[] };

function BoxList() {
  const t = useT();
  const [state, setState] = React.useState<ListState>({ kind: "loading" });
  const [round, setRound] = React.useState(0);

  React.useEffect(() => {
    const controller = new AbortController();
    const load = () =>
      getManagedBoxes({ signal: controller.signal })
        .then(({ boxes }) => setState({ kind: "ready", boxes }))
        .catch((error: unknown) => {
          if (isAbort(error)) return;
          setState({ kind: "failed", message: error instanceof Error ? error.message : "" });
        });
    void load();
    // A box's switch arrives with its next heartbeat; keep the list current.
    const timer = window.setInterval(() => void load(), 10_000);
    return () => {
      window.clearInterval(timer);
      controller.abort();
    };
  }, [round]);

  const statistics = useStatistics(loadStatistics);

  return (
    <>
      <PaneSection>
        <StatisticsWidget state={statistics} to="/statistics" />
      </PaneSection>
      <PaneHeader title={t("manage.boxes.title")} summary={t("manage.boxes.summary")} />
      <PaneSection>
        <Group>
          {state.kind === "loading" && (
            <StackRow last>
              <div className="flex items-center gap-2.5 text-[13px] text-muted">
                <Spinner size={16} label={t("manage.boxes.loading")} />
                {t("manage.boxes.loading")}
              </div>
            </StackRow>
          )}
          {state.kind === "failed" && (
            <StackRow last>
              <Alert variant="crit">
                <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                <AlertDescription>
                  <p>{t("manage.boxes.failed", { message: state.message })}</p>
                  <Button size="sm" variant="secondary" className="mt-2" onClick={() => setRound((n) => n + 1)}>
                    {t("manage.boxes.retry")}
                  </Button>
                </AlertDescription>
              </Alert>
            </StackRow>
          )}
          {state.kind === "ready" && state.boxes.length === 0 && (
            <StackRow last>
              <p className="text-[13px] text-muted">{t("manage.boxes.empty")}</p>
            </StackRow>
          )}
          {state.kind === "ready" &&
            state.boxes.map((box, index) => (
              <BoxRow key={box.id} box={box} last={index === state.boxes.length - 1} />
            ))}
        </Group>
      </PaneSection>
    </>
  );
}

/** Stable, so `useStatistics` does not start over on every render. */
function loadStatistics(signal: AbortSignal): Promise<unknown> {
  return getEdgeStatistics({ signal });
}

function seenAgo(t: ReturnType<typeof useT>, secs: number): string {
  if (secs < 60) return t("manage.boxes.justNow");
  return t("manage.boxes.minutes", { count: Math.floor(secs / 60) });
}

function BoxRow({ box, last }: { box: ManagedBox; last: boolean }) {
  const t = useT();
  const detail = [
    box.hostname !== box.id ? box.id : null,
    t("manage.boxes.seen", { ago: seenAgo(t, box.seenAgoSecs) }),
    box.managed ? null : t("manage.boxes.notManagedDetail"),
  ]
    .filter((part) => part !== null)
    .join(" · ");
  return (
    <Row last={last} data-testid={`box-${box.id}`}>
      <RowText title={box.hostname} detail={detail} />
      <div className="flex shrink-0 items-center gap-2">
        <Badge variant={box.managed ? "ok" : "neutral"}>
          {box.managed ? t("manage.boxes.managed") : t("manage.boxes.notManaged")}
        </Badge>
        {box.managed && (
          <Link
            to={`/boxes/${encodeURIComponent(box.id)}`}
            className={cn(
              "inline-flex items-center rounded-control px-2.5 py-1 text-[13px] font-medium",
              "bg-accent text-surface transition-opacity duration-150 hover:opacity-90",
            )}
          >
            {t("manage.boxes.open")}
          </Link>
        )}
      </div>
    </Row>
  );
}

// ── Statistics ────────────────────────────────────────────────────────────

function EdgeStatistics() {
  const t = useT();
  const statistics = useStatistics(loadStatistics);
  return (
    <>
      <BackToBoxes />
      <PaneHeader title={t("statistics.title")} summary={t("manage.statistics.summary")} />
      <StatisticsView state={statistics} />
    </>
  );
}

function BackToBoxes() {
  const t = useT();
  return (
    <Link
      to="/"
      className="mb-4 inline-flex items-center gap-1.5 text-[13px] text-accent hover:underline"
    >
      <HugeiconsIcon icon={ArrowLeft01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      {t("manage.box.back")}
    </Link>
  );
}

// ── One box ───────────────────────────────────────────────────────────────

function BoxView() {
  const t = useT();
  const { id = "" } = useParams();
  /* Every /api/ call below goes into this box through the edge. The route is
   * set before the parts mount: children run their effects before a
   * parent's, so they wait for `routed`. */
  const [routed, setRouted] = React.useState<string | null>(null);
  React.useLayoutEffect(() => {
    routeThrough(`/manage/api/boxes/${encodeURIComponent(id)}`);
    setRouted(id);
    return () => {
      routeThrough(null);
      setRouted(null);
    };
  }, [id]);

  return (
    <>
      <BackToBoxes />
      <PaneHeader title={id} summary={t("manage.box.summary")} />
      {routed === id && <BoxParts />}
    </>
  );
}

function BoxParts() {
  const t = useT();
  const form = useSiteForm();
  return (
    <>
      {form.failed !== null ? (
        <PaneSection>
          <Alert variant="crit">
            <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <AlertDescription>
              <p>{form.refused ? t("manage.box.refused") : t("manage.site.failed", { message: form.failed })}</p>
              <Button size="sm" variant="secondary" className="mt-2" onClick={form.reload}>
                {t("manage.boxes.retry")}
              </Button>
            </AlertDescription>
          </Alert>
        </PaneSection>
      ) : form.options === null ? (
        <PaneSection>
          <div className="flex items-center gap-2.5 text-[13px] text-muted">
            <Spinner size={16} label={t("manage.site.loading")} />
            {t("manage.site.loading")}
          </div>
        </PaneSection>
      ) : form.options.options.length === 0 ? (
        <PaneSection>
          <GroupTitle>LosOS Cloud · LosOS Git</GroupTitle>
          <GroupCaption>{t("manage.site.none")}</GroupCaption>
        </PaneSection>
      ) : (
        <>
          <SiteSettings form={form} app="cloud" title="LosOS Cloud" />
          <SiteSettings form={form} app="git" title="LosOS Git" />
        </>
      )}

      <BoxApps active={form.failed === null} disabled={form.applying} onBox={false} />

      <ApplyBar form={form} />
    </>
  );
}
