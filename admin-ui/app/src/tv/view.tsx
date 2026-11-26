import * as React from "react";
import { useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  getHealth,
  getSettings,
  getStatus,
  getStorage,
  hasToken,
  isAbort,
  loadMe,
  signInWithPassword,
  signOut,
  subscribeAuth,
  type SettingsResponse,
} from "@/lib/api";
import {
  deriveApps,
  formatBytes,
  probeApps,
  recallAvailability,
  rememberAvailability,
  type AppTileModel,
  type Availabilities,
} from "@/lib/apps";
import { getLocale, INTL_TAG, type MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { setTvLayout } from "@/lib/tv";
import { directionOf, nextFocus, type TvRect } from "@/tv/nav";

/* The television.
 *
 * The desk (the sidebar, the top bar, the dense panes) is the wrong
 * object at three metres. This is the other one: the whole window, type
 * and targets sized for a sofa, a ring the remote can see, and arrows
 * that move it. It reads the same box the homepage does (health, the
 * disk, the apps that actually answered) and refuses the forms: those
 * stay on the desk, one explicit step away, so a remote never lands in
 * a page of inputs by accident.
 *
 * The shell mounts this instead of the desk while the TV layout is on
 * (lib/tv.ts). The first-run wizard is not this screen and stays the
 * wizard. */

type Health = "unknown" | "ok" | "down";

type Screen =
  | { id: "home" }
  | { id: "health" }
  | { id: "storage" }
  | { id: "signin" }
  | { id: "desk" };

const HEALTH_PERIOD_MS = 60_000;

export default function TvView() {
  const t = useT();
  const navigate = useNavigate();
  const rootRef = React.useRef<HTMLDivElement>(null);
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);
  const [stack, setStack] = React.useState<Screen[]>([{ id: "home" }]);
  const screen = stack[stack.length - 1] ?? { id: "home" };
  const health = useHealth();
  const glance = useGlance(signedIn);
  const apps = useApps();
  const now = useClock();

  const goHome = React.useCallback(() => setStack([{ id: "home" }]), []);
  const back = React.useCallback(
    () => setStack((current) => (current.length > 1 ? current.slice(0, -1) : current)),
    [],
  );
  const push = React.useCallback((next: Screen) => setStack((current) => [...current, next]), []);
  const openDesk = React.useCallback(() => {
    setTvLayout("off");
    navigate("/settings");
  }, [navigate]);

  const actions = React.useRef({ back, goHome });
  actions.current = { back, goHome };
  useRemote(rootRef, actions);
  useInitialFocus(rootRef, screen.id);

  const host =
    glance.settings?.hostName && glance.settings.hostName.length > 0
      ? glance.settings.hostName
      : browserHost();
  const logo = logoFor(now);

  return (
    <div ref={rootRef} className="tv-root" data-testid="tv-view" role="application" aria-label={t("tv.landmark")}>
      <p className="sr-only">{t("tv.remoteHint")}</p>
      <header className="tv-mast">
        <div className="tv-brand">
          <img className="tv-logo" src={logo.url} alt="" width={128} height={128} />
          <div>
            <h1 className="tv-title">{host}</h1>
            <p className="tv-sub">
              <span className="numeric">{formatWhen(now)}</span>
              {glance.applying ? <span> · {t("home.applying.short")}</span> : null}
            </p>
          </div>
        </div>
        <div className="tv-actions">
          {screen.id !== "home" && (
            <button type="button" className="tv-btn" data-tv-item="back" onClick={back}>
              {t("tv.back")}
            </button>
          )}
          <button type="button" className="tv-btn" data-tv-item="home" onClick={goHome}>
            {t("tv.home")}
          </button>
          {screen.id !== "desk" && (
            <button type="button" className="tv-btn" data-tv-item="desk" onClick={() => push({ id: "desk" })}>
              {t("tv.desk")}
            </button>
          )}
        </div>
      </header>

      {screen.id === "home" && (
        <HomeScreen
          t={t}
          health={health}
          signedIn={signedIn}
          used={glance.used}
          total={glance.total}
          apps={apps}
          onHealth={() => push({ id: "health" })}
          onStorage={() => push(signedIn ? { id: "storage" } : { id: "signin" })}
          onDesk={() => push({ id: "desk" })}
        />
      )}
      {screen.id === "health" && <HealthScreen t={t} health={health} />}
      {screen.id === "storage" && (
        <StorageScreen t={t} used={glance.used} total={glance.total} />
      )}
      {screen.id === "signin" && <SignInScreen t={t} onDone={goHome} />}
      {screen.id === "desk" && (
        <DeskScreen t={t} signedIn={signedIn} onOpen={openDesk} onSignedOut={goHome} />
      )}
    </div>
  );
}

