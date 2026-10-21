import { LinkIndex, isNote, noteName } from "./links";
import type { Resolver } from "./markdown";

/* A vault as the box lists it (backend/src/cloud/notes/mod.rs), with the
 * links resolved: which note each link means, the backlinks, the tags and
 * the graph. One model serves LosOS Cloud's Notes and a knowledge base's
 * pages; where text and files come from is the `Source`. */

export interface ScannedLink {
  target: string;
  embed: boolean;
}

export interface VaultNote {
  id: number;
  path: string;
  size: number;
  mtime: number;
  etag: string;
  title: string | null;
  aliases: string[];
  tags: string[];
  links: ScannedLink[];
  private: boolean;
}

export interface VaultFile {
  id: number;
  path: string;
  mime: string;
  size: number;
}

export interface VaultIndex {
  root: string;
  name: string;
  folderId: number;
  notes: VaultNote[];
  canvases: VaultNote[];
  files: VaultFile[];
  folders: string[];
}

export interface Hit {
  path: string;
  line: number;
  text: string;
}

/** Where a vault's text and files come from. */
export interface Source {
  text(path: string, signal?: AbortSignal): Promise<string>;
  fileUrl(path: string): string;
  /** Where a note opens: `sub` is "Heading" or "^block" or "". */
  href(path: string, sub: string): string;
  search(q: string, signal?: AbortSignal): Promise<Hit[]>;
}

export type NodeKind = "note" | "canvas" | "tag" | "file" | "missing";

export interface GraphNode {
  id: string;
  kind: NodeKind;
  label: string;
  /** The note's, canvas's or file's path; the tag; the missing name. */
  key: string;
  degree: number;
}

export interface GraphEdge {
  source: string;
  target: string;
}

export interface GraphOptions {
  tags: boolean;
  attachments: boolean;
  missing: boolean;
  orphans: boolean;
  /** Only names containing this text, and what links them. */
  filter: string;
}

export const DEFAULT_GRAPH: GraphOptions = { tags: false, attachments: false, missing: false, orphans: true, filter: "" };

export interface Resolved {
  /** Notes and canvases this one links, by path, once each. */
  out: string[];
  /** Attachments it links or embeds. */
  files: string[];
  /** Names it links that the vault has no note for. */
  missing: string[];
}

export function displayName(note: Pick<VaultNote, "path" | "title">): string {
  return noteName(note.path).replace(/\.canvas$/i, "");
}

export class Vault {
  readonly byPath = new Map<string, VaultNote>();
  readonly resolved = new Map<string, Resolved>();
  readonly backlinks = new Map<string, Set<string>>();
  readonly tags = new Map<string, Set<string>>();
  private readonly links: LinkIndex<VaultNote, VaultFile | VaultNote>;

  constructor(readonly index: VaultIndex) {
    const all = [...index.notes, ...index.canvases];
    for (const n of all) this.byPath.set(n.path, n);
    // Canvases are linked by their full name, as attachments are.
    this.links = new LinkIndex(index.notes, [...index.files, ...index.canvases]);
    for (const n of all) {
      const r: Resolved = { out: [], files: [], missing: [] };
      for (const l of n.links) {
        const note = isNote(l.target) || !/\.[a-z0-9]+$/i.test(l.target) ? this.links.note(l.target, n.path) : null;
        if (note !== null) {
          if (note.path !== n.path && !r.out.includes(note.path)) r.out.push(note.path);
          continue;
        }
        const file = this.links.file(l.target, n.path);
        if (file !== null) {
          if (this.byPath.has(file.path)) {
            if (!r.out.includes(file.path)) r.out.push(file.path);
          } else if (!r.files.includes(file.path)) r.files.push(file.path);
          continue;
        }
        if (!r.missing.includes(l.target)) r.missing.push(l.target);
      }
      this.resolved.set(n.path, r);
      for (const to of r.out) {
        let set = this.backlinks.get(to);
        if (set === undefined) this.backlinks.set(to, (set = new Set()));
        set.add(n.path);
      }
      for (const tag of n.tags) {
        const key = tag.toLowerCase();
        let set = this.tags.get(key);
        if (set === undefined) this.tags.set(key, (set = new Set()));
        set.add(n.path);
      }
    }
  }

  get notes(): readonly VaultNote[] {
    return this.index.notes;
  }

  note(path: string): VaultNote | undefined {
    return this.byPath.get(path);
  }

  /** A link's note from the note at `from`. */
  find(target: string, from = ""): VaultNote | null {
    return this.links.note(target, from);
  }

  /** The renderer's resolver for the note at `from`. */
  resolver(from: string, source: Source): Resolver {
    return {
      note: (target) => {
        const n = this.links.note(target, from);
        if (n !== null) return { path: n.path };
        const canvas = /\.canvas$/i.test(target) ? this.links.file(target, from) : null;
        return canvas !== null && this.byPath.has(canvas.path) ? { path: canvas.path } : null;
      },
      file: (target) => {
        const f = this.links.file(target, from);
        return f === null || this.byPath.has(f.path) ? null : source.fileUrl(f.path);
      },
      href: (path, sub) => source.href(path, sub),
    };
  }

