/* What a canvas needs to know about the world beyond the snapshot itself,
 * as pure functions of it: lookups, the state of a cable or a port, which
 * devices are network gear, the badge a LosOS device wears, the rooms of the
 * physical view and the camera arithmetic. The store answers its own
 * questions through the same Scene, and any canvas behind <LabCanvas> (the
 * SVG one in svg-canvas.tsx, a Bevy one later) can build one from its props
 * so the two cannot disagree. */

import type { Catalog, Device, Link, Snapshot } from "./core";

export type View = "logical" | "physical";
export interface Cam {
  x: number;
  y: number;
  k: number;
}
export type PortState = "up" | "wait" | "down" | "none";

/** A cable this young blinks amber, as a real port does while it negotiates. */
export const LINK_SETTLE_MS = 1400;

const GEAR = ["router", "switch", "bus", "ap", "internet"];
const CONSOLEABLE = ["box", "edge-local", "edge-official", "router", "switch", "ap"];

export class Scene {
  private devIndex: Map<string, Device>;

  constructor(
    readonly snap: Snapshot,
    readonly catalog: Catalog,
    /** link id → performance.now() when it was laid in this tab */
    readonly linkBorn: ReadonlyMap<string, number> = new Map(),
  ) {
    this.devIndex = new Map(snap.world.devices.map((d) => [d.id, d]));
  }

  dev(id: string | null | undefined): Device | undefined {
    return id ? this.devIndex.get(id) : undefined;
  }
  link(id: string): Link | undefined {
    return this.snap.world.links.find((l) => l.id === id);
  }
  name(id: string): string {
    return this.dev(id)?.name ?? id;
  }
  linksOf(id: string): Link[] {
    return this.snap.world.links.filter((l) => l.a.dev === id || l.b.dev === id);
  }
  isGear(d: Device): boolean {
    return GEAR.includes(d.type);
  }
  consoleable(d: Device): boolean {
    return CONSOLEABLE.includes(d.type);
  }
  linkUp(l: Link): boolean {
    return !!this.dev(l.a.dev)?.power && !!this.dev(l.b.dev)?.power;
  }
  linkState(l: Link, now = performance.now()): "up" | "wait" | "down" {
    if (!this.linkUp(l)) return "down";
    const born = this.linkBorn.get(l.id);
    if (born !== undefined && now - born < LINK_SETTLE_MS) return "wait";
    return "up";
  }
  portState(devId: string, port: string): PortState {
    const l = this.snap.world.links.find(
      (x) => (x.a.dev === devId && x.a.port === port) || (x.b.dev === devId && x.b.port === port),
    );
    return l ? this.linkState(l) : "none";
  }
  /** Ethernet ports with nothing in them, in the catalogue's order. */
  freePorts(devId: string): string[] {
    const d = this.dev(devId);
    const T = d ? this.catalog.types[d.type] : undefined;
    if (!d || !T) return [];
    return T.ports.filter(([p, k]) => k === "eth" && this.portState(devId, p) === "none").map(([p]) => p);
  }
  /** The address under a device's name in the logical view. */
  ipOf(d: Device): string | undefined {
    const a = this.snap.net.addr;
    return d.type === "router" ? (a[d.id + "#lan"]?.ip ?? a[d.id]?.ip) : a[d.id]?.ip;
  }

  /** The dot on a LosOS device: its path to an edge, an uplink, a certificate.
   *  `tone` is a house state (ok, warn, crit, accent, faint). */
  badge(d: Device): { tone: "ok" | "warn" | "crit" | "accent" | "faint"; title: string } | null {
    if (!d.power) return null;
    const L = this.snap.net.losos;
    if (d.type === "box") {
      const st = L.box[d.id];
      if (!st) return null;
      if (!st.up) return { tone: "faint", title: "No address" };
      if (st.tunnel === "refused") return { tone: "crit", title: "Tunnel refused: " + st.reason };
      if (!st.path) return { tone: "warn", title: "No edge in reach" };
      return {
        tone: st.path.source === "lan" ? "accent" : "ok",
        title: `Edge: ${this.name(st.path.id)}${st.publicName ? " · " + st.publicName : " · LAN only"}`,
      };
    }
    if (d.type === "edge-local") {
      const sp = L.spoke[d.id];
      if (!sp) return null;
      return sp.uplink === "up"
        ? { tone: "ok", title: "Uplink to the official edge is up" }
        : { tone: d.cfg["uplink"] ? "warn" : "faint", title: "Uplink: " + sp.uplink };
    }
    if (d.type === "edge-official") {
      return d.cfg["certified"]
        ? { tone: "ok", title: "Certificate signed by the LosOS root" }
        : { tone: "warn", title: "Not certified: not official" };
    }
    return null;
  }