function HomeScreen({
  t,
  health,
  signedIn,
  used,
  total,
  apps,
  onHealth,
  onStorage,
  onDesk,
}: {
  t: (key: MessageKey, vars?: Record<string, string | number>) => string;
  health: Health;
  signedIn: boolean;
  used: number | null;
  total: number | null;
  apps: { tiles: readonly AppTileModel[]; pending: number };
  onHealth: () => void;
  onStorage: () => void;
  onDesk: () => void;
}) {
  const share = used !== null && total !== null && total > 0 ? Math.round((used / total) * 100) : null;
  const storageName = !signedIn ? t("tv.signIn") : share !== null ? `${share}%` : t("tv.storage.unread");
  const storageNote =
    used !== null && total !== null && total > 0
      ? t("tv.storage.of", { used: formatBytes(used), total: formatBytes(total) })
      : t("tv.storage.signIn");

  return (
    <div data-testid="tv-home">
      <section>
        <h2 className="tv-label">{t("tv.glance")}</h2>
        <div className="tv-grid">
          <TileButton
            id="health"
            initial
            kicker={t("tv.health")}
            name={t(healthName(health))}
            note={t(healthNote(health))}
            onClick={onHealth}
          />
          <TileButton
            id="storage"
            kicker={t("tv.storage")}
            name={storageName}
            note={storageNote}
            onClick={onStorage}
          />
        </div>
      </section>
      <section>
        <h2 className="tv-label">{t("tv.apps")}</h2>
        <div className="tv-grid">
          {Array.from({ length: apps.pending }, (_, index) => (
            <div key={`pending-${String(index)}`} className="tv-tile tv-tile-pending" aria-hidden="true" />
          ))}
          {apps.tiles.map((tile) =>
            tile.leavesAdmin ? (
              <a
                key={tile.app.id}
                className="tv-tile"
                data-tv-item={`app-${tile.app.id}`}
                href={tile.href}
              >
                <TileBody
                  icon={
                    <HugeiconsIcon icon={tile.app.icon} size={48} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  }
                  name={t(tile.app.name)}
                  note={tile.detail ?? t(tile.app.note)}
                />
              </a>
            ) : (
              <TileButton
                key={tile.app.id}
                id={`app-${tile.app.id}`}
                icon={
                  <HugeiconsIcon icon={tile.app.icon} size={48} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                }
                name={t(tile.app.name)}
                note={t(tile.app.note)}
                onClick={onDesk}
              />
            ),
          )}
        </div>
      </section>
    </div>
  );
}

function HealthScreen({
  t,
  health,
}: {
  t: (key: MessageKey) => string;
  health: Health;
}) {
  return (
    <div className="tv-panel" data-testid="tv-detail" data-tv-item="detail" data-tv-initial tabIndex={0}>
      <p className="tv-kicker">{t("tv.health")}</p>
      <p className="tv-hero">{t(healthName(health))}</p>
      <p className="tv-sub">{t(healthNote(health))}</p>
    </div>
  );
}

function StorageScreen({
  t,
  used,
  total,
}: {
  t: (key: MessageKey, vars?: Record<string, string | number>) => string;
  used: number | null;
  total: number | null;
}) {
  const share = used !== null && total !== null && total > 0 ? Math.round((used / total) * 100) : null;
  return (
    <div className="tv-panel" data-testid="tv-detail" data-tv-item="detail" data-tv-initial tabIndex={0}>
      <p className="tv-kicker">{t("tv.storage")}</p>
      <p className="tv-hero numeric">{share !== null ? `${share}%` : t("tv.storage.unread")}</p>
      <p className="tv-sub">
        {used !== null && total !== null && total > 0
          ? t("tv.storage.of", { used: formatBytes(used), total: formatBytes(total) })
          : t("tv.storage.detail")}
      </p>
    </div>
  );
}

