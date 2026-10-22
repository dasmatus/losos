/* The Markdown document's formatting bar: a button for each thing Obsidian
 * and GitHub's Markdown can write, in the look of the Office bar
 * (../toolbar.tsx). The buttons change the text in the editor (format.ts);
 * pressed is what the cursor is in. */

import type { EditorView } from "@codemirror/view";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Add01Icon,
  BookmarkAdd01Icon,
  CheckListIcon,
  CodeSquareIcon,
  FileAttachmentIcon,
  HashtagIcon,
  HighlighterIcon,
  Image01Icon,
  KeyboardIcon,
  LeftToRightBlockQuoteIcon,
  LeftToRightListBulletIcon,
  LeftToRightListNumberIcon,
  Link01Icon,
  Link04Icon,
  ListViewIcon,
  Note01Icon,
  RedoIcon,
  SigmaIcon,
  SolidLine01Icon,
  SourceCodeIcon,
  SquareRootSquareIcon,
  Table01Icon,
  TextBoldIcon,
  TextItalicIcon,
  TextStrikethroughIcon,
  TextSubscriptIcon,
  TextSuperscriptIcon,
  UndoIcon,
  UnfoldMoreIcon,
  ViewOffIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { CALLOUTS, insertCallout, redo, run, setHeading, undo, type Action, type Callout, type Words } from "./format";

interface Tool {
  action: Action;
  icon: IconSvgElement;
  label: MessageKey;
  keys?: string;
  /** Shown pressed while the cursor is in it. */
  toggle?: boolean;
}

const TEXT: Tool[] = [
  { action: "bold", icon: TextBoldIcon, label: "office.tool.bold", keys: "Ctrl B", toggle: true },
  { action: "italic", icon: TextItalicIcon, label: "office.tool.italic", keys: "Ctrl I", toggle: true },
  { action: "strike", icon: TextStrikethroughIcon, label: "office.tool.strikethrough", keys: "Ctrl Shift X", toggle: true },
  { action: "highlight", icon: HighlighterIcon, label: "office.md.highlight", keys: "Ctrl Shift H", toggle: true },
  { action: "code", icon: SourceCodeIcon, label: "office.md.code", keys: "Ctrl E", toggle: true },
  { action: "sub", icon: TextSubscriptIcon, label: "office.md.sub", toggle: true },
  { action: "sup", icon: TextSuperscriptIcon, label: "office.md.sup", toggle: true },
  { action: "kbd", icon: KeyboardIcon, label: "office.md.kbd", toggle: true },
  { action: "comment", icon: ViewOffIcon, label: "office.md.comment", toggle: true },
];

const LINES: Tool[] = [
  { action: "bullets", icon: LeftToRightListBulletIcon, label: "office.tool.bullets", toggle: true },
  { action: "numbers", icon: LeftToRightListNumberIcon, label: "office.tool.numbering", toggle: true },
  { action: "tasks", icon: CheckListIcon, label: "office.md.tasks", toggle: true },
  { action: "quote", icon: LeftToRightBlockQuoteIcon, label: "office.md.quote", toggle: true },
];

const BLOCKS: Tool[] = [
  { action: "codeBlock", icon: CodeSquareIcon, label: "office.md.codeBlock", toggle: true },
  { action: "mathInline", icon: SigmaIcon, label: "office.md.mathInline", toggle: true },
  { action: "mathBlock", icon: SquareRootSquareIcon, label: "office.md.mathBlock" },
  { action: "table", icon: Table01Icon, label: "office.tool.table" },
  { action: "details", icon: UnfoldMoreIcon, label: "office.md.details" },
  { action: "rule", icon: SolidLine01Icon, label: "office.md.rule" },
];

const LINKS: Tool[] = [
  { action: "link", icon: Link01Icon, label: "office.tool.link", keys: "Ctrl K" },
  { action: "image", icon: Image01Icon, label: "office.md.image" },
  { action: "wikilink", icon: Link04Icon, label: "office.md.wikilink" },
  { action: "embed", icon: FileAttachmentIcon, label: "office.md.embed" },
  { action: "tag", icon: HashtagIcon, label: "office.md.tag" },
  { action: "blockId", icon: BookmarkAdd01Icon, label: "office.md.blockId" },
  { action: "footnote", icon: Note01Icon, label: "office.md.footnote" },
  { action: "properties", icon: ListViewIcon, label: "office.md.properties" },
];

const SLIDE: Tool = { action: "slide", icon: Add01Icon, label: "office.tool.newSlide" };

