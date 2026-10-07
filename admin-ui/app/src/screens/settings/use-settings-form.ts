import * as React from "react";
import {
  ApiError,
  buildOverridesNix,
  createStatusPoller,
  getOptions,
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
  type OptionsResponse,
  type SettingsResponse,
  type StatusResponse,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import type { HandbookEntry } from "@/lib/handbook";
import { t, type MessageKey } from "@/lib/i18n";
import { useLocale } from "@/lib/i18n-react";
import { fits, OWNED, type Misfit } from "@/lib/option-value";

/* The one status the form raises: it names the rebuild so the outcome can
 * dismiss it, and a second apply replaces rather than stacks it. */
const REBUILD_STATUS = "rebuild";

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

/* The sticky bar only ever shows a rebuild in flight. Its outcome, done or
 * failed, arrives as a toast (components/ui/toast.tsx) and the bar goes: an
 * outcome is an event the owner glances at, not a strip to keep clearing. */
export interface RebuildBanner {
  phase: "building";
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
  /* The rest of modules/overrides.nix: every `losos.<name>` line the sixteen
   * keys above do not own, as Nix literals by name (lib/option-value.ts).
   * The Advanced pane edits these; Apply writes them after the sixteen. A
   * name the box declares no option for (a stray) is carried too, so it is
   * shown rather than silently dropped, and `valid` stays false until it is
   * removed, because lososd's gate would refuse it. */
  extra: Readonly<Record<string, string>>;
  savedExtra: Readonly<Record<string, string>>;
  changedExtra: readonly string[];
  /** Pending edits, the sixteen keys and the extras together. */
  changeCount: number;
  /** Every option this box declares, or null while loading. `available`
   *  false when the box serves no document (an older lososd). */
  options: OptionsResponse | null;
  problems: FormProblems;
  /** Extras whose literal does not fit, by name, with the sentence. */
  extraProblems: Readonly<Record<string, string>>;
  valid: boolean;
  /** A rebuild this screen started, or one it joined, is in flight. */
  applying: boolean;
  rebuild: RebuildBanner | null;
  set: <K extends keyof SettingsResponse>(key: K, value: SettingsResponse[K]) => void;
  /** Set one extra line's literal, or remove the line with null. */
  setExtra: (name: string, raw: string | null) => void;
  discard: () => void;
  apply: () => void;
  factoryReset: () => void;
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

/* The overrides.nix lines the sixteen keys do not own, as the option
 * document reports them: each declared option's `set` literal, plus the
 * strays. Owned names are left out even when set — the form's own field
 * carries them, and buildOverridesNix filters them anyway. */
function extrasOf(options: OptionsResponse | null): Record<string, string> {
  const extra: Record<string, string> = {};
  if (options === null) return extra;
  for (const option of options.options) {
    if (option.set !== null && !(option.name in OWNED)) extra[option.name] = option.set;
  }
  for (const stray of options.stray) extra[stray.key] = stray.value;
  return extra;
}

function changedExtraOf(
  saved: Readonly<Record<string, string>>,
  draft: Readonly<Record<string, string>>,
): string[] {
  const names = new Set([...Object.keys(saved), ...Object.keys(draft)]);
  return [...names].filter((name) => saved[name] !== draft[name]).sort();
}

/* Which extras lososd's gate would refuse, by name. A stray is always one;
 * a declared option's literal is checked against its editor kind. */
function extraProblemsOf(
  options: OptionsResponse | null,
  draft: Readonly<Record<string, string>>,
): Record<string, Misfit> {
  const problems: Record<string, Misfit> = {};
  if (options === null) return problems;
  for (const [name, raw] of Object.entries(draft)) {
    const option = options.options.find((o) => o.name === name);
    if (option === undefined) {
      problems[name] = { key: "advanced.fit.stray" };
      continue;
    }
    const misfit = fits(option.editor, raw);
    if (misfit !== null) problems[name] = misfit;
  }
  return problems;
}

/* Text held in state is held as a message key, or as the box's own words
 * when it sent some, and only turned into a sentence when the hook returns.
 * A string resolved when the event happened would stay in whichever language
 * was on screen then. */
type Msg = { key: MessageKey } | { raw: string };

function say(msg: Msg): string {
  return "raw" in msg ? msg.raw : t(msg.key);
}

/** The box's status message, or our own sentence when it sent none. */
function statusMsg(message: string, fallback: MessageKey): Msg {
  return message.length > 0 ? { raw: message } : { key: fallback };
}

interface HeldBanner {
  title: MessageKey;
  message: Msg;
}

type ProblemKeys = { [K in keyof FormProblems]: MessageKey | null };

function problemsOf(draft: SettingsResponse | null): ProblemKeys {
  if (draft === null) return { hostName: null, apachePort: null, computeWindow: null };

  const host = draft.hostName;
  let hostName: MessageKey | null = null;
  if (host.length === 0) hostName = "settings.form.hostEmpty";
  else if (host.length > 63) hostName = "settings.form.hostTooLong";
  else if (!isValidHostName(host)) hostName = "settings.form.hostChars";

  const apachePort = isValidPort(draft.apachePort) ? null : "settings.form.port";

  /* Any order is legal — an end before the start wraps midnight, which is
   * the default 23:00 to 07:00 — so there is nothing to compare between the
   * two bounds, only a shape to check on each. One message for the pair:
   * aria-invalid on the fields is what says which of the two is at fault. */
  const computeWindow =
    isValidTime(draft.computeWindowStart) && isValidTime(draft.computeWindowEnd)
      ? null
      : "settings.form.window";

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

function describe(error: unknown): Msg {
  if (error instanceof Error && error.message.length > 0) return { raw: error.message };
  return { key: "settings.form.noAnswer" };
}

export function useSettingsForm(): SettingsForm {
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);

  const [saved, setSaved] = React.useState<SettingsResponse | null>(null);
  const [draft, setDraft] = React.useState<SettingsResponse | null>(null);
  const [options, setOptions] = React.useState<OptionsResponse | null>(null);
  const [savedExtra, setSavedExtra] = React.useState<Record<string, string>>({});
  const [extra, setExtra_] = React.useState<Record<string, string>>({});
  const [heldLoadError, setLoadError] = React.useState<Msg | null>(null);
  const [applying, setApplying] = React.useState(false);
  const [heldRebuild, setRebuild] = React.useState<HeldBanner | null>(null);
  const locale = useLocale();

  const draftRef = React.useRef<SettingsResponse | null>(null);
  draftRef.current = draft;
  const savedRef = React.useRef<SettingsResponse | null>(null);
  savedRef.current = saved;
  const extraRef = React.useRef<Record<string, string>>({});
  extraRef.current = extra;

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
    const request = signal === undefined ? {} : { signal };
    const fresh = normalize(await getSettings(request));
    /* The option document rides along with the settings: the Advanced pane
     * needs it, and the extras it carries are part of the same draft. A box
     * without the route (an older lososd) still gets its settings — the
     * pane then says the document is not served, and Apply writes the
     * sixteen lines as before. */
    let doc: OptionsResponse | null = null;
    try {
      doc = await getOptions(request);
    } catch (error) {
      if (isAbort(error) || isUnauthorized(error)) throw error;
      doc = null;
    }
    // An answer that is not the document (a proxy's page, an older daemon's
    // empty object) counts as no document, not as a broken form.
    if (doc === null || !Array.isArray(doc.options) || !Array.isArray(doc.stray)) {
      doc = { available: false, version: 0, options: [], stray: [], excluded: {} };
    }
    const lines = extrasOf(doc);
    setSaved(fresh);
    setDraft(fresh);
    setOptions(doc);
    setSavedExtra(lines);
    setExtra_(lines);
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
          title: "settings.form.applyingYours",
          message: statusMsg(status.message, "settings.form.buildingMessage"),
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
      setRebuild(null);

      /* The outcome is a confirmation, in the language on screen at this
       * moment: a toast is gone in seconds, so there is nothing to re-resolve
       * on a language change, unlike the held texts below. It settles the
       * "applying" status, which has stayed up for the whole rebuild. */
      if (status.state === "done") {
        toast.success(
          t("settings.form.doneTitle"),
          say(statusMsg(status.message, "settings.form.doneMessage")),
          { settles: REBUILD_STATUS },
        );
        void load().catch((error: unknown) => setLoadError(describe(error)));
        return;
      }

      if (status.state === "failed") {
        toast.error(
          t("settings.form.failedTitle"),
          say(statusMsg(status.message, "settings.form.failedMessage")),
          { settles: REBUILD_STATUS, help: "apply-fails" },
        );
        return;
      }

      // idle: nothing is running, and nothing of ours ever was.
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
      setOptions(null);
      setSavedExtra({});
      setExtra_({});
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
          title: "settings.form.joinedTitle",
          message: statusMsg(status.message, "settings.form.joinedMessage"),
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
    async (
      trigger: () => Promise<{ job: string }>,
      starting: MessageKey,
      /* The handbook page a refusal leads to; the default is the apply page,
       * and apply() names the sharing page when the box refuses a mesh switch
       * (lososd answers 409 while no edge is reachable). */
      helpFor: (error: unknown) => HandbookEntry = () => "apply-fails",
    ) => {
      sawBuildingRef.current = false;
      jobRef.current = null;
      settleUntilRef.current = Date.now() + SETTLE_MS;
      setApplyingBoth(true);
      setRebuild({
        title: starting,
        message: { key: "settings.form.starting" },
      });
      try {
        const ack = await trigger();
        jobRef.current = ack.job.length > 0 ? ack.job : null;
        // A status, not a confirmation: it stays until the outcome settles it.
        toast.status(t(starting), t("settings.form.buildingMessage"), {
          id: REBUILD_STATUS,
          duration: Infinity,
        });
        poller.start();
      } catch (error) {
        setApplyingBoth(false);
        jobRef.current = null;
        setRebuild(null);
        if (isUnauthorized(error)) return;
        toast.error(t("settings.form.didNotStart"), say(describe(error)), {
          settles: REBUILD_STATUS,
          help: helpFor(error),
        });
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

  const setExtra = React.useCallback((name: string, raw: string | null) => {
    setExtra_((current) => {
      if (raw === null) {
        if (!(name in current)) return current;
        const { [name]: _dropped, ...rest } = current;
        return rest;
      }
      return current[name] === raw ? current : { ...current, [name]: raw };
    });
  }, []);

  const discard = React.useCallback(() => {
    setDraft(saved);
    setExtra_(savedExtra);
  }, [saved, savedExtra]);

  const apply = React.useCallback(() => {
    const current = draftRef.current;
    if (current === null) return;
    const body = buildOverridesNix(sanitize(current), extraRef.current);
    /* A 409 on an apply that turns on disk sharing or the mesh join is the
     * edge gate: no reachable edge, so the switch is refused. Any other
     * refusal (a rebuild already running, a bad body) is the apply page. */
    const before = savedRef.current;
    const turnsOnMesh =
      (current.sharingMyStorage && !(before?.sharingMyStorage ?? false)) ||
      (current.clusterEnable && !(before?.clusterEnable ?? false));
    void runRebuild(() => postApply(body), "settings.form.applyingYours", (error) =>
      turnsOnMesh && error instanceof ApiError && error.status === 409 ? "sharing-refused" : "apply-fails",
    );
  }, [runRebuild]);

  const factoryReset = React.useCallback(() => {
    void runRebuild(() => postFactoryReset(), "settings.form.resetTitle");
  }, [runRebuild]);

  const changedKeys = React.useMemo(() => changedKeysOf(saved, draft), [saved, draft]);
  const changedExtra = React.useMemo(() => changedExtraOf(savedExtra, extra), [savedExtra, extra]);
  const problemKeys = React.useMemo(() => problemsOf(draft), [draft]);
  const extraMisfits = React.useMemo(() => extraProblemsOf(options, extra), [options, extra]);
  const valid =
    problemKeys.hostName === null &&
    problemKeys.apachePort === null &&
    problemKeys.computeWindow === null &&
    Object.keys(extraMisfits).length === 0;

  /* Everything below turns held keys into sentences. `locale` is a dep of
   * each, so a language change re-resolves them rather than serving the
   * memoised text of the previous language. */
  const problems = React.useMemo<FormProblems>(() => {
    const resolve = (key: MessageKey | null): string | null => (key === null ? null : t(key));
    return {
      hostName: resolve(problemKeys.hostName),
      apachePort: resolve(problemKeys.apachePort),
      computeWindow: resolve(problemKeys.computeWindow),
    };
  }, [problemKeys, locale]);

  const extraProblems = React.useMemo<Record<string, string>>(
    () =>
      Object.fromEntries(
        Object.entries(extraMisfits).map(([name, misfit]) => [name, t(misfit.key, misfit.vars)]),
      ),
    [extraMisfits, locale],
  );

  const rebuild = React.useMemo<RebuildBanner | null>(
    () =>
      heldRebuild === null
        ? null
        : {
            phase: "building",
            title: t(heldRebuild.title),
            message: say(heldRebuild.message),
          },
    [heldRebuild, locale],
  );

  const loadError = React.useMemo(
    () => (heldLoadError === null ? null : say(heldLoadError)),
    [heldLoadError, locale],
  );

  return {
    locked: !signedIn,
    ready: draft !== null,
    loadError,
    saved,
    draft,
    dirty: changedKeys.length + changedExtra.length > 0,
    changedKeys,
    extra,
    savedExtra,
    changedExtra,
    changeCount: changedKeys.length + changedExtra.length,
    options,
    problems,
    extraProblems,
    valid,
    applying,
    rebuild,
    set,
    setExtra,
    discard,
    apply,
    factoryReset,
  };
}
