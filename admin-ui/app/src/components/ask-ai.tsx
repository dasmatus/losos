/* Ask the box's own AI about something the person is looking at: a mail
 * thread, a file in LosOS Cloud, a note.
 *
 *   <AskAiButton label="Summarise" sources={() => [{ title, text }]}
 *     question="Summarise this in a few sentences." context="mail" />
 *
 * The page reads the material with the person's own session and sends it to
 * /api/assistant (backend/plugins/assistant), so the model sees only what
 * the person chose to show it, and nothing leaves the box. With `question`
 * the ask goes at once; without one the person types it. The button is drawn
 * only while the box's model is pulled and answering: a box without one has
 * nothing to ask, and a button there could only fail. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { AiMagicIcon, Copy01Icon, NoteAddIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  ApiError,
  askAssistant,
  getAssistant,
  getAssistantAsk,
  stopAssistantAsk,
  type AssistantAsk,
  type AssistantSource,
} from "@/lib/api";
import { saveToCloud } from "@/lib/cloud-files";
import { useLocale, useT } from "@/lib/i18n-react";

/** Where an ask was made from, for the help link and the tests. */
export type AskContext = "mail" | "cloud" | "notes" | "chat" | "git" | "office" | "diagrams";

let status: Promise<boolean> | null = null;

/** Whether the box's AI answers, asked once per page. */
export function assistantReady(): Promise<boolean> {
  status ??= getAssistant().then(
    (s) => s.available && s.ready === true,
    () => false,
  );
  return status;
}

export function useAssistant(): boolean {
  const [ready, setReady] = React.useState(false);
  React.useEffect(() => {
    let live = true;
    void assistantReady().then((yes) => live && setReady(yes));
    return () => {
      live = false;
    };
  }, []);
  return ready;
}

export interface AskAiProps {
  /** What the model reads, gathered when the person asks. */
  sources: () => AssistantSource[] | Promise<AssistantSource[]>;
  /** Ask this at once; without it the person types the question. */
  question?: string;
  context: AskContext;
}

export function AskAiButton({ compact = false, label, ...ask }: AskAiProps & { compact?: boolean; label: string }) {
  const ready = useAssistant();
  const [open, setOpen] = React.useState(false);
  if (!ready) return null;
  const icon = <HugeiconsIcon icon={AiMagicIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />;
  return (
    <>
      {compact ? (
        <Tooltip>
          <TooltipTrigger
            render={
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={label}
                data-action="ask-ai"
                onClick={() => setOpen(true)}
              />
            }
          >
            {icon}
          </TooltipTrigger>
          <TooltipContent side="bottom">{label}</TooltipContent>
        </Tooltip>
      ) : (
        <Button variant="ghost" size="sm" data-action="ask-ai" onClick={() => setOpen(true)}>
          {icon}
          {label}
        </Button>
      )}
      <AskAiDialog open={open} onOpenChange={setOpen} {...ask} />
    </>
  );
}

type Run =
  | { state: "asking" }
  | { state: "reading" }
  | { state: "waiting" }
  | { state: "answering"; ask: AssistantAsk }
  | { state: "error"; message: string };

const POLL_MS = 1000;
const BUSY_RETRY_MS = 4000;

const sleep = (ms: number, signal: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(resolve, ms);
    signal.addEventListener("abort", () => {
      window.clearTimeout(timer);
      reject(new DOMException("stopped", "AbortError"));
    });
  });

const finished = (a: AssistantAsk) => a.state === "done" || a.state === "failed" || a.state === "stopped";

