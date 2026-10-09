/* Write a widget by hand.
 *
 * The other editor (custom-editor.tsx) builds a widget out of a small
 * expression language, because nothing typed into the admin page can be run
 * by the admin page. This one takes the full thing — HTML, style, script,
 * in `index.html` and the files it links — and keeps it on the box, where
 * lososd stores the text and the board draws it in a sandboxed frame of its
 * own (hand-frame.tsx). The preview below IS that frame, so what it shows is
 * what a tile will show. The code is typed into CodeMirror
 * (components/code-editor.tsx), which completes the frame's own API.
 *
 * Saving writes the box (POST /api/look/widgets) and nothing else: no
 * rebuild, no overrides.nix. A new widget is also put on this browser's
 * board, which is the thing the owner opened the gallery to do.
 */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, Delete02Icon, PlusSignIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { CodeEditor } from "@/components/code-editor";
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
import { lookLimits, saveHandWidget, type HandSpan, type HandWidget, type WidgetFile } from "@/lib/look";
import { cn } from "@/lib/utils";
import { BuilderPanel } from "./builder-panel";
import {
  ENTRY_FILE,
  isFileName,
  languageOf,
  sameFiles,
  sortFiles,
  widgetBytes,
  withContent,
} from "./files";
import { HandFrame } from "./hand-frame";
import { METRIC_NAMES } from "./types";

/** What a new widget opens with: a figure, a caption, one reading, in the
 *  three files a widget usually has. The caption is in the language on
 *  screen; once saved it is the owner's text. */
