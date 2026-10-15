/* The miniature of this page every tour scene is drawn inside.
 *
 * It is the tour's one constant: the same top bar, the same sidebar, and
 * the sidebar's teal entry travelling down it from step to step, so each
 * step says where its feature lives as well as what it does. The content
 * area on the right is the scene's own (scenes.tsx).
 *
 * All of it is SVG with presentation attributes and Tailwind classes. No
 * style attribute anywhere: the admin CSP (`style-src 'self'`) refuses
 * those, and an SVG attribute such as `transform` or `opacity` is not one.
 * Colours are the palette's classes (fill-ink, stroke-line, fill-accent), so
 * the miniature follows the light and dark themes with the page around it. */

import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import { HardDriveIcon, Home01Icon, LayoutGridIcon, Settings01Icon, Share08Icon } from "@hugeicons/core-free-icons";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { paneById } from "@/screens/settings/panes";
import { E, mix, prog } from "./motion";

export const VIEW_W = 640;
export const VIEW_H = 360;
/** Where a scene's content sits, and how much room it has. */
export const CONTENT = { x: 166, y: 42, w: 460, h: 304 } as const;

/** The sidebar entries the miniature draws, in the real sidebar's order. */
export type MiniEntry = "overview" | "apps" | "storage" | "mesh" | "settings" | "history" | "about";

interface EntryDef {
  id: MiniEntry;
  label: MessageKey;
  icon: IconSvgElement;
  /** Under Settings, as the real fold shows it when open. */
  sub?: boolean;
  /** Settings is a fold: a chevron, never the current entry. */
  fold?: boolean;
}

const ENTRIES: readonly EntryDef[] = [
  { id: "overview", label: "shell.nav.overview", icon: Home01Icon },
  { id: "apps", label: "shell.nav.apps", icon: LayoutGridIcon },
  { id: "storage", label: "shell.nav.storage", icon: HardDriveIcon },
  { id: "mesh", label: "shell.nav.mesh", icon: Share08Icon },
  { id: "settings", label: "shell.nav.settings", icon: Settings01Icon, fold: true },
  { id: "history", label: paneById("history").labelKey, icon: paneById("history").icon, sub: true },
  { id: "about", label: paneById("about").labelKey, icon: paneById("about").icon, sub: true },
];

const ROW_Y = 74;
const ROW_H = 21;

function pillFor(id: MiniEntry): { x: number; y: number; w: number } {
  const index = Math.max(0, ENTRIES.findIndex((entry) => entry.id === id));
  const sub = ENTRIES[index]?.sub === true;
  return { x: sub ? 32 : 20, y: ROW_Y + index * ROW_H, w: sub ? 108 : 120 };
}

