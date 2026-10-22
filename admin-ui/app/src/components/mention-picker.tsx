import * as React from "react";
import { createPortal } from "react-dom";
import { File02Icon, Folder01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { PersonAvatar } from "@/components/person";
import { useT } from "@/lib/i18n-react";
import { mentionText, queryAt, searchMentions, type Mention } from "@/lib/mentions";
import { cn } from "@/lib/utils";

/* Typing @ in a text box offers people on this box and files in LosOS
 * Cloud (lib/mentions.ts). The list is drawn on <body> under the box, so a
 * small card or a scrolled pane does not clip it, and it is placed by
 * style props, which the admin page's `style-src 'self'` allows. The text
 * box keeps focus throughout: arrows move the highlight, Enter or Tab puts
 * the mention in, Escape closes the list and nothing else. */

type Field = HTMLTextAreaElement | HTMLInputElement;

export interface MentionState {
  /** Pass to the text box's onKeyDown first; true when the list used the key. */
  onKeyDown: (event: React.KeyboardEvent<Field>) => boolean;
  /** Call after every change and caret move. */
  sync: () => void;
  picker: React.ReactNode;
}

export function useMentions(
  ref: React.RefObject<Field | null>,
  setText: (text: string) => void,
): MentionState {
  const t = useT();
  const [query, setQuery] = React.useState<{ start: number; q: string } | null>(null);
  const [items, setItems] = React.useState<Mention[]>([]);
  const [active, setActive] = React.useState(0);
  const [looking, setLooking] = React.useState(false);
  const [box, setBox] = React.useState<DOMRect | null>(null);
  const listId = React.useId();
  const pending = React.useRef<{ text: string; at: number } | null>(null);

  const sync = React.useCallback(() => {
    const el = ref.current;
    if (el === null) return;
    const caret = el.selectionStart ?? el.value.length;
    const next = el.selectionEnd === caret ? queryAt(el.value, caret) : null;
    setQuery((prev) => (prev?.start === next?.start && prev?.q === next?.q ? prev : next));
    if (next !== null) setBox(el.getBoundingClientRect());
  }, [ref]);

  React.useEffect(() => {
    if (query === null) {
      setItems([]);
      return;
    }
    const ctl = new AbortController();
    setLooking(true);
    const timer = window.setTimeout(() => {
      searchMentions(query.q, ctl.signal).then(
        (found) => {
          setItems(found);
          setActive(0);
          setLooking(false);
        },
        () => setLooking(false),
      );
    }, 120);
    return () => {
      window.clearTimeout(timer);
      ctl.abort();
    };
  }, [query]);

  const choose = React.useCallback(
    (m: Mention) => {
      const el = ref.current;
      if (el === null || query === null) return;
      const caret = el.selectionStart ?? el.value.length;
      const insert = `${mentionText(m)} `;
      const text = el.value.slice(0, query.start) + insert + el.value.slice(caret);
      pending.current = { text, at: query.start + insert.length };
      setText(text);
      setQuery(null);
    },
    [ref, query, setText],
  );

  // The caret goes after the mention in the same commit that writes it, not
  // a frame later: a key typed in between would land where the caret was.
  React.useLayoutEffect(() => {
    const el = ref.current;
    const want = pending.current;
    if (el === null || want === null || el.value !== want.text) return;
    pending.current = null;
    el.focus();
    el.setSelectionRange(want.at, want.at);
  });

  const open = query !== null && (items.length > 0 || !looking);

  const onKeyDown = React.useCallback(
    (event: React.KeyboardEvent<Field>): boolean => {
      if (!open) return false;
      if (event.key === "Escape") {
        setQuery(null);
        return true;
      }
      if (items.length === 0) return false;
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        const step = event.key === "ArrowDown" ? 1 : -1;
        setActive((i) => (i + step + items.length) % items.length);
        return true;
      }
      if (event.key === "Enter" || event.key === "Tab") {
        const m = items[active];
        if (m !== undefined) choose(m);
        return true;
      }
      return false;
    },
    [open, items, active, choose],
  );

  React.useEffect(() => {
    const el = ref.current;
    if (el === null || !open) return;
    el.setAttribute("aria-controls", listId);
    el.setAttribute("aria-expanded", "true");
    const item = items[active];
    if (item !== undefined) el.setAttribute("aria-activedescendant", `${listId}-${active}`);
    else el.removeAttribute("aria-activedescendant");
    return () => {
      el.removeAttribute("aria-controls");
      el.removeAttribute("aria-activedescendant");
      el.setAttribute("aria-expanded", "false");
    };
  }, [ref, open, items, active, listId]);

  let picker: React.ReactNode = null;
  if (open && box !== null) {
    const below = window.innerHeight - box.bottom > 260 || box.top < 260;
    const left = Math.max(8, Math.min(box.left, window.innerWidth - 328));
    const people = items.filter((m) => m.kind === "person");
    const files = items.filter((m) => m.kind === "file");
    const row = (m: Mention) => {
      const index = items.indexOf(m);
      return (
        <div
          key={m.id}
          id={`${listId}-${index}`}
          role="option"
          aria-selected={index === active}
          data-testid="mention-option"
          data-kind={m.kind}
          onMouseMove={index === active ? undefined : () => setActive(index)}
          onClick={() => choose(m)}
          className={cn(
            "flex cursor-default items-center gap-2.5 rounded-control px-2 py-1.5",
            index === active ? "bg-accent-wash text-ink" : "text-ink",
          )}
        >
          {m.kind === "person" ? (
            <PersonAvatar person={m.person} size="xs" />
          ) : (
            <span className="flex size-6 shrink-0 items-center justify-center rounded-control border border-line bg-sunk text-muted">
              <HugeiconsIcon icon={m.dir ? Folder01Icon : File02Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </span>
          )}
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[13px] leading-tight font-medium">{m.label}</span>
            {m.detail !== "" && <span className="block truncate text-[11.5px] leading-tight text-muted">{m.detail}</span>}
          </span>
        </div>
      );
    };
    const group = (name: string, list: Mention[]) =>
      list.length > 0 && (
        <div role="group" aria-label={name}>
          <p role="presentation" className="px-2 pt-1.5 pb-1 text-[11px] font-medium tracking-wide text-faint">
            {name}
          </p>
          {list.map(row)}
        </div>
      );
    picker = createPortal(
      <div
        id={listId}
        role="listbox"
        aria-label={t("mention.label")}
        data-testid="mention-picker"
        // A portal's events still bubble through the React tree: keep the
        // press from reaching whatever holds the text box (a canvas that
        // would take it as a click on empty space) and from taking focus.
        onPointerDown={(event) => event.stopPropagation()}
        onMouseDown={(event) => {
          event.preventDefault();
          event.stopPropagation();
        }}
        onClick={(event) => event.stopPropagation()}
        onDoubleClick={(event) => event.stopPropagation()}
        onWheel={(event) => event.stopPropagation()}
        className="fixed z-50 max-h-64 w-80 overflow-y-auto rounded-card border border-line bg-surface p-1 shadow-card animate-fade-in"
        style={below ? { left, top: box.bottom + 6 } : { left, bottom: window.innerHeight - box.top + 6 }}
      >
        {items.length === 0 ? (
          <p className="px-2 py-3 text-center text-[12.5px] text-muted">
            {query.q === "" ? t("mention.start") : t("mention.none")}
          </p>
        ) : (
          <>
            {group(t("mention.people"), people)}
            {group(t("mention.files"), files)}
          </>
        )}
      </div>,
      document.body,
    );
  }

  return { onKeyDown, sync, picker };
}
