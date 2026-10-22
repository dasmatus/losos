import * as React from "react";
import { getProfiles, type Profile } from "@/lib/api";

/* @ tags: a person on this box or a file in the signed-in person's LosOS
 * Cloud, picked by typing @ and written into the text as Markdown that
 * any other reader still makes sense of:
 *
 *   a person   @eva                                  (their sign-in name)
 *   a file     [@plan.canvas](/diagrams?file=...)    (a link on this box)
 *
 * People come from GET /api/profiles and files from the Cloud service's
 * own search (GET /cloud/api/files/search), which answers for whoever is
 * signed in, so nobody is offered a file they cannot open. Either source
 * failing (the hosted Diagrams, a box without Cloud) leaves the other. */

export type Mention =
  | { kind: "person"; id: string; label: string; detail: string; person: Profile }
  | { kind: "file"; id: string; label: string; detail: string; href: string; dir: boolean };

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

/** A link a file mention may carry: only these two apps on this origin. */
export function isMentionHref(href: string): boolean {
  return /^\/(cloud\/files|diagrams)(?:[/?#]|$)/.test(href);
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
  const [list, files] = await Promise.all([
    loadPeople(),
    searchFiles(q, signal).catch((err: unknown) => {
      if (signal.aborted) throw err;
      return [] as Mention[];
    }),
  ]);
  const found: Mention[] = list
    .filter((p) => p.name.includes(needle) || p.displayName.toLocaleLowerCase().includes(needle))
    .sort((a, b) => Number(!a.name.startsWith(needle)) - Number(!b.name.startsWith(needle)))
    .slice(0, 5)
    .map((p) => ({ kind: "person", id: `person:${p.name}`, label: p.displayName.trim() || p.name, detail: `@${p.name}`, person: p }));
  return [...found, ...files];
}

/** The Markdown a picked mention becomes. */
export function mentionText(m: Mention): string {
  if (m.kind === "person") return `@${m.person.name}`;
  return `[@${m.label.replace(/[[\]\n]/g, "")}](${m.href})`;
}

/** The @ query being typed just before the caret, if any. */
export function queryAt(text: string, caret: number): { start: number; q: string } | null {
  const m = /(^|[\s([{])@([^\s@[\]()]{0,40})$/.exec(text.slice(0, caret));
  if (m === null) return null;
  return { start: caret - m[2]!.length - 1, q: m[2]! };
}
