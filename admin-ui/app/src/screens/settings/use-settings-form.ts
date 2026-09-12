import * as React from "react";
import {
  buildOverridesNix,
  createStatusPoller,
  getSettings,
  getStatus,
  hasToken,
  isAbort,
  isUnauthorized,
  isValidHostName,
  isValidPort,
  isValidTime,
  postApply,
  postFactoryReset,
  subscribeAuth,
  STATUS_PERIOD_MS,
  type SettingsResponse,
  type StatusResponse,
} from "@/lib/api";

/* The whole settings screen is one form over one document.
 *
 * /api/apply takes the ENTIRE modules/overrides.nix body, not a patch: all
 * twelve losos.* keys are rewritten on every apply. So the draft here is a
 * complete SettingsResponse, every pane edits a slice of the same object, and
 * a key nobody touched still round-trips through buildOverridesNix unchanged.
 * A pane that posted only its own slice would silently reset the other
 * eleven to their defaults — on a box with no shell to fix it from.
 *
 * This mirrors admin-ui/settings/app.js, which is the contract of record: the
 * same substitution of defaults for unusable values at generate time, the
 * same "dirty AND valid" arming of Apply, the same resume-if-building on
 * load, and the same stale-terminal guard on the status poll.
 */

/** What lososd falls back to, and what an unusable field is replaced with. */
export const DEFAULTS = {
  hostName: "mattbox",
  apachePort: 11000,
  windowStart: "23:00",
  windowEnd: "07:00",
} as const;

/* How long a freshly started rebuild may go without ever reporting
 * `building` before a terminal status is believed anyway.
 *
 * The ack from /api/apply carries a job id and statusResponse carries one
 * too, so the primary guard is comparing them. This is the fallback for the
 * window before the daemon has written the new job into state.json: a `done`
 * arriving there is the PREVIOUS rebuild's, and believing it would re-arm
 * Apply and say "changes applied" while this one is still starting. */
const SETTLE_MS = 20000;

export type RebuildPhase = "building" | "done" | "failed";

export interface RebuildBanner {
  phase: RebuildPhase;
  title: string;
  message: string;
}

export interface FormProblems {
  hostName: string | null;
  apachePort: string | null;
  computeWindow: string | null;
}

export interface SettingsForm {
  /** No admin key in this tab. The shell is showing its unlock prompt. */
  locked: boolean;
  /** The draft holds real values rather than placeholders. */
  ready: boolean;
  loadError: string | null;
  saved: SettingsResponse | null;
  draft: SettingsResponse | null;
  dirty: boolean;
  changedKeys: readonly (keyof SettingsResponse)[];
  problems: FormProblems;
  valid: boolean;
  /** A rebuild this screen started, or one it joined, is in flight. */
  applying: boolean;
  rebuild: RebuildBanner | null;
  set: <K extends keyof SettingsResponse>(key: K, value: SettingsResponse[K]) => void;
  discard: () => void;
  apply: () => void;
  factoryReset: () => void;
  dismissRebuild: () => void;
}

const KEYS = [
  "sharingMyStorage",
  "nextcloudMode",
  "forgejoMode",
  "hostName",
  "https",
  "gpuEnable",
  "apachePort",
  "proxyEnable",
  "clusterEnable",
  "shareCompute",
  "computeWindowStart",
  "computeWindowEnd",
  "hardeningApparmor",
  "hardeningMalloc",
  "hardeningNosmt",
  "hardeningUsbguard",
] as const satisfies readonly (keyof SettingsResponse)[];

function sameValue(a: unknown, b: unknown): boolean {
  // A half-typed port parses to NaN, and NaN !== NaN would leave the form
  // permanently dirty while the user is still typing.
  if (typeof a === "number" && typeof b === "number") {
    return Number.isNaN(a) && Number.isNaN(b) ? true : a === b;
  }
  return a === b;
}

function changedKeysOf(
  saved: SettingsResponse | null,
  draft: SettingsResponse | null,
): (keyof SettingsResponse)[] {
  if (saved === null || draft === null) return [];
  return KEYS.filter((key) => !sameValue(saved[key], draft[key]));
}

function problemsOf(draft: SettingsResponse | null): FormProblems {
  if (draft === null) return { hostName: null, apachePort: null, computeWindow: null };

  const host = draft.hostName;
  let hostName: string | null = null;
  if (host.length === 0) hostName = "Give this box a name.";
  else if (host.length > 63) hostName = "A name is at most 63 characters.";
  else if (!isValidHostName(host))
    hostName =
      "Use letters, digits and hyphens only, starting and ending with a letter or a digit.";

  const apachePort = isValidPort(draft.apachePort) ? null : "Pick a number between 1024 and 65535.";

  /* Any order is legal — an end before the start wraps midnight, which is
   * the default 23:00 to 07:00 — so there is nothing to compare between the
   * two bounds, only a shape to check on each. One message for the pair:
   * aria-invalid on the fields is what says which of the two is at fault. */
  const computeWindow =
    isValidTime(draft.computeWindowStart) && isValidTime(draft.computeWindowEnd)
      ? null
      : "Set both ends of the window as a 24-hour time, HH:MM.";

  return { hostName, apachePort, computeWindow };
}

