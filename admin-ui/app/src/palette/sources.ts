import * as React from "react";
import { isAbort, isUnauthorized } from "@/lib/api";
import { searchCatalogue, type CatalogueApp } from "@/screens/settings/catalogue";
import { authedGet, isRecord, nonEmptyString, notPresent } from "@/screens/settings/http";

/* What the palette asks the box for while the owner types: files in LosOS
 * cloud, repositories in LosOS Git, text inside the files of both, and apps
 * to install.
 *
 * Every one of them goes through lososd. The page is served under
 * `connect-src 'self'`, and the two apps' own APIs want credentials this
 * page does not hold, so the box looks things up and the palette lists
 * what it found. A route the box does not serve (404) hides its group for
 * the rest of the visit rather than painting an error under every key. */

export interface CloudFile {
  name: string;
  /** Where it sits in LosOS Cloud, from the owner's root. */
  path: string;
  /** LosOS Cloud's file id: /cloud/files?fileid=<id> opens it. */
  id: number;
  folder: boolean;
}

export interface GitRepo {
  owner: string;
  name: string;
  private: boolean;
  description: string | null;
}

/** A file in LosOS Cloud whose text matched. */
export interface CloudText {
  /** From the owner's root, without a leading slash. */
  path: string;
  /** Alternating runs: even indices plain, odd ones what matched. */
  snippet: readonly string[];
}

/** A line on a repository's default branch that matched. */
export interface GitText {
  owner: string;
  repo: string;
  branch: string;
  path: string;
  line: number;
  snippet: readonly string[];
}

/* Why a group has nothing to list even though its route answered:
 *   signIn  file search needs the app password lososd keeps from the
 *           owner's last password sign-in, and there has not been one yet
 *   notUp   the app is not running (yet)
 *   noGit   this box runs no LosOS Git
 *   off     the owner turned the search inside files off */
export type Blocked = "signIn" | "notUp" | "noGit" | "off";

export type Remote<T> =
  | { kind: "idle" }
  | { kind: "loading" }
  /** `query` is what the items answer; while the next answer is on its way
   *  the palette keeps them, narrowed to what is typed now. `partial`: the
   *  box has not finished reading everything yet. */
  | { kind: "ready"; items: readonly T[]; query: string; partial: boolean }
  | { kind: "blocked"; reason: Blocked }
  | { kind: "absent" }
  | { kind: "failed" };

type Answer<T> = { items: readonly T[]; partial?: boolean } | { blocked: Blocked };

function blockedReason(value: unknown): Blocked {
  return value === "signIn" || value === "noGit" || value === "off" ? value : "notUp";
}

/** GET /api/cloud/files?q= : `{available, reason, entries: [...]}`. */
export async function searchFiles(query: string, signal: AbortSignal): Promise<Answer<CloudFile>> {
  const body = await authedGet(`/api/cloud/files?q=${encodeURIComponent(query)}`, { signal });
  if (!isRecord(body)) return { items: [] };
  if (body["available"] === false) return { blocked: blockedReason(body["reason"]) };
  const items: CloudFile[] = [];
  for (const entry of Array.isArray(body["entries"]) ? body["entries"] : []) {
    if (!isRecord(entry)) continue;
    const name = nonEmptyString(entry["name"]);
    const path = nonEmptyString(entry["path"]);
    const id = entry["id"];
    if (name === null || path === null || typeof id !== "number" || !Number.isSafeInteger(id)) continue;
    items.push({ name, path, id, folder: entry["folder"] === true });
  }
  return { items };
}

/** GET /api/git/repos?q= : `{available, reason, repos: [...]}`. */
export async function searchRepos(query: string, signal: AbortSignal): Promise<Answer<GitRepo>> {
  const url = query.length > 0 ? `/api/git/repos?q=${encodeURIComponent(query)}` : "/api/git/repos";
  const body = await authedGet(url, { signal });
  if (!isRecord(body)) return { items: [] };
  if (body["available"] === false) return { blocked: blockedReason(body["reason"]) };
  const items: GitRepo[] = [];
  for (const entry of Array.isArray(body["repos"]) ? body["repos"] : []) {
    if (!isRecord(entry)) continue;
    const owner = nonEmptyString(entry["owner"]);
    const name = nonEmptyString(entry["name"]);
    // Both go into a path: keep the characters Forgejo allows in names.
    if (owner === null || name === null || !/^[\w.-]+$/.test(owner) || !/^[\w.-]+$/.test(name)) continue;
    items.push({ owner, name, private: entry["private"] === true, description: nonEmptyString(entry["description"]) });
  }
  return { items };
}

/** Alternating plain and matched runs, as the box sends them. */
function snippetOf(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((run): run is string => typeof run === "string") : [];
}

