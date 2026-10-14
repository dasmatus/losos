import { Box, Text } from "ink";
import { LOGO_LARGE, LOGO_SMALL, type Pixel } from "../generated/logo";
import { PALETTES } from "../generated/palette";
import { hexOf, nearest256, type Theme } from "../theme";

/* The salmon (admin-ui/themes/brand/salmon-128.png), two pixels per cell:
 * the upper half block takes the top pixel as its colour and the bottom one
 * as its background. Edge pixels are blended onto the page's ground colour,
 * so the outline is soft on either theme. */

function blend(pixel: Pixel, ground: [number, number, number]): [number, number, number] | null {
  if (pixel === null || pixel[3] < 40) return null;
  const a = pixel[3] / 255;
  return [0, 1, 2].map((i) => (pixel[i] ?? 0) * a + ground[i]! * (1 - a)) as [number, number, number];
}

function rgbOfHex(hex: string): [number, number, number] {
  return [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];
}

export interface Cell {
  char: string;
  fg?: string;
  bg?: string;
}

export function logoCells(size: "small" | "large", theme: Theme, groundHex: string): Cell[][] {
  const pixels = size === "small" ? LOGO_SMALL : LOGO_LARGE;
  const ground = rgbOfHex(groundHex);
  const colour = (rgb: [number, number, number]): string =>
    theme.truecolor ? hexOf(...rgb) : `ansi256(${nearest256(...rgb)})`;
  const rows: Cell[][] = [];
  for (let y = 0; y < pixels.length; y += 2) {
    const top = pixels[y] ?? [];
    const bottom = pixels[y + 1] ?? [];
    const row: Cell[] = [];
    for (let x = 0; x < top.length; x += 1) {
      const up = blend(top[x] ?? null, ground);
      const down = blend(bottom[x] ?? null, ground);
      if (up === null && down === null) row.push({ char: " " });
      else if (down === null) row.push({ char: "▀", fg: colour(up!) });
      else if (up === null) row.push({ char: "▄", fg: colour(down) });
      else row.push({ char: "▀", fg: colour(up), bg: colour(down) });
    }
    rows.push(row);
  }
  return rows;
}

export function Logo({ size, theme }: { size: "small" | "large"; theme: Theme }) {
  const rows = logoCells(size, theme, PALETTES[theme.mode].ground.hex);
  return (
    <Box flexDirection="column">
      {rows.map((row, y) => (
        <Text key={y} backgroundColor={theme.c.ground}>
          {row.map((cell, x) => (
            <Text key={x} {...(cell.fg === undefined ? {} : { color: cell.fg })} backgroundColor={cell.bg ?? theme.c.ground}>
              {cell.char}
            </Text>
          ))}
        </Text>
      ))}
    </Box>
  );
}
