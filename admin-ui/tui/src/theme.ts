/* The web UI's palette (admin-ui/app/src/styles/tokens.css, read at build
 * time into src/generated/palette.ts) in the terminal: truecolor where the
 * terminal says it has it, the nearest of the 256 xterm colours otherwise.
 *
 * Dark or light: --theme, else COLORFGBG (the background's index, which
 * rxvt, Konsole and others set), else dark. */

import { PALETTES, type ColourName } from "./generated/palette";

export type Mode = "dark" | "light";

export interface Theme {
  mode: Mode;
  truecolor: boolean;
  /** A colour as Ink's `color` and `backgroundColor` props take it. */
  c: Record<ColourName, string>;
}

export function detectMode(env: NodeJS.ProcessEnv, flag?: string): Mode {
  if (flag === "dark" || flag === "light") return flag;
  const fgbg = env["COLORFGBG"];
  if (fgbg !== undefined && fgbg !== "") {
    const parts = fgbg.split(";");
    const bg = Number.parseInt(parts[parts.length - 1] ?? "", 10);
    if (Number.isInteger(bg)) return bg === 7 || bg >= 9 ? "light" : "dark";
  }
  return "dark";
}

export function detectTruecolor(env: NodeJS.ProcessEnv): boolean {
  const colorterm = (env["COLORTERM"] ?? "").toLowerCase();
  if (colorterm === "truecolor" || colorterm === "24bit") return true;
  if (env["FORCE_COLOR"] === "3") return true;
  const term = env["TERM"] ?? "";
  return /-direct$|truecolor|24bit/.test(term) || env["TERM_PROGRAM"] === "iTerm.app" || env["WT_SESSION"] !== undefined;
}

export function makeTheme(mode: Mode, truecolor: boolean): Theme {
  const palette = PALETTES[mode];
  const c = {} as Record<ColourName, string>;
  for (const [name, colour] of Object.entries(palette) as [ColourName, { hex: string; ansi256: number }][]) {
    c[name] = truecolor ? colour.hex : `ansi256(${colour.ansi256})`;
  }
  return { mode, truecolor, c };
}

/** The nearest xterm-256 index of any colour, for the logo's pixels. */
export function nearest256(r: number, g: number, b: number): number {
  const levels = [0, 95, 135, 175, 215, 255];
  const step = (v: number): number => {
    let best = 0;
    for (let i = 1; i < levels.length; i += 1) {
      if (Math.abs((levels[i] ?? 0) - v) < Math.abs((levels[best] ?? 0) - v)) best = i;
    }
    return best;
  };
  const ri = step(r);
  const gi = step(g);
  const bi = step(b);
  const cube = 16 + 36 * ri + 6 * gi + bi;
  const cubeRgb = [levels[ri] ?? 0, levels[gi] ?? 0, levels[bi] ?? 0];
  const grey = Math.max(0, Math.min(23, Math.round(((r + g + b) / 3 - 8) / 10)));
  const greyV = 8 + grey * 10;
  const d = (x: number[]): number => (r - (x[0] ?? 0)) ** 2 + (g - (x[1] ?? 0)) ** 2 + (b - (x[2] ?? 0)) ** 2;
  return d([greyV, greyV, greyV]) < d(cubeRgb) ? 232 + grey : cube;
}

export function hexOf(r: number, g: number, b: number): string {
  return `#${[r, g, b].map((v) => Math.round(v).toString(16).padStart(2, "0")).join("")}`;
}
