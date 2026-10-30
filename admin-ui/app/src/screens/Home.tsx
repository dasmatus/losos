import * as React from "react";
import { AppGrid } from "@/components/AppGrid";
import { useWaitGame } from "@/components/play-while-waiting";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { HelpLink } from "@/components/ui/help-link";
import { StatusDot, type DotState } from "@/components/ui/badge";
import { Card, CardContent, CardFooter } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Separator } from "@/components/ui/separator";
import { DisplayMenu } from "@/components/preferences-menu";
import {
  getHealth,
  getSettings,
  getStorage,
  getState,
  createStatusPoller,
  hasToken,
  isAbort,
  subscribeAuth,
  IDLE_STATUS_PERIOD_MS,
  STATUS_PERIOD_MS,
  type SettingsResponse,
  type StateResponse,
  type StatusResponse,
} from "@/lib/api";
import {
  deriveApps,
  formatBytes,
  CODE_BASE,
  formatUptime,
  probeApps,
  recallAvailability,
  rememberAvailability,
  type AppAttention,
  type AppFacts,
  type AppId,
  type Availabilities,
} from "@/lib/apps";
import { t as translate, type MessageKey } from "@/lib/i18n";
import { Rich, useT } from "@/lib/i18n-react";
import { cn, setCssVar } from "@/lib/utils";
import { Page, type BlockMap } from "@/pages/page";

/* The homepage — the grid of apps, and the two facts worth a glance above and
 * below it.
 *
 * This is the page a browser opens at the box's name, so the order it does
 * things matters more than on any other screen:
 *
 *   1. It paints. The name comes from the address bar, the grid comes from
 *      what this browser last saw (lib/apps.ts), and neither costs a request.
 *   2. It checks that the box is answering, and measures which apps are
 *      actually there. No admin key is needed for either — both are public,
 *      same-origin routes.
 *   3. Once the key has been pasted in, it fills in the box's real name, its
 *      storage mode, and whether a change is being applied.
 *
 * Nothing here waits for step 3 before drawing something useful, because most
 * visits to this page are somebody going to their photos.
 */

// ── Props ─────────────────────────────────────────────────────────────────

/** Measured storage, when something has measured it. See {@link HomeProps}. */
export interface HomeStorage {
  /** Room in use on this box. */
  usedBytes: number;
  /** Room this box has in total. */
  totalBytes: number;
  /** Room lent to other boxes. Only meaningful while sharing is on. */
  lentBytes?: number;
}

export interface HomeProps {
  /* The widget board, which another module owns. Home draws the slot and the
   * heading; it never draws a widget. Pass <HomeWidgetPlaceholder /> during
   * integration to see where the slot sits. */
  widgets?: React.ReactNode;
  /* The app shell has the theme and the language in its person menu. This
   * page is also reachable as a bare homepage with no shell around it, and
   * then they have to be here, in a menu of their own; the shell turns it
   * off. */
  showThemeSwitch?: boolean;
  /* How long the box has been up, in seconds.
   *
   * No route reports this: backend/schema.json has eight of them and not one
   * carries an uptime or a boot time. Rather than print a number nothing
   * measured, the header simply says whether the box is answering until
   * something hands it a real figure — the widget layer's `uptime.days`
   * metric is the intended source. */
  uptimeSeconds?: number | null;
  /* Measured storage, from the same place. Without it the strip at the foot
   * says where things are and claims no amounts, which is all this page can
   * honestly say today. */
  storage?: HomeStorage | null;
}

// ── The screen ────────────────────────────────────────────────────────────

