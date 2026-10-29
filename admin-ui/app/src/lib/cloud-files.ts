/* LosOS Cloud's files, for every other app on this box.
 *
 * Mail, Chat, Diagrams and the rest live on the same origin as LosOS Cloud,
 * and nginx signs the person in with LosOS Login before /cloud/api/ is
 * reached, so another page reads and writes the person's own files with
 * the same session cookie Cloud's own page uses. Nothing here sees more
 * than the person could open in Cloud themselves.
 *
 * This is the one place another app saves into Cloud or reads out of it,
 * so a file that arrives by mail or in chat lands where everything else is
 * and opens in the app that edits it (`openFileHref`, Cloud's /open route
 * picks the opener). */

import { cloud, CloudError, contentUrl, joinPath, nameOf, upload, type Entry } from "@/cloud/api";

export type { Entry as CloudEntry } from "@/cloud/api";

let reachable: Promise<boolean> | null = null;

/** Whether this box runs LosOS Cloud for the person signed in. Asked once
 *  per page; a box without it hides every "LosOS Cloud" button. */
export function cloudAvailable(): Promise<boolean> {
  reachable ??= fetch("/cloud/api/me", { credentials: "same-origin" }).then(
    (res) => res.ok,
    () => false,
  );
  return reachable;
}

/** The folders other apps save into, by app. Plain names in the person's
 *  home, so they are where a person looks for them. */
export const INBOX_FOLDERS = { mail: "Mail", chat: "Chat" } as const;

/** "report.docx" -> "report (2).docx". */
export function numbered(name: string, n: number): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? `${name.slice(0, dot)} (${n})${name.slice(dot)}` : `${name} (${n})`;
}

/** A file name Cloud will take: no slashes, no control characters, not
 *  empty, not "." or "..". */
export function safeName(name: string): string {
  // eslint-disable-next-line no-control-regex
  const cleaned = name.replace(/[/\\\u0000-\u001f]/g, "_").trim();
  return cleaned === "" || cleaned === "." || cleaned === ".." ? "file" : cleaned.slice(0, 200);
}

async function ensureFolder(folder: string): Promise<void> {
  if (folder === "") return;
  let at = "";
  for (const part of folder.split("/")) {
    at = joinPath(at, part);
    try {
      await cloud.mkdir(at);
    } catch (err) {
      // There already: that is the point.
      if (!(err instanceof CloudError) || (err.status !== 409 && err.code !== "exists")) throw err;
    }
  }
}

/** Save `blob` as `name` in `folder` of the person's Cloud and give back
 *  what Cloud made. The folder is made when missing; a name already taken
 *  gets a number, so nothing the person had is replaced. */
export async function saveToCloud(blob: Blob, where: { folder: string; name: string }): Promise<Entry> {
  await ensureFolder(where.folder);
  const base = safeName(where.name);
  for (let n = 1; n < 100; n++) {
    const name = n === 1 ? base : numbered(base, n);
    try {
      return await upload(joinPath(where.folder, name), blob, { ifNew: true }).done;
    } catch (err) {
      if (err instanceof CloudError && (err.status === 409 || err.status === 412 || err.code === "exists")) continue;
      throw err;
    }
  }
  throw new CloudError(409, "exists", "too many files with that name");
}

/** A file's bytes, typed by the name for the apps that care (a mail
 *  attachment, a chat upload). */
export async function readCloudFile(path: string, signal?: AbortSignal): Promise<File> {
  const init: RequestInit = { credentials: "same-origin" };
  if (signal !== undefined) init.signal = signal;
  const res = await fetch(contentUrl(path), init);
  if (!res.ok) throw new CloudError(res.status, res.status === 404 ? "notFound" : "internal", res.statusText);
  const blob = await res.blob();
  return new File([blob], nameOf(path), { type: blob.type || "application/octet-stream" });
}

/** A folder's entries, for the picker. */
export function listCloudFolder(path: string, signal?: AbortSignal): Promise<{ folder: Entry; entries: Entry[] }> {
  return cloud.list(path, signal);
}

/** A file's text, at most `max` characters, for the AI. Binary files and
 *  ones too big to be worth reading give null. */
export async function readCloudText(entry: Entry, max: number, signal?: AbortSignal): Promise<string | null> {
  if (entry.dir || entry.size > 4 * 1024 * 1024 || !textual(entry.name, entry.mime)) return null;
  const file = await readCloudFile(entry.path, signal);
  const text = await file.text();
  return text.slice(0, max);
}

/** Whether a file is text a person could read in a plain editor. */
export function textual(name: string, mime: string): boolean {
  if (mime.startsWith("text/") || /json|xml|yaml|toml|javascript|x-sh/.test(mime)) return true;
  return /\.(note|md|markdown|txt|csv|tsv|json|ya?ml|toml|ini|conf|log|canvas|html?|css|js|jsx|ts|tsx|rs|py|go|nix|sh|c|h|cpp|java|kt|rb|php|sql|tex|org|rst)$/i.test(
    name,
  );
}