/* Substitute the daemon's own defaults for anything unusable before the Nix
 * is generated. Apply is armed only while the form is valid, so in practice
 * nothing here fires — it is the guard that stops a future caller writing
 * `losos.nextcloud.apachePort = NaN;` into a file evaluated as root on the
 * next rebuild. */
function sanitize(draft: SettingsResponse): SettingsResponse {
  return {
    ...draft,
    hostName: isValidHostName(draft.hostName) ? draft.hostName : DEFAULTS.hostName,
    apachePort: isValidPort(draft.apachePort) ? draft.apachePort : DEFAULTS.apachePort,
    computeWindowStart: isValidTime(draft.computeWindowStart)
      ? draft.computeWindowStart
      : DEFAULTS.windowStart,
    computeWindowEnd: isValidTime(draft.computeWindowEnd)
      ? draft.computeWindowEnd
      : DEFAULTS.windowEnd,
  };
}

/* An <input type="time"> silently drops anything that is not HH:MM, so a
 * malformed stored window cannot be shown back the way a malformed name is —
 * the field would simply read empty. Substitute the default, which is what
 * the daemon falls back to as well, and take `saved` from the substituted
 * document so the substitution is not counted as a pending edit. */
function normalize(settings: SettingsResponse): SettingsResponse {
  return {
    ...settings,
    computeWindowStart: isValidTime(settings.computeWindowStart)
      ? settings.computeWindowStart
      : DEFAULTS.windowStart,
    computeWindowEnd: isValidTime(settings.computeWindowEnd)
      ? settings.computeWindowEnd
      : DEFAULTS.windowEnd,
  };
}

function describe(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return "This box did not answer.";
}