function SignInScreen({
  t,
  onDone,
}: {
  t: (key: MessageKey) => string;
  onDone: () => void;
}) {
  const [name, setName] = React.useState("");
  const [password, setPassword] = React.useState("");
  const [error, setError] = React.useState<MessageKey | null>(null);
  const [working, setWorking] = React.useState(false);

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (working) return;
    setWorking(true);
    setError(null);
    void (async () => {
      const result = await signInWithPassword(name, password);
      if (result === "signedIn") {
        await loadMe().catch(() => null);
        onDone();
        return;
      }
      setWorking(false);
      setError(result === "refused" ? "tv.signIn.refused" : "tv.signIn.notYet");
    })();
  };

  return (
    <form className="tv-panel" data-testid="tv-signin" onSubmit={submit}>
      <div>
        <h2 className="tv-title">{t("tv.signIn")}</h2>
        <p className="tv-sub">{t("tv.signIn.body")}</p>
      </div>
      <label className="tv-field-label">
        <span>{t("tv.signIn.name")}</span>
        <input
          className="tv-field"
          name="username"
          autoComplete="username"
          data-tv-item="signin-name"
          data-tv-initial
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
      </label>
      <label className="tv-field-label">
        <span>{t("tv.signIn.password")}</span>
        <input
          className="tv-field"
          name="password"
          type="password"
          autoComplete="current-password"
          data-tv-item="signin-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
      </label>
      {error !== null && (
        <p className="tv-error" role="alert">
          {t(error)}
        </p>
      )}
      <button type="submit" className="tv-btn" data-tv-item="signin-submit" disabled={working}>
        {working ? t("tv.signIn.working") : t("tv.signIn.submit")}
      </button>
    </form>
  );
}

function DeskScreen({
  t,
  signedIn,
  onOpen,
  onSignedOut,
}: {
  t: (key: MessageKey) => string;
  signedIn: boolean;
  onOpen: () => void;
  onSignedOut: () => void;
}) {
  return (
    <div className="tv-panel" data-testid="tv-desk">
      <h2 className="tv-title">{t("tv.desk.title")}</h2>
      <p className="tv-sub">{t("tv.desk.body")}</p>
      <div className="tv-actions">
        <button type="button" className="tv-btn" data-tv-item="desk-open" data-tv-initial onClick={onOpen}>
          {t("tv.desk.open")}
        </button>
        {signedIn && (
          <button
            type="button"
            className="tv-btn"
            data-tv-item="desk-signout"
            onClick={() => {
              signOut();
              onSignedOut();
            }}
          >
            {t("tv.desk.signOut")}
          </button>
        )}
      </div>
    </div>
  );
}

function TileButton({
  id,
  initial = false,
  kicker,
  icon,
  name,
  note,
  onClick,
}: {
  id: string;
  initial?: boolean;
  kicker?: string;
  icon?: React.ReactNode;
  name: string;
  note: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="tv-tile"
      data-tv-item={id}
      data-tv-initial={initial ? "" : undefined}
      onClick={onClick}
    >
      <TileBody kicker={kicker} icon={icon} name={name} note={note} />
    </button>
  );
}

function TileBody({
  kicker,
  icon,
  name,
  note,
}: {
  kicker?: string;
  icon?: React.ReactNode;
  name: string;
  note: string;
}) {
  return (
    <>
      {icon !== undefined && <span className="tv-glyph">{icon}</span>}
      {kicker !== undefined && <span className="tv-kicker">{kicker}</span>}
      <span className="tv-tile-name">{name}</span>
      <span className="tv-tile-note">{note}</span>
    </>
  );
}

function healthName(health: Health): MessageKey {
  if (health === "ok") return "home.header.answering";
  if (health === "down") return "home.header.notAnswering";
  return "home.header.checking";
}

function healthNote(health: Health): MessageKey {
  if (health === "ok") return "tv.health.ok";
  if (health === "down") return "tv.health.down";
  return "tv.health.checking";
}

function browserHost(): string {
  try {
    return window.location.hostname === "" ? "" : window.location.hostname;
  } catch {
    return "";
  }
}

function formatWhen(now: Date): string {
  const tag = INTL_TAG[getLocale()];
  const time = new Intl.DateTimeFormat(tag, { hour: "2-digit", minute: "2-digit" }).format(now);
  const day = new Intl.DateTimeFormat(tag, { weekday: "long", day: "numeric", month: "long" }).format(now);
  return `${time} · ${day}`;
}