export function Home({
  widgets,
  showThemeSwitch = true,
  uptimeSeconds = null,
  storage = null,
}: HomeProps) {
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);
  const health = useHealth();
  const { settings, boxState, status, measured } = useBoxFacts(signedIn);
  const availability = useAvailability();
  const disk = storage ?? measured;

  const assigned = useAssignedIssues(signedIn && availability.seen.code === "present");
  const facts = buildFacts({ availability, status, storage: disk, assigned });
  const model = deriveApps(facts);

  const hostName = settings?.hostName ?? browserHostName();
  const sharing = boxState?.sharing ?? settings?.sharingMyStorage ?? false;
  /* The switch alone lends nothing: room goes to the mesh only once the box
   * has joined one. Without this the strip said "lent to the mesh" on a box
   * whose About pane said "On the mesh: not joined". */
  const lending: Lending = !sharing ? "off" : settings?.clusterEnable ? "lent" : "waiting";
  const building = status !== null && status.state === "building";
  const game = useWaitGame(building);

  /* The page is src/pages/overview.mdx, which places the blocks below; an
   * administrator may place them otherwise (src/pages/README.md). Each
   * block reads what it shows from here. */
  const overview: OverviewFacts = {
    hostName,
    uptimeSeconds,
    health,
    showThemeSwitch,
    status: building ? status : null,
    game,
    model,
    lending,
    disk,
    widgets,
  };

  return (
    <div className="flex flex-col gap-4">
      <OverviewContext.Provider value={overview}>
        <Page id="overview" blocks={OVERVIEW_BLOCKS} />
      </OverviewContext.Provider>
    </div>
  );
}

// ── The page's blocks (src/pages/blocks.json) ─────────────────────────────

interface OverviewFacts {
  hostName: string;
  uptimeSeconds: number | null;
  health: Health;
  showThemeSwitch: boolean;
  /** The change being applied, while one is. */
  status: StatusResponse | null;
  game: ReturnType<typeof useWaitGame>;
  model: ReturnType<typeof deriveApps>;
  lending: Lending;
  disk: HomeStorage | null;
  widgets: React.ReactNode | undefined;
}

const OverviewContext = React.createContext<OverviewFacts | null>(null);

function useOverview(): OverviewFacts {
  const facts = React.useContext(OverviewContext);
  if (facts === null) throw new Error("an overview block outside the overview");
  return facts;
}

const OVERVIEW_BLOCKS: BlockMap<"overview"> = {
  BoxName() {
    const o = useOverview();
    return (
      <HomeHeader
        hostName={o.hostName}
        uptimeSeconds={o.uptimeSeconds}
        health={o.health}
        showThemeSwitch={o.showThemeSwitch}
      />
    );
  },
  Applying() {
    const o = useOverview();
    return (
      <>
        {o.status !== null && <ApplyingChange status={o.status} offer={o.game.offer} />}
        {o.game.panel}
      </>
    );
  },
  Apps({ props }) {
    const o = useOverview();
    return (
      <Card className="overflow-hidden">
        <CardContent className="pt-5">
          <AppGrid model={o.model} />
        </CardContent>

        {props["hideStorage"] !== true && (
          <CardFooter className="bg-sunk/40">
            <StorageStrip lending={o.lending} storage={o.disk} />
          </CardFooter>
        )}
      </Card>
    );
  },
  Glance() {
    const t = useT();
    const o = useOverview();
    if (o.widgets === undefined) return null;
    return (
      <section aria-label={t("home.glance.label")} className="flex flex-col gap-3">
        {o.widgets}
      </section>
    );
  },
};

export default Home;

// ── Header ────────────────────────────────────────────────────────────────

type Health = "unknown" | "ok" | "down";

function HomeHeader({
  hostName,
  uptimeSeconds,
  health,
  showThemeSwitch,
}: {
  hostName: string;
  uptimeSeconds: number | null;
  health: Health;
  showThemeSwitch: boolean;
}) {
  const t = useT();
  return (
    <header className="flex flex-wrap items-center gap-x-4 gap-y-2 animate-fade-in">
      <div className="min-w-0 flex-1">
        <h1 className="numeric truncate text-[19px] leading-tight font-semibold text-ink">
          {hostName}
        </h1>
        <p className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[12.5px] text-muted">
          <span className="inline-flex items-center gap-1.5">
            <StatusDot state={healthDot(health)} />
            {t(healthWord(health))}
          </span>
          {health === "down" && <HelpLink entry="cannot-reach-the-box" />}

          {uptimeSeconds !== null && uptimeSeconds >= 0 && (
            <>
              <Separator orientation="vertical" className="h-3" />
              <span>
                <Rich
                  k="home.header.up"
                  vars={{ uptime: <span className="numeric">{formatUptime(uptimeSeconds)}</span> }}
                />
              </span>
            </>
          )}
        </p>
      </div>

      {showThemeSwitch && <DisplayMenu />}
    </header>
  );
}

