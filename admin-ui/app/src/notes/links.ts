/* What a link names, and which note or file in a vault that is, the way
 * Obsidian finds it: by the whole path when the link has one, else by the
 * file's name, nearest to the linking note first, then the shortest path;
 * then by a note's aliases. Notes are .note (MDX) and .md files; anything else in the
 * vault is an attachment named with its extension. */

export interface LinkParts {
  /** The note or file named, as written: no heading, no alias. */
  target: string;
  /** "Heading" or "^block" after the #, or "". */
  sub: string;
  /** What the link shows instead of the target, or the size of an embed. */
  alias: string | null;
}

/** "Note#Heading|shown" into its parts. A pipe escaped for a table (\|)
 * counts as the pipe. */
export function parseLink(inner: string): LinkParts {
  const text = inner.replace(/\\\|/g, "|");
  const bar = text.indexOf("|");
  const head = bar < 0 ? text : text.slice(0, bar);
  const alias = bar < 0 ? null : text.slice(bar + 1).trim();
  const hash = head.indexOf("#");
  const target = (hash < 0 ? head : head.slice(0, hash)).trim();
  const sub = hash < 0 ? "" : head.slice(hash + 1).trim().replace(/#/g, " ").replace(/\s+/g, " ");
  return { target, sub, alias: alias === "" ? null : alias };
}

/** A heading's text as an anchor: lower case, letters and digits joined by
 * dashes. The same text always gives the same slug. */
export function slug(text: string): string {
  return text
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "");
}

export function folderOf(path: string): string {
  const at = path.lastIndexOf("/");
  return at < 0 ? "" : path.slice(0, at);
}

export function baseName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

/** A note's extension: `.note`, MDX, which new notes get, or `.md`. */
export const NOTE_EXT = /\.(note|md)$/i;

/** The name a note goes by: its file name without the extension. */
export function noteName(path: string): string {
  return baseName(path).replace(NOTE_EXT, "");
}

export function isNote(path: string): boolean {
  return NOTE_EXT.test(path);
}

/** A note in MDX, read with its components, rather than plain Markdown. */
export function isMdx(path: string): boolean {
  return /\.note$/i.test(path);
}

/** `a/./b/../c` to `a/c`; null when it climbs out of the vault. */
export function normalise(path: string): string | null {
  const out: string[] = [];
  for (const part of path.split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") {
      if (out.length === 0) return null;
      out.pop();
    } else out.push(part);
  }
  return out.join("/");
}

interface Named {
  path: string;
  aliases?: readonly string[];
}

/** Finds notes and files by what links call them. */
export class LinkIndex<N extends Named, F extends { path: string }> {
  private readonly byPath = new Map<string, N>();
  private readonly byName = new Map<string, N[]>();
  private readonly byAlias = new Map<string, N[]>();
  private readonly fileByPath = new Map<string, F>();
  private readonly fileByName = new Map<string, F[]>();

  constructor(notes: readonly N[], files: readonly F[]) {
    for (const n of notes) {
      const key = n.path.toLowerCase().replace(NOTE_EXT, "");
      this.byPath.set(key, n);
      push(this.byName, noteName(n.path).toLowerCase(), n);
      for (const a of n.aliases ?? []) push(this.byAlias, a.toLowerCase(), n);
    }
    for (const f of files) {
      this.fileByPath.set(f.path.toLowerCase(), f);
      push(this.fileByName, baseName(f.path).toLowerCase(), f);
    }
  }

  /** The note `target` names, linked from the note at `from`. */
  note(target: string, from = ""): N | null {
    const t = target.replace(/^\/+/, "").replace(NOTE_EXT, "").toLowerCase();
    if (t === "") return null;
    // A relative path, as a Markdown link writes one.
    if (t.startsWith("./") || t.startsWith("../")) {
      const p = normalise(`${folderOf(from)}/${t}`);
      return p === null ? null : (this.byPath.get(p.toLowerCase()) ?? null);
    }
    const exact = this.byPath.get(t);
    if (exact !== undefined) return exact;
    const here = normalise(`${folderOf(from)}/${t}`);
    if (here !== null) {
      const near = this.byPath.get(here.toLowerCase());
      if (near !== undefined) return near;
    }
    const name = t.slice(t.lastIndexOf("/") + 1);
    const candidates = (this.byName.get(name) ?? []).filter((n) => !t.includes("/") || n.path.toLowerCase().replace(NOTE_EXT, "").endsWith(`/${t}`));
    const best = nearest(candidates, from);
    if (best !== null) return best;
    return nearest(this.byAlias.get(t) ?? [], from);
  }

  /** The attachment `target` names, linked from the note at `from`. */
  file(target: string, from = ""): F | null {
    const t = target.replace(/^\/+/, "").toLowerCase();
    if (t === "") return null;
    const rel = normalise(`${folderOf(from)}/${t}`);
    if (rel !== null) {
      const near = this.fileByPath.get(rel.toLowerCase());
      if (near !== undefined) return near;
    }
    const exact = this.fileByPath.get(t);
    if (exact !== undefined) return exact;
    const name = t.slice(t.lastIndexOf("/") + 1);
    const candidates = (this.fileByName.get(name) ?? []).filter((f) => !t.includes("/") || f.path.toLowerCase().endsWith(`/${t}`));
    return nearest(candidates, from);
  }
}

function push<V>(map: Map<string, V[]>, key: string, value: V): void {
  const list = map.get(key);
  if (list === undefined) map.set(key, [value]);
  else list.push(value);
}

/** Of several by one name: the one in the linking note's folder, then the
 * one with the shortest path, then the first by path. */
function nearest<T extends { path: string }>(candidates: readonly T[], from: string): T | null {
  if (candidates.length === 0) return null;
  const folder = folderOf(from).toLowerCase();
  return [...candidates].sort((a, b) => {
    const fa = folderOf(a.path).toLowerCase() === folder ? 0 : 1;
    const fb = folderOf(b.path).toLowerCase() === folder ? 0 : 1;
    if (fa !== fb) return fa - fb;
    if (a.path.length !== b.path.length) return a.path.length - b.path.length;
    return a.path.localeCompare(b.path);
  })[0] as T;
}
