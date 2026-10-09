/* Write a widget by hand.
 *
 * The other editor (custom-editor.tsx) builds a widget out of a small
 * expression language, because nothing typed into the admin page can be run
 * by the admin page. This one takes the full thing — HTML, style, script —
 * and keeps it on the box, where lososd stores the text and the board draws
 * it in a sandboxed frame of its own (hand-frame.tsx). The preview below IS
 * that frame, so what it shows is what a tile will show.
 *
 * Saving writes the box (POST /api/look/widgets) and nothing else: no
 * rebuild, no overrides.nix. A new widget is also put on this browser's
 * board, which is the thing the owner opened the gallery to do.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { FieldError, Input } from "@/components/ui/input";
import { Label, LabelHint } from "@/components/ui/label";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { lookLimits, saveHandWidget, type HandSpan, type HandWidget } from "@/lib/look";
import { cn } from "@/lib/utils";
import { BuilderPanel } from "./builder-panel";
import { HandFrame } from "./hand-frame";
import { METRIC_NAMES } from "./types";

/** What a new widget opens with: a figure, a caption, one reading. The
 *  caption is in the language on screen; once saved it is the owner's text. */
export function templateSource(foot: string): string {
  return [
    "<style>",
    "  .figure { font: 600 28px/1.1 var(--font-ui, system-ui); color: var(--accent); }",
    "  .foot { color: var(--muted); font-size: 12px; margin-top: 6px; }",
    "</style>",
    '<div class="figure" id="figure">…</div>',
    `<div class="foot">${foot.replace(/[<>&]/g, "")}</div>`,
    "<script>",
    '  losos.metric("storage.bytes").then((m) => {',
    "    const gib = (m.freeBytes ?? 0) / 2 ** 30;",
    '    document.getElementById("figure").textContent = gib.toFixed(1) + " GiB";',
    "  });",
    "</script>",
    "",
  ].join("\n");
}

export interface HandEditorProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The widget to edit, or undefined to write a new one. */
  widget?: HandWidget;
  /** Called with the widget as the box kept it. `created` is true for a
   *  new one, so the caller can put it on the board. */
  onSaved: (widget: HandWidget, created: boolean) => void;
}