export function useSettingsForm(): SettingsForm {
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);

  const [saved, setSaved] = React.useState<SettingsResponse | null>(null);
  const [draft, setDraft] = React.useState<SettingsResponse | null>(null);
  const [loadError, setLoadError] = React.useState<string | null>(null);
  const [applying, setApplying] = React.useState(false);
  const [rebuild, setRebuild] = React.useState<RebuildBanner | null>(null);

  const draftRef = React.useRef<SettingsResponse | null>(null);
  draftRef.current = draft;

  /* Poll bookkeeping. Refs, not state: the poller is built once and its
   * handlers have to see current values without the poller being rebuilt —
   * rebuilding it mid-rebuild would drop the generation guard that keeps a
   * late `building` from landing on top of a fresh `done`. */
  const jobRef = React.useRef<string | null>(null);
  const sawBuildingRef = React.useRef(false);
  const settleUntilRef = React.useRef(0);
  const applyingRef = React.useRef(false);

  /* The handlers are reached through refs for the same reason: the poller is
   * created once, the closures it would otherwise capture are not. */
  const onStatusRef = React.useRef<(status: StatusResponse) => void>(() => undefined);
  const onUnauthorizedRef = React.useRef<() => void>(() => undefined);

  const poller = React.useMemo(
    () =>
      createStatusPoller({
        periodMs: STATUS_PERIOD_MS,
        onStatus: (status) => onStatusRef.current(status),
        onUnauthorized: () => onUnauthorizedRef.current(),
        onError: () => {
          /* A 502 while lososd restarts itself mid-rebuild is the normal
           * shape of a rebuild, not an outage: `nixos-rebuild switch`
           * restarts the very daemon being polled. The state is what reports
           * a real failure; painting every blip would cry wolf through the
           * whole of every apply. */
        },
      }),
    [],
  );

  const setApplyingBoth = React.useCallback((value: boolean) => {
    applyingRef.current = value;
    setApplying(value);
  }, []);

  const load = React.useCallback(async (signal?: AbortSignal): Promise<void> => {
    const fresh = normalize(await getSettings(signal === undefined ? {} : { signal }));
    setSaved(fresh);
    setDraft(fresh);
    setLoadError(null);
  }, []);

  const handleStatus = React.useCallback(
    (status: StatusResponse) => {
      /* Someone else's rebuild — the nightly auto-upgrade, or another tab.
       * Ours is the one whose id came back in the /api/apply ack. */
      if (status.job !== undefined && jobRef.current !== null && status.job !== jobRef.current) {
        return;
      }

      if (status.state === "building") {
        sawBuildingRef.current = true;
        setApplyingBoth(true);
        setRebuild({
          phase: "building",
          title: "Applying your changes",
          message:
            status.message.length > 0
              ? status.message
              : "This box is rebuilding itself. It stays reachable while it works.",
        });
        return;
      }

      // Terminal, and possibly the previous job's. See SETTLE_MS.
      if (applyingRef.current && !sawBuildingRef.current && Date.now() < settleUntilRef.current) {
        return;
      }

      poller.stop();
      jobRef.current = null;
      setApplyingBoth(false);

      if (status.state === "done") {
        setRebuild({
          phase: "done",
          title: "Changes applied",
          message:
            status.message.length > 0 ? status.message : "This box is running the new settings.",
        });
        void load().catch((error: unknown) => setLoadError(describe(error)));
        return;
      }

      if (status.state === "failed") {
        setRebuild({
          phase: "failed",
          title: "The changes could not be applied",
          message:
            status.message.length > 0
              ? status.message
              : "Nothing changed. This box is still running its previous settings.",
        });
        return;
      }

      // idle: nothing is running, and nothing of ours ever was.
      setRebuild(null);
    },
    [poller, load, setApplyingBoth],
  );

  const handleUnauthorized = React.useCallback(() => {
    /* api.ts has already dropped the token, which wakes the shell's unlock
     * prompt through subscribeAuth. Nothing to report here — just stop
     * pretending a rebuild is being watched. */
    setApplyingBoth(false);
    setRebuild(null);
  }, [setApplyingBoth]);

  // Declared before the loading effect so the refs are current by the time
  // anything can start a poll.
  React.useEffect(() => {
    onStatusRef.current = handleStatus;
    onUnauthorizedRef.current = handleUnauthorized;
  }, [handleStatus, handleUnauthorized]);

  React.useEffect(() => () => poller.stop(), [poller]);

  /* First load, and the reload after a re-prompt. Gated on the token: with
   * none, the shell has its unlock dialog up and a fetch would only be a 401
   * against a box that is answering perfectly well. */
  React.useEffect(() => {
    if (!signedIn) {
      poller.stop();
      setSaved(null);
      setDraft(null);
      setLoadError(null);
      setApplyingBoth(false);
      setRebuild(null);
      return;
    }

    const controller = new AbortController();
    let cancelled = false;

    void (async () => {
      try {
        await load(controller.signal);
      } catch (error) {
        if (cancelled || isAbort(error) || isUnauthorized(error)) return;
        setLoadError(describe(error));
        return;
      }

      /* Pick up a rebuild that was already running when this page loaded.
       * Without it a reload mid-rebuild comes back as an idle form with
       * Apply live, and a second /api/apply can be fired on top of the one
       * still running. */
      try {
        const status = await getStatus({ signal: controller.signal });
        if (cancelled || status.state !== "building") return;
        sawBuildingRef.current = true; // joining it mid-flight, not starting it
        jobRef.current = status.job ?? null;
        settleUntilRef.current = 0;
        setApplyingBoth(true);
        setRebuild({
          phase: "building",
          title: "Applying changes",
          message:
            status.message.length > 0 ? status.message : "This box is already rebuilding itself.",
        });
        poller.start();
      } catch {
        /* No status is not a reason to refuse to show the settings. */
      }
    })();

    return () => {
      cancelled = true;
      controller.abort();
    };
  }, [signedIn, load, poller, setApplyingBoth]);

  const runRebuild = React.useCallback(
    async (trigger: () => Promise<{ job: string }>, starting: string) => {
      sawBuildingRef.current = false;
      jobRef.current = null;
      settleUntilRef.current = Date.now() + SETTLE_MS;
      setApplyingBoth(true);
      setRebuild({ phase: "building", title: starting, message: "Starting…" });
      try {
        const ack = await trigger();
        jobRef.current = ack.job.length > 0 ? ack.job : null;
        poller.start();
      } catch (error) {
        setApplyingBoth(false);
        jobRef.current = null;
        if (isUnauthorized(error)) {
          setRebuild(null);
          return;
        }
        setRebuild({ phase: "failed", title: "That did not start", message: describe(error) });
      }
    },
    [poller, setApplyingBoth],
  );

  const set = React.useCallback(
    <K extends keyof SettingsResponse>(key: K, value: SettingsResponse[K]) => {
      setDraft((current) => (current === null ? current : { ...current, [key]: value }));
    },
    [],
  );

  const discard = React.useCallback(() => setDraft(saved), [saved]);

  const apply = React.useCallback(() => {
    const current = draftRef.current;
    if (current === null) return;
    const body = buildOverridesNix(sanitize(current));
    void runRebuild(() => postApply(body), "Applying your changes");
  }, [runRebuild]);

  const factoryReset = React.useCallback(() => {
    void runRebuild(() => postFactoryReset(), "Putting everything back");
  }, [runRebuild]);

  const dismissRebuild = React.useCallback(() => setRebuild(null), []);

  const changedKeys = React.useMemo(() => changedKeysOf(saved, draft), [saved, draft]);
  const problems = React.useMemo(() => problemsOf(draft), [draft]);
  const valid =
    problems.hostName === null && problems.apachePort === null && problems.computeWindow === null;

  return {
    locked: !signedIn,
    ready: draft !== null,
    loadError,
    saved,
    draft,
    dirty: changedKeys.length > 0,
    changedKeys,
    problems,
    valid,
    applying,
    rebuild,
    set,
    discard,
    apply,
    factoryReset,
    dismissRebuild,
  };
}
