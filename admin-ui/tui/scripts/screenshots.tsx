/* The PNGs in .github/screenshots/tui/after/: each screen drawn at 120x36
 * over the fixtures, in truecolour, turned from ANSI into HTML and shot in
 * Chromium with a monospace font. Dark and light for each.
 *
 *   npm run screenshots
 *   LOSOS_SHOTS=<dir> npm run screenshots     # somewhere else
 *   LOSOS_CHROMIUM=<path> npm run screenshots # another Chromium
 */

// Ink's colours come from chalk, which reads FORCE_COLOR when it loads; the
// modules that pull it in are imported below, after this line.
process.env["FORCE_COLOR"] = "3";

import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type * as ReactNs from "react";

const COLS = 120;
const ROWS = 36;
const root = fileURLToPath(new URL("..", import.meta.url));
const out = process.env["LOSOS_SHOTS"] ?? join(root, "..", "..", ".github", "screenshots", "tui", "after");
const chromium = process.env["LOSOS_CHROMIUM"] ?? "/opt/pw-browsers/chromium-1194/chrome-linux/chrome";

const { render } = await import("ink-testing-library");
const { chromium: playwright } = await import("playwright-core");
const { setLocale } = await import("../src/lang");
const { makeTheme } = await import("../src/theme");
const { BoxApp, EdgeApp } = await import("../src/tui/app");
const { Login } = await import("../src/tui/login");
const { PALETTES } = await import("../src/generated/palette");
const { boxRoute, edgeRoute } = await import("./../test/fixtures");

type Mode = "dark" | "light";

const BOX = {
  url: "https://mattbox.local",
  kind: "box" as const,
  fingerprint: "3F2A9C1E0B7D44A1C2D3E4F5061728394A5B6C7D",
  publicKey: "",
  token: "",
  keyFingerprint: "8E41D07CB2A95F3361C0E7AD4B19F2C805D6A3E7",
};
const EDGE = { ...BOX, url: "https://register.losos.dasmat.us", kind: "edge" as const, fingerprint: "C07A55E2918B4DF06A3C1B7E92D4F8A0613E5B29" };

function transport(route: typeof boxRoute) {
  return {
    request: async (method: string, path: string, body?: unknown) => ({ ...route(method, path, body ?? null), sealed: true }),
  };
}

