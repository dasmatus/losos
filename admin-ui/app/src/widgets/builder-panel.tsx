/* Have a model on this box write a widget or an app.
 *
 * The owner describes a widget, or an app of their own, and a small
 * language model that runs on the box writes it (backend/src/builder.rs,
 * Ollama on loopback, served as /api/builder*). What the owner types, and
 * what they ask to change, never leave the box, and a build costs nothing.
 * On a mini PC's processor a build takes minutes, so it runs in the
 * background: the panel polls it, shows each step as it happens (the
 * model writes, the box checks the code, the page tries an app, the model
 * fixes what was found) and lets the owner stop it.
 *
 * What comes back is only text. A widget's files go into the hand editor
 * and an app's source into the app editor (src/apps/), each previewed in
 * its own sandboxed frame, and nothing is saved until the owner presses
 * Save.
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
  type BuildKind,
  type BuildStep,
  type BuilderReason,
  type BuilderResponse,
  type BuildView,
  type WidgetFile,
} from "@/lib/api";
import type { MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { builtFiles } from "./files";

/** The box's own limit on a description. */
export const MAX_PROMPT_CHARS = 2000;
/** How often a running build is looked at. */
const POLL_MS = 3000;

const FAULT_TEXT: Record<BuildFault, MessageKey> = {
  noWidget: "look.builder.fault.noWidget",
  noApp: "userApps.builder.fault.noApp",
  unusable: "look.builder.fault.unusable",
  upstream: "look.builder.fault.upstream",
  memory: "look.builder.fault.memory",
};

/** The sentences that name what is being built. */
type KindText =
  | "intro"
  | "field"
  | "hint"
  | "change"
  | "writing"
  | "fixing"
  | "problem"
  | "doneTitle"
  | "doneNote"
  | "done"
  | "stopped"
  | "atLimit"
  | "local";

const KIND_TEXT: Record<BuildKind, Record<KindText, MessageKey>> = {
  widget: {
    intro: "look.builder.intro",
    field: "look.builder.field.prompt",
    hint: "look.builder.promptHint",
    change: "look.builder.change",
    writing: "look.builder.writing",
    fixing: "look.builder.fixing",
    problem: "look.builder.problem.prompt",
    doneTitle: "look.builder.doneTitle",
    doneNote: "look.builder.doneNote",
    done: "look.builder.done",
    stopped: "look.builder.stopped",
    atLimit: "look.builder.atLimit",
    local: "look.builder.local",
  },
  app: {
    intro: "userApps.builder.intro",
    field: "userApps.builder.field.prompt",
    hint: "userApps.builder.promptHint",
    change: "userApps.builder.change",
    writing: "userApps.builder.writing",
    fixing: "userApps.builder.fixing",
    problem: "userApps.builder.problem.prompt",
    doneTitle: "userApps.builder.doneTitle",
    doneNote: "userApps.builder.doneNote",
    done: "userApps.builder.done",
    stopped: "userApps.builder.stopped",
    atLimit: "userApps.builder.atLimit",
    local: "userApps.builder.local",
  },
};

const REASON_TEXT: Record<BuilderReason, MessageKey> = {
  off: "look.builder.off",
  starting: "look.builder.starting",
  downloading: "look.builder.downloading",
};

/** Qwen2.5-Coder is under the Apache License 2.0, which asks that whoever
 *  passes the model on gives its licence along; the page names the model,
 *  who made it and links both. */
export const QWEN_TERMS = [
  { key: "look.builder.terms.license", href: "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/blob/main/LICENSE" },
  { key: "look.builder.terms.model", href: "https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct" },
] as const;

/** The Qwen2.5-Coder sizes under the Apache License 2.0, the ones the
 *  notice is written for. The 3B one has a research licence of its own. */
export function isQwenCoder(model: string): boolean {
  const [name, tag = "latest"] = model.split(":");
  return name === "qwen2.5-coder" && !tag.startsWith("3b");
}

/** A widget goes back to the hand editor as files, an app to the app
 *  editor as the whole finished build (its source, name and summary). */
