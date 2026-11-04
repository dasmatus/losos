/* A document to Word, OpenDocument, PDF or a web page, a presentation to
 * PowerPoint or a web page, and a Word or OpenDocument file to a document.
 *
 * The box does the work (backend/plugins/office, then Pandoc in a container
 * with no network, modules/office-convert.nix). The page sends the
 * document's text and the pictures it shows, read here from LosOS Cloud,
 * and saves what comes back. An imported file becomes a `.paper` beside the
 * original, which stays as it was, with its pictures in a folder next to
 * it. */

import {
  CloudError,
  cloud,
  contentUrl,
  joinPath,
  nameOf,
  parentOf,
  upload,
  type Entry,
} from "../../api";
import type { Kind } from "./new";

export type ExportFormat = "docx" | "odt" | "pdf" | "html" | "pptx";

/** What each kind of file can be downloaded as, in the menu's order. */
export const EXPORTS: Record<Kind, ExportFormat[]> = {
  paper: ["docx", "odt", "pdf", "html"],
  slides: ["pptx", "html"],
};

/** The Word and OpenDocument files that open as a document. */
export function importsAs(name: string): "docx" | "odt" | null {
  const m = /\.(docx|odt)$/i.exec(name);
  return m === null ? null : (m[1]!.toLowerCase() as "docx" | "odt");
}

const API = "/api/office";

export interface Formats {
  exports: ExportFormat[];
  imports: string[];
}

let formats: Promise<Formats> | null = null;

/** What this box converts. A box whose converter is off answers 503, and
 * then neither Download nor opening a Word file is offered. Asked once per
 * page. */
export function officeFormats(): Promise<Formats> {
  formats ??= fetch(`${API}/formats`, { credentials: "same-origin" }).then(
    async (res) => {
      if (!res.ok) throw await failed(res);
      return (await res.json()) as Formats;
    },
  );
  return formats;
}

export class ConvertError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function failed(res: Response): Promise<ConvertError> {
  let message = res.statusText;
  try {
    const body = (await res.json()) as { error?: string };
    if (typeof body.error === "string") message = body.error;
  } catch {
    /* not JSON: nginx's own page */
  }
  return new ConvertError(res.status, message);
}

/** Pictures are named as the document writes them; another site's never
 * leaves the box's pages. */
function local(src: string): boolean {
  return (
    src !== "" &&
    !/^[a-z][a-z0-9+.-]*:/i.test(src) &&
    !src.startsWith("//") &&
    !src.startsWith("/")
  );
}

function resolve(folder: string, src: string): string {
  const out: string[] = [];
  for (const p of joinPath(folder, src.replace(/^\.\//, "")).split("/")) {
    if (p === "..") out.pop();
    else if (p !== "." && p !== "") out.push(p);
  }
  return out.join("/");
}

function base64(bytes: ArrayBuffer): string {
  const view = new Uint8Array(bytes);
  let text = "";
  for (let i = 0; i < view.length; i += 0x8000)
    text += String.fromCharCode(...view.subarray(i, i + 0x8000));
  return btoa(text);
}

/** The file's bytes, from the box, ready to save under `name`. */
export async function exportDocument(
  source: string,
  pictures: readonly string[],
  folder: string,
  format: ExportFormat,
  lang: string,
  signal?: AbortSignal,
): Promise<Blob> {
  const carried: Record<string, string> = {};
  for (const src of new Set(pictures)) {
    if (!local(src)) continue;
    const res = await fetch(contentUrl(resolve(folder, src)), {
      credentials: "same-origin",
      signal,
    });
    // A picture that is gone leaves an empty frame, as on the page.
    if (res.ok) carried[src] = base64(await res.arrayBuffer());
  }
  const res = await fetch(`${API}/export/${format}`, {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ source, lang, pictures: carried }),
    signal,
  });
  if (!res.ok) throw await failed(res);
  return await res.blob();
}

/** Hands the browser a file to save. */
export function save(blob: Blob, name: string): void {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 60_000);
}

