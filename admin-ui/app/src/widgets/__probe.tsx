/* Throwaway harness — NOT part of the app. Deleted before this lands.
 *
 * Mounts the board with a seeded journal so the heatmap, the bar, a custom
 * spec widget and the failure path can all be looked at in a real browser
 * under the production CSP. Errors and CSP violations are written into the
 * DOM so `--dump-dom` picks them up.
 */

import { createRoot } from "react-dom/client";
import { StrictMode } from "react";
import { Toaster } from "@/components/ui/toast";
import { STORAGE_KEY as BOARD_KEY } from "@/lib/widgets";
import { WidgetBoard } from "./board";
import "@/styles/index.css";

const log: string[] = [];

function note(text: string): void {
  log.push(text);
  const element = document.getElementById("probe-log");
  if (element !== null) element.textContent = log.join("\n");
}

window.addEventListener("error", (event) => note(`ERROR ${event.message}`));
window.addEventListener("unhandledrejection", (event) => note(`REJECT ${String(event.reason)}`));
document.addEventListener("securitypolicyviolation", (event) => {
  note(`CSP ${event.violatedDirective} ${event.blockedURI} ${event.sourceFile ?? ""}`);
});

// ── Seed ──────────────────────────────────────────────────────────────────

function key(date: Date): string {
  const m = `${date.getMonth() + 1}`.padStart(2, "0");
  const d = `${date.getDate()}`.padStart(2, "0");
  return `${date.getFullYear()}-${m}-${d}`;
}

const days: Record<string, { u: number; d: number; o: number; l: number }> = {};
const today = new Date();
for (let offset = 0; offset < 300; offset += 1) {
  const date = new Date(today.getFullYear(), today.getMonth(), today.getDate() - offset);
  // A gap: nothing watched between 120 and 150 days ago.
  if (offset > 120 && offset < 150) continue;
  if (offset === 40 || offset === 41) {
    days[key(date)] = { u: 0, d: 24, o: offset === 41 ? 1 : 0, l: offset === 40 ? 21600 : 0 };
    continue;
  }
  const down = offset % 37 === 0 ? 3 : offset % 11 === 0 ? 1 : 0;
  days[key(date)] = { u: 24 - down, d: down, o: down > 0 ? 1 : 0, l: down * 3600 };
}

window.localStorage.setItem(
  "losos-uptime-journal",
  JSON.stringify({ v: 1, days, lastUp: true, outageStart: null }),
);
window.localStorage.setItem(
  "losos-storage-observation",
  JSON.stringify({
    at: Date.now(),
    grew: false,
    beforeBytes: 412_316_860_416,
    afterBytes: 412_316_860_416,
    claimedBytes: 107_374_182_400,
  }),
);
window.localStorage.setItem(
  "losos-storage-used",
  JSON.stringify({ at: Date.now(), used: 351_843_720_888, total: 412_316_860_416 }),
);

window.localStorage.setItem(
  BOARD_KEY,
  JSON.stringify({
    version: 1,
    widgets: [
      { id: "a", source: { kind: "builtin", id: "uptime" } },
      { id: "b", source: { kind: "builtin", id: "disk" } },
      {
        id: "c",
        source: {
          kind: "custom",
          spec: {
            version: 1,
            title: "Days watched",
            type: "number",
            metric: "uptime.days",
            value: "m.observedDays",
            format: "number",
            caption: "days this browser has a reading for",
            foot: "A custom widget, compiled from an expression.",
          },
        },
      },
      {
        id: "d",
        source: {
          kind: "custom",
          spec: {
            version: 1,
            title: "Needs the box",
            type: "number",
            metric: "box.status",
            value: "m.progress",
            format: "percent",
          },
        },
      },
      { id: "e", source: { kind: "builtin", id: "apps" } },
    ],
  }),
);

// ── Mount ─────────────────────────────────────────────────────────────────

const host = document.getElementById("root");
if (host !== null) {
  createRoot(host).render(
    <StrictMode>
      <div className="mx-auto max-w-5xl bg-ground p-6">
        <WidgetBoard />
        <Toaster />
      </div>
    </StrictMode>,
  );
}

note("mounted");