  /** Tags with how many notes carry each, most first. */
  tagCounts(): { tag: string; count: number }[] {
    const shown = new Map<string, string>();
    for (const n of this.index.notes) for (const t of n.tags) if (!shown.has(t.toLowerCase())) shown.set(t.toLowerCase(), t);
    return [...this.tags.entries()]
      .map(([key, set]) => ({ tag: shown.get(key) ?? key, count: set.size }))
      .sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag));
  }

  /** Notes carrying `tag` or a tag below it (`area` takes `area/sub`). */
  tagged(tag: string): VaultNote[] {
    const want = tag.toLowerCase();
    const out = new Set<string>();
    for (const [key, set] of this.tags) if (key === want || key.startsWith(`${want}/`)) for (const p of set) out.add(p);
    return [...out].map((p) => this.byPath.get(p)).filter((n): n is VaultNote => n !== undefined);
  }

  /** Notes that mention this one's name in their text without linking it
   * are Obsidian's "unlinked mentions"; this is the linked half. */
  linkedFrom(path: string): VaultNote[] {
    return [...(this.backlinks.get(path) ?? [])]
      .map((p) => this.byPath.get(p))
      .filter((n): n is VaultNote => n !== undefined)
      .sort((a, b) => displayName(a).localeCompare(displayName(b)));
  }

  /** The graph, whole or around `focus` to `depth` links. */
  graph(options: GraphOptions, focus?: string, depth = 1): { nodes: GraphNode[]; edges: GraphEdge[] } {
    const nodes = new Map<string, GraphNode>();
    const edges: GraphEdge[] = [];
    const seen = new Set<string>();
    const add = (id: string, kind: NodeKind, label: string, key: string) => {
      if (!nodes.has(id)) nodes.set(id, { id, kind, label, key, degree: 0 });
    };
    const edge = (a: string, b: string) => {
      const k = a < b ? `${a}\n${b}` : `${b}\n${a}`;
      if (a === b || seen.has(k)) return;
      seen.add(k);
      edges.push({ source: a, target: b });
    };
    for (const n of this.byPath.values()) {
      const kind: NodeKind = isNote(n.path) ? "note" : "canvas";
      add(`n:${n.path}`, kind, displayName(n), n.path);
    }
    for (const n of this.byPath.values()) {
      const r = this.resolved.get(n.path);
      if (r === undefined) continue;
      for (const to of r.out) edge(`n:${n.path}`, `n:${to}`);
      if (options.attachments) {
        for (const f of r.files) {
          add(`f:${f}`, "file", f.slice(f.lastIndexOf("/") + 1), f);
          edge(`n:${n.path}`, `f:${f}`);
        }
      }
      if (options.missing) {
        for (const m of r.missing) {
          add(`m:${m.toLowerCase()}`, "missing", m, m);
          edge(`n:${n.path}`, `m:${m.toLowerCase()}`);
        }
      }
      if (options.tags) {
        for (const t of n.tags) {
          add(`t:${t.toLowerCase()}`, "tag", `#${t}`, t);
          edge(`n:${n.path}`, `t:${t.toLowerCase()}`);
        }
      }
    }
    for (const e of edges) {
      const a = nodes.get(e.source);
      const b = nodes.get(e.target);
      if (a !== undefined) a.degree++;
      if (b !== undefined) b.degree++;
    }
    let keep = new Set(nodes.keys());
    if (focus !== undefined) {
      const start = `n:${focus}`;
      keep = new Set([start]);
      let frontier = [start];
      for (let d = 0; d < depth; d++) {
        const next: string[] = [];
        for (const e of edges) {
          for (const [a, b] of [
            [e.source, e.target],
            [e.target, e.source],
          ] as const) {
            if (frontier.includes(a) && !keep.has(b)) {
              keep.add(b);
              next.push(b);
            }
          }
        }
        frontier = next;
      }
    }
    const filter = options.filter.trim().toLowerCase();
    if (filter !== "") {
      const matched = new Set([...keep].filter((id) => nodes.get(id)?.label.toLowerCase().includes(filter)));
      keep = matched;
    }
    if (!options.orphans && focus === undefined) {
      keep = new Set([...keep].filter((id) => (nodes.get(id)?.degree ?? 0) > 0));
    }
    return {
      nodes: [...keep].map((id) => nodes.get(id)).filter((n): n is GraphNode => n !== undefined),
      edges: edges.filter((e) => keep.has(e.source) && keep.has(e.target)),
    };
  }

  /** A file name for a new note that is not taken yet, in `folder`. */
  freeName(folder: string, base: string): string {
    const clean = base.replace(/[\\/:*?"<>|#^[\]]+/g, " ").trim() || "Untitled";
    const prefix = folder === "" ? "" : `${folder}/`;
    for (let i = 1; ; i++) {
      const name = `${prefix}${i === 1 ? clean : `${clean} ${i}`}.md`;
      if (!this.byPath.has(name)) return name;
    }
  }

  /** The note a site or a vault opens on: index, README or Home, or one
   * named like the vault, else the first. */
  home(): VaultNote | null {
    const top = this.index.notes.filter((n) => !n.path.includes("/"));
    const named = (name: string) => top.find((n) => noteName(n.path).toLowerCase() === name.toLowerCase());
    return (
      named("index") ??
      named("README") ??
      named("Home") ??
      (this.index.name !== "" ? named(this.index.name) : undefined) ??
      top[0] ??
      this.index.notes[0] ??
      null
    );
  }
}
