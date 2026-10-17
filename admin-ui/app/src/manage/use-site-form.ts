import * as React from "react";
import {
  ApiError,
  getSite,
  getStatus,
  isAbort,
  postSite,
  type OptionsResponse,
  type StatusResponse,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { t } from "@/lib/i18n";
import { useLocale } from "@/lib/i18n-react";
import { fits, type Misfit } from "@/lib/option-value";
import type { ApplyBarForm } from "@/screens/settings/apply-bar";
import type { SiteForm } from "@/screens/settings/site-settings";

/* The settings of LosOS cloud and LosOS Git on a company edge's page for a
 * box: the same rows as the box's Apps pane (SiteSettings) and the same bar
 * (ApplyBar), over GET and POST /api/site instead of the box's whole
 * settings form, which a managing edge may not read or write. A change is
 * still a rebuild on the box; the bar follows it on GET /api/status. */

export interface EdgeSiteForm extends SiteForm, ApplyBarForm {
  /** Why the settings could not be read, or null. */
  failed: string | null;
  /** The edge or the box refused this edge (a 403): the owner switched
   *  management off, or never on. */
  refused: boolean;
  reload: () => void;
}

const POLL_MS = 3000;
/* A rebuild that restarts lososd answers through the edge with an error for
 * a while; give up following it only after this long. */
const FOLLOW_MS = 30 * 60 * 1000;

function setsOf(doc: OptionsResponse): Record<string, string> {
  const set: Record<string, string> = {};
  for (const option of doc.options) if (option.set !== null) set[option.name] = option.set;
  return set;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

export function useSiteForm(): EdgeSiteForm {
  const locale = useLocale();
  const [options, setOptions] = React.useState<OptionsResponse | null>(null);
  const [failed, setFailed] = React.useState<string | null>(null);
  const [refused, setRefused] = React.useState(false);
  const [saved, setSaved] = React.useState<Record<string, string>>({});
  const [extra, setExtra_] = React.useState<Record<string, string>>({});
  const [applying, setApplying] = React.useState(false);
  const [status, setStatus] = React.useState<StatusResponse | null>(null);
  const alive = React.useRef(true);
  React.useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const load = React.useCallback(async (signal?: AbortSignal) => {
    try {
      const doc = await getSite(signal === undefined ? {} : { signal });
      const set = setsOf(doc);
      setOptions(doc);
      setSaved(set);
      setExtra_(set);
      setFailed(null);
      setRefused(false);
    } catch (error) {
      if (isAbort(error)) return;
      setFailed(messageOf(error));
      setRefused(error instanceof ApiError && error.status === 403);
    }
  }, []);

  React.useEffect(() => {
    const controller = new AbortController();
    void load(controller.signal);
    return () => controller.abort();
  }, [load]);

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

  const changed = React.useMemo(() => {
    const names = new Set([...Object.keys(saved), ...Object.keys(extra)]);
    return [...names].filter((name) => saved[name] !== extra[name]).sort();
  }, [saved, extra]);

  const misfits = React.useMemo(() => {
    const out: Record<string, Misfit> = {};
    if (options === null) return out;
    for (const [name, raw] of Object.entries(extra)) {
      const option = options.options.find((o) => o.name === name);
      const misfit = option === undefined ? { key: "advanced.fit.stray" as const } : fits(option.editor, raw);
      if (misfit !== null) out[name] = misfit;
    }
    return out;
  }, [options, extra]);

  const extraProblems = React.useMemo(
    () =>
      Object.fromEntries(Object.entries(misfits).map(([name, misfit]) => [name, t(misfit.key, misfit.vars)])),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- locale re-resolves the sentences
    [misfits, locale],
  );

  /* Follow the rebuild `job` started until the box says how it ended. */
  const follow = React.useCallback(
    async (job: string) => {
      const until = Date.now() + FOLLOW_MS;
      while (alive.current && Date.now() < until) {
        await sleep(POLL_MS);
        let now: StatusResponse;
        try {
          now = await getStatus();
        } catch {
          continue;
        }
        if (!alive.current) return;
        setStatus(now);
        if (now.job !== job || now.state === "building") continue;
        if (now.state === "done") {
          toast.done(t("manage.site.done"));
          await load();
        } else {
          toast.error(t("manage.site.buildFailed"), now.message, { help: "apply-fails" });
        }
        return;
      }
    },
    [load],
  );

  const apply = React.useCallback(() => {
    const set = Object.fromEntries(changed.map((name) => [name, extra[name] ?? null]));
    setApplying(true);
    setStatus(null);
    void (async () => {
      try {
        const ack = await postSite(set);
        await follow(ack.job);
      } catch (error) {
        toast.error(t("manage.site.didNotStart"), messageOf(error), { help: "apply-fails" });
      } finally {
        if (alive.current) {
          setApplying(false);
          setStatus(null);
        }
      }
    })();
  }, [changed, extra, follow]);

  const discard = React.useCallback(() => setExtra_(saved), [saved]);

  return {
    options,
    extra,
    extraProblems,
    locked: applying,
    ready: options !== null,
    setExtra,
    rebuild: applying
      ? {
          phase: "building",
          title: t("manage.site.applying"),
          message: status?.message ?? "",
        }
      : null,
    dirty: changed.length > 0,
    changeCount: changed.length,
    valid: Object.keys(misfits).length === 0,
    discard,
    apply,
    applying,
    failed,
    refused,
    reload: () => void load(),
  };
}
