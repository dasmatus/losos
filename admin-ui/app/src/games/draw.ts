import type { Palette } from "./types";

/* What every game draws with: the palette from tokens.css and a few shapes.
 *
 * The colours are read off <html> rather than copied, so a game is in the
 * owner's theme, light or dark, and repaints when the theme changes
 * (arcade.tsx reads again on that). */

export function readPalette(): Palette {
  const root = getComputedStyle(document.documentElement);
  const read = (name: string, fallback: string) => root.getPropertyValue(name).trim() || fallback;
  return {
    surface: read("--surface", "#ffffff"),
    sunk: read("--sunk", "#eaedf0"),
    ink: read("--ink", "#18212a"),
    muted: read("--muted", "#5c6a77"),
    faint: read("--faint", "#8c98a4"),
    line: read("--line", "#dce1e6"),
    accent: read("--accent", "#0e6e7d"),
    accentWash: read("--accent-wash", "#e2eff1"),
    ok: read("--ok", "#2e7357"),
    warn: read("--warn", "#8c6512"),
    font: getComputedStyle(document.body).fontFamily || "system-ui, sans-serif",
  };
}

function channels(colour: string): [number, number, number] | null {
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(colour);
  if (hex?.[1] !== undefined) {
    const h = hex[1].length === 3 ? [...hex[1]].map((c) => c + c).join("") : hex[1];
    return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)) as [number, number, number];
  }
  const rgb = /^rgba?\(\s*(\d+)[ ,]+(\d+)[ ,]+(\d+)/i.exec(colour);
  if (rgb !== null) return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])];
  return null;
}

/** `from` at t = 0, `to` at t = 1. A colour it cannot read gives `to`. */
export function mix(from: string, to: string, t: number): string {
  const a = channels(from);
  const b = channels(to);
  if (a === null || b === null) return to;
  const k = Math.min(1, Math.max(0, t));
  const c = a.map((v, i) => Math.round(v + ((b[i] ?? v) - v) * k));
  return `rgb(${c[0]}, ${c[1]}, ${c[2]})`;
}

export function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
): void {
  const radius = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.arcTo(x + w, y, x + w, y + h, radius);
  ctx.arcTo(x + w, y + h, x, y + h, radius);
  ctx.arcTo(x, y + h, x, y, radius);
  ctx.arcTo(x, y, x + w, y, radius);
  ctx.closePath();
}

/* The salmon from the logo, as four strokes: a body, a tail, a fin and an
 * eye. Centred on (x, y), `size` long, nose to the right, tilted by
 * `angle` radians. Upstream swims it and Pairs has it on a card. */
export function salmon(
  ctx: CanvasRenderingContext2D,
  p: Palette,
  x: number,
  y: number,
  size: number,
  angle = 0,
): void {
  const half = size / 2;
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(angle);
  ctx.fillStyle = p.accent;
  // Tail
  ctx.beginPath();
  ctx.moveTo(-half * 0.62, 0);
  ctx.lineTo(-half, -half * 0.42);
  ctx.lineTo(-half * 0.9, 0);
  ctx.lineTo(-half, half * 0.42);
  ctx.closePath();
  ctx.fill();
  // Body
  ctx.beginPath();
  ctx.ellipse(half * 0.08, 0, half * 0.75, half * 0.34, 0, 0, Math.PI * 2);
  ctx.fill();
  // Dorsal fin
  ctx.beginPath();
  ctx.moveTo(-half * 0.1, -half * 0.28);
  ctx.lineTo(half * 0.12, -half * 0.52);
  ctx.lineTo(half * 0.28, -half * 0.26);
  ctx.closePath();
  ctx.fill();
  // Eye
  ctx.fillStyle = p.surface;
  ctx.beginPath();
  ctx.arc(half * 0.52, -half * 0.07, Math.max(1, size * 0.05), 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}
