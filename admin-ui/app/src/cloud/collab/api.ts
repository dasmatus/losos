/* Editing together and sharing with people on other boxes: the web app's
 * half of /cloud/api/collab and /cloud/api/mesh (backend/src/cloud/collab/,
 * backend/src/cloud/mesh/). */

import { call } from "../api";

export type Perm = "read" | "edit";

/** What the editor is editing, from GET /cloud/api/collab/doc. */
export interface DocMeta {
  name: string;
  /** The cloud login or, for a file from another box, the owner's name. */
  owner: string;
  /** `matus@mattbox.local`, for a file from another box. */
  ownerAddress?: string;
  /** The box that holds the file. */
  box: string;
  perm: Perm | "none";
  /** The file lives on another box and is edited here as a copy. */
  remote: boolean;
  /** For a file from another box: whether that box is linked now. */
  linked?: boolean;
  me: { name: string; address: string };
}

/** A share sent from this box. */
export interface Outgoing {
  id: string;
  fileId: number;
  name: string;
  folder: boolean;
  /** `eva@kitchenbox.local`. */
  recipient: string;
  remote: string;
  perm: Perm;
  state: "unsent" | "pending" | "accepted" | "declined";
  created: number;
}

/** A share another box sent to the person here. */
export interface Incoming {
  id: string;
  remote: string;
  ownerRemote: string;
  ownerName: string;
  name: string;
  /** The file or folder's id on the owner's box. */
  fileId: number;
  folder: boolean;
  size: number;
  perm: Perm;
  state: "pending" | "accepted" | "declined";
  created: number;
  /** Edited here while the other box was out of reach. */
  unsynced: boolean;
}

/** A file or folder inside a shared folder, read from the other box. */
export interface RemoteEntry {
  id: number;
  name: string;
  folder: boolean;
  size: number;
  etag: string;
  parent: number | null;
}

/** Where a file is edited together. */
export function editHref(file: number, share?: string): string {
  return share === undefined ? `/cloud/edit/${file}` : `/cloud/edit/${file}?share=${encodeURIComponent(share)}`;
}

const which = (file: number, share?: string): string =>
  share === undefined ? `file=${file}` : `share=${encodeURIComponent(share)}&file=${file}`;

export const collab = {
  doc: (file: number, share?: string, signal?: AbortSignal) =>
    call<DocMeta>("GET", `/collab/doc?${which(file, share)}`, undefined, signal),
  /** The y-sync WebSocket's address, without the room. */
  server: (): string => `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}/cloud/api/collab`,
  params: (file: number, share?: string): Record<string, string> =>
    share === undefined ? { file: String(file) } : { file: String(file), share },
};

export const mesh = {
  box: (signal?: AbortSignal) => call<{ name: string; key: string; me: string }>("GET", "/mesh/box", undefined, signal),
  shares: (fileId: number, signal?: AbortSignal) =>
    call<Outgoing[]>("GET", `/mesh/shares?fileId=${fileId}`, undefined, signal),
  share: (fileId: number, to: string, perm: Perm) => call<Outgoing>("POST", "/mesh/shares", { fileId, to, perm }),
  setPerm: (id: string, perm: Perm) => call<null>("PATCH", `/mesh/shares/${encodeURIComponent(id)}`, { perm }),
  unshare: (id: string) => call<null>("DELETE", `/mesh/shares/${encodeURIComponent(id)}`),
  incoming: (signal?: AbortSignal) => call<Incoming[]>("GET", "/mesh/incoming", undefined, signal),
  accept: (id: string) => call<Incoming>("POST", `/mesh/incoming/${encodeURIComponent(id)}/accept`),
  decline: (id: string) => call<Incoming>("POST", `/mesh/incoming/${encodeURIComponent(id)}/decline`),
  list: (id: string, folder?: number, signal?: AbortSignal) =>
    call<RemoteEntry[]>(
      "GET",
      `/mesh/incoming/${encodeURIComponent(id)}/list${folder === undefined ? "" : `?folder=${folder}`}`,
      undefined,
      signal,
    ),
  fileUrl: (id: string, file: number): string =>
    `/cloud/api/mesh/incoming/${encodeURIComponent(id)}/files/${file}`,
};

/** Text the editor opens: plain text, Markdown and notes. Office documents
 * go to LosOS Office instead, on the box that holds them. */
export function editable(name: string, mime?: string): boolean {
  if (mime !== undefined && mime.startsWith("text/")) return true;
  return /\.(md|markdown|txt|text|note|org|csv|tsv|log|ya?ml|toml|json|ini|conf)$/i.test(name);
}