function useClock(): Date {
  const [now, setNow] = React.useState(() => new Date());
  React.useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

function useHealth(): Health {
  const [health, setHealth] = React.useState<Health>("unknown");
  React.useEffect(() => {
    let live = true;
    const controller = new AbortController();
    const check = async () => {
      try {
        const { ok } = await getHealth({ signal: controller.signal });
        if (live) setHealth(ok ? "ok" : "down");
      } catch (error) {
        if (live && !isAbort(error)) setHealth("down");
      }
    };
    void check();
    const id = window.setInterval(() => void check(), HEALTH_PERIOD_MS);
    return () => {
      live = false;
      controller.abort();
      window.clearInterval(id);
    };
  }, []);
  return health;
}

function useGlance(signedIn: boolean): {
  settings: SettingsResponse | null;
  used: number | null;
  total: number | null;
  applying: boolean;
} {
  const [settings, setSettings] = React.useState<SettingsResponse | null>(null);
  const [used, setUsed] = React.useState<number | null>(null);
  const [total, setTotal] = React.useState<number | null>(null);
  const [applying, setApplying] = React.useState(false);

  React.useEffect(() => {
    if (!signedIn) {
      setSettings(null);
      setUsed(null);
      setTotal(null);
      setApplying(false);
      return;
    }
    let live = true;
    const controller = new AbortController();
    void (async () => {
      try {
        const next = await getSettings({ signal: controller.signal });
        if (live) setSettings(next);
      } catch {
        /* the home screen still has the address it was opened with */
      }
    })();
    void (async () => {
      try {
        const disk = await getStorage({ signal: controller.signal });
        if (!live || disk.usedBytes === null || disk.totalBytes === null) return;
        setUsed(disk.usedBytes);
        setTotal(disk.totalBytes);
      } catch {
        /* no reading: the tile says so, and invents no number */
      }
    })();
    void (async () => {
      try {
        const status = await getStatus({ signal: controller.signal });
        if (live) setApplying(status.state === "building");
      } catch {
        /* a quiet homepage is the right failure */
      }
    })();
    return () => {
      live = false;
      controller.abort();
    };
  }, [signedIn]);

  return { settings, used, total, applying };
}

function useApps(): { tiles: readonly AppTileModel[]; pending: number } {
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

  const model = deriveApps({ ...seen, measured });
  return { tiles: model.tiles, pending: model.pendingCount };
}

function useRemote(
  rootRef: React.RefObject<HTMLDivElement | null>,
  actions: React.RefObject<{ back: () => void; goHome: () => void }>,
) {
  React.useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const root = rootRef.current;
      if (root === null) return;
      if (isBack(event)) {
        if (event.key === "Backspace" && fieldHasText(event)) return;
        event.preventDefault();
        actions.current?.back();
        return;
      }
      if (event.key === "Home" || event.key === "GoHome") {
        event.preventDefault();
        actions.current?.goHome();
        return;
      }
      const direction = directionOf(event.key);
      if (direction === null) return;
      event.preventDefault();
      const items = rectsIn(root);
      const current = document.activeElement;
      const id = current instanceof HTMLElement ? current.dataset.tvItem : undefined;
      const next = id !== undefined && id !== "" ? nextFocus(items, id, direction) : (items[0]?.id ?? null);
      if (next !== null) focusItem(root, next);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rootRef, actions]);
}

function useInitialFocus(rootRef: React.RefObject<HTMLDivElement | null>, screenId: Screen["id"]) {
  React.useEffect(() => {
    const root = rootRef.current;
    if (root === null) return;
    const initial =
      root.querySelector<HTMLElement>("[data-tv-initial]") ??
      root.querySelector<HTMLElement>("[data-tv-item]");
    initial?.focus({ preventScroll: true });
  }, [rootRef, screenId]);
}

function isBack(event: KeyboardEvent): boolean {
  return (
    event.key === "Escape" ||
    event.key === "BrowserBack" ||
    event.key === "GoBack" ||
    event.key === "Backspace" ||
    event.keyCode === 461 ||
    event.keyCode === 10009
  );
}

function fieldHasText(event: KeyboardEvent): boolean {
  const target = event.target;
  if (!(target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement)) return false;
  return target.value.length > 0;
}

function rectsIn(root: HTMLElement): TvRect[] {
  const found: TvRect[] = [];
  for (const el of root.querySelectorAll<HTMLElement>("[data-tv-item]")) {
    const id = el.dataset.tvItem;
    if (id === undefined || id === "") continue;
    const rect = el.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) continue;
    found.push({ id, x: rect.x, y: rect.y, width: rect.width, height: rect.height });
  }
  return found;
}

function focusItem(root: HTMLElement, id: string): void {
  const el = root.querySelector<HTMLElement>(`[data-tv-item="${CSS.escape(id)}"]`);
  el?.focus({ preventScroll: true });
}
