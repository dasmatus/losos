import * as React from "react";
import { createPortal } from "react-dom";
import { Menu } from "@base-ui/react/menu";
import { Toolbar } from "@base-ui/react/toolbar";
import { HugeiconsIcon } from "@hugeicons/react";
import { AiMagicIcon, ArrowDown01Icon, ArrowLeft01Icon, Tick02Icon } from "@hugeicons/core-free-icons";
import { AskAiDialog, useAssistant, type AskContext } from "@/components/ask-ai";
import { Button } from "@/components/ui/button";
import { popupMotion } from "@/components/ui/motion";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { AssistantSource } from "@/lib/api";
import { ariaKeys, keyCaps } from "@/lib/context-menu";
import { useT } from "@/lib/i18n-react";
import {
  isQuickKey,
  placeBar,
  QUICK_KEYS,
  visibleQuickSections,
  type QuickAction,
  type QuickChoice,
  type QuickCommand,
  type QuickMenu,
  type QuickPrompt,
  type QuickSection,
  type QuickSuggestion,
} from "@/lib/quick-actions";
import { cn } from "@/lib/utils";

/* The bar that floats over a selection, as in Notion and Linear: pick words
 * in a document or a note, or cards in a diagram, and what can be done to
 * them is right there, above the selection, without a trip to the toolbar
 * or the right-click menu (lib/quick-actions.ts says what it holds).
 *
 * It never takes the focus from what holds the selection: a press on it is
 * kept from moving the focus, so the editor's selection stays put and a
 * button acts on it. From the keyboard, Alt+F10 moves into the bar, the
 * arrows walk it, Escape goes back. A prompt (a link, a mention) takes the
 * bar's place with a field; the field does take the focus, and Enter or
 * Escape hands it back.
 *
 * It grows out of the selection (scale from 95 %, a few pixels from the
 * selection's side), glides when the selection moves, and leaves faster
 * than it came; opacity, scale and translate only. Ask AI is the box's own
 * assistant (components/ask-ai.tsx), there only while it answers.
 *
 * Placed with React style props (CSSOM), which the admin CSP allows, and
 * drawn on <body> so a scrolled pane or a card cannot clip it. Its menus are
 * Base UI's, like the right-click menu's; the toolbar's roving focus is Base
 * UI's Toolbar. */

export interface QuickAsk {
  context: AskContext;
  /** What the assistant reads: the selection, gathered when it is asked. */
  sources: () => AssistantSource[] | Promise<AssistantSource[]>;
  /** Put the answer in place of the selection. */
  onUse?: (answer: string) => void;
}

export interface QuickActionsProps {
  /** Where the selection is drawn, in the window; null hides the bar. */
  anchor: DOMRect | null;
  /** What was selected, the bar's accessible name. */
  label: string;
  sections: readonly QuickSection[];
  ask?: QuickAsk;
  /** Give the keyboard back to what holds the selection. */
  onReturn: () => void;
  /** Escape was pressed in the bar: leave the selection, hide the bar. */
  onDismiss?: () => void;
  /** The top of the area the bar may cover, read when it is placed: the
   *  app's own toolbar above it stays clickable. */
  ceiling?: () => number;
  className?: string;
}

const EXIT_MS = 110;

/** Whether a node is part of a bar or one of its popups. */
export function insideQuickActions(node: EventTarget | null): boolean {
  return node instanceof Element && node.closest("[data-quick-actions], [data-quick-popup], [data-slot=ask-ai]") !== null;
}

