/* The Lab page's state: the core, its last snapshot, and the UI's own state
 * (view, tool, selection, cameras, the event list), with a pub-sub per kind
 * of change so the per-frame traffic redraws only the packets and the bar.
 *
 * The world itself lives in the core. Every edit goes through a core call
 * and then `refresh()`, which re-reads the snapshot; a drag moves the device
 * in the local copy at once and tells the core with move_device. */

import { useSyncExternalStore } from "react";
import {
  type Catalog,
  Core,
  type Device,
  type Err,
  type HttpResult,
  isErr,
  type LabEvent,
  type Link,
  type Packet,
  type Snapshot,
  type TickResult,
} from "./core";
import type { Ordering } from "./order";
import type { GpuState } from "./render";
import { type Cam, fitCam, Scene, type View, zoomCam } from "./scene";

export type { Cam, View } from "./scene";
export type Tool = "select" | "place" | "connect" | "delete";
export type Tab = "status" | "config" | "physical" | "console" | "desktop";
export type Sel = { kind: "dev" | "link"; id: string } | null;
export type Wide = "none" | "console" | "desk";
export interface Ui {
  view: View;
  tool: Tool;
  placeType: string | null;
  linkKind: string | null;
  connectFrom: string | null;
  /** The port on `connectFrom` the cable goes into; "" lets the core pick. */
  connectPort: string;
  sel: Sel;
  tab: Tab;
  cat: string;
  folded: boolean;
  wide: Wide;
  /** On a phone the inspector is a sheet; this says whether it is open. */
  sheet: boolean;
}

export interface Frame {
  packets: Packet[];
  status: string;
  clockText: string;
  inFlight: number;
  playing: boolean;
  frameFrac: number;
}

/** A picker entry for what is open from a file or from storage. */
export interface ExtraOption {
  key: "file" | "last";
  name: string;
}

export interface DeskState {
  app: "danube" | "term";
  open: ("danube" | "term")[];
  url: string;
  html: string;
  history: string[];
  loading: boolean;
  job: number | null;
  signedIn: boolean;
  err: boolean;
  notes: string;
}

type Channel = "model" | "frame" | "events" | "cam";

export type ConsoleSink = (lines: string[], done: boolean) => void;

const EVENT_CAP = 600;

export class LabStore {
  readonly core: Core;
  readonly catalog: Catalog;
  snap: Snapshot;
  ui: Ui = {
    view: "logical",
    tool: "select",
    placeType: null,
    linkKind: null,
    connectFrom: null,
    connectPort: "",
    sel: null,
    tab: "status",
    cat: "losos",
    folded: false,
    wide: "none",
    sheet: false,
  };
  /** Set by the layout: below 900 px the inspector is a sheet. */
  phone = false;
  cams: Record<View, Cam> = { logical: { x: 0, y: 0, k: 1 }, physical: { x: 0, y: 0, k: 1 } };
  frame: Frame = { packets: [], status: "", clockText: "", inFlight: 0, playing: false, frameFrac: 0 };
  events: LabEvent[] = [];
  extras: ExtraOption[] = [];
  /** Scenario keys, "this-box" first once it is there. */
  scenarios: Catalog["scenarios"];
  /** Set when "This box" could not be read: why, for the greyed entry. */
  thisBoxProblem: string | null = null;
  /** Whether this box lets the Lab order hardware, and what the edge sells. */
  ordering: Ordering | null = null;
  /** The text of the file opened in this tab, so the picker can go back to it. */
  openedText: { text: string; fileName: string } | null = null;
  linkBorn = new Map<string, number>();
  desks = new Map<string, DeskState>();
  /** Simulated consoles subscribe here by device; output of a busy job lands in them. */
  consoleSinks = new Map<string, ConsoleSink>();
  /** Bumped when consoles must forget their scrollback (a new setup). */
  consoleEpoch = 0;
  /** Consoles that print the power-on banner or the power-off note. */
  consolePower = new Map<string, (on: boolean, banner: string[]) => void>();
  /** The GPU canvas (render.ts): whether it is loading, drawing or not used. */
  gpu: GpuState = { state: "svg", backend: null, mod: null, lab: null, failed: null };
  /** Called with every toast the core or the store raises. */
  onToast: (text: string) => void = () => {};
  /** Called after every snapshot; the engine reconciles its guests there. */
  onRefresh: () => void = () => {};

  private version: Record<Channel, number> = { model: 0, frame: 0, events: 0, cam: 0 };
  private listeners: Record<Channel, Set<() => void>> = {
    model: new Set(),
    frame: new Set(),
    events: new Set(),
    cam: new Set(),
  };
  /** The snapshot with its lookups; rebuilt on every refresh. */
  scene: Scene;

  constructor(core: Core) {
    this.core = core;
    this.catalog = core.catalog();
    this.scenarios = this.catalog.scenarios;
    this.snap = core.snapshot();
    this.scene = new Scene(this.snap, this.catalog, this.linkBorn);
  }

  // ── pub-sub ────────────────────────────────────────────────────────────
  on(channel: Channel, fn: () => void): () => void {
    this.listeners[channel].add(fn);
    return () => this.listeners[channel].delete(fn);
  }
  emit(channel: Channel): void {
    this.version[channel] += 1;
    for (const fn of this.listeners[channel]) fn();
  }
  versionOf(channel: Channel): number {
    return this.version[channel];
  }