/** An SVG-safe id: useId's output can carry characters url(#…) does not take. */
export function useSvgId(prefix: string): string {
  return `${prefix}-${React.useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
}

/** A hugeicon placed inside an SVG, as a nested <svg>. */
export function Glyph({
  icon,
  x,
  y,
  size,
  className,
}: {
  icon: IconSvgElement;
  x: number;
  y: number;
  size: number;
  className?: string;
}) {
  return (
    <HugeiconsIcon
      icon={icon}
      x={x}
      y={y}
      size={size}
      strokeWidth={1.5}
      color="currentColor"
      className={className}
      aria-hidden="true"
    />
  );
}

/** The hatch the real UI draws for "on the mesh": the accent with light stripes. */
export function Hatch({ id }: { id: string }) {
  return (
    <pattern id={id} width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
      <rect width="5" height="5" className="fill-accent" />
      <rect width="1.6" height="5" className="fill-white" opacity="0.32" />
    </pattern>
  );
}

export interface MiniPageProps {
  /** The entry the pill rests on. */
  entry: MiniEntry;
  /** The entry it is arriving from, while `arrival` runs. */
  from: MiniEntry;
  /** Frames since the step changed; the pill and the content settle over the first 36. */
  arrival: number;
  children: React.ReactNode;
}

export const ARRIVAL = 36;

export function MiniPage({ entry, from, arrival, children }: MiniPageProps) {
  const t = useT();
  const clip = useSvgId("tour-pill");
  const logo = logoFor();

  const a = pillFor(from);
  const b = pillFor(entry);
  const move = prog(arrival, 0, 30, E.glide);
  const pill = { x: mix(a.x, b.x, move), y: mix(a.y, b.y, move), w: mix(a.w, b.w, move) };
  // The content arrives as the pill lands: a short rise in from the right.
  const enter = prog(arrival, 6, ARRIVAL, E.out);
  const shift = mix(10, 0, enter);

  const rows = ENTRIES.map((def, index) => {
    const y = ROW_Y + index * ROW_H;
    const left = def.sub === true ? 40 : 28;
    return { def, y, left };
  });

  const labels = (active: boolean) =>
    rows.map(({ def, y, left }) => (
      <g key={def.id} className={active ? "text-surface" : "text-ink"}>
        <Glyph icon={def.icon} x={left} y={y + 5} size={10} />
        <text x={left + 16} y={y + 13.2} fontSize="9.5" className={active ? "fill-surface" : "fill-ink"}>
          {t(def.label)}
        </text>
        {def.fold === true && (
          <path
            d={`M ${128} ${y + 8} l 3 2.5 l -3 2.5`}
            fill="none"
            strokeWidth="1"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={active ? "stroke-surface" : "stroke-muted"}
          />
        )}
      </g>
    ));

  return (
    <>
      <defs>
        <clipPath id={clip}>
          <rect x={pill.x} y={pill.y + 1} width={pill.w} height={ROW_H - 2} rx="5" />
        </clipPath>
      </defs>

      <rect width={VIEW_W} height={VIEW_H} className="fill-ground" />

      {/* Top bar: the logo, "this box", sign out. */}
      <rect width={VIEW_W} height={30} className="fill-surface" />
      <line x1="0" y1="30.5" x2={VIEW_W} y2="30.5" className="stroke-line" strokeWidth="1" />
      <image href={logo.url} x="16" y="7" width="16" height="16" />
      <rect x="38.5" y="8.5" width="52" height="13" rx="6.5" fill="none" className="stroke-line" />
      <text x="64.5" y="17.6" fontSize="8.5" textAnchor="middle" className="fill-muted">
        {t("shell.thisBox")}
      </text>
      <text x={VIEW_W - 18} y="18" fontSize="9" textAnchor="end" className="fill-muted">
        {t("shell.signOut")}
      </text>

      {/* The sidebar, with its search button. */}
      <rect x="14.5" y="42.5" width="132" height={ROW_Y + ENTRIES.length * ROW_H - 36} rx="8" className="fill-sunk stroke-line" />
      <g className="stroke-muted" fill="none" strokeWidth="1.2" strokeLinecap="round">
        <circle cx="129.5" cy="57.5" r="3.5" />
        <line x1="132.2" y1="60.2" x2="134.8" y2="62.8" />
      </g>
      <line
        x1="34.5"
        y1={ROW_Y + 5 * ROW_H}
        x2="34.5"
        y2={ROW_Y + 7 * ROW_H - 3}
        className="stroke-line"
        strokeWidth="1"
      />

      {labels(false)}
      <rect x={pill.x} y={pill.y + 1} width={pill.w} height={ROW_H - 2} rx="5" className="fill-accent" />
      {/* The same labels again in the pill's own colour, clipped to the pill,
          so an entry turns over exactly where the pill's edge crosses it. */}
      <g clipPath={`url(#${clip})`}>{labels(true)}</g>

      <g transform={`translate(${CONTENT.x + shift} ${CONTENT.y})`} opacity={enter}>
        {children}
      </g>
    </>
  );
}
