/* Notes and knowledge bases: the web app's half of /cloud/api/notes and
 * /cloud/api/kb (backend/src/cloud/notes/). A note's text is a file like
 * any other, read and written through the files API. */

import { call, CloudError, contentUrl, joinPath } from "../api";
import type { Hit, Source, VaultIndex } from "@/notes/vault";

export type Audience = "anyone" | "box";

export interface Site {
  id: number;
  folderId: number;
  /** The folder now; null once it was deleted. */
  folder: string | null;
  slug: string;
  title: string;
  audience: Audience;
  created: number;
}

const q = encodeURIComponent;

export const notes = {
  vault: (root: string, signal?: AbortSignal) => call<VaultIndex>("GET", `/notes/vault?path=${q(root)}`, undefined, signal),
  search: (root: string, text: string, signal?: AbortSignal) =>
    call<{ hits: Hit[] }>("GET", `/notes/search?path=${q(root)}&q=${q(text)}`, undefined, signal),
  /** The folder holding .obsidian at or above `path`, or null. */
  root: (path: string, signal?: AbortSignal) => call<{ root: string | null }>("GET", `/notes/root?path=${q(path)}`, undefined, signal),
};

export const kb = {
  list: (signal?: AbortSignal) => call<{ available: boolean; sites: Site[] }>("GET", "/kb", undefined, signal),
  create: (path: string, title: string, audience: Audience) => call<Site>("POST", "/kb", { path, title, audience }),
  update: (id: number, change: { title?: string; audience?: Audience }) => call<Site>("PATCH", `/kb/${id}`, change),
  remove: (id: number) => call<unknown>("DELETE", `/kb/${id}`),
};

/** A site's address on this box. */
export function siteUrl(slug: string): string {
  return `${window.location.origin}/kb/${slug}/`;
}

/** Read a note's text, uncached. */
export async function readText(path: string, signal?: AbortSignal): Promise<{ text: string; etag: string }> {
  const init: RequestInit = { credentials: "same-origin", cache: "no-store" };
  if (signal !== undefined) init.signal = signal;
  const res = await fetch(contentUrl(path), init);
  if (res.status === 401) {
    window.location.reload();
    throw new CloudError(401, "signIn", "signed out");
  }
  if (!res.ok) throw new CloudError(res.status, res.status === 404 ? "notFound" : "internal", res.statusText);
  return { text: await res.text(), etag: (res.headers.get("etag") ?? "").replace(/^W\//, "").replace(/"/g, "") };
}

/** Write a note. `etag` refuses to overwrite a newer version; `create`
 * refuses to overwrite anything. */
export async function writeText(path: string, text: string, options: { etag?: string; create?: boolean } = {}): Promise<void> {
  let url = contentUrl(path);
  if (options.etag !== undefined && options.etag !== "") url += `&ifMatch=${q(options.etag)}`;
  if (options.create === true) url += "&ifNew=1";
  const res = await fetch(url, {
    method: "PUT",
    credentials: "same-origin",
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
    body: text,
  });
  if (res.status === 401) {
    window.location.reload();
    throw new CloudError(401, "signIn", "signed out");
  }
  if (!res.ok) {
    let code = "internal";
    try {
      code = ((await res.json()) as { code?: string }).code ?? code;
    } catch {
      /* not JSON */
    }
    throw new CloudError(res.status, code, res.statusText);
  }
}

/** Where a note opens in the web app. */
export function noteHref(root: string, path: string, sub = ""): string {
  const params = new URLSearchParams({ vault: root, note: path });
  return `/notes?${params.toString()}${sub === "" ? "" : `#${encodeURIComponent(sub)}`}`;
}

/** The vault's text and files, from the person's own cloud. */
export function cloudSource(root: string): Source {
  return {
    text: async (path, signal) => (await readText(joinPath(root, path), signal)).text,
    fileUrl: (path) => contentUrl(joinPath(root, path)),
    href: (path, sub) => `/cloud${noteHref(root, path, sub)}`,
    search: async (text, signal) => (await notes.search(root, text, signal)).hits,
  };
}