function healthDot(health: Health): DotState {
  if (health === "ok") return "ok";
  if (health === "down") return "crit";
  return "pending";
}

function healthWord(health: Health): MessageKey {
  if (health === "ok") return "home.header.answering";
  if (health === "down") return "home.header.notAnswering";
  return "home.header.checking";
}

/* The name in the address bar, which is the name this browser reached the box
 * by and therefore a true answer even before anything has replied. Replaced by
 * the box's own name the moment the settings call lands. */
function browserHostName(): string {
  try {
    return window.location.hostname === ""
      ? translate("home.header.thisBox")
      : window.location.hostname;
  } catch {
    return translate("home.header.thisBox");
  }
}

// ── A change being applied ────────────────────────────────────────────────

/* lososd reports 0 for the whole of a rebuild and 100 on success, so this is
 * the indeterminate band in practice — which is honest: the box knows a
 * rebuild is running and does not know how far along it is. */
function ApplyingChange({ status, offer }: { status: StatusResponse; offer: React.ReactNode }) {
  const t = useT();
  return (
    <Alert className="animate-rise border-line bg-surface shadow-card">
      <AlertTitle className="flex items-center gap-2">
        <StatusDot state="pending" />
        <span className="flex-1">{t("home.applying.title")}</span>
        {offer}
      </AlertTitle>
      <AlertDescription>
        {status.message !== "" && (
          <p className="w-full truncate text-[12px] text-muted">{status.message}</p>
        )}
        <Progress className="mt-1.5" value={null} label={t("home.applying.short")} />
      </AlertDescription>
    </Alert>
  );
}

// ── Where things are ──────────────────────────────────────────────────────

/* One line at the foot of the grid: everything the owner keeps is on this box,
 * and — when this box is sharing — some of the room it is not using is lent to
 * other boxes.
 *
 * Filled is this box, hatched is the mesh, and they are the same accent. That
 * is the whole colour vocabulary: ok / warn / crit say how something is doing
 * and never where it lives.
 *
 * The widths are only a measurement when there is one. With `storage` the two
 * segments are real fractions, set through the CSSOM because a percentage is
 * not something a class can express and `style=""` is refused by the CSP.
 * Without it the lent segment is a fixed-width cap on a fluid bar — a marker
 * that cannot be read as a proportion, because it does not change with the
 * bar's width. Nothing here ever prints an amount it did not measure.
 */
type Lending = "off" | "waiting" | "lent";

