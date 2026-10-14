import * as React from "react";
import { Box, Text, useInput, type Key } from "ink";
import type { ColourName } from "../generated/palette";
import type { Theme } from "../theme";
import { width, type Line, type Span } from "./model";

/* Ink hands over whatever one read from the terminal returned. Keys typed
 * while the process was busy, or pasted, come as one string ("j\r"), which
 * matches no single key. This splits such a string into one key per
 * character, so each is handled as if typed on its own. Escape sequences
 * (arrows and the like) arrive parsed already and are left whole. */
export function splitKeys(input: string, key: Key): [string, Key][] {
  if ([...input].length <= 1 || key.ctrl || key.meta || key.escape) return [[input, key]];
  const blank = Object.fromEntries(Object.keys(key).map((name) => [name, false])) as unknown as Key;
  return [...input].map((ch) => [
    ch,
    {
      ...blank,
      return: ch === "\r" || ch === "\n",
      tab: ch === "\t",
      backspace: ch === "\u007f" || ch === "\b",
    },
  ]);
}

export const ThemeContext = React.createContext<Theme | null>(null);

export function useTheme(): Theme {
  const theme = React.useContext(ThemeContext);
  if (theme === null) throw new Error("no theme");
  return theme;
}

function spanProps(span: Span, theme: Theme, bg: ColourName) {
  const props: { color?: string; backgroundColor: string; bold?: boolean } = {
    backgroundColor: span.rawBg ?? theme.c[span.bg ?? bg],
  };
  const colour = span.rawColor ?? (span.color === undefined ? undefined : theme.c[span.color]);
  if (colour !== undefined) props.color = colour;
  if (span.bold === true) props.bold = true;
  return props;
}

/** One laid-out line, padded to `cols` with its background. */
export function LineView({ line, cols }: { line: Line; cols: number }) {
  const theme = useTheme();
  let used = 0;
  const parts: React.ReactNode[] = [];
  line.spans.forEach((span, i) => {
    const room = cols - used;
    if (room <= 0) return;
    const chars = [...span.text];
    const text = chars.length > room ? chars.slice(0, room).join("") : span.text;
    used += width(text);
    parts.push(
      <Text key={i} {...spanProps(span, theme, line.bg)}>
        {text}
      </Text>,
    );
  });
  if (used < cols) {
    parts.push(
      <Text key="pad" backgroundColor={theme.c[line.bg]}>
        {" ".repeat(cols - used)}
      </Text>,
    );
  }
  return (
    <Text wrap="truncate" backgroundColor={theme.c[line.bg]}>
      {parts}
    </Text>
  );
}

export function Lines({ lines, cols }: { lines: Line[]; cols: number }) {
  return (
    <Box flexDirection="column">
      {lines.map((line, i) => (
        <LineView key={i} line={line} cols={cols} />
      ))}
    </Box>
  );
}

/* A one-line text field. Masked fields show a dot per character and never
 * echo what was typed. Enter submits, Escape cancels. */
export function TextField({
  masked = false,
  initial = "",
  onSubmit,
  onCancel,
  active = true,
}: {
  masked?: boolean;
  initial?: string;
  onSubmit: (value: string) => void;
  onCancel?: () => void;
  active?: boolean;
}) {
  const theme = useTheme();
  const [value, setValueState] = React.useState(initial);
  // The handler reads the value through a ref: a token pasted with its
  // newline arrives as characters and Enter before the next render.
  const valueRef = React.useRef(initial);
  const done = React.useRef(false);
  const setValue = (next: string): void => {
    valueRef.current = next;
    setValueState(next);
  };
  useInput(
    (raw, rawKey) => {
      for (const [input, key] of splitKeys(raw, rawKey)) {
        if (done.current) return;
        if (key.return) {
          done.current = true;
          onSubmit(valueRef.current);
          return;
        }
        if (key.escape) {
          done.current = true;
          onCancel?.();
          return;
        }
        if (key.backspace || key.delete) {
          setValue([...valueRef.current].slice(0, -1).join(""));
          continue;
        }
        if (key.ctrl && input === "u") {
          setValue("");
          continue;
        }
        if (key.ctrl || key.meta || key.tab || key.upArrow || key.downArrow || key.leftArrow || key.rightArrow) continue;
        if (input.length > 0) setValue(valueRef.current + input.replace(/[\r\n]/g, ""));
      }
    },
    { isActive: active },
  );
  const shown = masked ? "•".repeat([...value].length) : value;
  return (
    <Text backgroundColor={theme.c.sunk}>
      <Text color={theme.c.ink} backgroundColor={theme.c.sunk}>
        {` ${shown}`}
      </Text>
      <Text color={theme.c.accent} backgroundColor={theme.c.sunk}>
        {"▏"}
      </Text>
      <Text backgroundColor={theme.c.sunk}>{" ".repeat(Math.max(1, 40 - [...shown].length))}</Text>
    </Text>
  );
}

/** A full-screen block of the ground colour, so the page has the web UI's
 *  background whatever the terminal's is. */
export function Fill({ cols, rows }: { cols: number; rows: number }) {
  const theme = useTheme();
  return (
    <Box flexDirection="column">
      {Array.from({ length: Math.max(0, rows) }, (_, i) => (
        <Text key={i} backgroundColor={theme.c.ground}>
          {" ".repeat(Math.max(0, cols))}
        </Text>
      ))}
    </Box>
  );
}
