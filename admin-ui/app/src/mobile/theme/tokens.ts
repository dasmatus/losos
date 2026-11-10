/* The theme layer: the generated palette (tokens.gen.ts), the types and the
 * mode the owner picks, in the words the panes read by.
 *
 * LosTheme.kt (the Compose file this app replaces) had three layers - a
 * Material colour scheme, an ok/warn/crit triple and global Typography and
 * Shapes - composed over one ThemeMode enum. This file is that layering on
 * React Native: PALETTES (generated) plus the port's Typography and Shapes
 * re-expressed as style numbers the panes use. The colours in
 * tokens.gen.ts are LosTheme.kt-exact by construction; the geometry here is
 * literal from LosTheme.kt too, so a pane ported line for line keeps its
 * proportions.
 *
 * The store-aware hook (how the mode arrives) lives in useTokens.ts; this
 * file stays React-free of the store to keep the import direction one-way.
 */

import { PALETTES, type LososPalette, type PaletteKey } from "./tokens.gen";

export type { LososPalette, PaletteKey };

/** Theme mode the owner can pick: follow the system, or pin one
 *  (LosTheme.kt enum ThemeMode; the Settings tab's three filter chips). */
export type ThemeMode = "system" | "light" | "dark";

/** What a mode resolves to once the system's own preference is in. */
export type ThemeResolved = "light" | "dark";

/** lososTheme(mode) when it was a Composable: the system arm asks the
 *  platform's colour scheme, the pinned arms answer directly. */
export function resolveTheme(mode: ThemeMode, systemPrefersDark: boolean): ThemeResolved {
  switch (mode) {
    case "light":
      return "light";
    case "dark":
      return "dark";
    default:
      return systemPrefersDark ? "dark" : "light";
  }
}

/** Typography by the port's names (LosTheme.kt lososTypography): the panes
 *  ask for these names, never for raw numbers. */
export interface LososTypeStyle {
  fontSize: number;
  lineHeight?: number;
  fontWeight?: "400" | "500" | "600";
  letterSpacing?: number;
}

export const LOSOS_TYPE = {
  headlineMedium: { fontSize: 26, fontWeight: "600", letterSpacing: -0.2 },
  headlineSmall: { fontSize: 21, fontWeight: "600", letterSpacing: -0.1 },
  titleMedium: { fontSize: 16, fontWeight: "600" },
  bodyMedium: { fontSize: 15, lineHeight: 21 },
  bodySmall: { fontSize: 13, lineHeight: 18 },
  labelSmall: { fontSize: 11, fontWeight: "600", letterSpacing: 0.5 },
} as const satisfies Record<string, LososTypeStyle>;

/** The radii the panes ask for by the Shapes port's words (lososShapes:
 *  8/12/16/24), plus the few one-off corners named for where they are used:
 *  the demo chip, the meter bar, app tiles' glyph plate. */
export const LOSOS_RADIUS = {
  extraSmall: 8,
  small: 12,
  medium: 16,
  large: 24,
  chip: 6,
  meter: 4,
  glyph: 10,
} as const;

/** The palette and the current-mode helpers a pane reads; `extra` is
 *  LosTheme.kt's OkWarnCrit - every PaneRow tone and the DemoChip tint. */
export interface LososTokens {
  palette: LososPalette;
  extra: { ok: string; warn: string; crit: string };
  dark: boolean;
}

export function tokensOf(mode: ThemeMode, systemPrefersDark: boolean): LososTokens {
  const palette = PALETTES[resolveTheme(mode, systemPrefersDark)];
  return {
    palette,
    extra: { ok: palette.ok, warn: palette.warn, crit: palette.crit },
    dark: resolveTheme(mode, systemPrefersDark) === "dark",
  };
}
