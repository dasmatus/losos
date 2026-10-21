/* Editing a diagram together. A diagram from LosOS Cloud opens in that
 * file's live-editing room (backend/src/cloud/collab/), the same room
 * LosOS Cloud's text editor uses: the room holds the .canvas text as one
 * Yjs text and writes it back to the file a moment after the last change.
 *
 * The core stays the one model. After each change made here, the core's
 * export goes into the shared text as line edits. The export is JSON with
 * one key a line, so two people's changes to different cards touch
 * different lines and merge. After anyone else's change the whole text
 * goes back to the core (canvas_sync), which also puts back a comma a
 * merge lost. When a merge leaves the text different from what the core
 * writes, one editor (the lowest client number that may write) writes the
 * core's text back, so every copy settles on the same file.
 *
 * Who is here, their pointer and their selection travel in awareness. */

import { WebsocketProvider } from "y-websocket";
import * as Y from "yjs";
import type { PresenceUser } from "@/cloud/collab/cursors";
import { ACCENTS, accentFor, nameOf } from "@/cloud/collab/people";
import { t } from "@/lib/i18n";
import { isErr } from "../core";
import type { LabStore } from "../store";
import { roomServer } from "./cloud";

/** The name of the shared text; the box uses the same (room.rs, TEXT). */
const TEXT = "content";
/** Changes made here, which undo can take back. */
const LOCAL = "diagrams";
/** The settling rewrite, which undo leaves alone. */
const TIDY = "diagrams-tidy";

export interface Peer {
  client: number;
  user: PresenceUser;
  /** Where their pointer is on the canvas, in canvas units. */
  pointer: { x: number; y: number } | null;
  /** The cards and arrows they have selected. */
  sel: string[];
}

export interface LiveView {
  state: "connecting" | "live" | "offline";
  /** They may look and not change: the room drops their edits. */
  readOnly: boolean;
  /** Everyone with the file open, this tab too. */
  people: Peer[];
  /** This tab's client number. */
  me: number;
  myAddress: string;
  /** A file from another box: whose, and where it lives. */
  from: { owner: string; box: string } | null;
  canUndo: boolean;
  canRedo: boolean;
}

export interface Room {
  file: number;
  /** A file on another box: the share it came in. */
  share?: string;
  readOnly: boolean;
  from: LiveView["from"];
  /** Called with the text on joining, for a file nothing here has opened
   *  yet: the page opens it as it would a file. */
  opening?: (text: string) => boolean;
}

/** The edits that turn `a` into `b` line by line, as [offset in `a`,
 *  characters to delete, text to insert], the last one first so each can
 *  be applied as it comes. */
export function lineEdits(a: string, b: string): [number, number, string][] {
  if (a === b) return [];
  const split = (s: string) => s.match(/[^\n]*\n|[^\n]+/g) ?? [];
  const A = split(a);
  const B = split(b);
  let p = 0;
  while (p < A.length && p < B.length && A[p] === B[p]) p++;
  let ea = A.length;
  let eb = B.length;
  while (ea > p && eb > p && A[ea - 1] === B[eb - 1]) {
    ea--;
    eb--;
  }
  const at = new Array<number>(A.length + 1);
  at[0] = 0;
  for (let i = 0; i < A.length; i++) at[i + 1] = at[i]! + A[i]!.length;
  const n = ea - p;
  const m = eb - p;
  // Hunks in the middle: [a from, a to, b from, b to], in line numbers.
  const hunks: [number, number, number, number][] = [];
  if (n * m > 4_000_000 || n === 0 || m === 0) {
    hunks.push([p, ea, p, eb]);
  } else {
    // The longest run of lines both keep, by the usual table.
    const w = m + 1;
    const len = new Uint32Array((n + 1) * w);
    for (let i = n - 1; i >= 0; i--) {
      for (let j = m - 1; j >= 0; j--) {
        len[i * w + j] = A[p + i] === B[p + j] ? len[(i + 1) * w + j + 1]! + 1 : Math.max(len[(i + 1) * w + j]!, len[i * w + j + 1]!);
      }
    }
    let i = 0;
    let j = 0;
    let si = 0;
    let sj = 0;
    while (i < n && j < m) {
      if (A[p + i] === B[p + j]) {
        if (si < i || sj < j) hunks.push([p + si, p + i, p + sj, p + j]);
        i++;
        j++;
        si = i;
        sj = j;
      } else if (len[(i + 1) * w + j]! >= len[i * w + j + 1]!) i++;
      else j++;
    }
    if (si < n || sj < m) hunks.push([p + si, p + n, p + sj, p + m]);
  }
  return hunks.reverse().map(([a0, a1, b0, b1]) => [at[a0]!, at[a1]! - at[a0]!, B.slice(b0, b1).join("")]);
}

