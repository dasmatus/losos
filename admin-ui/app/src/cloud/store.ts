import * as React from "react";
import { CloudError, cloud, joinPath, upload, type Me } from "./api";

/* The web app's shared state, outside React so every view reads the same:
 *
 *   * a revision number that every change bumps, which the views list in
 *     their effects' deps and so fetch again after an upload, a move or a
 *     delete wherever it happened;
 *   * the signed-in person and their storage, fetched once and again on
 *     every bump;
 *   * the upload queue the panel in the corner draws. */

type Listener = () => void;

function store<T>(initial: T) {
  let value = initial;
  const listeners = new Set<Listener>();
  return {
    get: () => value,
    set: (next: T) => {
      value = next;
      for (const l of listeners) l();
    },
    subscribe: (l: Listener) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
  };
}

const revision = store(0);

/** Something changed: every view fetches again. */
export function bump(): void {
  revision.set(revision.get() + 1);
  void loadMe();
}

export function useRevision(): number {
  return React.useSyncExternalStore(revision.subscribe, revision.get);
}

/* Changes made anywhere else: a WebDAV client, the desktop or phone app,
 * Samba, another tab. Every one of them gives the person's root folder a
 * new ETag (the store hands one up to every folder above a change), so
 * one small read of that folder every few seconds says whether anything
 * moved, and a bump has every view fetch again. Before this a file copied
 * in over WebDAV showed up only after a reload. Only while the page is in
 * front, and at once when it comes back. */
const FOLLOW_MS = 3000;

export function useFollowChanges(): void {
  React.useEffect(() => {
    let root: { id: number; etag: string } | null = null;
    let busy = false;
    let live = true;
    const ctl = new AbortController();
    const tick = async (): Promise<void> => {
      if (busy || document.visibilityState !== "visible") return;
      busy = true;
      try {
        const now = root === null ? (await cloud.list("", ctl.signal)).folder : await cloud.byId(root.id, ctl.signal);
        if (!live) return;
        if (root !== null && now.etag !== root.etag) bump();
        root = { id: now.id, etag: now.etag };
      } catch {
        /* the next tick asks again; the views say what is wrong */
      } finally {
        busy = false;
      }
    };
    const timer = window.setInterval(() => void tick(), FOLLOW_MS);
    const onShow = (): void => void tick();
    document.addEventListener("visibilitychange", onShow);
    window.addEventListener("focus", onShow);
    void tick();
    return () => {
      live = false;
      ctl.abort();
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onShow);
      window.removeEventListener("focus", onShow);
    };
  }, []);
}

const me = store<Me | null>(null);

async function loadMe(): Promise<void> {
  try {
    const who = await cloud.me();
    me.set(who);
    // Read by the setup wizard's last step, which shows this page in a
    // frame and moves on once someone is signed in to it.
    document.documentElement.dataset["user"] = who.login;
  } catch {
    // The views say what failed; the bar keeps the last figures.
  }
}

export function useMe(): Me | null {
  React.useEffect(() => {
    if (me.get() === null) void loadMe();
  }, []);
  return React.useSyncExternalStore(me.subscribe, me.get);
}

// ── Uploads ──────────────────────────────────────────────────────────────

export type UploadState = "waiting" | "sending" | "done" | "exists" | "failed" | "cancelled";

export interface Upload {
  id: number;
  name: string;
  /** The folder it goes into. */
  folder: string;
  file: File;
  sent: number;
  total: number;
  state: UploadState;
  error?: string;
  abort?: () => void;
}

const uploads = store<readonly Upload[]>([]);
let nextId = 1;
const PARALLEL = 3;

export function useUploads(): readonly Upload[] {
  return React.useSyncExternalStore(uploads.subscribe, uploads.get);
}

function patch(id: number, change: Partial<Upload>): void {
  uploads.set(uploads.get().map((u) => (u.id === id ? { ...u, ...change } : u)));
}

/** Queue files for a folder. `relative` names a file's place under it, for
 * a dropped folder ("Holiday/day 1.jpg"). */
