/* The outline of a card of each shape, as SVG path data in the card's own
 * box (0,0 to w,h), and the box inside it the words fit in. The shape names
 * are Advanced Canvas's `styleAttributes.shape`, plus hexagon, cloud and note,
 * which this editor adds; a card with no shape is a rounded rectangle, as in
 * Obsidian. The export draws the same paths. */

export const SHAPES = [
  "pill",
  "diamond",
  "parallelogram",
  "circle",
  "predefined-process",
  "document",
  "database",
  "hexagon",
  "cloud",
  "note",
] as const;
export type Shape = (typeof SHAPES)[number];

const r = (v: number) => Math.round(v * 10) / 10;

/** The card's outline, and any inner lines a shape has (a process's bars,
 *  a database's rim, a note's fold). */
export function outline(shape: string | undefined, w: number, h: number): { d: string; inner?: string } {
  const R = Math.min(10, w / 4, h / 4);
  switch (shape) {
    case "pill": {
      const k = Math.min(h, w) / 2;
      return { d: `M${k} 0H${r(w - k)}A${k} ${k} 0 0 1 ${r(w - k)} ${h}H${k}A${k} ${k} 0 0 1 ${k} 0Z` };
    }
    case "diamond":
      return { d: `M${r(w / 2)} 0L${w} ${r(h / 2)}L${r(w / 2)} ${h}L0 ${r(h / 2)}Z` };
    case "parallelogram": {
      const s = Math.min(w / 4, h * 0.5);
      return { d: `M${r(s)} 0H${w}L${r(w - s)} ${h}H0Z` };
    }
    case "circle":
      return { d: `M0 ${r(h / 2)}A${r(w / 2)} ${r(h / 2)} 0 1 1 ${w} ${r(h / 2)}A${r(w / 2)} ${r(h / 2)} 0 1 1 0 ${r(h / 2)}Z` };
    case "predefined-process": {
      const b = Math.min(14, w / 8);
      return { d: `M0 0H${w}V${h}H0Z`, inner: `M${r(b)} 0V${h}M${r(w - b)} 0V${h}` };
    }
    case "document": {
      const a = Math.min(14, h / 6);
      return {
        d: `M0 0H${w}V${r(h - a)}C${r(w * 0.75)} ${r(h - 3 * a)} ${r(w * 0.25)} ${r(h + a)} 0 ${r(h - a)}Z`,
      };
    }
    case "database": {
      const e = Math.min(18, h / 5);
      return {
        d: `M0 ${e}A${r(w / 2)} ${e} 0 0 1 ${w} ${e}V${r(h - e)}A${r(w / 2)} ${e} 0 0 1 0 ${r(h - e)}Z`,
        inner: `M0 ${e}A${r(w / 2)} ${e} 0 0 0 ${w} ${e}`,
      };
    }
    case "hexagon": {
      const s = Math.min(w / 4, h / 2);
      return { d: `M${r(s)} 0H${r(w - s)}L${w} ${r(h / 2)}L${r(w - s)} ${h}H${r(s)}L0 ${r(h / 2)}Z` };
    }
    case "cloud": {
      // Five bumps round an ellipse's box, in proportion to the card.
      const p = (x: number, y: number) => `${r(x * w)} ${r(y * h)}`;
      return {
        d:
          `M${p(0.22, 0.88)}C${p(0.02, 0.88)} ${p(0.0, 0.58)} ${p(0.16, 0.5)}` +
          `C${p(0.1, 0.22)} ${p(0.36, 0.08)} ${p(0.48, 0.22)}` +
          `C${p(0.56, 0.02)} ${p(0.86, 0.06)} ${p(0.82, 0.34)}` +
          `C${p(1.0, 0.36)} ${p(1.02, 0.86)} ${p(0.8, 0.88)}Z`,
      };
    }
    case "note": {
      const f = Math.min(18, w / 5, h / 3);
      return { d: `M0 0H${r(w - f)}L${w} ${r(f)}V${h}H0Z`, inner: `M${r(w - f)} 0V${r(f)}H${w}` };
    }
    default:
      return {
        d: `M${R} 0H${r(w - R)}Q${w} 0 ${w} ${R}V${r(h - R)}Q${w} ${h} ${r(w - R)} ${h}H${R}Q0 ${h} 0 ${r(h - R)}V${R}Q0 0 ${R} 0Z`,
      };
  }
}