function StorageStrip({ lending, storage }: { lending: Lending; storage: HomeStorage | null }) {
  const t = useT();
  const sharing = lending === "lent";
  const measured = readStorage(storage, sharing);
  const usedPercent = measured === null ? null : measured.usedPercent;
  const lentPercent = measured === null ? null : measured.lentPercent;

  /* Ref callbacks rather than an effect over a ref, and the difference is a
   * bug that only shows up on some loads.
   *
   * The lent segment does not exist until the box says it is sharing, which
   * is a request away. An effect runs once after the first commit, when that
   * element is usually still absent — so `--losos-lent` was set on nothing,
   * the element mounted afterwards at the declared fallback of 0%, and the
   * mesh half of the bar silently vanished. Whether it did depended on
   * whether the response beat the effect flush, which is why it rendered
   * correctly on one load and not the next. A ref callback applies the value
   * whenever the node attaches, in any order, and React re-runs it only when
   * the percentage actually changes. */
  const fillRef = React.useCallback(
    (element: HTMLDivElement | null) => {
      if (element !== null && usedPercent !== null) {
        setCssVar(element, "--losos-fill", `${usedPercent}%`);
      }
    },
    [usedPercent],
  );

  const lentRef = React.useCallback(
    (element: HTMLDivElement | null) => {
      if (element !== null && lentPercent !== null) {
        setCssVar(element, "--losos-lent", `${lentPercent}%`);
      }
    },
    [lentPercent],
  );

  return (
    <div className="flex w-full min-w-0 flex-col gap-2">
      <div className="flex h-2.5 w-full overflow-hidden rounded-full bg-sunk" aria-hidden="true">
        <div
          ref={fillRef}
          className={cn(
            "h-full bg-accent transition-[width] duration-500 ease-out",
            measured === null ? "flex-1" : "w-[var(--losos-fill,0%)]",
          )}
        />
        {sharing && (
          <div
            ref={lentRef}
            className={cn(
              "hatched-solid h-full transition-[width] duration-500 ease-out",
              measured === null ? "w-14 shrink-0" : "w-[var(--losos-lent,0%)]",
            )}
          />
        )}
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[12px]">
        <span className="inline-flex items-center gap-1.5 text-muted">
          <span className="inline-block size-2.5 shrink-0 rounded-[3px] bg-accent" aria-hidden="true" />
          {t("home.storage.onThisBox")}
          {measured !== null && (
            <span className="numeric text-faint">{formatBytes(measured.usedBytes)}</span>
          )}
        </span>

        {sharing && (
          <span className="inline-flex items-center gap-1.5 text-muted">
            <span
              className="hatched-solid inline-block size-2.5 shrink-0 rounded-[3px]"
              aria-hidden="true"
            />
            {t("home.storage.lent")}
            {measured !== null && measured.lentBytes !== null && (
              <span className="numeric text-faint">{formatBytes(measured.lentBytes)}</span>
            )}
          </span>
        )}

        {/* The empty tail of the bar, named. Without this the track just stops
            somewhere and nothing says what the rest of it is. */}
        {measured !== null && measured.freeBytes > 0 && (
          <span className="inline-flex items-center gap-1.5 text-muted">
            <span
              className="inline-block size-2.5 shrink-0 rounded-[3px] bg-sunk ring-1 ring-line ring-inset"
              aria-hidden="true"
            />
            {t("home.storage.roomLeft")}
            <span className="numeric text-faint">{formatBytes(measured.freeBytes)}</span>
          </span>
        )}
      </div>

      <p className="text-[12px] text-faint">
        {lending === "lent"
          ? t("home.storage.sharing")
          : lending === "waiting"
            ? t("home.storage.waiting")
            : t("home.storage.local")}
      </p>
    </div>
  );
}

interface MeasuredStorage {
  usedBytes: number;
  lentBytes: number | null;
  freeBytes: number;
  usedPercent: number;
  lentPercent: number;
}

/* Reduce the measured figures to two percentages that fit in one bar.
 *
 * The lent segment is drawn after the fill, so the two have to add up to at
 * most 100 or the second one is clipped by the track's overflow and silently
 * misreports itself. Anything that does not parse as a real, positive total
 * gives null, and the bar falls back to the shape that claims nothing. */
function readStorage(storage: HomeStorage | null, sharing: boolean): MeasuredStorage | null {
  if (storage === null) return null;
  const { usedBytes, totalBytes } = storage;
  if (!Number.isFinite(usedBytes) || !Number.isFinite(totalBytes)) return null;
  if (totalBytes <= 0 || usedBytes < 0) return null;

  const lentBytes = sharing && typeof storage.lentBytes === "number" ? storage.lentBytes : null;
  const usedPercent = clampPercent((usedBytes / totalBytes) * 100);
  const rawLent = lentBytes === null || lentBytes < 0 ? 0 : (lentBytes / totalBytes) * 100;

  return {
    usedBytes,
    lentBytes,
    freeBytes: Math.max(0, totalBytes - usedBytes - (lentBytes ?? 0)),
    usedPercent,
    lentPercent: clampPercent(Math.min(rawLent, 100 - usedPercent)),
  };
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.round(Math.min(100, Math.max(0, value)) * 10) / 10;
}