/** A folder name the converter takes: letters, digits, `-`, `_` and `.`. */
export function picturesFolder(stem: string): string {
  const plain = stem
    .normalize("NFKD")
    .replace(/[^\p{L}\p{N}._-]+/gu, "-")
    .replace(/-+/g, "-")
    .replace(/^[-.]+|-+$/g, "")
    .slice(0, 100);
  return `${plain === "" ? "document" : plain}-files`;
}

async function uploadFresh(
  folder: string,
  name: string,
  body: Blob,
): Promise<Entry> {
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  const ext = dot > 0 ? name.slice(dot) : "";
  for (let n = 1; n < 100; n++) {
    const candidate = n === 1 ? name : `${stem} ${n}${ext}`;
    try {
      return await upload(joinPath(folder, candidate), body, { ifNew: true })
        .done;
    } catch (err) {
      if (err instanceof CloudError && err.code === "exists") continue;
      throw err;
    }
  }
  throw new CloudError(409, "exists", "no free name");
}

/** The first of NAME, NAME-2, … that is not in FOLDER yet, so a second
 * import never mixes its pictures with the first one's. */
async function freeFolder(
  folder: string,
  name: string,
  signal?: AbortSignal,
): Promise<string> {
  for (let n = 1; n < 100; n++) {
    const candidate = n === 1 ? name : `${name}-${n}`;
    try {
      await cloud.list(joinPath(folder, candidate), signal);
    } catch (err) {
      if (err instanceof CloudError && err.status === 404) return candidate;
      if (err instanceof CloudError && err.status === 409) continue;
      throw err;
    }
  }
  throw new CloudError(409, "exists", "no free folder name");
}

/** Makes a `.paper` from a Word or OpenDocument file, beside it, and
 * answers with the new file. */
export async function importDocument(
  entry: Entry,
  signal?: AbortSignal,
): Promise<Entry> {
  const format = importsAs(entry.name);
  if (format === null)
    throw new ConvertError(
      415,
      `${entry.name} is not a Word or OpenDocument file`,
    );
  const folder = parentOf(entry.path);
  const stem = nameOf(entry.path).replace(/\.(docx|odt)$/i, "");
  const pictures = await freeFolder(folder, picturesFolder(stem), signal);
  const file = await fetch(contentUrl(entry.path), {
    credentials: "same-origin",
    signal,
  });
  if (!file.ok) throw new ConvertError(file.status, file.statusText);
  const res = await fetch(
    `${API}/import/${format}?folder=${encodeURIComponent(pictures)}`,
    {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/octet-stream" },
      body: await file.blob(),
      signal,
    },
  );
  if (!res.ok) throw await failed(res);
  const got = (await res.json()) as {
    source: string;
    pictures: Record<string, string>;
  };
  const made = new Set<string>();
  for (const [name, data] of Object.entries(got.pictures)) {
    // Uploads make no folders: each one on the way is made first.
    const parts = name.split("/").slice(0, -1);
    for (let i = 1; i <= parts.length; i++) {
      const dir = joinPath(folder, parts.slice(0, i).join("/"));
      if (made.has(dir)) continue;
      await cloud.mkdir(dir);
      made.add(dir);
    }
    const bytes = Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
    await upload(joinPath(folder, name), new Blob([bytes]), { ifNew: true })
      .done;
  }
  return await uploadFresh(
    folder,
    `${stem}.paper`,
    new Blob([got.source], { type: "text/plain" }),
  );
}

/** The document made from a Word or OpenDocument file: the `.paper` beside
 * it with its name, when that is newer than the file, or else a new one. */
export async function openAsDocument(
  entry: Entry,
  signal?: AbortSignal,
): Promise<Entry> {
  const folder = parentOf(entry.path);
  const made =
    `${nameOf(entry.path).replace(/\.(docx|odt)$/i, "")}.paper`.toLowerCase();
  const { entries } = await cloud.list(folder, signal);
  const earlier = entries.find(
    (e) => !e.dir && e.name.toLowerCase() === made && e.mtime >= entry.mtime,
  );
  return earlier ?? (await importDocument(entry, signal));
}