export function templateFiles(foot: string): WidgetFile[] {
  return [
    {
      name: ENTRY_FILE,
      content: [
        '<link rel="stylesheet" href="style.css">',
        "",
        '<div class="figure" id="figure">…</div>',
        `<div class="foot">${foot.replace(/[<>&]/g, "")}</div>`,
        "",
        '<script src="app.js"></script>',
        "",
      ].join("\n"),
    },
    {
      name: "app.js",
      content: [
        'losos.metric("storage.bytes").then((m) => {',
        "  const gib = (m.freeBytes ?? 0) / 2 ** 30;",
        '  document.getElementById("figure").textContent = gib.toFixed(1) + " GiB";',
        "});",
        "",
      ].join("\n"),
    },
    {
      name: "style.css",
      content: [
        ".figure { font: 600 28px/1.1 var(--font-ui, system-ui); color: var(--accent); }",
        ".foot { color: var(--muted); font-size: 12px; margin-top: 6px; }",
        "",
      ].join("\n"),
    },
  ];
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
  const template = React.useMemo(() => templateFiles(t("look.editor.templateFoot")), [t]);
  const [files, setFiles] = React.useState<WidgetFile[]>(() => sortFiles(widget?.files ?? template));
  const [current, setCurrent] = React.useState(ENTRY_FILE);
  const [adding, setAdding] = React.useState<string | null>(null);
  const [fileProblem, setFileProblem] = React.useState<string | null>(null);
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
  const filesId = React.useId();
  const newFileId = React.useId();
  const sizeHintId = React.useId();
  const problemId = React.useId();

  /* The preview follows the files a moment after the last keystroke: a
   * frame per keystroke would be a page load per keystroke. */
  const [previewFiles, setPreviewFiles] = React.useState(files);
  React.useEffect(() => {
    const timer = setTimeout(() => {
      setPreviewFiles(files);
      setFrameError(null);
    }, 500);
    return () => clearTimeout(timer);
  }, [files]);

  const used = widgetBytes(files);
  const kib = (bytes: number): string => (bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0);
  const names = files.map((file) => file.name);
  const shown = files.find((file) => file.name === current) ?? files[0];

  const check = (): string | null => {
    if (name.trim().length === 0) return t("look.editor.problem.name");
    if ([...name.trim()].length > limits.nameChars) {
      return t("look.editor.problem.nameLong", { max: limits.nameChars });
    }
    const entry = files.find((file) => file.name === ENTRY_FILE);
    if (entry === undefined || entry.content.trim().length === 0) return t("look.editor.problem.source");
    if (used > limits.widgetBytes) {
      return t("look.editor.problem.sourceLong", { kb: Math.floor(limits.widgetBytes / 1024) });
    }
    return null;
  };

  const addFile = (): void => {
    const candidate = (adding ?? "").trim().toLowerCase();
    if (!isFileName(candidate, limits.fileKinds, limits.fileNameChars)) {
      setFileProblem(t("look.editor.problem.fileName", { kinds: limits.fileKinds.join(", ") }));
      return;
    }
    if (names.includes(candidate)) {
      setFileProblem(t("look.editor.problem.fileTaken", { name: candidate }));
      return;
    }
    if (files.length >= limits.files) {
      setFileProblem(t("look.editor.problem.tooManyFiles", { max: limits.files }));
      return;
    }
    setFiles((before) => sortFiles([...before, { name: candidate, content: "" }]));
    setCurrent(candidate);
    setAdding(null);
    setFileProblem(null);
  };

  const removeFile = (target: string): void => {
    if (target === ENTRY_FILE) return;
    setFiles((before) => before.filter((file) => file.name !== target));
    setCurrent(ENTRY_FILE);
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
        files,
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
                files={files}
                hasWidget={widget !== undefined || !sameFiles(files, template)}
                onWritten={(written) => {
                  setFiles(sortFiles(written));
                  setCurrent(ENTRY_FILE);
                  setAdding(null);
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

                <div className="flex flex-col gap-1.5">
                  <p id={filesId} className="text-sm font-medium text-ink">
                    {t("look.editor.field.files")}
                  </p>
                  <div role="group" aria-labelledby={filesId} className="flex flex-wrap items-center gap-1">
                    {files.map((file) => (
                      <Button
                        key={file.name}
                        variant={file.name === shown?.name ? "secondary" : "ghost"}
                        size="xs"
                        aria-pressed={file.name === shown?.name}
                        data-testid="hand-file"
                        className="font-mono"
                        onClick={() => setCurrent(file.name)}
                      >
                        {file.name}
                      </Button>
                    ))}
                    {shown !== undefined && shown.name !== ENTRY_FILE && (
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        disabled={busy}
                        aria-label={t("look.editor.removeFile", { name: shown.name })}
                        title={t("look.editor.removeFile", { name: shown.name })}
                        onClick={() => removeFile(shown.name)}
                      >
                        <HugeiconsIcon icon={Delete02Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                      </Button>
                    )}
                    {adding === null ? (
                      <Button
                        variant="ghost"
                        size="xs"
                        disabled={busy || files.length >= limits.files}
                        onClick={() => {
                          setAdding("");
                          setFileProblem(null);
                        }}
                      >
                        <HugeiconsIcon icon={PlusSignIcon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                        {t("look.editor.addFile")}
                      </Button>
                    ) : (
                      <form
                        className="flex items-center gap-1"
                        onSubmit={(event) => {
                          event.preventDefault();
                          addFile();
                        }}
                      >
                        <Input
                          id={newFileId}
                          aria-label={t("look.editor.newFile")}
                          placeholder="chart.js"
                          autoFocus
                          spellCheck={false}
                          autoCapitalize="off"
                          value={adding}
                          maxLength={limits.fileNameChars}
                          aria-invalid={fileProblem !== null}
                          className="h-7 w-36 font-mono text-[12.5px]"
                          onChange={(event) => {
                            setAdding(event.target.value);
                            setFileProblem(null);
                          }}
                          onKeyDown={(event) => {
                            if (event.key === "Escape") {
                              event.preventDefault();
                              event.stopPropagation();
                              setAdding(null);
                              setFileProblem(null);
                            }
                          }}
                        />
                        <Button type="submit" variant="secondary" size="xs">
                          {t("look.editor.newFileAdd")}
                        </Button>
                        <Button
                          variant="ghost"
                          size="xs"
                          onClick={() => {
                            setAdding(null);
                            setFileProblem(null);
                          }}
                        >
                          {t("look.editor.cancel")}
                        </Button>
                      </form>
                    )}
                  </div>
                  <FieldError>{fileProblem}</FieldError>
                  {shown !== undefined && (
                    <CodeEditor
                      key={shown.name}
                      value={shown.content}
                      language={languageOf(shown.name)}
                      label={t("look.editor.fileLabel", { name: shown.name })}
                      files={names}
                      disabled={busy}
                      invalid={problem !== null}
                      onChange={(content) => {
                        setFiles((before) => withContent(before, shown.name, content));
                        setProblem(null);
                      }}
                    />
                  )}
                  <LabelHint id={sizeHintId} className="font-normal">
                    {t("look.editor.sourceHint", {
                      used: kib(used),
                      max: Math.floor(limits.widgetBytes / 1024),
                    })}
                    {" · "}
                    {t("look.editor.completeHint")}
                  </LabelHint>
                </div>
                <FieldError id={problemId}>{problem}</FieldError>
              </div>

              <div className="flex flex-col gap-2">
                <p className="text-[12.5px] font-medium text-muted">{t("look.editor.preview")}</p>
                <div
                  data-testid="hand-preview"
                  className="rounded-card border border-line bg-surface p-3 shadow-card"
                >
                  <HandFrame
                    files={previewFiles}
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
              <p>{t("look.editor.help.files")}</p>
              <p>
                {t("look.editor.help.limits", {
                  widgets: limits.widgets,
                  files: limits.files,
                  kb: Math.floor(limits.widgetBytes / 1024),
                })}
              </p>
              <p className="font-medium text-ink">{t("look.editor.help.example")}</p>
              {sortFiles(template).map((file) => (
                <div key={file.name} className="flex flex-col gap-1">
                  <p className="font-mono text-[12px] text-ink">{file.name}</p>
                  <pre
                    className={cn(
                      "numeric overflow-x-auto rounded-control border border-line bg-sunk p-3",
                      "font-mono text-[12px] leading-relaxed text-ink",
                    )}
                  >
                    {file.content}
                  </pre>
                </div>
              ))}
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