export function QuickActions({ anchor, label, sections, ask, onReturn, onDismiss, ceiling, className }: QuickActionsProps) {
  const t = useT();
  const ready = useAssistant();
  const bar = React.useRef<HTMLDivElement>(null);
  const [prompt, setPrompt] = React.useState<QuickPrompt | null>(null);
  const [asking, setAsking] = React.useState<QuickAsk | null>(null);
  // The last place, kept for the exit after the anchor has gone.
  const [held, setHeld] = React.useState<DOMRect | null>(anchor);
  const [closing, setClosing] = React.useState(false);
  const [pos, setPos] = React.useState<{ x: number; y: number; side: "top" | "bottom" } | null>(null);
  const placed = React.useRef(false);

  const visible = visibleQuickSections(sections);
  const open = anchor !== null && (visible.length > 0 || (ask !== undefined && ready));

  React.useEffect(() => {
    if (open) {
      setHeld(anchor);
      setClosing(false);
      return;
    }
    setPrompt(null);
    if (held === null) return;
    setClosing(true);
    const timer = window.setTimeout(() => {
      setHeld(null);
      setClosing(false);
      setPos(null);
      placed.current = false;
    }, EXIT_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- held is the result
  }, [open, anchor?.left, anchor?.top, anchor?.right, anchor?.bottom]);

  // Measured before the paint, so the first frame is already in place.
  React.useLayoutEffect(() => {
    const el = bar.current;
    if (el === null || held === null) return;
    const next = placeBar(held, el.offsetWidth, el.offsetHeight, window.innerWidth, window.innerHeight, ceiling?.() ?? 0);
    setPos((p) => (p !== null && p.x === next.x && p.y === next.y && p.side === next.side ? p : next));
  }, [held, prompt, visible.length, ready]);

  React.useEffect(() => {
    if (pos === null) return;
    const frame = window.requestAnimationFrame(() => {
      placed.current = true;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [pos]);

  // Alt+F10 from the selection moves into the bar.
  React.useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent): void => {
      if (!isQuickKey(event)) return;
      const first = bar.current?.querySelector<HTMLElement>("button:not([disabled]), input");
      if (first === undefined || first === null) return;
      event.preventDefault();
      event.stopPropagation();
      first.focus();
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [open]);

  const back = React.useCallback(() => {
    setPrompt(null);
    onReturn();
  }, [onReturn]);

  // Before the tooltips, which would take the first Escape for themselves;
  // a menu of the bar's own closes first.
  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
    if (event.key !== "Escape") return;
    if (event.target instanceof Element && event.target.closest("[data-quick-popup]") !== null) return;
    event.preventDefault();
    event.stopPropagation();
    if (prompt !== null) {
      back();
      return;
    }
    onDismiss?.();
    onReturn();
  };

  const run = (action: QuickCommand): void => {
    action.onSelect();
  };

  const dialog =
    asking !== null ? (
      <AskAiDialog
        open
        onOpenChange={(next) => {
          if (next) return;
          setAsking(null);
          onReturn();
        }}
        sources={asking.sources}
        context={asking.context}
        use={
          asking.onUse === undefined
            ? undefined
            : {
                label: t("quick.ask.use"),
                onUse: (answer) => {
                  const use = asking.onUse;
                  setAsking(null);
                  use?.(answer);
                },
              }
        }
      />
    ) : null;

  const side = pos?.side ?? "top";

  // The dialog stays in one place in the tree, so the bar going (the
  // editor lost the focus to it) does not start it again.
  return (
    <>
      {held !== null && createPortal(
        <div
          className={cn(
            "pointer-events-none fixed top-0 left-0 z-40",
            placed.current && !closing && "transition-[translate] duration-150 ease-snap",
            pos === null && "invisible",
          )}
          style={{ translate: pos === null ? "0 0" : `${pos.x}px ${pos.y}px` }}
          data-quick-actions=""
        >
          <Toolbar.Root
            ref={bar}
            aria-label={label}
            aria-keyshortcuts={ariaKeys(QUICK_KEYS)}
            data-slot="quick-actions"
            data-side={side}
            data-closing={closing ? "" : undefined}
            onKeyDownCapture={onKeyDown}
            // A press keeps the focus, and so the selection, where it is;
            // only the prompt's field takes it.
            onMouseDown={(event) => {
              if (!(event.target instanceof HTMLInputElement)) event.preventDefault();
            }}
            className={cn(
              "pointer-events-auto flex max-w-[calc(100vw-1rem)] items-center gap-0.5 rounded-card border border-line bg-surface p-1 text-ink shadow-pop",
              "transition-[opacity,scale,translate] duration-150 ease-snap starting:scale-95 starting:opacity-0",
              side === "top" ? "origin-bottom starting:translate-y-1" : "origin-top starting:-translate-y-1",
              "data-closing:scale-[0.98] data-closing:opacity-0 data-closing:duration-100 data-closing:ease-exit",
              className,
            )}
          >
            {prompt !== null ? (
              <PromptField key={prompt.id} prompt={prompt} onBack={() => setPrompt(null)} onDone={back} />
            ) : (
              <>
                {ask !== undefined && ready && (
                  <>
                    <Toolbar.Button
                      render={<Button variant="ghost" size="sm" className="h-8 gap-1.5 px-2 text-accent hover:text-accent" />}
                      data-action="quick-ask-ai"
                      onClick={() => setAsking(ask)}
                    >
                      <HugeiconsIcon icon={AiMagicIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                      {t("quick.ask")}
                    </Toolbar.Button>
                    {visible.length > 0 && <Rule />}
                  </>
                )}
                {visible.map((section, i) => (
                  <React.Fragment key={i}>
                    {i > 0 && <Rule />}
                    <Toolbar.Group className="flex items-center gap-0.5">
                      {section.map((action) => (
                        <Action key={action.id} action={action} onRun={run} onPrompt={setPrompt} />
                      ))}
                    </Toolbar.Group>
                  </React.Fragment>
                ))}
              </>
            )}
          </Toolbar.Root>
        </div>,
        document.body,
      )}
      {dialog}
    </>
  );
}

function Rule() {
  return <Toolbar.Separator aria-hidden="true" className="mx-1 h-5 w-px shrink-0 bg-line" />;
}

const ICON_BUTTON = "size-8";
const PRESSED = "bg-accent-wash text-accent hover:bg-accent-wash hover:text-accent";

function Action({
  action,
  onRun,
  onPrompt,
}: {
  action: QuickAction;
  onRun: (action: QuickCommand) => void;
  onPrompt: (prompt: QuickPrompt) => void;
}) {
  if (action.kind === "choice") return <Choice action={action} />;
  if (action.kind === "menu") return <Commands action={action} />;
  const command = action.kind === "prompt" ? null : action;
  const keys = command?.keys;
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Toolbar.Button
            render={
              <Button
                variant="ghost"
                size="icon-sm"
                className={cn(
                  ICON_BUTTON,
                  command?.pressed === true && PRESSED,
                  command?.danger === true && "hover:bg-crit/10 hover:text-crit",
                )}
              />
            }
            aria-label={action.label}
            aria-pressed={command?.pressed}
            aria-keyshortcuts={keys !== undefined ? ariaKeys(keys) : undefined}
            aria-haspopup={action.kind === "prompt" ? "dialog" : undefined}
            data-action={action.id}
            onClick={() => (action.kind === "prompt" ? onPrompt(action) : onRun(action))}
          />
        }
      >
        <HugeiconsIcon icon={action.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </TooltipTrigger>
      <TooltipContent side="top" data-quick-popup="">
        {action.label}
        {keys !== undefined && <span className="ml-2 text-faint">{keyCaps(keys).join(" ")}</span>}
      </TooltipContent>
    </Tooltip>
  );
}

const MENU = cn(
  "max-h-(--available-height) min-w-44 overflow-y-auto rounded-card border border-line bg-surface p-1 text-ink shadow-pop outline-none",
  popupMotion,
);

const MENU_ITEM = cn(
  "flex min-h-8 cursor-default items-center gap-2 rounded-control px-2 py-1.5 text-[13px] outline-none select-none",
  "data-highlighted:bg-accent-wash data-highlighted:text-ink",
);

function Swatch({ color }: { color: string | undefined }) {
  return (
    <span
      aria-hidden="true"
      className={cn("size-3.5 shrink-0 rounded-full border border-line", color === undefined && "bg-surface")}
      style={color !== undefined ? { background: color } : undefined}
    />
  );
}

function Choice({ action }: { action: QuickChoice }) {
  const current = action.options.find((o) => o.value === action.value);
  const swatches = action.options.some((o) => o.swatch !== undefined);
  return (
    <Menu.Root modal={false}>
      <MenuButton action={action}>
        {swatches ? (
          <Swatch color={current?.swatch} />
        ) : (
          <HugeiconsIcon icon={current?.icon ?? action.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        )}
      </MenuButton>
      <MenuPopup label={action.label}>
        <Menu.RadioGroup value={action.value} onValueChange={(value: string) => action.onChange(value)}>
          {action.options.map((option) => (
            <Menu.RadioItem key={option.value} value={option.value} closeOnClick label={option.label} className={MENU_ITEM} data-option={option.value}>
              {swatches ? (
                <Swatch color={option.swatch} />
              ) : option.icon !== undefined ? (
                <HugeiconsIcon icon={option.icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0" />
              ) : (
                <span aria-hidden="true" className="size-[15px] shrink-0" />
              )}
              <span className="min-w-0 flex-1 truncate">{option.label}</span>
              <Menu.RadioItemIndicator className="text-accent">
                <HugeiconsIcon icon={Tick02Icon} size={14} strokeWidth={2} color="currentColor" aria-hidden="true" />
              </Menu.RadioItemIndicator>
            </Menu.RadioItem>
          ))}
        </Menu.RadioGroup>
      </MenuPopup>
    </Menu.Root>
  );
}

function MenuButton({ action, children }: { action: QuickChoice | QuickMenu; children: React.ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Menu.Trigger
            render={
              <Toolbar.Button
                render={<Button variant="ghost" size="sm" className="group/choice h-8 gap-1 px-1.5 data-popup-open:bg-sunk" />}
                aria-label={action.label}
                data-action={action.id}
              />
            }
          />
        }
      >
        {children}
        <HugeiconsIcon
          icon={ArrowDown01Icon}
          size={12}
          strokeWidth={1.5}
          color="currentColor"
          aria-hidden="true"
          className="text-faint transition-transform duration-150 ease-snap group-data-popup-open/choice:rotate-180"
        />
      </TooltipTrigger>
      <TooltipContent side="top" data-quick-popup="">
        {action.label}
      </TooltipContent>
    </Tooltip>
  );
}

function MenuPopup({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <Menu.Portal>
      <Menu.Positioner side="bottom" align="start" sideOffset={6} collisionPadding={8} className="isolate z-50 outline-none">
        <Menu.Popup data-quick-popup="" aria-label={label} className={MENU}>
          {children}
        </Menu.Popup>
      </Menu.Positioner>
    </Menu.Portal>
  );
}

function Commands({ action }: { action: QuickMenu }) {
  return (
    <Menu.Root modal={false}>
      <MenuButton action={action}>
        <HugeiconsIcon icon={action.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </MenuButton>
      <MenuPopup label={action.label}>
        {action.items.map((item) => (
          <Menu.Item key={item.id} label={item.label} className={MENU_ITEM} onClick={item.onSelect} data-item={item.id}>
            {item.icon !== undefined ? (
              <HugeiconsIcon icon={item.icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0" />
            ) : (
              <span aria-hidden="true" className="size-[15px] shrink-0" />
            )}
            <span className="min-w-0 flex-1 truncate">{item.label}</span>
          </Menu.Item>
        ))}
      </MenuPopup>
    </Menu.Root>
  );
}

/** A field in the bar's place: type a link, or pick a note or a person. */
function PromptField({ prompt, onBack, onDone }: { prompt: QuickPrompt; onBack: () => void; onDone: () => void }) {
  const t = useT();
  const id = React.useId();
  const [value, setValue] = React.useState(prompt.initial ?? "");
  const [found, setFound] = React.useState<readonly QuickSuggestion[]>([]);
  const [active, setActive] = React.useState(0);
  const field = React.useRef<HTMLInputElement>(null);

  React.useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, []);

  React.useEffect(() => {
    const suggest = prompt.suggest;
    if (suggest === undefined) return;
    const ac = new AbortController();
    const timer = window.setTimeout(() => {
      void Promise.resolve(suggest(value, ac.signal)).then(
        (list) => {
          if (ac.signal.aborted) return;
          setFound(list.slice(0, 6));
          setActive(0);
        },
        () => {
          if (!ac.signal.aborted) setFound([]);
        },
      );
    }, 80);
    return () => {
      ac.abort();
      window.clearTimeout(timer);
    };
  }, [prompt, value]);

  const submit = (picked?: QuickSuggestion): void => {
    if (picked === undefined && value.trim() === "") return;
    prompt.onSubmit(picked?.label ?? value.trim(), picked);
    onDone();
  };

  const listed = found.length > 0;

  return (
    <div className="relative flex items-center gap-1" data-prompt={prompt.id}>
      <Button
        variant="ghost"
        size="icon-sm"
        className={ICON_BUTTON}
        aria-label={t("quick.back")}
        title={t("quick.back")}
        onClick={onBack}
        data-action="quick-back"
      >
        <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </Button>
      <HugeiconsIcon icon={prompt.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0 text-muted" />
      <input
        ref={field}
        type="text"
        value={value}
        placeholder={prompt.placeholder}
        aria-label={prompt.label}
        role={prompt.suggest !== undefined ? "combobox" : undefined}
        aria-expanded={prompt.suggest !== undefined ? listed : undefined}
        aria-controls={prompt.suggest !== undefined ? `${id}-list` : undefined}
        aria-activedescendant={listed ? `${id}-${active}` : undefined}
        autoComplete="off"
        spellCheck={false}
        className="h-8 w-64 max-w-[60vw] rounded-control bg-transparent px-1.5 text-[13.5px] text-ink outline-none placeholder:text-faint"
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" && listed) {
            event.preventDefault();
            setActive((a) => (a + 1) % found.length);
          } else if (event.key === "ArrowUp" && listed) {
            event.preventDefault();
            setActive((a) => (a - 1 + found.length) % found.length);
          } else if (event.key === "Enter") {
            event.preventDefault();
            submit(listed ? found[active] : undefined);
          }
          // Keep the toolbar's arrow keys out of the field.
          if (event.key.startsWith("Arrow") || event.key === "Home" || event.key === "End") event.stopPropagation();
        }}
      />
      <Button size="sm" className="h-8" onClick={() => submit(listed ? found[active] : undefined)} disabled={value.trim() === "" && !listed} data-action="quick-submit">
        {t("quick.done")}
      </Button>
      {listed && (
        <ul
          id={`${id}-list`}
          role="listbox"
          aria-label={prompt.label}
          className="absolute top-full left-0 mt-2 w-full min-w-64 rounded-card border border-line bg-surface p-1 shadow-pop animate-pop"
          data-quick-popup=""
        >
          {found.map((s, i) => (
            <li
              key={s.id}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === active}
              className={cn(
                "flex min-h-8 cursor-pointer items-center gap-2 rounded-control px-2 py-1.5 text-[13px]",
                i === active ? "bg-accent-wash" : "hover:bg-sunk",
              )}
              onMouseDown={(event) => event.preventDefault()}
              onMouseEnter={() => setActive(i)}
              onClick={() => submit(s)}
              data-suggestion={s.id}
            >
              {s.icon !== undefined && <HugeiconsIcon icon={s.icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0 text-muted" />}
              <span className="min-w-0 truncate">{s.label}</span>
              {s.detail !== undefined && s.detail !== "" && <span className="ml-auto min-w-0 shrink truncate text-[12px] text-muted">{s.detail}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