/** Where the words go inside a card of this shape. */
export function textBox(shape: string | undefined, w: number, h: number): { x: number; y: number; w: number; h: number } {
  const pad = 12;
  switch (shape) {
    case "diamond":
      return { x: w * 0.22, y: h * 0.22, w: w * 0.56, h: h * 0.56 };
    case "circle":
      return { x: w * 0.15, y: h * 0.15, w: w * 0.7, h: h * 0.7 };
    case "cloud":
      return { x: w * 0.18, y: h * 0.28, w: w * 0.64, h: h * 0.52 };
    case "parallelogram":
    case "hexagon": {
      const s = Math.min(w / 4, h / 2);
      return { x: s * 0.7, y: pad / 2, w: w - s * 1.4, h: h - pad };
    }
    case "pill":
      return { x: Math.min(h, w) / 3, y: pad / 2, w: w - (2 * Math.min(h, w)) / 3, h: h - pad };
    case "predefined-process": {
      const b = Math.min(14, w / 8);
      return { x: b + 6, y: pad / 2, w: w - 2 * b - 12, h: h - pad };
    }
    case "database": {
      const e = Math.min(18, h / 5);
      return { x: pad, y: 2 * e, w: w - 2 * pad, h: h - 3 * e };
    }
    case "document":
      return { x: pad, y: pad / 2, w: w - 2 * pad, h: h - pad - Math.min(14, h / 6) };
    default:
      return { x: 0, y: 0, w, h };
  }
}

/** A card the words sit centred in (every shape but the plain card and the note). */
export function centred(shape: string | undefined): boolean {
  return shape !== undefined && shape !== "note" && (SHAPES as readonly string[]).includes(shape);
}

/** An arrowhead of each Advanced Canvas style, pointing along +x with its
 *  tip at the origin, `s` long. `fill`: drawn filled or outlined. */
export function head(style: string | undefined, s = 12): { d: string; fill: boolean } {
  const w = s * 0.55;
  switch (style) {
    case "triangle-outline":
      return { d: `M0 0L${-s} ${r(-w)}L${-s} ${r(w)}Z`, fill: false };
    case "thin-triangle":
      return { d: `M${-s} ${r(-w)}L0 0L${-s} ${r(w)}`, fill: false };
    case "halved-triangle":
      return { d: `M0 0L${-s} ${r(-w)}L${-s} 0Z`, fill: true };
    case "diamond":
      return { d: `M0 0L${r(-s / 2)} ${r(-w)}L${-s} 0L${r(-s / 2)} ${r(w)}Z`, fill: true };
    case "diamond-outline":
      return { d: `M0 0L${r(-s / 2)} ${r(-w)}L${-s} 0L${r(-s / 2)} ${r(w)}Z`, fill: false };
    case "circle":
      return { d: `M0 0A${r(s / 3)} ${r(s / 3)} 0 1 1 ${r((-2 * s) / 3)} 0A${r(s / 3)} ${r(s / 3)} 0 1 1 0 0Z`, fill: true };
    case "circle-outline":
      return { d: `M0 0A${r(s / 3)} ${r(s / 3)} 0 1 1 ${r((-2 * s) / 3)} 0A${r(s / 3)} ${r(s / 3)} 0 1 1 0 0Z`, fill: false };
    case "blunt":
      return { d: `M0 ${r(-w)}V${r(w)}`, fill: false };
    default:
      return { d: `M0 0L${-s} ${r(-w)}L${-s} ${r(w)}Z`, fill: true };
  }
}

/** The dash pattern of an arrow's line style. */
export function dash(path: string | undefined): string | undefined {
  return path === "dotted" ? "1.5 6" : path === "short-dashed" ? "6 6" : path === "long-dashed" ? "16 8" : undefined;
}