  // ── reading (the Scene's answers) ──────────────────────────────────────
  private index(): void {
    this.scene = new Scene(this.snap, this.catalog, this.linkBorn);
  }
  dev(id: string | null | undefined): Device | undefined {
    return this.scene.dev(id);
  }
  link(id: string): Link | undefined {
    return this.scene.link(id);
  }
  name(id: string): string {
    return this.scene.name(id);
  }
  linksOf(id: string): Link[] {
    return this.scene.linksOf(id);
  }
  isGear(d: Device): boolean {
    return this.scene.isGear(d);
  }
  consoleable(d: Device): boolean {
    return this.scene.consoleable(d);
  }
  linkUp(l: Link): boolean {
    return this.scene.linkUp(l);
  }
  linkState(l: Link): "up" | "wait" | "down" {
    return this.scene.linkState(l);
  }
  freePorts(devId: string): string[] {
    return this.scene.freePorts(devId);
  }

  // ── writing ────────────────────────────────────────────────────────────
  /** Re-read the world after the core changed it. */
  refresh(): void {
    this.snap = this.core.snapshot();
    this.index();
    // A selection whose device or cable is gone is dropped.
    const sel = this.ui.sel;
    if (sel && (sel.kind === "dev" ? !this.dev(sel.id) : !this.link(sel.id))) this.ui.sel = null;
    if (this.ui.view === "physical") this.placePhysical();
    this.onRefresh();
    this.emit("model");
  }

  setUi(patch: Partial<Ui>): void {
    this.ui = { ...this.ui, ...patch };
    this.emit("model");
  }

  setGpu(patch: Partial<GpuState>): void {
    this.gpu = { ...this.gpu, ...patch };
    this.emit("model");
  }

  toast(text: string): void {
    this.onToast(text);
  }

  /** A core answer: an error is a toast and `null`, anything else is passed on. */
  ok<T>(answer: T | Err): T | null {
    if (isErr(answer)) {
      this.toast(answer.error);
      return null;
    }
    return answer;
  }

  desk(id: string): DeskState {
    let s = this.desks.get(id);
    if (!s) {
      s = { app: "danube", open: ["danube"], url: "", html: "", history: [], loading: false, job: null, signedIn: false, err: false, notes: "" };
      this.desks.set(id, s);
    }
    return s;
  }

  /** A fresh world was loaded: consoles, desktops and cameras start over. */
  startedWorld(focus: string | null | undefined): void {
    this.desks.clear();
    this.consoleEpoch += 1;
    this.linkBorn.clear();
    this.events = [];
    this.emit("events");
    this.refresh();
    const d = this.dev(focus ?? null);
    this.ui = {
      ...this.ui,
      tool: "select",
      placeType: null,
      linkKind: null,
      connectFrom: null,
      sel: d ? { kind: "dev", id: d.id } : null,
      tab: d?.type === "laptop" ? "desktop" : "status",
    };
    this.placePhysical();
    this.fit();
    this.emit("model");
  }

  /** Devices without a spot in the physical view get one from the core. */
  placePhysical(): void {
    const unplaced = this.snap.world.devices.filter((d) => d.px === null || d.py === null);
    if (unplaced.length === 0) return;
    for (const d of unplaced) {
      const p = this.core.physPos(d.id);
      d.px = p.px;
      d.py = p.py;
    }
  }

  // ── the camera ─────────────────────────────────────────────────────────
  /** The canvas's size, measured by the canvas component. */
  viewport = { width: 0, height: 0 };

  fit(): void {
    const { width, height } = this.viewport;
    if (!width) return;
    const view = this.ui.view;
    this.cams[view] = fitCam(view, this.snap.world.devices, width, height);
    this.emit("cam");
  }

  zoomAt(mx: number, my: number, factor: number): void {
    this.setCam(zoomCam(this.cams[this.ui.view], mx, my, factor));
  }

  setCam(cam: Cam): void {
    this.cams[this.ui.view] = cam;
    this.emit("cam");
  }

  // ── time ───────────────────────────────────────────────────────────────
  /** Apply what a tick() or step() handed out. */
  absorb(r: TickResult, httpDone: (dev: string, job: number, url: string, result: HttpResult) => void): void {
    const prev = this.frame;
    this.frame = {
      packets: r.packets,
      status: r.sim.status,
      clockText: r.clock.text,
      inFlight: r.sim.inFlight,
      playing: r.sim.playing,
      frameFrac: r.sim.frameFrac,
    };
    // Nothing moving and the same words on the bar: no redraw this frame.
    const still =
      prev.packets.length === 0 &&
      r.packets.length === 0 &&
      prev.status === r.sim.status &&
      prev.clockText === r.clock.text;
    const modeChanged = r.sim.mode !== this.snap.sim.mode || r.sim.playing !== this.snap.sim.playing;
    if (r.reset) this.events = [];
    if (r.events.length > 0) {
      this.events = this.events.concat(r.events);
      if (this.events.length > EVENT_CAP) this.events = this.events.slice(-EVENT_CAP);
    }
    if (r.reset || r.events.length > 0) this.emit("events");
    for (const c of r.console) this.consoleSinks.get(c.dev)?.(c.lines, c.done);
    for (const h of r.http) httpDone(h.dev, h.job, h.url, h.result);
    for (const text of r.toasts) this.toast(text);
    if (r.message) this.toast(r.message);
    if (r.worldChanged || modeChanged) this.refresh();
    else if (r.clock.speed !== this.snap.clock.speed) this.refresh();
    if (!still) this.emit("frame");
  }
}

let current: LabStore | null = null;

export function setStore(store: LabStore): void {
  current = store;
}

export function store(): LabStore {
  if (current === null) throw new Error("the Lab store is not loaded yet");
  return current;
}

/** Subscribe the caller to one kind of change and hand back the store. */
export function useLab(channel: Channel = "model"): LabStore {
  const s = store();
  useSyncExternalStore(
    (fn) => s.on(channel, fn),
    () => s.versionOf(channel),
  );
  return s;
}