export function enqueue(folder: string, files: readonly { file: File; relative?: string }[]): void {
  const added: Upload[] = files.map(({ file, relative }) => {
    const rel = relative ?? file.name;
    const cut = rel.lastIndexOf("/");
    return {
      id: nextId++,
      name: cut < 0 ? rel : rel.slice(cut + 1),
      folder: cut < 0 ? folder : joinPath(folder, rel.slice(0, cut)),
      file,
      sent: 0,
      total: file.size,
      state: "waiting",
    };
  });
  uploads.set([...uploads.get(), ...added]);
  pump();
}

/** Folders made for the uploads in flight. Forgotten whenever the queue
 * empties, so a folder deleted or moved since is made again next time. */
const made = new Set<string>();

/** Make the folders a dropped folder needs, parents first. */
async function ensureFolder(path: string): Promise<void> {
  if (path === "" || made.has(path)) return;
  const parts = path.split("/");
  for (let i = 1; i <= parts.length; i++) {
    const p = parts.slice(0, i).join("/");
    if (made.has(p)) continue;
    try {
      await cloud.mkdir(p);
    } catch (err) {
      // 405-style "already there" comes back as exists.
      if (!(err instanceof CloudError && err.code === "exists")) throw err;
    }
    made.add(p);
  }
}

function pump(): void {
  const all = uploads.get();
  const running = all.filter((u) => u.state === "sending").length;
  if (running === 0 && !all.some((u) => u.state === "waiting")) made.clear();
  const ready = all.filter((u) => u.state === "waiting").slice(0, Math.max(0, PARALLEL - running));
  for (const u of ready) void send(u, true);
}

async function send(u: Upload, ifNew: boolean): Promise<void> {
  patch(u.id, { state: "sending", sent: 0, error: undefined });
  try {
    await ensureFolder(u.folder);
    // Cancelled, or cleared from the panel, while the folders were made.
    if (uploads.get().find((x) => x.id === u.id)?.state !== "sending") {
      pump();
      return;
    }
    const job = upload(joinPath(u.folder, u.name), u.file, {
      mtime: u.file.lastModified / 1000,
      ifNew,
      onProgress: (sent, total) => patch(u.id, { sent, total }),
    });
    patch(u.id, { abort: job.abort });
    await job.done;
    patch(u.id, { state: "done", sent: u.total, abort: undefined });
    bump();
  } catch (err) {
    if (err instanceof CloudError && err.code === "aborted") patch(u.id, { state: "cancelled", abort: undefined });
    else if (err instanceof CloudError && (err.code === "exists" || err.status === 412))
      patch(u.id, { state: "exists", abort: undefined });
    else patch(u.id, { state: "failed", error: err instanceof Error ? err.message : String(err), abort: undefined });
  }
  pump();
}

/** The file that was in the way is replaced. */
export function replace(id: number): void {
  const u = uploads.get().find((x) => x.id === id);
  if (u !== undefined) void send(u, false);
}

/** Both are kept: the upload takes the next free "name (2).ext". */
export async function keepBoth(id: number): Promise<void> {
  const u = uploads.get().find((x) => x.id === id);
  if (u === undefined) return;
  const taken = new Set((await cloud.list(u.folder)).entries.map((e) => e.name));
  const dot = u.name.lastIndexOf(".");
  const stem = dot > 0 ? u.name.slice(0, dot) : u.name;
  const ext = dot > 0 ? u.name.slice(dot) : "";
  let n = 2;
  while (taken.has(`${stem} (${n})${ext}`)) n++;
  const renamed = { ...u, name: `${stem} (${n})${ext}` };
  patch(id, { name: renamed.name });
  void send(renamed, true);
}

export function retry(id: number): void {
  const u = uploads.get().find((x) => x.id === id);
  if (u !== undefined) void send(u, true);
}

export function cancel(id: number): void {
  const u = uploads.get().find((x) => x.id === id);
  if (u === undefined) return;
  if (u.abort !== undefined) u.abort();
  else patch(id, { state: "cancelled" });
}

/** Clear what is finished from the panel. */
export function clearFinished(): void {
  uploads.set(uploads.get().filter((u) => u.state === "sending" || u.state === "waiting" || u.state === "exists"));
}