export type BuilderPanelProps =
  | {
      kind?: "widget";
      /** The files in the editor now, sent along when the owner asks for a change. */
      files: WidgetFile[];
      /** Whether those files are worth changing rather than starting over:
       *  a saved widget, or one the owner has typed into. */
      hasWidget: boolean;
      /** A build finished with a widget: put its files in the editor. */
      onWritten: (files: WidgetFile[]) => void;
    }
  | {
      kind: "app";
      /** The app's source in the editor now, for a change. */
      source: string;
      hasApp: boolean;
      /** A build finished with an app's source. */
      onWritten: (build: BuildView) => void;
      /** Every look at a running build, for a page that shows its progress. */
      onProgress?: (build: BuildView | null) => void;
      /** A request the page puts in the field for the owner to send, such as
       *  a mistake to fix; a new `key` puts it in again. */
      suggest?: { prompt: string; key: number } | null;
    };

/** The source of the app a finished build wrote, if it wrote one. */
export function builtApp(view: BuildView): string | null {
  const file = view.files?.find((f) => f.name === "app.jsx");
  return file !== undefined && file.content.trim() !== "" ? file.content : null;
}

type Loaded = { state: "loading" } | { state: "failed" } | { state: "ready"; view: BuilderResponse };

export function BuilderPanel(props: BuilderPanelProps) {
  const kind: BuildKind = props.kind ?? "widget";
  const text = KIND_TEXT[kind];
  const hasBase = props.kind === "app" ? props.hasApp : props.hasWidget;
  const t = useT();
  const locale = useLocale();
  const promptId = React.useId();
  const promptHintId = React.useId();
  const problemId = React.useId();
  const [loaded, setLoaded] = React.useState<Loaded>({ state: "loading" });
  const [prompt, setPrompt] = React.useState("");
  const [change, setChange] = React.useState(hasBase);
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
        const running = view.builds.find((b) => b.status === "running" && b.kind === kind);
        if (running !== undefined) setFollowId(running.id);
      }
    } catch {
      setLoaded({ state: "failed" });
    }
  }, [kind]);

  React.useEffect(() => {
    void load();
  }, [load]);

  /* The latest callbacks, so a build is followed by one effect however
   * often the page around it renders. */
  const written = React.useRef(props);
  written.current = props;

  const suggest = props.kind === "app" ? props.suggest : null;
  React.useEffect(() => {
    if (suggest === null || suggest === undefined) return;
    setPrompt(suggest.prompt.slice(0, MAX_PROMPT_CHARS));
    setChange(true);
    setProblem(null);
  }, [suggest]);

  React.useEffect(() => {
    if (followId === null) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const look = async (): Promise<void> => {
      try {
        const view = await getBuilderBuild(followId);
        if (stopped) return;
        setBuild(view);
        const now = written.current;
        if (now.kind === "app") now.onProgress?.(view);
        if (view.status === "running") {
          timer = setTimeout(() => void look(), POLL_MS);
          return;
        }
        setFollowId(null);
        if (view.status === "done") {
          let wrote = false;
          if (now.kind === "app") {
            wrote = builtApp(view) !== null;
            if (wrote) now.onWritten(view);
          } else {
            const built = builtFiles(view);
            wrote = built !== null;
            if (built !== null) now.onWritten(built);
          }
          if (wrote) toast.success(t(KIND_TEXT[now.kind ?? "widget"].doneTitle), t(KIND_TEXT[now.kind ?? "widget"].doneNote));
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
    const asked = prompt.trim();
    if (asked.length === 0) {
      setProblem(t(text.problem));
      return;
    }
    if ([...asked].length > MAX_PROMPT_CHARS) {
      setProblem(t("look.builder.problem.promptLong", { max: MAX_PROMPT_CHARS }));
      return;
    }
    setProblem(null);
    setBusy(true);
    try {
      const base = props.kind === "app" ? props.source : props.files;
      const started = await postBuilderBuild({
        prompt: asked,
        kind,
        ...(change && hasBase ? { base } : {}),
        lang: locale,
      });
      setBuild(started);
      if (props.kind === "app") props.onProgress?.(started);
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
    if (b.status === "done") return t(text.done);
    if (b.status === "stopped") return t(text.stopped);
    if (b.status !== "failed") return null;
    return t(FAULT_TEXT[b.fault ?? "upstream"]);
  };

  return (
    <div data-testid="builder-panel" className="flex flex-col gap-4">
      <p className="text-[13px] leading-relaxed text-muted">{t(text.intro)}</p>
      <p className="rounded-card border border-line bg-sunk p-3.5 text-[13px] leading-relaxed text-muted" data-testid="builder-local">
        {t(text.local, { model: view.model })}
      </p>

      <Label htmlFor={promptId} className="flex flex-col gap-1.5">
        <span>{t(text.field)}</span>
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
          {t(text.hint)}
        </LabelHint>
      </Label>

      {hasBase && (
        <label className="flex items-center gap-2 text-[13px] text-ink">
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={change}
            disabled={busy || running}
            onChange={(event) => setChange(event.target.checked)}
          />
          {t(text.change)}
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
            {build?.steps.at(-1)?.kind === "try" && build.steps.at(-1)?.running === true
              ? t("look.builder.step.try.running")
              : build?.fixing === true
                ? t(text.fixing, { lines: build.lines })
                : build !== null && build.lines > 0
                  ? t(text.writing, { lines: build.lines })
                  : t("look.builder.reading")}
          </span>
        </div>
      )}
      {build !== null && build.steps.length > 0 && <BuildSteps steps={build.steps} lines={build.lines} />}
      {build !== null && !running && outcome(build) !== null && (
        <Alert variant={build.status === "done" ? "ok" : "warn"} role="status" data-testid="builder-outcome">
          <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <AlertDescription>
            <p>{outcome(build)}</p>
            {build.at_limit && <p>{t(text.atLimit)}</p>}
            {build.notes !== null && build.notes.length > 0 && <p className="text-muted">{build.notes}</p>}
          </AlertDescription>
        </Alert>
      )}

      {isQwenCoder(view.model) && (
        <p className="text-[12.5px] leading-relaxed text-muted" data-testid="builder-terms">
          {t("look.builder.terms")}{" "}
          {QWEN_TERMS.map((term, i) => (
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

/** A step in the owner's words. */
function stepText(step: BuildStep, lines: number, t: ReturnType<typeof useT>): string {
  const found = step.problems.length > 0;
  switch (step.kind) {
    case "write":
      return step.running ? t("look.builder.step.write.running", { lines }) : t("look.builder.step.write.done");
    case "fix":
      return step.running ? t("look.builder.step.fix.running", { lines }) : t("look.builder.step.fix.done");
    case "check":
      if (step.running) return t("look.builder.step.check.running");
      return found ? t("look.builder.step.check.found") : t("look.builder.step.check.ok");
    case "try":
      if (step.running) return t("look.builder.step.try.running");
      if (step.skipped) return t("look.builder.step.try.skipped");
      return found ? t("look.builder.step.try.found") : t("look.builder.step.try.ok");
  }
}

/** What the build has done so far, a line per step, with what each check
 *  and try found. */
function BuildSteps({ steps, lines }: { steps: BuildStep[]; lines: number }) {
  const t = useT();
  return (
    <ol aria-label={t("look.builder.steps")} data-testid="builder-steps" className="flex flex-col gap-2 text-[13px]">
      {steps.map((step, i) => {
        const found = step.problems.length > 0;
        return (
          <li key={`${step.kind}-${i}`} data-step={step.kind} className="flex gap-2.5">
            <span
              aria-hidden="true"
              className={cn(
                "mt-[0.45rem] size-2 shrink-0 rounded-full",
                step.running ? "bg-accent" : found ? "bg-warn" : step.skipped ? "bg-line" : "bg-ok",
              )}
            />
            <div className="min-w-0">
              <p className={step.running ? "text-ink" : "text-muted"}>{stepText(step, lines, t)}</p>
              {found && (
                <ul className="mt-1 flex list-disc flex-col gap-0.5 pl-4 text-[12.5px] text-muted">
                  {step.problems.map((problem) => (
                    <li key={problem} className="break-words">
                      {problem}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}
