import { Link } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { StatusDot } from "@/components/ui/badge";
import type { AppTileModel } from "@/lib/apps";
import { cn } from "@/lib/utils";

/* One app on the homepage.
 *
 * A 50px rounded square in --sunk with a hairline border, a line glyph in
 * --muted that tints to --accent under the pointer, the app's name below it
 * and one quiet second line under that. The tile is a link and nothing else —
 * every tile the grid hands it is an app that answered, so there is no
 * disabled state to design: a tile that cannot be followed is not rendered
 * (see lib/apps.ts).
 *
 * No inline styles anywhere, including the stagger: under `style-src 'self'`
 * a `style=""` attribute is refused, so the per-tile delay comes out of the
 * fixed class list below, which Tailwind compiles into the stylesheet like
 * any other utility. Everything it drives is switched off in one place by the
 * `prefers-reduced-motion` block in styles/index.css.
 */

/* Ten steps at 35ms covers the full catalogue; anything past the end holds at
 * the last value rather than opening a gap. Written out as whole literal class
 * names on purpose — Tailwind scans source text, and a delay assembled from a
 * template string would compile to nothing and silently drop the stagger. */
const STAGGER: readonly string[] = [
  "[animation-delay:0ms]",
  "[animation-delay:35ms]",
  "[animation-delay:70ms]",
  "[animation-delay:105ms]",
  "[animation-delay:140ms]",
  "[animation-delay:175ms]",
  "[animation-delay:210ms]",
  "[animation-delay:245ms]",
  "[animation-delay:280ms]",
  "[animation-delay:315ms]",
];

export function staggerClass(index: number): string {
  const step = Math.max(0, Math.min(index, STAGGER.length - 1));
  return STAGGER[step] ?? "";
}

export interface AppTileProps {
  tile: AppTileModel;
  /** Position in the grid. Drives the enter stagger and nothing else. */
  index?: number;
}

export function AppTile({ tile, index = 0 }: AppTileProps) {
  const { app, href, leavesAdmin, detail, attention } = tile;
  const second = attention === null ? detail : attention.note;

  const body = (
    <>
      <span className="relative inline-flex">
        <span
          className={cn(
            "flex size-[50px] items-center justify-center",
            "rounded-card border border-line bg-sunk text-muted",
            "transition-[color,background-color,border-color] duration-150",
            "group-hover:border-accent/45 group-hover:bg-accent-wash group-hover:text-accent",
            "group-focus-visible:border-accent/45 group-focus-visible:text-accent",
            "group-active:translate-y-px",
          )}
        >
          <HugeiconsIcon
            icon={app.icon}
            size={24}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
        </span>

        {/* Semantic colour, never the accent: the accent means "this box", and
            a tile that needs looking at is not a different kind of box. The
            ring punches the dot out of the tile's own corner. */}
        {attention !== null && (
          <StatusDot
            state={attention.level}
            className="absolute -top-1 -right-1 size-2.5 ring-2 ring-surface"
          />
        )}
      </span>

      <span className="block w-full truncate text-center text-[12.5px] leading-tight font-medium text-ink">
        {app.name}
      </span>

      {/* The second line, when there is one. A tile that needs attention says
          so in words — the dot is aria-hidden, so this text is the only
          channel a screen reader has for it. Otherwise it is a figure
          something measured, and if nothing did, the line is not there:
          filler under a 78px tile is an ellipsis with a word in front of it. */}
      {second !== null && (
        <span
          className={cn(
            "block w-full truncate text-center text-[11px] leading-tight",
            attention === null ? "numeric text-faint" : toneClass(attention.level),
          )}
        >
          {second}
        </span>
      )}
    </>
  );

  const className = cn(
    "group flex flex-col items-center gap-2 rounded-card px-1 py-2",
    "no-underline outline-offset-4",
  );

  return (
    <li className={cn("animate-rise", staggerClass(index))}>
      {leavesAdmin ? (
        <a href={href} title={app.note} className={className}>
          {body}
        </a>
      ) : (
        <Link to={href} title={app.note} className={className}>
          {body}
        </Link>
      )}
    </li>
  );
}

function toneClass(level: "warn" | "crit" | "pending"): string {
  if (level === "warn") return "text-warn";
  if (level === "crit") return "text-crit";
  return "text-muted";
}

/** A tile-shaped placeholder, for the moment before anything has answered. */
export function AppTileSkeleton({ index = 0 }: { index?: number }) {
  return (
    <li
      aria-hidden="true"
      className={cn("flex flex-col items-center gap-2 px-1 py-2", "animate-rise", staggerClass(index))}
    >
      <span className="size-[50px] animate-breathe rounded-card border border-hair bg-sunk" />
      <span className="h-3 w-12 animate-breathe rounded-control bg-sunk" />
      <span className="h-2.5 w-16 animate-breathe rounded-control bg-sunk" />
    </li>
  );
}
