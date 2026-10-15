/* Have a model on this box write the widget.
 *
 * The owner describes a widget and a small language model that runs on the
 * box writes it (backend/src/builder.rs, Ollama on loopback, served as
 * /api/builder*). What the owner types, and the widget they ask to change,
 * never leave the box, and a build costs nothing. On a mini PC's processor
 * a build takes minutes, so it runs in the background: the panel polls it,
 * shows how far the answer has got, and lets the owner stop it.
 *
 * What comes back is only text: the widget's files go into the hand
 * editor, the preview draws them in the same sandboxed frame as any widget
 * written by hand, and nothing is saved until the owner presses Save.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, SparklesIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { FieldError } from "@/components/ui/input";
import { Label, LabelHint } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import {
  getBuilder,
  getBuilderBuild,
  postBuilderBuild,
  stopBuilderBuild,
  ApiError,
  type BuildFault,
  type BuilderReason,
  type BuilderResponse,
  type BuildView,
  type WidgetFile,
} from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { builtFiles } from "./files";

/** The box's own limit on a description. */
export const MAX_PROMPT_CHARS = 2000;
/** How often a running build is looked at. */
const POLL_MS = 3000;

const FAULT_TEXT: Record<BuildFault, MessageKey> = {
  noWidget: "look.builder.fault.noWidget",
  unusable: "look.builder.fault.unusable",
  upstream: "look.builder.fault.upstream",
  memory: "look.builder.fault.memory",
};

const REASON_TEXT: Record<BuilderReason, MessageKey> = {
  off: "look.builder.off",
  starting: "look.builder.starting",
  downloading: "look.builder.downloading",
};

/** Meta's licence for Llama 3.2, which asks for "Built with Llama" where the
 *  model is used, and its acceptable use policy. */
export const LLAMA_TERMS = [
  { key: "look.builder.terms.license", href: "https://www.llama.com/llama3_2/license/" },
  { key: "look.builder.terms.usage", href: "https://www.llama.com/llama3_2/use-policy/" },
] as const;

export interface BuilderPanelProps {
  /** The files in the editor now, sent along when the owner asks for a change. */
  files: WidgetFile[];
  /** Whether those files are worth changing rather than starting over:
   *  a saved widget, or one the owner has typed into. */
  hasWidget: boolean;
  /** A build finished with a widget: put its files in the editor. */
  onWritten: (files: WidgetFile[]) => void;
}

type Loaded = { state: "loading" } | { state: "failed" } | { state: "ready"; view: BuilderResponse };

