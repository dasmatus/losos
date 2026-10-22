/* The editor's formatting bar: LosOS buttons and lists that drive the
 * document through Collabora's command API, in place of Collabora's own
 * notebookbar (hidden by admin-ui/themes/office/branding.css).
 *
 * Each control names a LibreOffice command (`.uno:Bold`). Pressing it posts
 * Send_UNO_Command to the frame; what the control shows (pressed, the
 * current style, the size) is the command's state, which the editor reads
 * from the frame (editor.tsx, `useDocument`). One bar per kind of document,
 * with what a person reaches for most in it; everything else is in the
 * document's own right-click menu. */

import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Add01Icon,
  Comment01Icon,
  Dollar01Icon,
  LeftToRightListBulletIcon,
  LeftToRightListNumberIcon,
  Link01Icon,
  PercentIcon,
  PlayIcon,
  RedoIcon,
  RowInsertIcon,
  ColumnInsertIcon,
  SortByDown01Icon,
  SortByUp01Icon,
  Table01Icon,
  TableCellsMergeIcon,
  TextAlignCenterIcon,
  TextAlignJustifyCenterIcon,
  TextAlignLeftIcon,
  TextAlignRightIcon,
  TextBoldIcon,
  TextItalicIcon,
  TextStrikethroughIcon,
  TextUnderlineIcon,
  TextWrapIcon,
  UndoIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { cn } from "@/lib/utils";

export type DocKind = "text" | "sheet" | "slides";

const KINDS: Record<string, DocKind> = {
  doc: "text",
  docx: "text",
  docm: "text",
  dot: "text",
  dotx: "text",
  odt: "text",
  ott: "text",
  fodt: "text",
  rtf: "text",
  txt: "text",
  xls: "sheet",
  xlsx: "sheet",
  xlsm: "sheet",
  xlsb: "sheet",
  xltx: "sheet",
  ods: "sheet",
  ots: "sheet",
  fods: "sheet",
  csv: "sheet",
  ppt: "slides",
  pptx: "slides",
  pptm: "slides",
  pps: "slides",
  ppsx: "slides",
  potx: "slides",
  odp: "slides",
  otp: "slides",
  fodp: "slides",
  odg: "slides",
};

/** What kind of document `name` is, by its extension; text when unknown. */
export function docKind(name: string): DocKind {
  const dot = name.lastIndexOf(".");
  return KINDS[dot < 0 ? "" : name.slice(dot + 1).toLowerCase()] ?? "text";
}

/** A LibreOffice command's arguments, as Send_UNO_Command takes them. */
export type CommandArgs = Record<string, { type: string; value: string | number | boolean }>;

/** What the bar needs from the open document. */
export interface DocumentControl {
  /** A command's state as the document reports it ("true", "Heading 1"…). */
  state: (command: string) => string | undefined;
  run: (command: string, args?: CommandArgs) => void;
}

interface Tool {
  command: string;
  icon: IconSvgElement;
  label: MessageKey;
  /** Shown pressed while the command's state is "true". */
  toggle?: boolean;
}

const UNDO: Tool[] = [
  { command: ".uno:Undo", icon: UndoIcon, label: "office.tool.undo" },
  { command: ".uno:Redo", icon: RedoIcon, label: "office.tool.redo" },
];

const CHARACTER: Tool[] = [
  { command: ".uno:Bold", icon: TextBoldIcon, label: "office.tool.bold", toggle: true },
  { command: ".uno:Italic", icon: TextItalicIcon, label: "office.tool.italic", toggle: true },
  { command: ".uno:Underline", icon: TextUnderlineIcon, label: "office.tool.underline", toggle: true },
  { command: ".uno:Strikeout", icon: TextStrikethroughIcon, label: "office.tool.strikethrough", toggle: true },
];

const PARAGRAPH: Tool[] = [
  { command: ".uno:LeftPara", icon: TextAlignLeftIcon, label: "office.tool.alignLeft", toggle: true },
  { command: ".uno:CenterPara", icon: TextAlignCenterIcon, label: "office.tool.alignCenter", toggle: true },
  { command: ".uno:RightPara", icon: TextAlignRightIcon, label: "office.tool.alignRight", toggle: true },
  { command: ".uno:JustifyPara", icon: TextAlignJustifyCenterIcon, label: "office.tool.justify", toggle: true },
];

const LISTS: Tool[] = [
  { command: ".uno:DefaultBullet", icon: LeftToRightListBulletIcon, label: "office.tool.bullets", toggle: true },
  { command: ".uno:DefaultNumbering", icon: LeftToRightListNumberIcon, label: "office.tool.numbering", toggle: true },
];

const CELL_ALIGN: Tool[] = [
  { command: ".uno:AlignLeft", icon: TextAlignLeftIcon, label: "office.tool.alignLeft", toggle: true },
  { command: ".uno:AlignHorizontalCenter", icon: TextAlignCenterIcon, label: "office.tool.alignCenter", toggle: true },
  { command: ".uno:AlignRight", icon: TextAlignRightIcon, label: "office.tool.alignRight", toggle: true },
  { command: ".uno:WrapText", icon: TextWrapIcon, label: "office.tool.wrap", toggle: true },
  { command: ".uno:ToggleMergeCells", icon: TableCellsMergeIcon, label: "office.tool.merge", toggle: true },
];

const NUMBERS: Tool[] = [
  { command: ".uno:NumberFormatCurrency", icon: Dollar01Icon, label: "office.tool.currency", toggle: true },
  { command: ".uno:NumberFormatPercent", icon: PercentIcon, label: "office.tool.percent", toggle: true },
];