const strip = (s: string): string => s.replace(/\x1b\[[0-9;]*m/g, "");
const tick = (ms = 30): Promise<void> => new Promise((r) => setTimeout(r, ms));

interface Shot {
  name: string;
  element: (mode: Mode) => ReactNs.ReactElement;
  ready: string[];
  keys?: string[];
}

const SHOTS: Shot[] = [
  {
    name: "login",
    element: (mode) => (
      <Login
        theme={makeTheme(mode, true)}
        url={BOX.url}
        kind="box"
        fingerprint={BOX.fingerprint}
        keys={[{ fingerprint: BOX.keyFingerprint, userIds: ["Matus <matus@example.org>"] }]}
        cols={COLS}
        onDone={() => {}}
      />
    ),
    ready: ["3F2A 9C1E"],
  },
  ...(["overview", "storage", "apps", "security", "advanced"] as const).map((pane) => ({
    name: pane,
    element: (mode: Mode) => (
      <BoxApp host={BOX} transport={transport(boxRoute)} theme={makeTheme(mode, true)} locale="en" cols={COLS} rows={ROWS} initialPane={pane} />
    ),
    ready: { overview: ["jellyfin"], storage: ["Use reserve"], apps: ["vaultwarden"], security: ["Ignore USB"], advanced: ["losos.tpm.enable"] }[pane],
    // The security shot shows a change waiting in the Apply bar.
    ...(pane === "security" ? { keys: ["j", "\r"] } : {}),
  })),
  {
    name: "edge-overview",
    element: (mode) => (
      <EdgeApp host={EDGE} transport={transport(edgeRoute)} theme={makeTheme(mode, true)} locale="en" cols={COLS} rows={ROWS} initialPane="edge-overview" />
    ),
    ready: ["Relay routes"],
    // Down to the opt-ins, so the tenants table is in view.
    keys: Array.from({ length: 8 }, () => "j"),
  },
  {
    name: "edge-secrets",
    element: (mode) => (
      <EdgeApp host={EDGE} transport={transport(edgeRoute)} theme={makeTheme(mode, true)} locale="en" cols={COLS} rows={ROWS} initialPane="edge-secrets" />
    ),
    ready: ["claude-api-key"],
  },
];

async function frameOf(shot: Shot, mode: Mode): Promise<string> {
  const r = render(shot.element(mode));
  const end = Date.now() + 5000;
  while (!shot.ready.every((w) => strip(r.lastFrame() ?? "").includes(w))) {
    if (Date.now() > end) throw new Error(`${shot.name} never showed ${shot.ready.join(", ")}`);
    await tick();
  }
  for (const key of shot.keys ?? []) {
    r.stdin.write(key);
    await tick(40);
  }
  await tick(150);
  const frame = r.lastFrame() ?? "";
  r.unmount();
  return frame;
}

// ── ANSI to HTML ─────────────────────────────────────────────────────────

const XTERM16 = [
  "#000000", "#cd0000", "#00cd00", "#cdcd00", "#0000ee", "#cd00cd", "#00cdcd", "#e5e5e5",
  "#7f7f7f", "#ff0000", "#00ff00", "#ffff00", "#5c5cff", "#ff00ff", "#00ffff", "#ffffff",
];
function xterm256(n: number): string {
  if (n < 16) return XTERM16[n]!;
  if (n >= 232) {
    const v = 8 + (n - 232) * 10;
    return `rgb(${v},${v},${v})`;
  }
  const i = n - 16;
  const step = (c: number): number => (c === 0 ? 0 : 55 + c * 40);
  return `rgb(${step(Math.floor(i / 36))},${step(Math.floor(i / 6) % 6)},${step(i % 6)})`;
}

const escapeHtml = (s: string): string => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/* Half blocks drawn as two-colour boxes: a font's block glyphs rarely fill
 * the line box exactly, and the salmon would show seams between rows. */
const HALF = new Set(["▀", "▄", "█"]);

function ansiToHtml(text: string, ground: string): string {
  let fg: string | null = null;
  let bg: string | null = null;
  let bold = false;
  let dim = false;
  let html = "";
  let open = false;
  const flush = (chunk: string): void => {
    if (chunk.length === 0) return;
    const style = [fg && `color:${fg}`, bg && `background:${bg}`, bold && "font-weight:700", dim && "opacity:.7"].filter(Boolean).join(";");
    let run = "";
    const emit = (): void => {
      if (run.length === 0) return;
      html += style.length > 0 ? `<span style="${style}">${escapeHtml(run)}</span>` : escapeHtml(run);
      run = "";
    };
    for (const ch of chunk) {
      if (!HALF.has(ch)) {
        run += ch;
        continue;
      }
      emit();
      const ink = fg ?? "currentColor";
      const under = bg ?? ground;
      const [top, bottom] = ch === "▀" ? [ink, under] : ch === "▄" ? [under, ink] : [ink, ink];
      html += `<span class="b" style="background:linear-gradient(${top} 50%,${bottom} 50%)"></span>`;
    }
    emit();
    open = true;
  };
  const parts = text.split(/(\x1b\[[0-9;]*m)/);
  for (const part of parts) {
    const m = /^\x1b\[([0-9;]*)m$/.exec(part);
    if (m === null) {
      flush(part);
      continue;
    }
    const codes = (m[1] === "" ? "0" : m[1]!).split(";").map(Number);
    for (let i = 0; i < codes.length; i += 1) {
      const c = codes[i]!;
      if (c === 0) {
        fg = bg = null;
        bold = dim = false;
      } else if (c === 1) bold = true;
      else if (c === 2) dim = true;
      else if (c === 22) bold = dim = false;
      else if (c === 39) fg = null;
      else if (c === 49) bg = null;
      else if (c >= 30 && c <= 37) fg = XTERM16[c - 30]!;
      else if (c >= 90 && c <= 97) fg = XTERM16[c - 90 + 8]!;
      else if (c >= 40 && c <= 47) bg = XTERM16[c - 40]!;
      else if (c >= 100 && c <= 107) bg = XTERM16[c - 100 + 8]!;
      else if ((c === 38 || c === 48) && codes[i + 1] === 2) {
        const rgb = `rgb(${codes[i + 2]},${codes[i + 3]},${codes[i + 4]})`;
        if (c === 38) fg = rgb;
        else bg = rgb;
        i += 4;
      } else if ((c === 38 || c === 48) && codes[i + 1] === 5) {
        const col = xterm256(codes[i + 2]!);
        if (c === 38) fg = col;
        else bg = col;
        i += 2;
      }
    }
  }
  void open;
  return html;
}

function page(frame: string, mode: Mode): string {
  const ground = PALETTES[mode].ground.hex;
  const ink = PALETTES[mode].ink.hex;
  const lines = frame.split("\n");
  while (lines.length < ROWS) lines.push(`\x1b[48;2;${hexRgb(ground)}m${" ".repeat(COLS)}\x1b[49m`);
  const body = lines
    .slice(0, ROWS)
    .map((l) => {
      const pad = Math.max(0, COLS - [...strip(l)].length);
      return `<div class="l">${ansiToHtml(l, ground)}<span style="background:${ground}">${" ".repeat(pad)}</span></div>`;
    })
    .join("");
  return `<!doctype html><html><head><meta charset="utf-8"><style>
    html,body{margin:0;background:${ground};}
    #t{display:inline-block;padding:14px 16px;background:${ground};color:${ink};
       font-family:"DejaVu Sans Mono",monospace;font-size:15px;font-variant-ligatures:none;}
    .l{white-space:pre;height:18px;line-height:18px;overflow:hidden;}
    .b{display:inline-block;width:1ch;height:18px;vertical-align:top;}
  </style></head><body><div id="t">${body}</div></body></html>`;
}

function hexRgb(hex: string): string {
  const n = Number.parseInt(hex.slice(1), 16);
  return `${(n >> 16) & 255};${(n >> 8) & 255};${n & 255}`;
}

setLocale("en");
mkdirSync(out, { recursive: true });
const browser = await playwright.launch({ executablePath: chromium });
try {
  const tab = await browser.newPage({ viewport: { width: 1400, height: 900 }, deviceScaleFactor: 1 });
  for (const shot of SHOTS) {
    for (const mode of ["dark", "light"] as Mode[]) {
      const frame = await frameOf(shot, mode);
      await tab.setContent(page(frame, mode));
      const file = join(out, `${shot.name}-${mode}.png`);
      await tab.locator("#t").screenshot({ path: file });
      console.log(`screenshots: ${file}`);
    }
  }
} finally {
  await browser.close();
}
