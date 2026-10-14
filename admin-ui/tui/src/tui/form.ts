/* The settings form, the web UI's use-settings-form.ts in a terminal.
 *
 * /api/apply takes the WHOLE of modules/overrides.nix, so the draft is a
 * complete SettingsResponse and every pane edits a slice of it; Apply writes
 * all sixteen lines plus every other line the file already carries, through
 * the web UI's own buildOverridesNix. Nothing takes effect until Apply, and
 * Apply asks before it starts a rebuild. While one runs, /api/status is
 * polled every two seconds; the outcome is a message, and a done rebuild
 * reloads the settings. */

import * as React from "react";
import {
  buildOverridesNix,
  isValidHostName,
  isValidPort,
  isValidTime,
  OWNED_NAMES,
  type OptionsResponse,
  type SettingsResponse,
  type StatusResponse,
} from "@app/lib/api";
import { describeError, get, post, type Transport } from "../api";
import { t, type Key } from "../lang";

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

export interface Problems {
  hostName: string | null;
  apachePort: string | null;
  computeWindow: string | null;
}

export interface Notice {
  tone: "ok" | "crit" | "accent";
  title: string;
  text: string;
}

export interface SettingsForm {
  ready: boolean;
  loadError: string | null;
  saved: SettingsResponse | null;
  draft: SettingsResponse | null;
  options: OptionsResponse | null;
  dirty: boolean;
  changeCount: number;
  problems: Problems;
  valid: boolean;
  /** A rebuild is in flight: its title and the box's last line. */
  rebuild: { title: string; message: string } | null;
  set: <K extends keyof SettingsResponse>(key: K, value: SettingsResponse[K]) => void;
  discard: () => void;
  /** Start the rebuild. The caller asks first. */
  apply: () => Promise<void>;
  reload: () => Promise<void>;
}

function same(a: unknown, b: unknown): boolean {
  if (typeof a === "number" && typeof b === "number" && Number.isNaN(a) && Number.isNaN(b)) return true;
  return a === b;
}

function extrasOf(options: OptionsResponse | null): Record<string, string> {
  const extra: Record<string, string> = {};
  if (options === null) return extra;
  for (const option of options.options) {
    if (option.set !== null && !OWNED_NAMES.has(option.name)) extra[option.name] = option.set;
  }
  for (const stray of options.stray) extra[stray.key] = stray.value;
  return extra;
}

export function problemsOf(draft: SettingsResponse | null): Problems {
  if (draft === null) return { hostName: null, apachePort: null, computeWindow: null };
  let hostName: Key | null = null;
  if (draft.hostName.length === 0) hostName = "settings.form.hostEmpty";
  else if (draft.hostName.length > 63) hostName = "settings.form.hostTooLong";
  else if (!isValidHostName(draft.hostName)) hostName = "settings.form.hostChars";
  return {
    hostName: hostName === null ? null : t(hostName),
    apachePort: isValidPort(draft.apachePort) ? null : t("settings.form.port"),
    computeWindow:
      isValidTime(draft.computeWindowStart) && isValidTime(draft.computeWindowEnd) ? null : t("settings.form.window"),
  };
}

export function useSettingsForm(transport: Transport, notify: (notice: Notice) => void): SettingsForm {
  const [saved, setSaved] = React.useState<SettingsResponse | null>(null);
  const [draft, setDraft] = React.useState<SettingsResponse | null>(null);
  const [options, setOptions] = React.useState<OptionsResponse | null>(null);
  const [loadError, setLoadError] = React.useState<string | null>(null);
  const [rebuild, setRebuild] = React.useState<{ title: string; message: string } | null>(null);
  const job = React.useRef<string | null>(null);
  const timer = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const live = React.useRef(true);

  const load = React.useCallback(async () => {
    try {
      const fresh = await get<SettingsResponse>(transport, "/api/settings");
      let doc: OptionsResponse | null = null;
      try {
        doc = await get<OptionsResponse>(transport, "/api/options");
      } catch {
        doc = null;
      }
      if (doc === null || !Array.isArray(doc.options) || !Array.isArray(doc.stray)) {
        doc = { available: false, version: 0, options: [], stray: [], excluded: {} };
      }
      if (!live.current) return;
      setSaved(fresh);
      setDraft(fresh);
      setOptions(doc);
      setLoadError(null);
    } catch (error) {
      if (live.current) setLoadError(describeError(error));
    }
  }, [transport]);

  const poll = React.useCallback(
    function poll(): void {
      timer.current = setTimeout(async () => {
        timer.current = null;
        if (!live.current) return;
        let status: StatusResponse;
        try {
          status = await get<StatusResponse>(transport, "/api/status");
        } catch {
          /* lososd restarts itself mid-rebuild; keep asking */
          poll();
          return;
        }
        if (!live.current) return;
        if (status.job !== undefined && job.current !== null && status.job !== job.current) {
          poll();
          return;
        }
        if (status.state === "building" || status.state === "idle") {
          if (status.state === "building") {
            setRebuild({
              title: t("settings.form.applyingYours"),
              message: status.message.length > 0 ? status.message : t("settings.form.buildingMessage"),
            });
          }
          poll();
          return;
        }
        job.current = null;
        setRebuild(null);
        if (status.state === "done") {
          notify({
            tone: "ok",
            title: t("settings.form.doneTitle"),
            text: status.message.length > 0 ? status.message : t("settings.form.doneMessage"),
          });
          void load();
        } else {
          notify({
            tone: "crit",
            title: t("settings.form.failedTitle"),
            text: status.message.length > 0 ? status.message : t("settings.form.failedMessage"),
          });
        }
      }, 2000);
    },
    [transport, load, notify],
  );

  React.useEffect(() => {
    live.current = true;
    void (async () => {
      await load();
      try {
        const status = await get<StatusResponse>(transport, "/api/status");
        if (live.current && status.state === "building") {
          job.current = status.job ?? null;
          setRebuild({
            title: t("settings.form.joinedTitle"),
            message: status.message.length > 0 ? status.message : t("settings.form.joinedMessage"),
          });
          poll();
        }
      } catch {
        /* no status is no reason to hide the settings */
      }
    })();
    return () => {
      live.current = false;
      if (timer.current !== null) clearTimeout(timer.current);
    };
  }, [load, poll, transport]);

  const set = React.useCallback(<K extends keyof SettingsResponse>(key: K, value: SettingsResponse[K]) => {
    setDraft((current) => (current === null ? current : { ...current, [key]: value }));
  }, []);

  const changed = saved === null || draft === null ? [] : KEYS.filter((key) => !same(saved[key], draft[key]));
  const problems = problemsOf(draft);
  const valid = problems.hostName === null && problems.apachePort === null && problems.computeWindow === null;

  const apply = React.useCallback(async () => {
    if (draft === null || !valid) return;
    const body = buildOverridesNix(draft, extrasOf(options));
    setRebuild({ title: t("settings.form.applyingYours"), message: t("settings.form.starting") });
    try {
      const ack = await post<{ job: string }>(transport, "/api/apply", body);
      job.current = ack.job.length > 0 ? ack.job : null;
      poll();
    } catch (error) {
      setRebuild(null);
      notify({ tone: "crit", title: t("settings.form.didNotStart"), text: describeError(error) });
    }
  }, [draft, valid, options, transport, poll, notify]);

  return {
    ready: draft !== null,
    loadError,
    saved,
    draft,
    options,
    dirty: changed.length > 0,
    changeCount: changed.length,
    problems,
    valid,
    rebuild,
    set,
    discard: () => setDraft(saved),
    apply,
    reload: load,
  };
}