export function HandEditor({ open, onOpenChange, widget, onSaved }: HandEditorProps) {
  const t = useT();
  const limits = lookLimits();
  const [name, setName] = React.useState(widget?.name ?? "");
  const [span, setSpan] = React.useState<HandSpan>(widget?.span ?? "half");
  const [source, setSource] = React.useState(
    () => widget?.source ?? templateSource(t("look.editor.templateFoot")),
  );
  const [problem, setProblem] = React.useState<string | null>(null);
  const [frameError, setFrameError] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [tab, setTab] = React.useState("write");
  /* The builder asks the edge when it mounts, so it mounts the first time
   * its tab is opened and stays mounted from then on. */
  const [builderOpened, setBuilderOpened] = React.useState(false);
  const titleId = React.useId();
  const hintId = React.useId();
  const nameId = React.useId();
  const spanId = React.useId();
  const sourceId = React.useId();
  const problemId = React.useId();

  /* The preview follows the source a moment after the last keystroke: a
   * frame per keystroke would be a page load per keystroke. */
  const [previewSource, setPreviewSource] = React.useState(source);
  React.useEffect(() => {
    const timer = setTimeout(() => {
      setPreviewSource(source);
      setFrameError(null);
    }, 500);
    return () => clearTimeout(timer);
  }, [source]);

  const used = new TextEncoder().encode(source).length;
  const kib = (bytes: number): string => (bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0);

  const check = (): string | null => {
    if (name.trim().length === 0) return t("look.editor.problem.name");
    if ([...name.trim()].length > limits.nameChars) {
      return t("look.editor.problem.nameLong", { max: limits.nameChars });
    }
    if (source.trim().length === 0) return t("look.editor.problem.source");
    if (used > limits.sourceBytes) {
      return t("look.editor.problem.sourceLong", { kb: Math.floor(limits.sourceBytes / 1024) });
    }
    return null;
  };

  const save = async (): Promise<void> => {
    const fault = check();
    if (fault !== null) {
      setProblem(fault);
      return;
    }
    setBusy(true);
    setProblem(null);
    try {
      const kept = await saveHandWidget({
        ...(widget === undefined ? {} : { id: widget.id }),
        name: name.trim(),
        span,
        source,
      });
      toast.success(t("look.editor.saved", { name: kept.name }));
      onSaved(kept, widget === undefined);
      onOpenChange(false);
    } catch (error) {
      setProblem(error instanceof Error && error.message.length > 0 ? error.message : t("look.editor.notSaved"));
    } finally {
      setBusy(false);
    }
  };

  const title = widget === undefined ? t("look.editor.title") : t("look.editor.editTitle", { name: widget.name });

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!busy) onOpenChange(next);
      }}
      labelledBy={titleId}
      describedBy={hintId}
      dialogClassName="w-[min(58rem,calc(100vw-2rem))] max-w-[58rem]"
    >
      <DialogHeader>
        <DialogTitle id={titleId}>{title}</DialogTitle>
        <DialogDescription id={hintId}>{t("look.editor.description")}</DialogDescription>
      </DialogHeader>

      <DialogBody>
        <Tabs
          value={tab}
          onValueChange={(next) => {
            setTab(next);
            if (next === "claude") setBuilderOpened(true);
          }}
        >
          <TabsList>
            <TabsTrigger value="write">{t("look.editor.tab.write")}</TabsTrigger>
            <TabsTrigger value="claude">{t("look.editor.tab.claude")}</TabsTrigger>
            <TabsTrigger value="help">{t("look.editor.tab.help")}</TabsTrigger>
          </TabsList>

          {/* Kept mounted so a build in progress is still followed, and its
           * widget lands in the editor, while the owner is on another tab. */}
          <TabsContent value="claude" keepMounted>
            {builderOpened && (
              <BuilderPanel
                source={source}
                hasWidget={widget !== undefined || source !== templateSource(t("look.editor.templateFoot"))}
                onWritten={(written) => {
                  setSource(written);
                  setProblem(null);
                  setTab("write");
                }}
              />
            )}
          </TabsContent>

          <TabsContent value="write">
            <div className="grid gap-5 md:grid-cols-[minmax(0,1fr)_minmax(0,20rem)]">
              <div className="flex flex-col gap-3.5">
                <div className="grid gap-3.5 sm:grid-cols-[minmax(0,1fr)_auto]">
                  <Label htmlFor={nameId} className="flex flex-col gap-1.5">
                    <span>{t("look.editor.field.name")}</span>
                    <Input
                      id={nameId}
                      value={name}
                      maxLength={limits.nameChars}
                      disabled={busy}
                      onChange={(event) => {
                        setName(event.target.value);
                        setProblem(null);
                      }}
                    />
                  </Label>
                  <Label htmlFor={spanId} className="flex flex-col gap-1.5">
                    <span>{t("look.editor.field.span")}</span>
                    <NativeSelect
                      id={spanId}
                      value={span}
                      disabled={busy}
                      onChange={(event) => setSpan(event.target.value === "full" ? "full" : "half")}
                    >
                      <NativeSelectOption value="half">{t("look.widgets.half")}</NativeSelectOption>
                      <NativeSelectOption value="full">{t("look.widgets.full")}</NativeSelectOption>
                    </NativeSelect>
                  </Label>
                </div>

                {/* Where the code gets pasted, so the warning sits right above
                 * it: a widget is script the owner chose to run, and the
                 * sandbox limits what it can reach, not what it can say. */}
                <Alert variant="warn" role="note" data-testid="hand-paste-note">
                  <HugeiconsIcon icon={Alert02Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  <AlertTitle>{t("look.editor.pasted.title")}</AlertTitle>
                  <AlertDescription>
                    <p>{t("look.editor.pasted.body")}</p>
                  </AlertDescription>
                </Alert>

                <Label htmlFor={sourceId} className="flex flex-col gap-1.5">
                  <span>{t("look.editor.field.source")}</span>
                  <textarea
                    id={sourceId}
                    rows={16}
                    spellCheck={false}
                    autoCapitalize="off"
                    autoCorrect="off"
                    value={source}
                    disabled={busy}
                    aria-describedby={problem !== null ? problemId : undefined}
                    aria-invalid={problem !== null}
                    onChange={(event) => {
                      setSource(event.target.value);
                      setProblem(null);
                    }}
                    className={cn(
                      "numeric w-full resize-y rounded-control border border-line bg-surface p-3",
                      "font-mono text-[12.5px] leading-relaxed text-ink",
                      "focus-visible:border-accent focus-visible:outline-none",
                      "focus-visible:ring-2 focus-visible:ring-accent/35",
                      "aria-invalid:border-crit",
                    )}
                  />
                  <LabelHint className="font-normal">
                    {t("look.editor.sourceHint", {
                      used: kib(used),
                      max: Math.floor(limits.sourceBytes / 1024),
                    })}
                  </LabelHint>
                </Label>
                <FieldError id={problemId}>{problem}</FieldError>
              </div>

              <div className="flex flex-col gap-2">
                <p className="text-[12.5px] font-medium text-muted">{t("look.editor.preview")}</p>
                <div
                  data-testid="hand-preview"
                  className="rounded-card border border-line bg-surface p-3 shadow-card"
                >
                  <HandFrame
                    source={previewSource}
                    name={name.trim().length > 0 ? name : t("look.editor.preview")}
                    onError={setFrameError}
                  />
                </div>
                {frameError !== null && (
                  <p role="status" className="text-[12.5px] leading-snug text-crit">
                    {t("look.frame.threw", { message: frameError })}
                  </p>
                )}
              </div>
            </div>
          </TabsContent>

          <TabsContent value="help">
            <div className="flex flex-col gap-4 text-[13px] leading-relaxed text-muted">
              <p>{t("look.editor.help.sandbox")}</p>
              <p>{t("look.editor.help.api", { names: METRIC_NAMES.join(", ") })}</p>
              <p>
                {t("look.editor.help.limits", {
                  widgets: limits.widgets,
                  kb: Math.floor(limits.sourceBytes / 1024),
                })}
              </p>
              <p className="font-medium text-ink">{t("look.editor.help.example")}</p>
              <pre
                className={cn(
                  "numeric overflow-x-auto rounded-control border border-line bg-sunk p-3",
                  "font-mono text-[12px] leading-relaxed text-ink",
                )}
              >
                {templateSource(t("look.editor.templateFoot"))}
              </pre>
            </div>
          </TabsContent>
        </Tabs>
      </DialogBody>

      <DialogFooter>
        {busy && <Spinner label={t("look.editor.saving")} className="mr-auto text-muted" />}
        <Button variant="secondary" disabled={busy} onClick={() => onOpenChange(false)}>
          {t("look.editor.cancel")}
        </Button>
        <Button disabled={busy} onClick={() => void save()}>
          {widget === undefined ? t("look.editor.saveAndAdd") : t("look.editor.save")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