/** The dialog on its own, for a caller that opens it from a menu. */
export function AskAiDialog({
  open,
  onOpenChange,
  sources,
  question: preset,
  context,
  use,
}: AskAiProps & {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Put the answer back where the material came from: a selection it replaces. */
  use?: { label: string; onUse: (answer: string) => void };
}) {
  const t = useT();
  const lang = useLocale();
  const id = React.useId();
  const [question, setQuestion] = React.useState("");
  const [run, setRun] = React.useState<Run>({ state: "asking" });
  const [title, setTitle] = React.useState("");
  const ctl = React.useRef<AbortController | null>(null);

  const go = React.useCallback(
    async (q: string) => {
      ctl.current?.abort();
      const abort = new AbortController();
      ctl.current = abort;
      const { signal } = abort;
      try {
        setRun({ state: "reading" });
        const material = await sources();
        setTitle(material[0]?.title ?? "");
        let ask: AssistantAsk | null = null;
        while (ask === null) {
          try {
            ask = await askAssistant({ question: q, sources: material, lang }, { signal });
          } catch (error) {
            if (!(error instanceof ApiError && error.status === 429)) throw error;
            setRun({ state: "waiting" });
            await sleep(BUSY_RETRY_MS, signal);
          }
        }
        setRun({ state: "answering", ask });
        while (!finished(ask)) {
          await sleep(POLL_MS, signal);
          ask = await getAssistantAsk(ask.id, { signal });
          setRun({ state: "answering", ask });
        }
      } catch (error) {
        if (signal.aborted) return;
        setRun({ state: "error", message: error instanceof Error ? error.message : "" });
      }
    },
    [sources, lang],
  );

  // Opening with a preset question asks it at once; closing stops waiting.
  React.useEffect(() => {
    if (!open) {
      ctl.current?.abort();
      return;
    }
    setQuestion("");
    if (preset !== undefined) void go(preset);
    else {
      setRun({ state: "asking" });
      void Promise.resolve(sources()).then(
        (m) => setTitle(m[0]?.title ?? ""),
        () => setTitle(""),
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per opening
  }, [open]);

  const stop = () => {
    if (run.state === "answering" && !finished(run.ask)) void stopAssistantAsk(run.ask.id).catch(() => undefined);
    ctl.current?.abort();
    setRun(run.state === "answering" ? { state: "answering", ask: { ...run.ask, state: "stopped" } } : { state: "asking" });
  };

  const answer = run.state === "answering" ? run.ask.answer : "";
  const working =
    run.state === "reading" || run.state === "waiting" || (run.state === "answering" && !finished(run.ask));

  const copy = () =>
    void navigator.clipboard.writeText(answer).then(
      () => toast.done(t("together.ai.copy")),
      () => undefined,
    );

  const saveNote = async () => {
    const name = `${(title || t("together.ai.title")).replace(/[\\/:*?"<>|]+/g, " ").trim().slice(0, 80)}.md`;
    const body = `# ${title || t("together.ai.title")}\n\n${answer}\n`;
    try {
      await saveToCloud(new Blob([body], { type: "text/markdown" }), { folder: "Notes", name });
      toast.done(t("together.save.done"), t("together.save.where", { folder: "Notes" }));
    } catch (error) {
      toast.error(t("together.save.failed"), error instanceof Error ? error.message : undefined, { help: context === "mail" ? "mail" : "apps" });
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      labelledBy={`${id}-title`}
      describedBy={`${id}-private`}
      dialogClassName="w-[min(34rem,calc(100vw-2rem))]"
      data-slot="ask-ai"
      data-context={context}
    >
      <DialogHeader>
        <DialogTitle id={`${id}-title`} className="flex items-center gap-2">
          <HugeiconsIcon icon={AiMagicIcon} size={18} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("together.ai.title")}
        </DialogTitle>
        {title !== "" && <DialogDescription className="truncate">{t("together.ai.about", { title })}</DialogDescription>}
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        {run.state === "asking" ? (
          <form
            id={`${id}-form`}
            className="flex flex-col gap-1.5"
            onSubmit={(event) => {
              event.preventDefault();
              if (question.trim() !== "") void go(question.trim());
            }}
          >
            <label htmlFor={`${id}-q`} className="text-[13px] font-medium">
              {t("together.ai.question")}
            </label>
            <Textarea
              id={`${id}-q`}
              rows={3}
              autoFocus
              maxLength={2000}
              value={question}
              placeholder={t("together.ai.placeholder")}
              onChange={(event) => setQuestion(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) event.currentTarget.form?.requestSubmit();
              }}
            />
          </form>
        ) : run.state === "error" ? (
          <p className="text-[13px] text-crit" role="alert">
            {t("together.ai.failed")}
            {run.message !== "" && `: ${run.message}`}
          </p>
        ) : (
          <div className="flex flex-col gap-2" aria-live="polite" aria-busy={working}>
            {answer !== "" && (
              <p data-slot="ai-answer" className="rounded-control bg-sunk px-3 py-2.5 text-[14px] leading-relaxed whitespace-pre-wrap">
                {answer}
              </p>
            )}
            {working && (
              <p className="flex items-center gap-2 text-[13px] text-muted">
                <Spinner className="size-3.5" label={t("together.ai.thinking")} />
                {run.state === "waiting" ? t("together.ai.waiting") : t("together.ai.thinking")}
              </p>
            )}
            {run.state === "answering" && run.ask.state === "failed" && (
              <p className="text-[13px] text-crit" role="alert">
                {t("together.ai.failed")}
                {run.ask.error !== undefined && `: ${run.ask.error}`}
              </p>
            )}
            {run.state === "answering" && run.ask.cut && <p className="text-[12px] text-faint">{t("together.ai.cut")}</p>}
          </div>
        )}
        <p id={`${id}-private`} className="text-[12px] text-faint">
          {t("together.ai.private")}
        </p>
      </DialogBody>
      <DialogFooter>
        {run.state === "asking" ? (
          <>
            <Button variant="ghost" size="sm" className="mr-auto" onClick={() => void go(t("together.ai.summary"))} data-action="summarise">
              {t("together.ai.summarise")}
            </Button>
            <Button variant="ghost" onClick={() => onOpenChange(false)}>
              {t("together.cancel")}
            </Button>
            <Button type="submit" form={`${id}-form`} disabled={question.trim() === ""} data-action="ask">
              {t("together.ai.send")}
            </Button>
          </>
        ) : working ? (
          <Button variant="secondary" onClick={stop} data-action="stop">
            {t("together.ai.stop")}
          </Button>
        ) : (
          <>
            {answer !== "" && (
              <>
                <Button variant="ghost" size="sm" onClick={copy}>
                  <HugeiconsIcon icon={Copy01Icon} className="size-4" aria-hidden="true" />
                  {t("together.ai.copy")}
                </Button>
                <Button variant="ghost" size="sm" onClick={() => void saveNote()} data-action="save-note">
                  <HugeiconsIcon icon={NoteAddIcon} className="size-4" aria-hidden="true" />
                  {t("together.ai.saveNote")}
                </Button>
              </>
            )}
            <Button variant="secondary" onClick={() => setRun({ state: "asking" })}>
              {t("together.ai.again")}
            </Button>
            {use !== undefined && answer !== "" ? (
              <>
                <Button variant="ghost" onClick={() => onOpenChange(false)}>
                  {t("together.close")}
                </Button>
                <Button onClick={() => use.onUse(answer)} data-action="use-answer">
                  {use.label}
                </Button>
              </>
            ) : (
              <Button onClick={() => onOpenChange(false)}>{t("together.close")}</Button>
            )}
          </>
        )}
      </DialogFooter>
    </Dialog>
  );
}
