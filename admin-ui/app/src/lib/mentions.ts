import * as React from "react";
import { getProfiles, type Profile } from "@/lib/api";

/* @ tags: a person on this box, a file in the signed-in person's LosOS
 * Cloud or an issue of LosOS Git's tracker, picked by typing @ and written
 * into the text as Markdown that any other reader still makes sense of:
 *
 *   a person   @eva                                  (their sign-in name)
 *   a file     [@plan.canvas](/diagrams?file=...)    (a link on this box)
 *   an issue   [LOS-12 Fix the header](/git/issues/LOS-12)
 *
 * People come from GET /api/profiles, files from the Cloud service's own
 * search (GET /cloud/api/files/search) and issues from LosOS Git's
 * (GET /git/api/v1/tracker/issues), both of which answer for whoever is
 * signed in, so nobody is offered a file or an issue they cannot open. Any
 * source failing (the hosted Diagrams, a box without Cloud or Git) leaves
 * the others. */

export type Mention =
  | { kind: "person"; id: string; label: string; detail: string; person: Profile }
  | { kind: "file"; id: string; label: string; detail: string; href: string; dir: boolean }
  | { kind: "issue"; id: string; label: string; detail: string; href: string; key: string };

interface CloudEntry {
  id: number;
  path: string;
  name: string;
  dir: boolean;
}

let people: Promise<Profile[]> | null = null;

/** Everyone on this box with a profile, fetched once per page. */
export function loadPeople(): Promise<Profile[]> {
  people ??= getProfiles().then(
    (r) => (Array.isArray(r.people) ? r.people : []),
    () => {
      people = null;
      return [];
    },
  );
  return people;
}

/** The box's people by sign-in name, once they have loaded. */
export function usePeople(): ReadonlyMap<string, Profile> {
  const [byName, setByName] = React.useState<ReadonlyMap<string, Profile>>(new Map());
  React.useEffect(() => {
    let live = true;
    void loadPeople().then((list) => {
      if (live) setByName(new Map(list.map((p) => [p.name, p])));
    });
    return () => {
      live = false;
    };
  }, []);
  return byName;
}

/** Where a file mention goes: a diagram opens in Diagrams, anything else in
 * its folder in LosOS Cloud with the file marked. */
export function fileHref(entry: CloudEntry): string {
  if (!entry.dir && /\.canvas$/i.test(entry.name)) return `/diagrams?file=${encodeURIComponent(entry.path).replace(/[()]/g, (c) => (c === "(" ? "%28" : "%29"))}`;
  return `/cloud/files?fileid=${entry.id}`;
}

/** A link a file or issue mention may carry: only these apps on this origin. */
export function isMentionHref(href: string): boolean {
  return /^\/(cloud\/files|diagrams|git\/issues)(?:[/?#]|$)/.test(href);
}

async function searchIssues(q: string, signal: AbortSignal): Promise<Mention[]> {
  if (q.trim() === "") return [];
  const res = await fetch(`/git/api/v1/tracker/issues?${new URLSearchParams({ q, limit: "5", sort: "updated" }).toString()}`, {
    credentials: "same-origin",
    signal,
  });
  if (!res.ok) return [];
  const body = (await res.json()) as { issues?: unknown };
  return (Array.isArray(body.issues) ? (body.issues as { key: unknown; title: unknown }[]) : [])
    .filter((i): i is { key: string; title: string } => typeof i.key === "string" && /^[A-Z][A-Z0-9]{1,5}-\d+$/.test(i.key) && typeof i.title === "string")
    .slice(0, 5)
    .map((i) => ({ kind: "issue", id: `issue:${i.key}`, label: i.title, detail: i.key, key: i.key, href: `/git/issues/${i.key}` }));
}

async function searchFiles(q: string, signal: AbortSignal): Promise<Mention[]> {
  if (q.trim() === "") return [];
  const res = await fetch(`/cloud/api/files/search?q=${encodeURIComponent(q)}&limit=6`, {
    credentials: "same-origin",
    signal,
  });
  if (!res.ok) return [];
  const body = (await res.json()) as { entries?: unknown };
  return (Array.isArray(body.entries) ? (body.entries as CloudEntry[]) : []).slice(0, 6).map((e) => ({
    kind: "file",
    id: `file:${e.id}`,
    label: e.name,
    detail: e.path.includes("/") ? e.path.slice(0, e.path.lastIndexOf("/")) : "",
    href: fileHref(e),
    dir: e.dir,
  }));
}

/** People whose sign-in or display name has `q` in it, then files. */
export async function searchMentions(q: string, signal: AbortSignal): Promise<Mention[]> {
  const needle = q.toLocaleLowerCase();
  const quiet = (err: unknown): Mention[] => {
    if (signal.aborted) throw err;
    return [];
  };
  const [list, files, issues] = await Promise.all([loadPeople(), searchFiles(q, signal).catch(quiet), searchIssues(q, signal).catch(quiet)]);
  const found: Mention[] = list
    .filter((p) => p.name.includes(needle) || p.displayName.toLocaleLowerCase().includes(needle))
    .sort((a, b) => Number(!a.name.startsWith(needle)) - Number(!b.name.startsWith(needle)))
    .slice(0, 5)
    .map((p) => ({ kind: "person", id: `person:${p.name}`, label: p.displayName.trim() || p.name, detail: `@${p.name}`, person: p }));
  return [...found, ...files, ...issues];
}

/** The Markdown a picked mention becomes. */
export function mentionText(m: Mention): string {
  if (m.kind === "person") return `@${m.person.name}`;
  if (m.kind === "issue") return `[${m.key} ${m.label.replace(/[[\]\n]/g, "")}](${m.href})`;
  return `[@${m.label.replace(/[[\]\n]/g, "")}](${m.href})`;
}

/** The @ query being typed just before the caret, if any. */
export function queryAt(text: string, caret: number): { start: number; q: string } | null {
  const m = /(^|[\s([{])@([^\s@[\]()]{0,40})$/.exec(text.slice(0, caret));
  if (m === null) return null;
  return { start: caret - m[2]!.length - 1, q: m[2]! };
}