const SHEET: Tool[] = [
  { command: ".uno:SortAscending", icon: SortByUp01Icon, label: "office.tool.sortUp" },
  { command: ".uno:SortDescending", icon: SortByDown01Icon, label: "office.tool.sortDown" },
  { command: ".uno:InsertRowsBefore", icon: RowInsertIcon, label: "office.tool.insertRow" },
  { command: ".uno:InsertColumnsBefore", icon: ColumnInsertIcon, label: "office.tool.insertColumn" },
];

const INSERT_TEXT: Tool[] = [
  { command: ".uno:HyperlinkDialog", icon: Link01Icon, label: "office.tool.link" },
  { command: ".uno:InsertTable", icon: Table01Icon, label: "office.tool.table" },
  { command: ".uno:InsertAnnotation", icon: Comment01Icon, label: "office.tool.comment" },
];

const SLIDES: Tool[] = [{ command: ".uno:InsertPage", icon: Add01Icon, label: "office.tool.newSlide" }];

const SHOW: Tool[] = [{ command: ".uno:Presentation", icon: PlayIcon, label: "office.tool.present" }];

/** The paragraph styles offered, by LibreOffice's own (English) names;
 *  a .docx keeps Word's names for the same styles. */
const STYLES: ReadonlyArray<readonly [string, MessageKey]> = [
  ["Default Paragraph Style", "office.style.normal"],
  ["Title", "office.style.title"],
  ["Heading 1", "office.style.heading1"],
  ["Heading 2", "office.style.heading2"],
  ["Heading 3", "office.style.heading3"],
  ["Quotations", "office.style.quote"],
];

const SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 72];

/** The commands whose state the bar shows, for the editor to watch. */
export function watchedCommands(kind: DocKind): string[] {
  const tools = groups(kind).flat();
  return [
    ...tools.map((tool) => tool.command),
    ".uno:StyleApply",
    ".uno:FontHeight",
  ];
}

function groups(kind: DocKind): Tool[][] {
  switch (kind) {
    case "sheet":
      return [UNDO, CHARACTER.slice(0, 3), CELL_ALIGN, NUMBERS, SHEET];
    case "slides":
      return [UNDO, SLIDES, CHARACTER.slice(0, 3), LISTS, PARAGRAPH.slice(0, 3), SHOW];
    default:
      return [UNDO, CHARACTER, LISTS, PARAGRAPH, INSERT_TEXT];
  }
}

export function OfficeToolbar({
  kind,
  doc,
  disabled,
}: {
  kind: DocKind;
  doc: DocumentControl;
  disabled: boolean;
}) {
  const t = useT();

  const button = (tool: Tool) => {
    const label = t(tool.label);
    const pressed = tool.toggle ? doc.state(tool.command) === "true" : undefined;
    const off = disabled || doc.state(tool.command) === "disabled";
    return (
      <Tooltip key={tool.command}>
        <TooltipTrigger
          render={
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={label}
              aria-pressed={pressed}
              data-command={tool.command}
              disabled={off}
              className={cn(pressed && "bg-accent-wash text-accent hover:bg-accent-wash")}
              onClick={() => doc.run(tool.command)}
            />
          }
        >
          <HugeiconsIcon icon={tool.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </TooltipTrigger>
        <TooltipContent side="bottom">{label}</TooltipContent>
      </Tooltip>
    );
  };

  const style = doc.state(".uno:StyleApply") ?? "";
  const known = STYLES.some(([name]) => name === style);
  const size = Number.parseFloat(doc.state(".uno:FontHeight") ?? "");

  const before =
    kind === "text" ? (
      <>
        <NativeSelect
          aria-label={t("office.tool.style")}
          className="w-40 [&_select]:h-8"
          value={style}
          disabled={disabled}
          onChange={(e) =>
            doc.run(".uno:StyleApply", {
              Style: { type: "string", value: e.target.value },
              FamilyName: { type: "string", value: "ParagraphStyles" },
            })
          }
        >
          {!known && <NativeSelectOption value={style}>{style}</NativeSelectOption>}
          {STYLES.map(([name, key]) => (
            <NativeSelectOption key={name} value={name}>
              {t(key)}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </>
    ) : null;

  const sizes = (
    <NativeSelect
      aria-label={t("office.tool.size")}
      className="w-20 [&_select]:h-8"
      value={Number.isFinite(size) ? String(size) : ""}
      disabled={disabled}
      onChange={(e) =>
        doc.run(".uno:FontHeight", { "FontHeight.Height": { type: "float", value: Number(e.target.value) } })
      }
    >
      {!Number.isFinite(size) && <NativeSelectOption value="">{""}</NativeSelectOption>}
      {Number.isFinite(size) && !SIZES.includes(size) && (
        <NativeSelectOption value={String(size)}>{size}</NativeSelectOption>
      )}
      {SIZES.map((n) => (
        <NativeSelectOption key={n} value={String(n)}>
          {n}
        </NativeSelectOption>
      ))}
    </NativeSelect>
  );

  const [undo, ...rest] = groups(kind);
  return (
    <div
      role="toolbar"
      aria-label={t("office.toolbar")}
      className="flex h-11 shrink-0 items-center gap-1 overflow-x-auto border-b border-hair bg-surface px-2 sm:px-3"
    >
      {undo?.map(button)}
      <Rule />
      {before}
      {sizes}
      {rest.map((group, i) => (
        <span key={i} className="contents">
          <Rule />
          {group.map(button)}
        </span>
      ))}
    </div>
  );
}

function Rule() {
  return <span aria-hidden="true" className="mx-1 h-5 w-px shrink-0 bg-line" />;
}
