/* The pieces every scene draws with: the heading, the caption, wrapped
 * lines, a card, a button and a person's round picture.
 *
 * Shared by scenes.tsx and more-scenes.tsx, so a heading or a card looks the
 * same in every step. Like the rest of the tour, all of it is SVG
 * presentation attributes and palette classes; see mini-page.tsx for why
 * there is no style attribute. */

import * as React from "react";
import { clamp, fd } from "./motion";

export interface SceneProps {
  frame: number;
}

/** The width of a scene's content box (mini-page.tsx, CONTENT). */
export const W = 460;

export function Heading({ children, mono = false }: { children: React.ReactNode; mono?: boolean }) {
  return (
    <text x="0" y="14" fontSize="14" fontWeight="600" className={mono ? "fill-ink font-mono" : "fill-ink"}>
      {children}
    </text>
  );
}

/** Lines of at most about `width` characters, broken at spaces: SVG text does not wrap. */
export function wrap(text: string, width: number): string[] {
  const lines: string[] = [];
  let line = "";
  for (const word of text.split(" ")) {
    if (line.length > 0 && line.length + 1 + word.length > width) {
      lines.push(line);
      line = word;
    } else line = line.length > 0 ? `${line} ${word}` : word;
  }
  if (line.length > 0) lines.push(line);
  return lines;
}

export function Caption({ y, children }: { y: number; children: React.ReactNode }) {
  return (
    <text x="2" y={y} fontSize="9" className="fill-muted">
      {children}
    </text>
  );
}

/** The real UI's card: the surface with a one-pixel line, half a pixel in so it lands on the grid. */
export function Panel({ x = 0, y, w = W, h, sunk = false }: { x?: number; y: number; w?: number; h: number; sunk?: boolean }) {
  return (
    <rect
      x={x + 0.5}
      y={y + 0.5}
      width={Math.max(0, w - 1)}
      height={Math.max(0, h - 1)}
      rx="8"
      className={sunk ? "fill-sunk stroke-line" : "fill-surface stroke-line"}
    />
  );
}

/** A small button, centred on (`cx`, `cy`), sunk by `press` (0..1) the way a click looks. */
export function Pill({
  cx,
  cy,
  w,
  label,
  press = 0,
  primary = true,
}: {
  cx: number;
  cy: number;
  w: number;
  label: string;
  press?: number;
  primary?: boolean;
}) {
  return (
    <g transform={`translate(${cx} ${cy}) scale(${1 - 0.06 * press}) translate(${-w / 2} -11)`}>
      <rect x={primary ? 0 : 0.5} y={primary ? 0 : 0.5} width={primary ? w : w - 1} height={primary ? 22 : 21} rx="5" className={primary ? "fill-accent" : "fill-surface stroke-line"} />
      <text x={w / 2} y="14.2" fontSize="9.5" textAnchor="middle" className={primary ? "fill-surface" : "fill-ink"}>
        {label}
      </text>
    </g>
  );
}

/** A person's round picture: their initial on a coloured disc, the way the UI draws someone with no photo yet. */
export function Face({
  cx,
  cy,
  r,
  initial,
  tone = "accent",
}: {
  cx: number;
  cy: number;
  r: number;
  initial: string;
  tone?: "accent" | "ink" | "warn";
}) {
  const disc = tone === "accent" ? "fill-accent" : tone === "warn" ? "fill-warn" : "fill-ink";
  return (
    <g>
      <circle cx={cx} cy={cy} r={r} className={disc} />
      <text x={cx} y={cy + r * 0.36} fontSize={r * 1.05} fontWeight="600" textAnchor="middle" className="fill-surface">
        {initial}
      </text>
    </g>
  );
}

/** How many letters of `text` are typed by frame `f`, starting at `from`, one every `every` frames. */
export function typed(f: number, from: number, text: string, every = 4): string {
  return text.slice(0, clamp(fd((f - from) / every), 0, text.length));
}
