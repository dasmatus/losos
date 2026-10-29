/* The web app's half of /cloud/api/ (backend/src/cloud/api.rs).
 *
 * nginx signs the person in with LosOS Login before any of these reach the
 * service, so a call carries no token: the browser's session cookie does
 * it. A call that comes back 401 means the session ran out, and the page
 * reloads into the sign-in page. Paths are relative to the person's home,
 * `/`-separated with no leading slash; the home itself is "". */

export interface Entry {
  id: number;
  path: string;
  name: string;
  dir: boolean;
  size: number;
  mtime: number;
  etag: string;
  mime: string;
  checksum?: string | null;
  favorite: boolean;
}

export interface Trashed {
  id: number;
  original: string;
  name: string;
  dir: boolean;
  size: number;
  deleted: number;
}

export interface Version {
  mtime: number;
  size: number;
  file: number;
}

export interface Device {
  id: number;
  name: string;
  created: number;
  lastUsed: number;
}

export interface Me {
  login: string;
  name: string;
  email: string;
  used: number;
  free: number;
  product: string;
}

export interface Flow {
  client: string;
  created: number;
  granted: boolean;
}

export class CloudError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

const BASE = "/cloud/api";

/** A picture with what Photos groups it by (backend/src/cloud/photos/). The
 * camera's fields are there once the box has read the picture. */
export interface Photo extends Pick<Entry, "id" | "path" | "name" | "mime" | "mtime" | "size" | "etag"> {
  /** As the camera's clock read it, `2019-06-08T10:12:31`, no zone. */
  taken?: string;
  camera?: string;
  lens?: string;
  lat?: number;
  lon?: number;
  width?: number;
  height?: number;
  /** The people in it, left to right. */
  people: number[];
}

export interface Person {
  id: number;
  /** Empty until the owner names them. */
  name: string;
  photos: number;
  /** The face that shows them best, for `faceUrl`. */
  face: number;
}

export interface PhotoIndex {
  total: number;
  done: number;
  /** Whether the box looks for faces (`losos.cloud.photos.faces`). */
  faces: boolean;
  last: number | null;
}

export interface PhotoField {
  group: "picture" | "camera" | "place" | "other";
  name: string;
  value: string;
}

export interface PhotoDetails {
  id: number;
  indexed: boolean;
  summary?: {
    taken?: string;
    camera?: string;
    lens?: string;
    exposure?: string;
    aperture?: string;
    iso?: number;
    focal?: string;
    lat?: number;
    lon?: number;
    altitude?: number;
    width: number;
    height: number;
  };
  fields?: PhotoField[];
  faces: { id: number; x: number; y: number; w: number; h: number; person: number | null; name: string }[];
}

function signedOut(): never {
  window.location.reload();
  throw new CloudError(401, "signIn", "signed out");
}

export async function call<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const init: RequestInit = { method, credentials: "same-origin" };
  if (signal !== undefined) init.signal = signal;
  if (body !== undefined) {
    init.headers = { "Content-Type": "application/json" };
    init.body = JSON.stringify(body);
  }
  const res = await fetch(BASE + path, init);
  if (res.status === 401) signedOut();
  const text = await res.text();
  let data: unknown = null;
  try {
    data = text.length > 0 ? JSON.parse(text) : null;
  } catch {
    data = null;
  }
  if (!res.ok) {
    const err = (data ?? {}) as { error?: string; code?: string };
    throw new CloudError(res.status, err.code ?? "internal", err.error ?? res.statusText);
  }
  return data as T;
}

const q = (path: string): string => encodeURIComponent(path);