// ── The widget slot ───────────────────────────────────────────────────────

// ── Facts ─────────────────────────────────────────────────────────────────

/* Everything the grid is told, assembled in one place. Its notes are
 * translated, so it runs during Home's render, which used `useT()`.
 *
 * The two rules this function exists to keep: a second line is a measurement
 * or it is the catalogue's static sentence, never a guess; and an attention
 * dot is a state the box actually reported, never a hunch. */
function buildFacts({
  availability,
  status,
  storage,
  assigned,
}: {
  availability: { seen: Availabilities; measured: boolean };
  status: StatusResponse | null;
  storage: HomeStorage | null;
  /** Open issues given to the person, from LosOS Git's tracker. */
  assigned?: number | null;
}): AppFacts {
  const details: Partial<Record<AppId, string>> = {};
  const attention: Partial<Record<AppId, AppAttention>> = {};
  if (assigned !== undefined && assigned !== null && assigned > 0) details.issues = translate("apps.issues.assigned", { count: assigned });

  /* The reading is the whole disk, the system included, so it marks the
   * Files tile only when the disk is filling up and puts no size under it:
   * that would read as the size of the owner's files. */
  if (storage !== null && Number.isFinite(storage.usedBytes) && storage.usedBytes >= 0) {
    const total = storage.totalBytes;
    if (Number.isFinite(total) && total > 0) {
      const ratio = storage.usedBytes / total;
      if (ratio >= 0.95) attention.files = { level: "crit", note: translate("home.attention.outOfRoom") };
      else if (ratio >= 0.9) attention.files = { level: "warn", note: translate("home.attention.almostFull") };
    }
  }

  if (status !== null) {
    if (status.state === "building") {
      attention.settings = { level: "pending", note: translate("home.applying.short") };
    } else if (status.state === "failed") {
      attention.settings = { level: "crit", note: translate("home.attention.changeFailed") };
    }
  }

  return {
    files: availability.seen.files,
    code: availability.seen.code,
    measured: availability.measured,
    details,
    attention,
  };
}

/* How many open issues LosOS Git's tracker has given the person, for the
 * line under the Issues tile. Asked of LosOS Git as whoever is signed in
 * through LosOS Login; anything else (the spare key, no tracker) is no
 * line at all. */
function useAssignedIssues(ask: boolean): number | null {
  const [count, setCount] = React.useState<number | null>(null);
  React.useEffect(() => {
    if (!ask) return;
    const controller = new AbortController();
    void fetch(`${CODE_BASE}/api/v1/tracker`, { credentials: "same-origin", headers: { Accept: "application/json" }, signal: controller.signal })
      .then(async (res) => {
        if (!res.ok) return;
        const body = (await res.json()) as { assigned?: unknown };
        if (typeof body.assigned === "number") setCount(body.assigned);
      })
      .catch(() => {});
    return () => controller.abort();
  }, [ask]);
  return count;
}

// ── Is the box answering? ─────────────────────────────────────────────────

/** How often to ask, while the tab is in front of somebody. */
const HEALTH_PERIOD_MS = 60_000;

/* /api/health is the one route that takes no admin key, which is what makes it
 * the right liveness check for a page that renders before anyone has unlocked
 * anything. Skipped entirely while the tab is hidden, and re-run the moment it
 * comes back, so a laptop that was shut for a week does not paint a week-old
 * answer. */
function useHealth(): Health {
  const [health, setHealth] = React.useState<Health>("unknown");

  React.useEffect(() => {
    let live = true;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const controller = new AbortController();

    function schedule(): void {
      if (!live) return;
      timer = setTimeout(() => void check(), HEALTH_PERIOD_MS);
    }

    async function check(): Promise<void> {
      timer = null;
      if (!live) return;
      if (document.visibilityState === "hidden") {
        schedule();
        return;
      }
      try {
        const { ok } = await getHealth({ signal: controller.signal });
        if (live) setHealth(ok ? "ok" : "down");
      } catch (error) {
        if (live && !isAbort(error)) setHealth("down");
      }
      schedule();
    }

    function onVisible(): void {
      if (!live || document.visibilityState !== "visible" || timer === null) return;
      clearTimeout(timer);
      timer = null;
      void check();
    }

    document.addEventListener("visibilitychange", onVisible);
    void check();

    return () => {
      live = false;
      document.removeEventListener("visibilitychange", onVisible);
      if (timer !== null) clearTimeout(timer);
      controller.abort();
    };
  }, []);

  return health;
}