const CALLOUT_LABEL: Record<Callout, MessageKey> = {
  note: "office.md.callout.note",
  tip: "office.md.callout.tip",
  important: "office.md.callout.important",
  warning: "office.md.callout.warning",
  caution: "office.md.callout.caution",
  abstract: "office.md.callout.abstract",
  info: "office.md.callout.info",
  todo: "office.md.callout.todo",
  success: "office.md.callout.success",
  question: "office.md.callout.question",
  failure: "office.md.callout.failure",
  danger: "office.md.callout.danger",
  bug: "office.md.callout.bug",
  example: "office.md.callout.example",
  quote: "office.md.callout.quote",
};

const HEADINGS: ReadonlyArray<readonly [number, MessageKey]> = [
  [0, "office.style.normal"],
  [1, "office.style.heading1"],
  [2, "office.style.heading2"],
  [3, "office.style.heading3"],
  [4, "office.md.heading4"],
  [5, "office.md.heading5"],
  [6, "office.md.heading6"],
];

export function MarkdownToolbar({
  view,
  active,
  heading,
  deck,
  words,
  disabled,
}: {
  view: EditorView | null;
  active: ReadonlySet<Action>;
  heading: number;
  deck: boolean;
  words: Words;
  disabled: boolean;
}) {
  const t = useT();
  const off = disabled || view === null;

  const icon = (key: string, label: string, image: IconSvgElement, onClick: () => void, pressed?: boolean, keys?: string, testid?: string) => (
    <Tooltip key={key}>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={label}
            aria-pressed={pressed}
            aria-keyshortcuts={keys?.replace(/ /g, "+").replace("Ctrl", "Control")}
            data-action={testid ?? key}
            disabled={off}
            className={cn("size-7", pressed && "bg-accent-wash text-accent hover:bg-accent-wash")}
            // Keep the editor's selection: the button acts on it.
            onMouseDown={(e) => e.preventDefault()}
            onClick={onClick}
          />
        }
      >
        <HugeiconsIcon icon={image} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </TooltipTrigger>
      <TooltipContent side="bottom">
        {label}
        {keys !== undefined && <span className="ml-2 text-faint">{keys}</span>}
      </TooltipContent>
    </Tooltip>
  );

  const button = (tool: Tool) =>
    icon(tool.action, t(tool.label), tool.icon, () => view !== null && run(view, tool.action, words, deck), tool.toggle ? active.has(tool.action) : undefined, tool.keys);

  return (
    <div
      role="toolbar"
      aria-label={t("office.toolbar")}
      className="flex min-h-11 shrink-0 flex-wrap items-center gap-x-0.5 gap-y-1 border-b border-hair bg-surface px-2 py-1.5 sm:px-3"
      data-testid="md-toolbar"
    >
      {icon("undo", t("office.tool.undo"), UndoIcon, () => view !== null && undo(view), undefined, "Ctrl Z")}
      {icon("redo", t("office.tool.redo"), RedoIcon, () => view !== null && redo(view), undefined, "Ctrl Y")}
      <Rule />
      {deck && (
        <>
          {button(SLIDE)}
          <Rule />
        </>
      )}
      <NativeSelect
        aria-label={t("office.tool.style")}
        className="w-32 [&_select]:h-8"
        value={String(heading)}
        disabled={off}
        data-testid="md-heading"
        onChange={(e) => {
          if (view !== null) setHeading(view, Number(e.target.value));
        }}
      >
        {HEADINGS.map(([level, key]) => (
          <NativeSelectOption key={level} value={String(level)}>
            {t(key)}
          </NativeSelectOption>
        ))}
      </NativeSelect>
      <Rule />
      {TEXT.map(button)}
      <Rule />
      {LINES.map(button)}
      <Rule />
      {BLOCKS.map(button)}
      <NativeSelect
        aria-label={t("office.md.callout")}
        className="ml-0.5 w-28 [&_select]:h-8"
        value=""
        disabled={off}
        data-testid="md-callout"
        onChange={(e) => {
          const kind = e.target.value as Callout;
          if (view !== null && CALLOUTS.includes(kind)) insertCallout(view, kind, words);
        }}
      >
        <NativeSelectOption value="">{t("office.md.callout")}</NativeSelectOption>
        {CALLOUTS.map((kind) => (
          <NativeSelectOption key={kind} value={kind}>
            {t(CALLOUT_LABEL[kind])}
          </NativeSelectOption>
        ))}
      </NativeSelect>
      <Rule />
      {LINKS.map(button)}
    </div>
  );
}

function Rule() {
  return <span aria-hidden="true" className="mx-1.5 h-5 w-px shrink-0 bg-line" />;
}
