/* The bar above the page, as in a word processor: undo, the paragraph
 * style, the marks, lists, and an Insert menu with every component. A
 * button acts on the selection and shows pressed for what the cursor is
 * in; the menu puts a component in and selects it, so its fields come up
 * under the bar (fields.tsx). */

import * as React from "react";
import type { EditorState } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Add01Icon,
  ArrowDown01Icon,
  CheckListIcon,
  CodeSquareIcon,
  HighlighterIcon,
  KeyboardIcon,
  LeftToRightBlockQuoteIcon,
  LeftToRightListBulletIcon,
  LeftToRightListNumberIcon,
  Link01Icon,
  RedoIcon,
  SourceCodeIcon,
  TextBoldIcon,
  TextIndentLessIcon,
  TextIndentMoreIcon,
  TextItalicIcon,
  TextStrikethroughIcon,
  TextSubscriptIcon,
  TextSuperscriptIcon,
  TextUnderlineIcon,
  UndoIcon,
  File01Icon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import {
  applyMark,
  codeBlock,
  indent,
  insertBlock,
  insertColumns,
  insertContainer,
  insertFootnote,
  insertNode,
  insertPageEdge,
  insertSlide,
  insertTable,
  insertTabs,
  listAt,
  markActive,
  marks,
  nodes,
  outdent,
  quoteActive,
  setStyle,
  styleAt,
  toggle,
  toggleList,
  toggleQuote,
  today,
  type ListKind,
  type MarkName,
} from "./commands";
import type { Command } from "prosemirror-state";
import type { Kind } from "./new";
import { STEP } from "./sync";

export type { Kind };

interface MarkTool {
  mark: MarkName;
  icon: IconSvgElement;
  label: MessageKey;
  keys?: string;
}

const MARKS: MarkTool[] = [
  { mark: "strong", icon: TextBoldIcon, label: "office.tool.bold", keys: "Ctrl B" },
  { mark: "em", icon: TextItalicIcon, label: "office.tool.italic", keys: "Ctrl I" },
  { mark: "underline", icon: TextUnderlineIcon, label: "paper.underline", keys: "Ctrl U" },
  { mark: "strike", icon: TextStrikethroughIcon, label: "office.tool.strikethrough", keys: "Ctrl Shift X" },
  { mark: "sub", icon: TextSubscriptIcon, label: "office.md.sub" },
  { mark: "sup", icon: TextSuperscriptIcon, label: "office.md.sup" },
  { mark: "highlight", icon: HighlighterIcon, label: "office.md.highlight", keys: "Ctrl Shift H" },
  { mark: "code", icon: SourceCodeIcon, label: "office.md.code", keys: "Ctrl E" },
  { mark: "kbd", icon: KeyboardIcon, label: "office.md.kbd" },
];

const LISTS: { kind: ListKind; icon: IconSvgElement; label: MessageKey }[] = [
  { kind: "bullets", icon: LeftToRightListBulletIcon, label: "office.tool.bullets" },
  { kind: "numbers", icon: LeftToRightListNumberIcon, label: "office.tool.numbering" },
  { kind: "tasks", icon: CheckListIcon, label: "office.md.tasks" },
];

const STYLES: ReadonlyArray<readonly [number, MessageKey]> = [
  [0, "office.style.normal"],
  [1, "office.style.heading1"],
  [2, "office.style.heading2"],
  [3, "office.style.heading3"],
  [4, "office.md.heading4"],
];

export interface RibbonProps {
  view: EditorView | null;
  state: EditorState | null;
  kind: Kind;
  disabled: boolean;
  me: string;
  undo: () => void;
  redo: () => void;
  onPicture: () => void;
  onPageSetup: () => void;
}

