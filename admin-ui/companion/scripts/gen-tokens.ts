/* Generate src/theme/tokens.gen.ts from the admin SPA's palette.
 *
 * Run with: bun scripts/gen-tokens.ts   (or `bun run tokens` in this folder)
 *
 * The companion shares the admin front ends' one palette
 * (admin-ui/app/src/styles/tokens.css): LosOS Git, Nextcloud and LosOS Login
 * ship that file verbatim, and the Compose app this project used to be
 * ported it into LosTheme.kt by hand. This script is that port, redone on
 * every change so a colour moving in tokens.css cannot drift past this app
 * silently.
 *
 * Two provenance rules, so nobody re-derives the wrong palette:
 *
 *  1. tokens.css carries no Material colours a cards-and-chips phone UI
 *     wants: onPrimary/onSecondary and friends. Those slots come from the
 *     Compose port verbatim (they are what the app shipped).
 *
 *  2. Where the document and the port disagree — the palette grew after the
 *     Compose port was written — the port's shipped colours win, exactly as
 *     this app looked the day Compose was removed:
 *
 *     - dark ok/warn/crit: tokens.css #4fa583/#c79a3e/#d1756c, port
 *       #5fb894/#ce9a44/#d67c72 (LosTheme.kt ExtraDark)
 *     - dark secondary family: tokens.css person-blue #7aa5e0 over
 *       #12213a, port #7fa6d2 over #12233b with #9dbbd8 text (the tab bar's
 *       selected pill)
 *     - light hair: tokens.css #eaeef1, port #eaeff1
 *
 *     Each such row lives in OVERRIDES below with the LosTheme.kt line it
 *     came from. When a palette change is confirmed for the phone, delete
 *     the row here and let the document speak again.
 *
 * The parser reads custom properties from the light arm (the first that
 * declares --ground) and the explicit-dark arm (the one that wins over the
 * media arm in tokens.css's own three-state scheme); the media arm carries
 * the same values, so picking the explicit one is what a reader of
 * tokens.css's header would do.
 */

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const TOKENS_CSS = resolve(HERE, "../../app/src/styles/tokens.css");
const OUT = resolve(HERE, "../src/theme/tokens.gen.ts");

/* The palette's slots (LosTheme.kt, lightColorScheme/darkColorScheme +
 * ExtraLight/ExtraDark): everything a pane reads, named the Compose port
 * words. */
const COLOR_KEYS = [
  "primary",
  "onPrimary",
  "primaryContainer",
  "onPrimaryContainer",
  "secondary",
  "onSecondary",
  "secondaryContainer",
  "onSecondaryContainer",
  "background",
  "onBackground",
  "surface",
  "onSurface",
  "surfaceVariant",
  "onSurfaceVariant",
  "outline",
  "outlineVariant",
  "error",
  "ok",
  "warn",
  "crit",
] as const;

type ColorKey = (typeof COLOR_KEYS)[number];
type Palette = Partial<Record<ColorKey, string>>;

/** The Compose port, in tokens.css words: which `--custom-property` each
 *  slot reads from. `sameAs` derives a slot from another slot (the port
 *  reused accent/ink/etc. on the on-colours exactly that way, and ties
 *  error to crit in both modes). */
const FROM_VAR: Partial<Record<ColorKey, string>> = {
  primary: "accent",
  primaryContainer: "accent-wash",
  secondary: "person-blue",
  secondaryContainer: "person-blue-wash",
  background: "ground",
  onBackground: "ink",
  surface: "surface",
  onSurface: "ink",
  surfaceVariant: "sunk",
  onSurfaceVariant: "muted",
  outline: "line",
  outlineVariant: "hair",
  ok: "ok",
  warn: "warn",
  crit: "crit",
};
const SAME_AS: Partial<Record<ColorKey, ColorKey>> = {
  onPrimaryContainer: "primary",
  onSecondaryContainer: "secondary",
  error: "crit",
};

/** LosTheme.kt's own values, where not (or not still) in tokens.css. See
 *  the header for the derived property each row replaces. */
const OVERRIDES: Record<"both" | "light" | "dark", Palette> = {
  both: {
    onPrimary: "#ffffff",
    onSecondary: "#ffffff",
  },
  light: {
    outlineVariant: "#eaeff1", // Hair; port spelling (LosTheme.kt Light).
  },
  dark: {
    onPrimary: "#06282d",
    onSecondary: "#10253f",
    onSecondaryContainer: "#9dbbd8",
    secondary: "#7fa6d2",
    secondaryContainer: "#12233b",
    ok: "#5fb894", // LosTheme.kt ExtraDark.
    warn: "#ce9a44",
    crit: "#d67c72",
  },
};

// ── Parsing (tokens.css stays plain custom properties; header note) ───────

interface Arm {
  selector: string;
  props: Map<string, string>;
}

/** Match `selector { body }` blocks by brace depth. tokens.css's foreword
 *  says nothing but declarations, and its comments carry no braces, so this
 *  is the whole grammar. */