export function BuilderPanel({ files, hasWidget, onWritten }: BuilderPanelProps) {
  const t = useT();
  const locale = useLocale();
  const promptId = React.useId();
  const promptHintId = React.useId();
  const problemId = React.useId();
  const [loaded, setLoaded] = React.useState<Loaded>({ state: "loading" });
  const [prompt, setPrompt] = React.useState("");
  const [change, setChange] = React.useState(hasWidget);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [build, setBuild] = React.useState<BuildView | null>(null);
  const [followId, setFollowId] = React.useState<string | null>(null);

  const load = React.useCallback(async (): Promise<void> => {
    try {
      const view = await getBuilder();
      setLoaded({ state: "ready", view });
      // A build left running (the dialog closed, the page reloaded) is
      // followed again, so its widget is not lost.
      if (view.available) {
        const running = view.builds.find((b) => b.status === "running");
        if (running !== undefined) setFollowId(running.id);
      }
    } catch {
      setLoaded({ state: "failed" });
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  const written = React.useRef(onWritten);
  written.current = onWritten;

  React.useEffect(() => {
    if (followId === null) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const look = async (): Promise<void> => {
      try {
        const view = await getBuilderBuild(followId);
        if (stopped) return;
        setBuild(view);
        if (view.status === "running") {
          timer = setTimeout(() => void look(), POLL_MS);
          return;
        }
        setFollowId(null);
        const built = view.status === "done" ? builtFiles(view) : null;
        if (built !== null) {
          written.current(built);
          toast.success(t("look.builder.doneTitle"), t("look.builder.doneNote"));
        }
        void load();
      } catch (error) {
        if (stopped) return;
        // lososd restarted and forgot the build: stop following it.
        if (error instanceof ApiError && error.status === 404) {
          setFollowId(null);
          setBuild(null);
          void load();
          return;
        }
        timer = setTimeout(() => void look(), POLL_MS * 2);
      }
    };
    void look();
    return () => {
      stopped = true;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [followId, load, t]);

  if (loaded.state === "loading") {
    return <Spinner label={t("look.builder.loading")} className="text-muted" />;
  }
  if (loaded.state === "failed") {
    return (
      <div className="flex flex-col items-start gap-3 text-[13px] text-muted">
        <p>{t("look.builder.loadFailed")}</p>
        <Button variant="secondary" size="sm" onClick={() => void load()}>
          {t("look.builder.retry")}
        </Button>
      </div>
    );
  }
  const view = loaded.view;
  if (!view.available) {
    return (
      <div className="flex flex-col items-start gap-3">
        <p data-testid="builder-unavailable" className="text-[13px] leading-relaxed text-muted">
          {t(REASON_TEXT[view.reason], { model: view.model ?? "" })}
        </p>
        {view.reason !== "off" && (
          <Button variant="secondary" size="sm" onClick={() => void load()}>
            {t("look.builder.retry")}
          </Button>
        )}
      </div>
    );
  }

  const running = followId !== null || build?.status === "running";

  const start = async (): Promise<void> => {
    const text = prompt.trim();
    if (text.length === 0) {
      setProblem(t("look.builder.problem.prompt"));
      return;
    }
    if ([...text].length > MAX_PROMPT_CHARS) {
      setProblem(t("look.builder.problem.promptLong", { max: MAX_PROMPT_CHARS }));
      return;
    }
    setProblem(null);
    setBusy(true);
    try {
      const started = await postBuilderBuild({
        prompt: text,
        ...(change && hasWidget ? { base: files } : {}),
        lang: locale,
      });
      setBuild(started);
      setFollowId(started.id);
    } catch (error) {
      setProblem(
        error instanceof Error && error.message.length > 0 ? error.message : t("look.builder.startFailed"),
      );
    } finally {
      setBusy(false);
    }
  };

  const stop = async (): Promise<void> => {
    const id = followId ?? build?.id;
    if (id === undefined) return;
    setBusy(true);
    try {
      const view = await stopBuilderBuild(id);
      setBuild(view);
      setFollowId(null);
    } catch {
      // The poll picks up whatever the build did instead.
    } finally {
      setBusy(false);
    }
  };

  const outcome = (b: BuildView): string | null => {
    if (b.status === "done") return t("look.builder.done");
    if (b.status === "stopped") return t("look.builder.stopped");
    if (b.status !== "failed") return null;
    return t(FAULT_TEXT[b.fault ?? "upstream"]);
  };

  return (
    <div data-testid="builder-panel" className="flex flex-col gap-4">
      <p className="text-[13px] leading-relaxed text-muted">{t("look.builder.intro")}</p>
      <p className="rounded-card border border-line bg-sunk p-3.5 text-[13px] leading-relaxed text-muted" data-testid="builder-local">
        {t("look.builder.local", { model: view.model })}
      </p>

      <Label htmlFor={promptId} className="flex flex-col gap-1.5">
        <span>{t("look.builder.field.prompt")}</span>
        <Textarea
          id={promptId}
          rows={4}
          value={prompt}
          maxLength={MAX_PROMPT_CHARS}
          disabled={busy || running}
          aria-describedby={problem !== null ? `${promptHintId} ${problemId}` : promptHintId}
          aria-invalid={problem !== null}
          onChange={(event) => {
            setPrompt(event.target.value);
            setProblem(null);
          }}
        />
        <LabelHint id={promptHintId} className="font-normal">
          {t("look.builder.promptHint")}
        </LabelHint>
      </Label>

      {hasWidget && (
        <label className="flex items-center gap-2 text-[13px] text-ink">
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={change}
            disabled={busy || running}
            onChange={(event) => setChange(event.target.checked)}
          />
          {t("look.builder.change")}
        </label>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button disabled={busy || running} onClick={() => void start()}>
          <HugeiconsIcon icon={SparklesIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("look.builder.build")}
        </Button>
        {running && (
          <Button variant="secondary" disabled={busy} onClick={() => void stop()}>
            {t("look.builder.stop")}
          </Button>
        )}
      </div>
      <FieldError id={problemId}>{problem}</FieldError>

      {running && (
        <div role="status" className="flex items-center gap-2.5 text-[13px] text-muted" data-testid="builder-running">
          <Spinner label={t("look.builder.runningShort")} />
          <span>
            {build !== null && build.lines > 0
              ? t("look.builder.writing", { lines: build.lines })
              : t("look.builder.reading")}
          </span>
        </div>
      )}
      {build !== null && !running && outcome(build) !== null && (
        <Alert variant={build.status === "done" ? "ok" : "warn"} role="status" data-testid="builder-outcome">
          <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertDescription>
            <p>{outcome(build)}</p>
            {build.at_limit && <p>{t("look.builder.atLimit")}</p>}
            {build.notes !== null && build.notes.length > 0 && <p className="text-muted">{build.notes}</p>}
          </AlertDescription>
        </Alert>
      )}

      {view.model.startsWith("llama") && (
        <p className="text-[12.5px] leading-relaxed text-muted" data-testid="builder-terms">
          {t("look.builder.terms")}{" "}
          {LLAMA_TERMS.map((term, i) => (
            <React.Fragment key={term.href}>
              {i > 0 && " · "}
              <a href={term.href} target="_blank" rel="noreferrer" className="text-accent underline underline-offset-2">
                {t(term.key)}
              </a>
            </React.Fragment>
          ))}
        </p>
      )}
    </div>
  );
}