// ── What the box says about itself ────────────────────────────────────────

/* Settings, mode and rebuild status — everything behind the admin key.
 *
 * The status poller idles at thirty seconds and drops to two while a rebuild
 * is running: this page is a homepage, and a homepage left open all day should
 * not poll a box every two seconds for nothing. */
function useBoxFacts(signedIn: boolean): {
  settings: SettingsResponse | null;
  boxState: StateResponse | null;
  status: StatusResponse | null;
  measured: HomeStorage | null;
} {
  const [settings, setSettings] = React.useState<SettingsResponse | null>(null);
  const [boxState, setBoxState] = React.useState<StateResponse | null>(null);
  const [status, setStatus] = React.useState<StatusResponse | null>(null);
  const [measured, setMeasured] = React.useState<HomeStorage | null>(null);

  React.useEffect(() => {
    if (!signedIn) {
      setSettings(null);
      setBoxState(null);
      setStatus(null);
      setMeasured(null);
      return;
    }

    let live = true;
    const controller = new AbortController();

    void (async () => {
      try {
        const [next, mode] = await Promise.all([
          getSettings({ signal: controller.signal }),
          getState({ signal: controller.signal }),
        ]);
        if (!live) return;
        setSettings(next);
        setBoxState(mode);
      } catch {
        /* A 401 has already dropped the token and the shell will re-prompt;
         * anything else leaves the page on what it could render without a
         * key, which is most of it. */
      }
    })();

    /* The box's df reading (GET /api/storage), on its own so a box that
     * cannot measure itself still shows everything else. */
    void (async () => {
      try {
        const disk = await getStorage({ signal: controller.signal });
        if (!live || disk.usedBytes === null || disk.totalBytes === null) return;
        setMeasured({ usedBytes: disk.usedBytes, totalBytes: disk.totalBytes });
      } catch {
        /* no reading: the strip claims no amounts */
      }
    })();

    const poller = createStatusPoller({
      periodMs: IDLE_STATUS_PERIOD_MS,
      onStatus: (next) => {
        if (!live) return;
        setStatus(next);
        poller.setPeriod(next.state === "building" ? STATUS_PERIOD_MS : IDLE_STATUS_PERIOD_MS);
      },
      onUnauthorized: () => {
        if (live) setStatus(null);
      },
    });
    poller.start();

    return () => {
      live = false;
      controller.abort();
      poller.stop();
    };
  }, [signedIn]);

  return { settings, boxState, status, measured };
}

// ── Which apps are actually there ─────────────────────────────────────────

/* Remembered first, measured second.
 *
 * The probe runs once per visit. A result that says "could not tell" never
 * overwrites a remembered "present": a box does not lose its apps because one
 * request failed, and the grid emptying itself on a flaky network is the exact
 * failure this whole module exists to avoid. */
function useAvailability(): {
  seen: Availabilities;
  measured: boolean;
} {
  const [seen, setSeen] = React.useState<Availabilities>(() => recallAvailability());
  const [measured, setMeasured] = React.useState(false);

  React.useEffect(() => {
    const controller = new AbortController();
    let live = true;

    void (async () => {
      const probed = await probeApps({ signal: controller.signal });
      if (!live) return;
      setSeen((previous) => {
        const next: Availabilities = {
          files: probed.files === "unknown" ? previous.files : probed.files,
          code: probed.code === "unknown" ? previous.code : probed.code,
        };
        rememberAvailability(next);
        return next;
      });
      setMeasured(true);
    })();

    return () => {
      live = false;
      controller.abort();
    };
  }, []);

  return { seen, measured };
}
