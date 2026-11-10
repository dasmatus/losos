/* Field reads in the pages' spirit (admin-ui/app/src/lib/api.ts): a field
 * that is missing reads as absent, never as a failed card. Names follow the
 * wire, camelCase, so every route's types line up with the SPA's
 * (admin-ui/app/src/lib/api.ts, whose shapes are transcribed from
 * backend/schema.json). The Compose port disclaimed more than the document
 * guarantees: no shape is checked, a nonconforming value is a dash.
 */

import type { JsonBody } from "./api";

/** A String slot, absent/blank strained out. */
export function maybeString(body: JsonBody, name: string): string | null {
  const v = body[name];
  if (typeof v !== "string") return null;
  const trimmed = v.trim();
  return trimmed.length > 0 ? trimmed : null;
}

/** A numeric slot as a whole count (times, event counts). */
export function maybeNumber(body: JsonBody, name: string): number | null {
  const v = body[name];
  return typeof v === "number" && Number.isFinite(v) ? Math.trunc(v) : null;
}

/** A numeric slot as read; null when the field did not answer. */
export function maybeDouble(body: JsonBody, name: string): number | null {
  const v = body[name];
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

/** A boolean slot, defaulting false where the port did (maybeBool). */
export function maybeBool(body: JsonBody, name: string, dflt = false): boolean {
  const v = body[name];
  return typeof v === "boolean" ? v : dflt;
}

export function maybeObject(body: JsonBody, name: string): JsonBody | null {
  const v = body[name];
  return v !== null && typeof v === "object" && !Array.isArray(v)
    ? (v as JsonBody)
    : null;
}

export function itemsOf(body: JsonBody, name: string): JsonBody[] {
  const v = body[name];
  if (!Array.isArray(v)) return [];
  return v.filter(
    (item): item is JsonBody => item !== null && typeof item === "object" && !Array.isArray(item),
  );
}

/** "%" escaped, ApiClient.kt's URLEncoder: form encoding with + for space -
 *  the same shape a query string reads back with (used on /api/apps/search
 *  and /api/apps/chart). */
export function encodeQuery(value: string): string {
  return encodeURIComponent(value);
}

/* The shapes a pane shows, formatted the terminal client's way (tui i18n's
 * format_bytes / format_iso / format_unix). */

/** 1073741824.0 and up the ladder, dashes when the field did not answer. */
export function formatBytes(bytes: number | null): string {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return "–";
  const gib = bytes / 1073741824.0;
  if (gib >= 1024.0) return `${(gib / 1024.0).toFixed(1)} TB`;
  if (gib >= 10.0) return `${Math.round(gib)} GB`;
  if (gib >= 1.0) return `${gib.toFixed(1)} GB`;
  return `${Math.round(gib * 1024.0)} MB`;
}

/** ISO 8601 down to the minute: 2026-01-02T10:33:22Z -> 2026-01-02 10:33. */
export function formatStamp(iso: string | null): string {
  return iso === null ? "–" : iso.slice(0, 16).replace("T", " ") || "–";
}

/** A unix timestamp in the phone's own zone; dashes when unset. */
export function formatUnix(seconds: number | null): string {
  if (seconds === null) return "–";
  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) return "–";
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}`
  );
}

export function shortSha(sha: string | null): string {
  return sha === null ? "–" : sha.slice(0, 7) || "–";
}

/* The compute window, in words (admin-ui/app/src/screens/settings/window.ts
 * as the port carried it). */

function minutesOf(hhmm: string): number | null {
  const match = /^(\d{2}):(\d{2})$/.exec(hhmm);
  if (match === null) return null;
  const h = Number(match[1]);
  const m = Number(match[2]);
  if (h > 23 || m > 59) return null;
  return h * 60 + m;
}

export function describeWindow(start: string, end: string): string {
  const s = minutesOf(start);
  if (s === null) return "Not set";
  const e = minutesOf(end);
  if (e === null) return "Not set";
  if (s === e) return "Set, to nothing";
  const total = s < e ? e - s : 1440 - s + e;
  const length = total % 60 === 0 ? `${total / 60} h` : `${total} min`;
  const shape = s > e ? ", wrapping midnight" : "";
  return `${start}–${end}, ${length} a day${shape}`;
}