/** GET /api/cloud/text?q= : `{available, reason, partial, hits: [...]}`. */
export async function searchCloudText(query: string, signal: AbortSignal): Promise<Answer<CloudText>> {
  const body = await authedGet(`/api/cloud/text?q=${encodeURIComponent(query)}`, { signal });
  if (!isRecord(body)) return { items: [] };
  if (body["available"] === false) return { blocked: blockedReason(body["reason"]) };
  const items: CloudText[] = [];
  for (const hit of Array.isArray(body["hits"]) ? body["hits"] : []) {
    if (!isRecord(hit)) continue;
    const path = nonEmptyString(hit["path"]);
    if (path === null) continue;
    items.push({ path, snippet: snippetOf(hit["snippet"]) });
  }
  return { items, partial: body["partial"] === true };
}

/** GET /api/git/text?q= : `{available, reason, hits: [...]}`. */
export async function searchGitText(query: string, signal: AbortSignal): Promise<Answer<GitText>> {
  const body = await authedGet(`/api/git/text?q=${encodeURIComponent(query)}`, { signal });
  if (!isRecord(body)) return { items: [] };
  if (body["available"] === false) return { blocked: blockedReason(body["reason"]) };
  const items: GitText[] = [];
  for (const hit of Array.isArray(body["hits"]) ? body["hits"] : []) {
    if (!isRecord(hit)) continue;
    const owner = nonEmptyString(hit["owner"]);
    const repo = nonEmptyString(hit["repo"]);
    const branch = nonEmptyString(hit["branch"]);
    const path = nonEmptyString(hit["path"]);
    const line = hit["line"];
    if (owner === null || repo === null || branch === null || path === null) continue;
    if (!/^[\w.-]+$/.test(owner) || !/^[\w.-]+$/.test(repo)) continue;
    if (typeof line !== "number" || !Number.isSafeInteger(line) || line < 1) continue;
    items.push({ owner, repo, branch, path, line, snippet: snippetOf(hit["snippet"]) });
  }
  return { items };
}

export async function searchApps(query: string, signal: AbortSignal): Promise<Answer<CatalogueApp>> {
  return { items: (await searchCatalogue(query, signal)).apps };
}

export function fileHref(file: CloudFile): string {
  return `/cloud/files?fileid=${file.id}`;
}

export function repoHref(repo: GitRepo): string {
  return `/forgejo/${encodeURIComponent(repo.owner)}/${encodeURIComponent(repo.name)}`;
}

/** Each segment of a path, escaped on its own. */
function segments(path: string): string {
  return path.split("/").filter((part) => part.length > 0).map(encodeURIComponent).join("/");
}

/* A cloud hit opens its folder in LosOS Cloud's Files: the index knows
 * paths, not the file ids the web app's ?fileid= wants. */
export function cloudTextHref(hit: CloudText): string {
  const cut = hit.path.lastIndexOf("/");
  return cut < 0 ? "/cloud/files" : `/cloud/files/${segments(hit.path.slice(0, cut))}`;
}

/** The line in LosOS Git's file view. */
export function gitTextHref(hit: GitText): string {
  return `${repoHref({ owner: hit.owner, name: hit.repo, private: false, description: null })}/src/branch/${segments(hit.branch)}/${segments(hit.path)}#L${hit.line}`;
}

/** Debounce: typing a word is one request, not one per letter. */
const DEBOUNCE_MS = 250;

/* One remote group's state for the query on screen. `minLength` is how much
 * has to be typed before the box is asked at all; 0 asks on open too (the
 * repositories, listed before anything is typed). */
export function useRemote<T>(
  enabled: boolean,
  query: string,
  minLength: number,
  fetcher: (query: string, signal: AbortSignal) => Promise<Answer<T>>,
): Remote<T> {
  const [state, setState] = React.useState<Remote<T>>({ kind: "idle" });
  // Once the box has said it does not serve the route, stop asking.
  const absent = React.useRef(false);
  const trimmed = query.trim();

  React.useEffect(() => {
    if (absent.current) return;
    if (!enabled || trimmed.length < minLength) {
      setState({ kind: "idle" });
      return;
    }
    const controller = new AbortController();
    setState((current) => (current.kind === "ready" ? current : { kind: "loading" }));
    const timer = window.setTimeout(
      () => {
        void (async () => {
          try {
            const answer = await fetcher(trimmed, controller.signal);
            if (controller.signal.aborted) return;
            setState(
              "blocked" in answer
                ? { kind: "blocked", reason: answer.blocked }
                : { kind: "ready", items: answer.items, query: trimmed, partial: answer.partial === true },
            );
          } catch (error) {
            if (isAbort(error) || controller.signal.aborted) return;
            if (isUnauthorized(error)) return setState({ kind: "idle" });
            if (notPresent(error)) {
              absent.current = true;
              return setState({ kind: "absent" });
            }
            setState({ kind: "failed" });
          }
        })();
      },
      trimmed.length === 0 ? 0 : DEBOUNCE_MS,
    );
    return () => {
      controller.abort();
      window.clearTimeout(timer);
    };
  }, [enabled, trimmed, minLength, fetcher]);

  return state;
}
