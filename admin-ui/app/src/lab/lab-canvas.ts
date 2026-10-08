/* The boundary between the Lab's React UI and whatever draws its canvas.
 *
 * Two implementations are meant to sit behind these props: the SVG canvas
 * (svg-canvas.tsx), and a Bevy one (Rust to wasm on WebGL2, the crate
 * admin-ui/lab/render) that is being built. The SVG one stays as the
 * fallback where WebGL2 is missing, and can paint first while the Bevy
 * module loads. So the contract is data in, intents out:
 *
 *   in   the snapshot and catalogue (the world), this frame's packets, the
 *        view, the tool, the selection, the camera, the theme and its
 *        resolved palette, which devices run a real guest
 *   out  select, hover, move, place, connect (with the port when the canvas
 *        knows which one was clicked), delete, power, camera, viewport size
 *
 * Every interaction state (tool, selection, the camera and its fit) lives in
 * React (store.ts) and comes in as props, so switching implementations keeps
 * it. A canvas holds only what a gesture needs while the pointer is down: a
 * drag's origin, the rubber band's end. It never calls the core or the
 * store; the host (canvas.tsx) turns each intent into an action. Geometry
 * both canvases need (rooms, the camera fit, a dropped device's room) is in
 * scene.ts, so they cannot disagree. */

import type { ComponentType } from "react";
import type { Catalog, Packet, Snapshot } from "./core";
import type { Cam, View } from "./scene";

export type CanvasTool =
  | { kind: "select" }
  | { kind: "delete" }
  /** Placing a device of `type` at the next click. */
  | { kind: "place"; type: string }
  /** Laying a `linkKind` cable; `from` is the first end once it is chosen. */
  | { kind: "connect"; linkKind: string; from: string | null };

export type CanvasTarget = { kind: "dev" | "link"; id: string };

/** Where a placed device goes: a logical position, or a spot in a room. */
export type PlaceAt = { view: "logical"; x: number; y: number } | { view: "physical"; site: string; px: number; py: number };

/** Where a dragged device is now; `done` on the drop. */
export type MoveTo = { view: "logical"; x: number; y: number } | { view: "physical"; site: string; px: number; py: number };

/** The house tokens the canvas draws with, resolved for the current theme
 *  (a WebGL canvas cannot read CSS variables). Keys are the token names
 *  without the leading dashes: "surface", "ink", "accent", "lab-copper", … */
export type Palette = Readonly<Record<string, string>>;

export interface LabCanvasProps {
  snapshot: Snapshot;
  catalog: Catalog;
  /** Packets in flight this frame, as tick() hands them out (logical view). */
  packets: readonly Packet[];
  view: View;
  tool: CanvasTool;
  selection: CanvasTarget | null;
  camera: Cam;
  theme: "light" | "dark";
  palette: Palette;
  /** Devices whose guest runs (a VM tag on the logical node). */
  running: ReadonlySet<string>;
  /** Cables laid in this tab and when (performance.now()); young ones blink. */
  linkBorn: ReadonlyMap<string, number>;

  onSelect(target: CanvasTarget | null, how: { tap: boolean }): void;
  onHover?(target: CanvasTarget | null): void;
  onMove(id: string, to: MoveTo, done: boolean): void;
  /** `keep`: the tool stays (shift held), to place or wire several. */
  onPlace(at: PlaceAt, keep: boolean): void;
  /** A device clicked with the connect tool; `port` when the canvas could
   *  tell which port was clicked (the physical view's drawings). */
  onConnect(id: string, opts: { port?: string; keep: boolean }): void;
  onDelete(target: CanvasTarget): void;
  /** A click on a device's power switch. */
  onPower(id: string): void;
  onCamera(cam: Cam): void;
  onViewport(size: { width: number; height: number }): void;
}

export type LabCanvasComponent = ComponentType<LabCanvasProps>;
