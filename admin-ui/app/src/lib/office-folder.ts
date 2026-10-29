/* Whether this box edits a folder of documents (losos.office.folder), which
 * puts Documents in the sidebar. losos-office answers GET /office/api/files
 * with JSON for anyone LosOS Login names; with no folder that address is the
 * page itself, HTML, and the entry stays away. Asked once per sign-in. */

import { useSyncExternalStore } from "react";
import { getMe, subscribeAuth } from "@/lib/api";

export type Folder = "unknown" | "yes" | "no";

let folder: Folder = "unknown";
let askedFor: unknown = undefined;
const listeners = new Set<() => void>();

function set(next: Folder): void {
  if (next === folder) return;
  folder = next;
  for (const l of listeners) l();
}

async function ask(): Promise<void> {
  try {
    const res = await fetch("/office/api/files", { credentials: "same-origin" });
    const json = (res.headers.get("content-type") ?? "").includes("application/json");
    set(res.ok && json ? "yes" : "no");
  } catch {
    set("no");
  }
}

function subscribe(onChange: () => void): () => void {
  listeners.add(onChange);
  const follow = (): void => {
    const me = getMe();
    if (me === askedFor) return;
    askedFor = me;
    if (me === null) set("no");
    else void ask();
  };
  follow();
  const stop = subscribeAuth(follow);
  return () => {
    listeners.delete(onChange);
    stop();
  };
}

/** Whether Documents belongs in the sidebar. */
export function useOfficeFolder(): boolean {
  return useSyncExternalStore(subscribe, () => folder === "yes", () => false);
}

/** The same, with "unknown" until the box has answered. */
export function useOfficeFolderState(): Folder {
  return useSyncExternalStore(subscribe, () => folder, () => "unknown" as Folder);
}