function armsOf(css: string): Arm[] {
  const arms: Arm[] = [];
  let depth = 0;
  let selector = "";
  let body = "";
  for (let i = 0; i < css.length; i++) {
    const ch = css[i] as string;
    if (ch === "{") {
      if (depth === 0) {
        body = "";
        depth = 1;
        continue;
      }
      depth++;
    } else if (ch === "}") {
      depth--;
      if (depth === 0) {
        arms.push({ selector: selector.trim(), props: parse(body) });
        selector = "";
        body = "";
        continue;
      }
      if (depth < 0) break;
    }
    if (depth === 0) selector += ch;
    else if (depth >= 1) body += ch;
  }
  if (depth !== 0) throw new Error("tokens.css: unbalanced braces");
  return arms;

  function parse(body: string): Map<string, string> {
    const bare = body.replace(/\/\*[\s\S]*?\*\//g, "");
    const props = new Map<string, string>();
    const re = /--([a-z0-9-]+)\s*:\s*([^;{}]+);/g;
    for (let m = re.exec(bare); m !== null; m = re.exec(bare)) {
      props.set((m[1] as string).trim(), (m[2] as string).trim());
    }
    return props;
  }
}

/** The palette arm the document declares each mode in. */
function paletteArm(arms: Arm[], mode: "light" | "dark"): Arm {
  const selectors = (a: Arm) => a.selector.split(",").map((s) => s.trim());
  const hit = arms.find((a) =>
    a.props.has("ok") &&
    a.props.has("ground") &&
    (mode === "light"
      ? selectors(a).some((s) => s === ":root" || s === "[data-theme-light]")
      : selectors(a).some(
          (s) =>
            s === ':root[data-theme="dark"]' ||
            s === '[data-theme-dark]' ||
            s === ':root[data-theme="losos-dark"]',
        )),
  );
  if (!hit) {
    throw new Error(
      `tokens.css: no arm declaring --ok and --ground for ${mode} found`,
    );
  }
  return hit;
}

// ── Assembly ──────────────────────────────────────────────────────────────

const HEX6 = /^#[0-9a-f]{6}$/;

function normalize(value: string, slot: string, source: string): string {
  const hex = value.trim().toLowerCase();
  if (!HEX6.test(hex)) {
    throw new Error(`${slot} (${modeWord(source)}): '${value}' is not a #rrggbb colour`);
  }
  return hex;
}
const modeWord = (source: string) => (source.startsWith("--") ? "tokens.css" : source);

function paletteFor(arm: Arm, mode: "light" | "dark"): Record<ColorKey, string> {
  const out = {} as Record<ColorKey, string>;

  // Parsed slots, in FROM_VAR; derivations resolve from already-set slots.
  const slotOrder = COLOR_KEYS.filter((k) => !SAME_AS[k]);
  for (const key of slotOrder) {
    // The port wins over the document, both-over-mode: OVERRIDES.both, then
    // the mode's own port rows, then the parsed property.
    const port = OVERRIDES[mode][key] ?? OVERRIDES.both[key];
    if (port !== undefined) {
      out[key] = normalize(port, key, "LosTheme.kt (OVERRIDES)");
      continue;
    }
    const prop = FROM_VAR[key];
    const parsed = prop ? arm.props.get(prop) : undefined;
    if (prop === undefined || parsed === undefined) {
      throw new Error(`mode ${mode}: slot ${key} has no OVERRIDES row and no --${prop ?? "?"} to read`);
    }
    out[key] = normalize(parsed, key, `--${prop}`);
  }
  for (const [key, base] of Object.entries(SAME_AS) as [ColorKey, ColorKey][]) {
    const port = OVERRIDES[mode][key] ?? OVERRIDES.both[key];
    out[key] = port !== undefined
      ? normalize(port, key, "LosTheme.kt (OVERRIDES)")
      : out[base];
  }

  for (const key of COLOR_KEYS) {
    if (out[key] === undefined) throw new Error(`mode ${mode}: slot ${key} left unset`);
  }
  return out;
}

// ── Emission ──────────────────────────────────────────────────────────────

function paletteBlock(values: Record<ColorKey, string>, indent: string): string {
  return COLOR_KEYS.map((k) => `${indent}${k}: "${values[k]}",`).join("\n");
}

function main(): void {
  const arms = armsOf(readFileSync(TOKENS_CSS, "utf8"));
  const light = paletteFor(paletteArm(arms, "light"), "light");
  const dark = paletteFor(paletteArm(arms, "dark"), "dark");

  const file = `/* GENERATED FILE - do not edit by hand.
 *
 * Generated by scripts/gen-tokens.ts from admin-ui/app/src/styles/tokens.css,
 * the palette the admin SPA, LosOS Git, Nextcloud and LosOS Login all ship;
 * the source file's foreword is its own grammar. Where the document and the
 * Compose port (admin-ui/companion/.../LosTheme.kt, the palette this app
 * shipped) disagreed, the port's values stayed, so a reader comparing this
 * file to the screen before a launcher icon change should not see a jump:
 * scripts/gen-tokens.ts's OVERRIDES table carries each such row with a note.
 *
 * Re-run with \`bun run tokens\` in admin-ui/companion after any tokens.css
 * change; the parser throws on a slot that stops existing, so a renamed
 * custom property is a build break, not a silent restyle.
 */

export const PALETTES = {
  light: {
${paletteBlock(light, "    ")}
  },
  dark: {
${paletteBlock(dark, "    ")}
  },
} as const;

/** One mode's palette: every slot is lowercase #rrggbb. */
// The palette's shape, not one side's literals: a consumer must be able to
// hold the dark palette in a slot typed "the palette" (tokens.ts does), so a
// keyed map to string — same slots, widened values.
export type LososPalette = { [K in keyof (typeof PALETTES)["light"]]: string }
export type PaletteKey = keyof LososPalette;
`;

  mkdirSync(dirname(OUT), { recursive: true });
  writeFileSync(OUT, file);
  console.log(
    `wrote ${OUT} (light ${light.primary} / dark ${dark.primary}); ` +
      `${COLOR_KEYS.length} slots each`,
  );
}

main();