function readPointer(v: unknown): { x: number; y: number } | null {
  const p = v as { x?: unknown; y?: unknown } | null;
  return p && typeof p.x === "number" && typeof p.y === "number" && Number.isFinite(p.x) && Number.isFinite(p.y) ? { x: p.x, y: p.y } : null;
}

function readUser(v: unknown): PresenceUser | null {
  const u = v as Partial<PresenceUser> | null;
  if (!u || typeof u.name !== "string" || typeof u.address !== "string") return null;
  const accent = (ACCENTS as readonly string[]).includes(u.accent ?? "") ? u.accent! : accentFor(nameOf(u.address));
  return { name: u.name.slice(0, 80), address: u.address.slice(0, 200), accent, picture: typeof u.picture === "string" ? u.picture : null };
}

export class LiveDiagram {
  private readonly doc = new Y.Doc();
  private readonly text = this.doc.getText(TEXT);
  private readonly meta = this.doc.getMap<{ name: string; address: string; at: number }>("meta");
  private readonly provider: WebsocketProvider;
  private readonly undo: Y.UndoManager;
  /** The export the core and the shared text last agreed on. */
  private synced = "";
  private seenCanvas: unknown = null;
  private seenSnap: unknown = null;
  private seenSel: unknown = null;
  private joined = false;
  private busy = false;
  private warned = false;
  private settle = 0;
  private pointerAt = 0;
  private stamped = 0;
  private readonly off: (() => void)[] = [];

  readonly file: number;
  readonly readOnly: boolean;

  constructor(
    private readonly s: LabStore,
    private readonly room: Room,
    private readonly me: PresenceUser,
    /** The world was built again: the guests and consoles start over. */
    private readonly rebuilt: () => void,
  ) {
    const { file, readOnly } = room;
    this.file = file;
    this.readOnly = readOnly;
    this.provider = new WebsocketProvider(roomServer(), "ws", this.doc, {
      params: room.share === undefined ? { file: String(file) } : { file: String(file), share: room.share },
      // One room name for every file: tabs must not pass updates to each
      // other by the room's name.
      disableBc: true,
    });
    const aw = this.provider.awareness;
    aw.setLocalState({ user: me, pointer: null, sel: [], write: !readOnly });
    this.undo = new Y.UndoManager(this.text, { trackedOrigins: new Set([LOCAL]), captureTimeout: 600 });
    s.live = { state: "connecting", readOnly, people: [], me: this.doc.clientID, myAddress: me.address, from: room.from, canUndo: false, canRedo: false };

    this.provider.on("status", ({ status }: { status: string }) => {
      if (status !== "connected") this.view({ state: status === "connecting" && !this.joined ? "connecting" : "offline" });
    });
    this.provider.on("sync", (synced: boolean) => {
      if (!synced) return;
      this.view({ state: "live" });
      if (!this.joined) {
        this.joined = true;
        if (this.room.opening) this.open(this.room.opening);
        else if (this.text.length === 0) this.push(true);
        else this.pull();
      }
    });
    this.text.observe((_, tr) => {
      if (tr.origin !== LOCAL && tr.origin !== TIDY && this.joined) this.pull();
    });
    const stacks = () => this.view({ canUndo: this.undo.canUndo(), canRedo: this.undo.canRedo() });
    this.undo.on("stack-item-added", stacks);
    this.undo.on("stack-item-popped", stacks);
    aw.on("change", () => this.people());
    this.off.push(s.on("model", () => this.push(false)));
    this.people();
  }

  private view(patch: Partial<LiveView>): void {
    const s = this.s;
    if (!s.live) return;
    s.live = { ...s.live, ...patch };
    s.emit("people");
    if ("canUndo" in patch || "readOnly" in patch) s.emit("model");
  }

  private people(): void {
    const out: Peer[] = [];
    this.provider.awareness.getStates().forEach((st, client) => {
      const user = readUser((st as { user?: unknown }).user);
      if (!user) return;
      const raw = (st as { sel?: unknown }).sel;
      const sel = Array.isArray(raw) ? raw.filter((x): x is string => typeof x === "string").slice(0, 500) : [];
      out.push({ client, user, pointer: readPointer((st as { pointer?: unknown }).pointer), sel });
    });
    this.view({ people: out });
  }

