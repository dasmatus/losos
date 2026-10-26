/* A calendar's colour. CalDAV keeps it as #rrggbb, which phones show, so
 * the page offers the six person colours of tokens.css by their light
 * values and draws a calendar holding one of them with the token itself,
 * dark theme included (`data-accent`). A colour some phone picked is drawn
 * as it is. */

import type * as React from "react";
import { ACCENTS, type Accent } from "@/lib/api";

/** The light values of tokens.css's person colours, in ACCENTS order. */
export const CALENDAR_COLORS: Record<Accent, string> = {
  teal: "#0e6e7d",
  blue: "#2f5f9e",
  violet: "#6a4c9c",
  rose: "#a3445e",
  amber: "#86600f",
  green: "#2e7357",
};

/** The person colour a stored value is, if it is one. */
export function accentOf(color: string | undefined): Accent | null {
  if (color === undefined) return null;
  const c = color.slice(0, 7).toLowerCase();
  return ACCENTS.find((a) => CALENDAR_COLORS[a] === c) ?? null;
}

/** Props that colour an element with a calendar's colour: `data-accent`
 *  for one of ours (inside, `bg-person` and `text-person` follow), a style
 *  for anything else. */
export function colorProps(color: string | undefined): { "data-accent"?: Accent; style?: React.CSSProperties } {
  const accent = accentOf(color);
  if (accent !== null) return { "data-accent": accent };
  if (color === undefined || !/^#[0-9a-fA-F]{6}/.test(color)) return { "data-accent": "teal" };
  const hex = color.slice(0, 7);
  return { style: { "--person": hex, "--person-wash": `color-mix(in oklab, ${hex} 16%, transparent)` } as React.CSSProperties };
}

/** The colour a new calendar gets: the first one none of `taken` has. */
export function nextColor(taken: readonly (string | undefined)[]): string {
  const used = new Set(taken.map((c) => accentOf(c)));
  const free = ACCENTS.find((a) => !used.has(a)) ?? "teal";
  return CALENDAR_COLORS[free];
}