  /** A bus is drawn as the cable it is: a backbone with each device dropping
   *  onto it at its own x. */
  busSpan(b: Device): { x1: number; x2: number } {
    const xs = this.linksOf(b.id).map((l) => this.dev(l.a.dev === b.id ? l.b.dev : l.a.dev)?.x ?? b.x);
    return { x1: Math.min(b.x - 120, ...xs) - 30, x2: Math.max(b.x + 120, ...xs) + 30 };
  }
  /** Where a cable meets a device in the logical view (a bus: under the other end). */
  anchor(d: Device, other: Device | undefined): { x: number; y: number } {
    if (d.type !== "bus" || !other) return d;
    const sp = this.busSpan(d);
    return { x: Math.max(sp.x1 + 20, Math.min(sp.x2 - 20, other.x)), y: d.y };
  }
}

// ── the physical view's floor plan ──────────────────────────────────────
/** Hardware is drawn 1.3× its nominal size. */
export const HWK = 1.3;
export const FLOOR = { width: 1240, height: 730 };
export const ROOMS: Record<string, { x: number; y: number; w: number; h: number }> = {
  home: { x: 20, y: 20, w: 590, h: 360 },
  office: { x: 630, y: 20, w: 590, h: 360 },
  isp: { x: 20, y: 400, w: 590, h: 310 },
  dc: { x: 630, y: 400, w: 590, h: 310 },
};

export function roomPos(d: Device): { x: number; y: number } {
  const R = ROOMS[d.site] ?? ROOMS["home"]!;
  return { x: R.x + (d.px ?? 0), y: R.y + (d.py ?? 0) };
}
/** The room a floor point is in, if any. */
export function roomAt(x: number, y: number): string | null {
  for (const [k, R] of Object.entries(ROOMS)) if (x >= R.x && x <= R.x + R.w && y >= R.y && y <= R.y + R.h) return k;
  return null;
}
/** A dropped device's spot: the room under its middle, kept inside it. */
export function dropSpot(d: Device, x: number, y: number): { site: string; px: number; py: number } {
  const site = roomAt(x + 40, y + 20) ?? d.site;
  const R = ROOMS[site] ?? ROOMS["home"]!;
  return {
    site,
    px: Math.max(0, Math.min(R.w - 60, Math.round(x - R.x))),
    py: Math.max(30, Math.min(R.h - 40, Math.round(y - R.y))),
  };
}

// ── the camera ──────────────────────────────────────────────────────────
export function zoomCam(c: Cam, mx: number, my: number, factor: number): Cam {
  const k = Math.max(0.3, Math.min(2.5, c.k * factor));
  return { k, x: mx - (mx - c.x) * (k / c.k), y: my - (my - c.y) * (k / c.k) };
}

/** The camera that shows the whole view in a viewport of this size. */
export function fitCam(view: View, devices: readonly Device[], width: number, height: number): Cam {
  if (view === "physical") {
    // The hint runs along the bottom edge and the zoom buttons down the right.
    const w = Math.max(120, width - 56);
    const h = Math.max(120, height - 44);
    const k = Math.min(w / FLOOR.width, h / FLOOR.height) * 0.98;
    return { k, x: (w - FLOOR.width * k) / 2, y: (h - FLOOR.height * k) / 2 };
  }
  if (devices.length === 0) return { x: 0, y: 0, k: 1 };
  const xs = devices.map((d) => d.x);
  const ys = devices.map((d) => d.y);
  const minX = Math.min(...xs) - 90;
  const maxX = Math.max(...xs) + 90;
  const minY = Math.min(...ys) - 70;
  const maxY = Math.max(...ys) + 80;
  // The legend sits along the bottom edge and the zoom buttons down the
  // right one: keep the devices clear of both.
  const h = Math.max(120, height - 44);
  const w = Math.max(120, width - 56);
  const k = Math.min(1.25, Math.min(w / (maxX - minX), h / (maxY - minY)));
  return { k, x: (w - (maxX - minX) * k) / 2 - minX * k, y: (h - (maxY - minY) * k) / 2 - minY * k };
}