  /** A file this page has not read: opened from the room's text. */
  private open(opening: (text: string) => boolean): void {
    const s = this.s;
    this.busy = true;
    let ok = false;
    try {
      ok = opening(this.text.toString());
    } finally {
      this.busy = false;
    }
    if (!ok) return;
    this.synced = s.core.exportSetup().text;
    this.seenCanvas = s.canvas;
    this.seenSnap = s.snap;
    if (this.synced !== this.text.toString()) this.tidyLater(400);
  }

  /** The whole shared text into the core. */
  private pull(): void {
    const s = this.s;
    const text = this.text.toString();
    const r = s.core.canvasSync(text);
    if (isErr(r)) {
      // A merge it cannot read even with its commas put back: the core
      // keeps the last good diagram, and the settling writer puts it back.
      this.tidyLater(1500);
      return;
    }
    this.busy = true;
    try {
      if (r.full) this.rebuilt();
      s.refresh();
    } finally {
      this.busy = false;
    }
    this.synced = s.core.exportSetup().text;
    this.seenCanvas = s.canvas;
    this.seenSnap = s.snap;
    if (this.synced !== text) this.tidyLater(400);
  }

  /** A change made here into the shared text. */
  private push(force: boolean): void {
    const s = this.s;
    const sel = s.ui.csel;
    if (sel !== this.seenSel) {
      this.seenSel = sel;
      this.provider.awareness.setLocalStateField("sel", [...sel.nodes, ...sel.edges]);
    }
    if (this.busy || !this.joined) return;
    if (!force && s.canvas === this.seenCanvas && s.snap === this.seenSnap) return;
    this.seenCanvas = s.canvas;
    this.seenSnap = s.snap;
    const next = s.core.exportSetup().text;
    if (!force && next === this.synced) return;
    if (this.readOnly) {
      if (!this.warned) s.toast(t("dg.live.viewOnly"));
      this.warned = true;
      this.pull();
      return;
    }
    this.write(next, LOCAL);
    this.synced = next;
    if (Date.now() - this.stamped > 4000) {
      this.stamped = Date.now();
      this.meta.set("edited", { name: this.me.name, address: this.me.address, at: this.stamped });
    }
  }

  private write(next: string, origin: string): void {
    const edits = lineEdits(this.text.toString(), next);
    if (edits.length === 0) return;
    this.doc.transact(() => {
      for (const [at, del, ins] of edits) {
        if (del > 0) this.text.delete(at, del);
        if (ins) this.text.insert(at, ins);
      }
    }, origin);
  }

  private tidyLater(ms: number): void {
    window.clearTimeout(this.settle);
    this.settle = window.setTimeout(() => this.tidy(), ms);
  }

  /** The core's text back into the shared one, by one editor only. */
  private tidy(): void {
    if (this.readOnly || !this.joined) return;
    let lowest = this.doc.clientID;
    this.provider.awareness.getStates().forEach((st, client) => {
      if ((st as { write?: unknown }).write === true && client < lowest) lowest = client;
    });
    if (lowest !== this.doc.clientID) return;
    const mine = this.s.core.exportSetup().text;
    if (this.text.toString() !== mine) this.write(mine, TIDY);
    this.synced = mine;
  }

  /** Where this person's pointer is, for the others; null when it left. */
  point(p: { x: number; y: number } | null): void {
    const now = performance.now();
    if (p && now - this.pointerAt < 40) return;
    this.pointerAt = now;
    this.provider.awareness.setLocalStateField("pointer", p && { x: Math.round(p.x), y: Math.round(p.y) });
  }

  undoStep(): void {
    this.undo.undo();
  }

  redoStep(): void {
    this.undo.redo();
  }

  stop(): void {
    window.clearTimeout(this.settle);
    for (const f of this.off) f();
    this.undo.destroy();
    this.provider.destroy();
    this.doc.destroy();
    this.s.live = null;
    this.s.emit("people");
    this.s.emit("model");
  }
}

let current: LiveDiagram | null = null;

/** The diagram's live session, while there is one. */
export function live(): LiveDiagram | null {
  return current;
}

export function setLive(l: LiveDiagram | null): void {
  if (current && current !== l) current.stop();
  current = l;
}