export const cloud = {
  me: (signal?: AbortSignal) => call<Me>("GET", "/me", undefined, signal),
  list: (path: string, signal?: AbortSignal) =>
    call<{ folder: Entry; entries: Entry[] }>("GET", `/files?path=${q(path)}`, undefined, signal),
  byId: (id: number, signal?: AbortSignal) => call<Entry>("GET", `/files/by-id/${id}`, undefined, signal),
  mkdir: (path: string) => call<Entry>("POST", "/files/folder", { path }),
  move: (from: string, to: string, overwrite = false) =>
    call<Entry>("POST", "/files/move", { from, to, overwrite }),
  copy: (from: string, to: string, overwrite = false) =>
    call<Entry>("POST", "/files/copy", { from, to, overwrite }),
  remove: (paths: string[]) => call<{ trashed: Trashed[] }>("POST", "/files/delete", { paths }),
  favorite: (path: string, favorite: boolean) => call<Entry>("POST", "/files/favorite", { path, favorite }),
  recent: (signal?: AbortSignal) => call<{ entries: Entry[] }>("GET", "/files/recent", undefined, signal),
  favorites: (signal?: AbortSignal) => call<{ entries: Entry[] }>("GET", "/files/favorites", undefined, signal),
  search: (text: string, signal?: AbortSignal) =>
    call<{ entries: Entry[] }>("GET", `/files/search?q=${q(text)}`, undefined, signal),
  media: (signal?: AbortSignal) =>
    call<{ entries: Entry[] }>("GET", "/files/media?kind=image&limit=2000", undefined, signal),
  photos: (signal?: AbortSignal) =>
    call<{ photos: Photo[]; people: Person[]; index: PhotoIndex }>("GET", "/photos", undefined, signal),
  photo: (id: number, signal?: AbortSignal) => call<PhotoDetails>("GET", `/photos/${id}`, undefined, signal),
  namePerson: (id: number, name: string) => call<{ id: number }>("POST", `/photos/people/${id}`, { name }),
  versions: (path: string, signal?: AbortSignal) =>
    call<{ versions: Version[] }>("GET", `/files/versions?path=${q(path)}`, undefined, signal),
  restoreVersion: (id: number, mtime: number) => call<Entry>("POST", "/files/versions/restore", { id, mtime }),
  trash: (signal?: AbortSignal) => call<{ entries: Trashed[]; days: number }>("GET", "/trash", undefined, signal),
  restore: (id: number) => call<Entry>("POST", "/trash/restore", { id }),
  purge: (id: number) => call<unknown>("POST", "/trash/delete", { id }),
  emptyTrash: () => call<{ deleted: number }>("POST", "/trash/empty"),
  devices: (signal?: AbortSignal) => call<{ devices: Device[] }>("GET", "/devices", undefined, signal),
  revokeDevice: (id: number) => call<unknown>("DELETE", `/devices/${id}`),
  flow: (token: string, signal?: AbortSignal) =>
    call<Flow>("GET", `/connect/${encodeURIComponent(token)}`, undefined, signal),
  grant: (token: string) => call<{ granted: boolean }>("POST", `/connect/${encodeURIComponent(token)}`),
  grantV1: (client: string) =>
    call<{ server: string; login: string; appPassword: string }>("POST", "/connect-v1", { client }),
};

export const contentUrl = (path: string, download = false): string =>
  `${BASE}/files/content?path=${q(path)}${download ? "&download=1" : ""}`;

export const versionUrl = (id: number, mtime: number): string => `${BASE}/files/versions/${id}/${mtime}`;

export const previewUrl = (entry: Pick<Entry, "id" | "etag">, size: number): string =>
  `${BASE}/preview/${entry.id}?size=${size}&v=${encodeURIComponent(entry.etag)}`;

export const faceUrl = (face: number): string => `${BASE}/photos/faces/${face}`;

/** The thumbnail kinds the service renders (backend/src/cloud/preview.rs). */
export function hasPreview(entry: Entry): boolean {
  return (
    !entry.dir &&
    ["image/jpeg", "image/png", "image/gif", "image/webp", "image/bmp", "image/tiff"].includes(entry.mime)
  );
}

/**
 * Upload one file with progress. XMLHttpRequest because fetch reports no
 * upload progress, and a phone's video is gigabytes. `ifNew` refuses to
 * replace a file that is already there (409/412 comes back as "exists").
 */
export function upload(
  path: string,
  file: Blob,
  options: { mtime?: number; ifNew?: boolean; onProgress?: (sent: number, total: number) => void } = {},
): { done: Promise<Entry>; abort: () => void } {
  const xhr = new XMLHttpRequest();
  let url = `${BASE}/files/content?path=${q(path)}`;
  if (options.mtime !== undefined) url += `&mtime=${Math.floor(options.mtime)}`;
  if (options.ifNew === true) url += "&ifNew=1";
  const done = new Promise<Entry>((resolve, reject) => {
    xhr.open("PUT", url);
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable) options.onProgress?.(event.loaded, event.total);
    };
    xhr.onload = () => {
      if (xhr.status === 401) signedOut();
      let data: { error?: string; code?: string } & Partial<Entry> = {};
      try {
        data = JSON.parse(xhr.responseText) as typeof data;
      } catch {
        data = {};
      }
      if (xhr.status >= 200 && xhr.status < 300) resolve(data as Entry);
      // Over HTTP/2 there is no status text: the number, then.
      else reject(new CloudError(xhr.status, data.code ?? "internal", data.error ?? (xhr.statusText || `HTTP ${xhr.status}`)));
    };
    xhr.onerror = () => reject(new CloudError(0, "network", "the box did not answer"));
    xhr.onabort = () => reject(new CloudError(0, "aborted", "cancelled"));
    xhr.send(file);
  });
  return { done, abort: () => xhr.abort() };
}

export function joinPath(folder: string, name: string): string {
  return folder === "" ? name : `${folder}/${name}`;
}

export function parentOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut < 0 ? "" : path.slice(0, cut);
}

export function nameOf(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

export function isAbort(err: unknown): boolean {
  return err instanceof DOMException && err.name === "AbortError";
}
