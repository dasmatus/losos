/* The people picker every app shares: a text field that offers the
 * person's contacts and the people on this box as they type
 * (GET /api/contacts/suggest, backend/src/contacts.rs). Mail's To and Cc,
 * starting a chat, a contact's chat address and mesh sharing use it.
 *
 * It stays a plain text field underneath: an address typed by hand is kept
 * as typed, and where the box has no contacts (off the shared side) it
 * offers nothing and is just the field. With `multiple`, the value is a
 * comma-separated list and the suggestions answer the part being typed. */

import * as React from "react";
import { PersonAvatar } from "@/components/person";
import { Input } from "@/components/ui/input";
import { suggestContacts, type ContactSuggestion } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

export type Want = "any" | "email" | "matrix" | "mesh";

/** What a suggestion puts in the field, for each kind of field. */
export function valueFor(s: ContactSuggestion, want: Want): string | null {
  switch (want) {
    case "email":
      return s.emails[0] ?? null;
    case "matrix":
      return s.matrixId ?? null;
    case "mesh":
      return s.meshAddress ?? null;
    default:
      return s.emails[0] ?? s.matrixId ?? s.meshAddress ?? null;
  }
}

/* The part of a comma-separated value being typed: from after the last
 * comma to the end. */
function lastPart(value: string): { head: string; part: string } {
  const at = Math.max(value.lastIndexOf(","), value.lastIndexOf(";"));
  return at < 0 ? { head: "", part: value } : { head: value.slice(0, at + 1), part: value.slice(at + 1) };
}

/** The suggestions for `q`, fetched as the person types, newest answer only. */
export function useSuggestions(q: string, want: Want, enabled = true): ContactSuggestion[] {
  const [found, setFound] = React.useState<ContactSuggestion[]>([]);
  React.useEffect(() => {
    if (!enabled) {
      setFound([]);
      return;
    }
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      suggestContacts(q.trim(), want, { signal: controller.signal })
        .then(setFound)
        .catch(() => {
          /* nothing to offer, then: the field still takes what is typed */
        });
    }, 120);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [q, want, enabled]);
  return found;
}

export function PeopleField({
  id,
  value,
  onChange,
  want,
  multiple = false,
  className,
  ...props
}: {
  id: string;
  value: string;
  onChange: (value: string) => void;
  want: Want;
  multiple?: boolean;
} & Omit<React.ComponentPropsWithoutRef<"input">, "value" | "onChange" | "id">) {
  const t = useT();
  const [open, setOpen] = React.useState(false);
  const [active, setActive] = React.useState(0);
  const { head, part } = multiple ? lastPart(value) : { head: "", part: value };
  const found = useSuggestions(part, want, open);
  // Offer what this field can take, and not what is in it already.
  const taken = new Set(
    value
      .split(/[,;]/)
      .map((s) => s.trim().toLowerCase())
      .filter((s) => s !== ""),
  );
  const offered = found.filter((s) => {
    const v = valueFor(s, want);
    return v !== null && !(multiple && taken.has(v.toLowerCase()));
  });
  const listId = `${id}-people`;
  const shown = open && offered.length > 0;

  React.useEffect(() => setActive(0), [part]);

  const pick = (s: ContactSuggestion) => {
    const v = valueFor(s, want);
    if (v === null) return;
    onChange(multiple ? `${head}${head === "" ? "" : " "}${v}, ` : v);
    setOpen(multiple);
  };

  return (
    <div className="relative">
      <Input
        id={id}
        value={value}
        role="combobox"
        aria-expanded={shown}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={shown ? `${listId}-${active}` : undefined}
        autoComplete="off"
        onChange={(event) => {
          onChange(event.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onKeyDown={(event) => {
          if (!shown) {
            if (event.key === "ArrowDown") setOpen(true);
            return;
          }
          if (event.key === "ArrowDown") {
            event.preventDefault();
            setActive((a) => (a + 1) % offered.length);
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setActive((a) => (a - 1 + offered.length) % offered.length);
          } else if ((event.key === "Enter" || event.key === "Tab") && part.trim() !== "") {
            const s = offered[active];
            if (s !== undefined) {
              event.preventDefault();
              pick(s);
            }
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            setOpen(false);
          }
        }}
        className={className}
        {...props}
      />
      <ul
        id={listId}
        role="listbox"
        aria-label={t("contacts.suggest.label")}
        hidden={!shown}
        className={cn(
          "absolute inset-x-0 top-full z-50 mt-1 max-h-64 overflow-y-auto rounded-control border border-line bg-surface p-1 text-ink shadow-md",
        )}
      >
        {offered.map((s, i) => (
          <li
            key={s.key}
            id={`${listId}-${i}`}
            role="option"
            aria-selected={i === active}
            // mousedown, so the field keeps its focus and the pick lands.
            onMouseDown={(event) => {
              event.preventDefault();
              pick(s);
            }}
            onMouseEnter={() => setActive(i)}
            className={cn(
              "flex cursor-pointer items-center gap-2.5 rounded-control px-2 py-1.5",
              i === active && "bg-sunk",
            )}
          >
            <PersonAvatar person={{ name: s.key, displayName: s.name, picture: s.picture ?? null }} size="xs" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[13.5px]">{s.name}</span>
              <span className="block truncate text-[12px] text-muted">{valueFor(s, want)}</span>
            </span>
            <span className="shrink-0 text-[11.5px] text-muted">
              {s.source === "box" ? t("contacts.suggest.box") : t("contacts.suggest.contact")}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