export function Ribbon({ view, state, kind, disabled, me, undo, redo, onPicture, onPageSetup }: RibbonProps) {
  const t = useT();
  const off = disabled || view === null || state === null;
  const run = (cmd: Command) => {
    if (view === null) return;
    cmd(view.state, (tr) => view.dispatch(tr.setMeta(STEP, true)), view);
    view.focus();
  };
  const can = (cmd: Command) => state !== null && cmd(state);

  const icon = (key: string, label: string, image: IconSvgElement, onClick: () => void, pressed?: boolean, keys?: string, enabled = true) => (
    <Tooltip key={key}>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={label}
            aria-pressed={pressed}
            aria-keyshortcuts={keys?.replace(/ /g, "+").replace("Ctrl", "Control")}
            data-action={key}
            disabled={off || !enabled}
            className={cn("size-7", pressed && "bg-accent-wash text-accent hover:bg-accent-wash")}
            // Keep the page's selection: the button acts on it.
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

  const list = state === null ? null : listAt(state);
  const style = state === null ? 0 : styleAt(state);
  const word = (key: MessageKey) => t(key);

  const inserts: { id: string; label: MessageKey; run: () => void; slidesOnly?: boolean; paperOnly?: boolean; sep?: boolean }[] = [
    { id: "table", label: "paper.insert.table", run: () => run(insertTable()) },
    { id: "picture", label: "paper.insert.picture", run: onPicture },
    { id: "chart", label: "paper.insert.chart", run: () => run(insertBlock(nodes["chart"]!.create({ src: "", type: "bar" }))) },
    { id: "equation", label: "paper.insert.equation", run: () => run(insertBlock(nodes["equation"]!.create({ tex: "", form: "dollar" }))) },
    { id: "math", label: "paper.insert.math", run: () => run(insertNode(nodes["math"]!.create({ tex: "", form: "dollar" }))) },
    { id: "rule", label: "paper.insert.rule", run: () => run(insertBlock(nodes["horizontal_rule"]!.create())), sep: true },
    { id: "callout", label: "paper.insert.callout", run: () => run(insertContainer("callout", { type: "note" })) },
    { id: "details", label: "paper.insert.details", run: () => run(insertContainer("details", { summary: word("paper.view.details") })) },
    { id: "columns", label: "paper.insert.columns", run: () => run(insertColumns()) },
    { id: "tabs", label: "paper.insert.tabs", run: () => run(insertTabs(t("paper.word.tab", { n: 1 }), t("paper.word.tab", { n: 2 }))) },
    { id: "codeBlock", label: "office.md.codeBlock", run: () => run(codeBlock), sep: true },
    { id: "toc", label: "paper.insert.toc", run: () => run(insertBlock(nodes["toc"]!.create({ title: "" }))), paperOnly: true },
    { id: "pageBreak", label: "paper.insert.pageBreak", run: () => run(insertBlock(nodes["page_break"]!.create())), paperOnly: true },
    { id: "header", label: "paper.insert.header", run: () => run(insertPageEdge("page_header")), paperOnly: true },
    {
      id: "footer",
      label: "paper.insert.footer",
      run: () => run(insertPageEdge("page_footer", [nodes["page_number"]!.create()])),
      paperOnly: true,
    },
    { id: "pageNumber", label: "paper.insert.pageNumber", run: () => run(insertNode(nodes["page_number"]!.create())), paperOnly: true, sep: true },
    { id: "footnote", label: "paper.insert.footnote", run: () => run(insertFootnote) },
    {
      id: "comment",
      label: "paper.insert.comment",
      run: () => run(applyMark(marks["comment"]!.create({ text: t("paper.comment.new"), author: me, date: today() }), t("paper.insert.comment"))),
    },
    { id: "mention", label: "paper.insert.mention", run: () => run(insertNode(nodes["mention"]!.create({ login: me }))) },
    { id: "date", label: "paper.insert.date", run: () => run(insertNode(nodes["date"]!.create({ value: today(), format: "long" }))) },
    { id: "embed", label: "paper.insert.embed", run: () => run(insertNode(nodes["embed"]!.create({ href: "https://", title: "" }))) },
    { id: "notes", label: "paper.insert.notes", run: () => run(insertContainer("notes")), slidesOnly: true },
  ];

  return (
    <div
      role="toolbar"
      aria-label={t("office.toolbar")}
      className="flex min-h-11 shrink-0 flex-wrap items-center gap-x-0.5 gap-y-1 border-b border-hair bg-surface px-2 py-1.5 sm:px-3"
      data-testid="paper-toolbar"
    >
      {icon("undo", t("office.tool.undo"), UndoIcon, undo, undefined, "Ctrl Z")}
      {icon("redo", t("office.tool.redo"), RedoIcon, redo, undefined, "Ctrl Y")}
      <Rule />
      {kind === "slides" && (
        <>
          <Button variant="ghost" size="sm" disabled={off} onMouseDown={(e) => e.preventDefault()} onClick={() => run(insertSlide())} data-action="slide">
            <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("office.tool.newSlide")}
          </Button>
          <Rule />
        </>
      )}
      <NativeSelect
        aria-label={t("office.tool.style")}
        className="w-36 [&_select]:h-8"
        value={String(style)}
        disabled={off}
        data-testid="paper-style"
        onChange={(e) => run(setStyle(Number(e.target.value)))}
      >
        {STYLES.map(([level, key]) => (
          <NativeSelectOption key={level} value={String(level)}>
            {t(key)}
          </NativeSelectOption>
        ))}
      </NativeSelect>
      <Rule />
      {MARKS.map((m) =>
        icon(m.mark, t(m.label), m.icon, () => run(toggle(m.mark)), state !== null && markActive(state, m.mark), m.keys, can(toggle(m.mark))),
      )}
      {icon(
        "link",
        t("office.tool.link"),
        Link01Icon,
        () => run(applyMark(marks["link"]!.create({ href: "https://" }), "https://")),
        state !== null && markActive(state, "link"),
        "Ctrl K",
      )}
      <Rule />
      {LISTS.map((l) => icon(l.kind, t(l.label), l.icon, () => run(toggleList(l.kind)), list === l.kind, undefined, can(toggleList(l.kind))))}
      {icon("outdent", t("paper.outdent"), TextIndentLessIcon, () => run(outdent), undefined, "Shift Tab", can(outdent))}
      {icon("indent", t("paper.indent"), TextIndentMoreIcon, () => run(indent), undefined, "Tab", can(indent))}
      {icon("quote", t("office.md.quote"), LeftToRightBlockQuoteIcon, () => run(toggleQuote), state !== null && quoteActive(state))}
      {icon("codeBlock", t("office.md.codeBlock"), CodeSquareIcon, () => run(codeBlock), state?.selection.$from.parent.type === nodes["code_block"])}
      <Rule />
      <DropdownMenu>
        <DropdownMenuTrigger
          render={<Button variant="ghost" size="sm" disabled={off} data-testid="paper-insert" onMouseDown={(e) => e.preventDefault()} />}
        >
          <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("paper.insert")}
          <HugeiconsIcon icon={ArrowDown01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </DropdownMenuTrigger>
        <DropdownMenuContent
          className="max-h-[70dvh] overflow-y-auto"
          // An insert puts the cursor in the page; closing the menu must not
          // take it back to this button, or the next keys press it. A slow
          // computer closes the menu after the insert has focused the page.
          finalFocus={(type) => {
            const a = document.activeElement;
            if (a !== null && a !== document.body && a.closest("[data-slot=dropdown-menu-content]") === null) return false;
            return type === "keyboard" || view === null ? true : view.dom;
          }}
        >
          {inserts
            .filter((i) => !(i.slidesOnly && kind !== "slides") && !(i.paperOnly && kind !== "paper"))
            .map((i) => (
              <React.Fragment key={i.id}>
                <DropdownMenuItem data-insert={i.id} onClick={i.run}>
                  {t(i.label)}
                </DropdownMenuItem>
                {i.sep && <DropdownMenuSeparator />}
              </React.Fragment>
            ))}
        </DropdownMenuContent>
      </DropdownMenu>
      {kind === "paper" && (
        <Button variant="ghost" size="sm" disabled={off} onClick={onPageSetup} data-action="pageSetup">
          <HugeiconsIcon icon={File01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <span className="max-md:sr-only">{t("paper.pageSetup")}</span>
        </Button>
      )}
    </div>
  );
}

function Rule() {
  return <span aria-hidden="true" className="mx-1.5 h-5 w-px shrink-0 bg-line" />;
}
