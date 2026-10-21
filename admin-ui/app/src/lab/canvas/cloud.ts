/* Diagrams in LosOS Cloud: whether this box runs it (a signed-in GET
 * /cloud/api/me), a folder's diagrams and folders, reading one, and saving
 * one back. A save names the version it read (`ifMatch`), so two tabs that
 * edit one file do not overwrite each other in silence: the second gets
 * "changed elsewhere" and chooses. The API is LosOS Cloud's own
 * (backend/src/cloud/api.rs, src/cloud/api.ts); this page has its own small
 * client because the cloud app's reloads the page on a 401, which would
 * throw away an unsaved diagram. */

import { BOX_COPY } from "../shape";

export interface CloudEntry {
  path: string;
  name: string;
  dir: boolean;
  size: number;
  mtime: number;
  etag: string;
}

export class CloudFail extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
  ) {
    super(code);
  }
}

const BASE = "/cloud/api";
const q = encodeURIComponent;

async function call<T>(path: string, init: RequestInit = {}): Promise<T> {
  let res: Response;
  try {
    res = await fetch(BASE + path, { credentials: "same-origin", cache: "no-store", ...init });
  } catch {
    throw new CloudFail(0, "network");
  }
  const text = await res.text();
  let data: unknown = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = null;
  }
  if (!res.ok) {
    const code = (data as { code?: string } | null)?.code;
    throw new CloudFail(res.status, res.status === 401 ? "signIn" : (code ?? "internal"));
  }
  return data as T;
}

/** Whether LosOS Cloud answers for the person on this page. */
export async function probeCloud(): Promise<boolean> {
  if (!BOX_COPY) return false;
  try {
    const r = await fetch(BASE + "/me", { credentials: "same-origin", cache: "no-store" });
    if (!r.ok) return false;
    const me = (await r.json()) as { login?: unknown };
    return typeof me.login === "string";
  } catch {
    return false;
  }
}

export const DIAGRAM = /\.canvas$/i;

export async function listFolder(path: string): Promise<CloudEntry[]> {
  const r = await call<{ entries: CloudEntry[] }>(`/files?path=${q(path)}`);
  return r.entries
    .filter((e) => e.dir || DIAGRAM.test(e.name))
    .sort((a, b) => (a.dir === b.dir ? a.name.localeCompare(b.name) : a.dir ? -1 : 1));
}

export async function readFile(path: string): Promise<{ text: string; etag: string }> {
  let res: Response;
  try {
    res = await fetch(`${BASE}/files/content?path=${q(path)}`, { credentials: "same-origin", cache: "no-store" });
  } catch {
    throw new CloudFail(0, "network");
  }
  if (!res.ok) throw new CloudFail(res.status, res.status === 401 ? "signIn" : res.status === 404 ? "notFound" : "internal");
  const text = await res.text();
  let etag = (res.headers.get("ETag") ?? "").replace(/^W\//, "").replace(/"/g, "");
  if (!etag) etag = (await listFolder(parentOf(path))).find((e) => e.path === path)?.etag ?? "";
  return { text, etag };
}

/** Writes the diagram. `etag`: the version this tab read (refused when the
 *  file changed since), or null for a new file (refused when one is there). */
export async function writeFile(path: string, text: string, etag: string | null): Promise<CloudEntry> {
  const guard = etag ? `&ifMatch=${q(etag)}` : "&ifNew=1";
  return call<CloudEntry>(`/files/content?path=${q(path)}${guard}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: text,
  });
}

export function parentOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut < 0 ? "" : path.slice(0, cut);
}

export function joinPath(folder: string, name: string): string {
  return folder ? `${folder}/${name}` : name;
}
