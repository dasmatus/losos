/* JSON Canvas colours: "1" to "6" are the presets (red, orange, yellow,
 * green, cyan, purple, as Obsidian draws them), anything else a hex colour.
 * The presets are house tokens in lab.css (--dg-c1 … --dg-c6), so they follow
 * the theme; a hex colour is the file's and stays as written. */

export const PRESETS = ["1", "2", "3", "4", "5", "6"] as const;
export type Preset = (typeof PRESETS)[number];

const HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/;

export function isColor(c: string | undefined): c is string {
  return c !== undefined && ((PRESETS as readonly string[]).includes(c) || HEX.test(c));
}

/** The CSS value a card or an arrow of colour `c` draws with; none for no colour. */
export function colorValue(c: string | undefined): string | undefined {
  if (!isColor(c)) return undefined;
  return HEX.test(c) ? c : `var(--dg-c${c})`;
}

/** The token names of the presets, for the export, which resolves them. */
export const PRESET_TOKENS = PRESETS.map((p) => `dg-c${p}`);
