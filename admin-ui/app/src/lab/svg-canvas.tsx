/* The SVG canvas behind <LabCanvas> (lab-canvas.ts): the logical view (the
 * devices and cables as a diagram, packets riding them) and the physical
 * view (rooms, hardware, cables with their lengths). Ported from app.js's
 * renderCanvas / onDown / onMove / onUp.
 *
 * Props in, intents out: it reads nothing from the store and calls no
 * action. While a pointer is down it keeps the gesture (a drag's origin, a
 * pan's start, the dragged device's position) to itself and reports it with
 * onMove / onCamera; everything else comes back as props. The world layers
 * are memoised on the snapshot, so a frame that only moves packets redraws
 * the packet layer alone.
 *
 * Colours are classes in lab.css or presentation attributes naming house
 * tokens; nothing writes a style attribute (the page's style-src 'self'). */

import * as React from "react";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import type { Device, Link, Packet } from "./core";
import { hwDrawing, IconGlyph } from "./icons";
import type { CanvasTarget, LabCanvasProps } from "./lab-canvas";
import { dropSpot, HWK, roomAt, roomPos, ROOMS, Scene, zoomCam } from "./scene";

/* The box a node's name and address take below its icon, relative to the
 * node's centre, from the label sizes in lab.css (12px name, 10.5px mono). */
function labelBox(scene: Scene, d: Device): { x1: number; x2: number; y1: number; y2: number } {
  const T = scene.catalog.types[d.type];
  const ip = scene.ipOf(d);
  const sub = !d.power ? "off" : ip ? ip : T?.endpoint ? "no address" : "";
  const w = Math.max(d.name.length * 7.2, sub.length * 6.4) + 10;
  return { x1: -w / 2, x2: w / 2, y1: 26, y2: sub ? 57 : 43 };
}
// The icon and the hover/selection halo drawn round it (88×98 from -44,-36):
// a mark inside it would be cut by the halo's edge the moment the device is picked.
const ICON_BOX = { x1: -47, x2: 47, y1: -39, y2: 65 };

function hits(b: { x1: number; x2: number; y1: number; y2: number }, x: number, y: number, rx: number, ry: number): boolean {
  return x + rx > b.x1 && x - rx < b.x2 && y + ry > b.y1 && y - ry < b.y2;
}

function ledClass(st: string): string {
  return "led " + (st === "up" ? "up" : st === "wait" ? "wait" : "down");
}

function Logical({
  scene,
  selection,
  running,
  pos,
}: {
  scene: Scene;
  selection: CanvasTarget | null;
  running: ReadonlySet<string>;
  /** A device being dragged, at where the pointer has it. */
  pos: { id: string; x: number; y: number } | null;
}) {
  const at = (d: Device): Device => (pos && pos.id === d.id ? { ...d, x: pos.x, y: pos.y } : d);
  const devices = scene.snap.world.devices.map(at);
  const view = pos ? new Scene({ ...scene.snap, world: { ...scene.snap.world, devices } }, scene.catalog, scene.linkBorn) : scene;
  const marks = placeMarks(view);
  return (
    <>
      <rect x={-4000} y={-4000} width={8000} height={8000} fill="url(#lab-grid)" />
      <g>
        {devices
          .filter((d) => d.type === "bus")
          .map((b) => {
            const sp = view.busSpan(b);
            return (
              <g key={"bus" + b.id}>
                <line x1={sp.x1} y1={b.y} x2={sp.x2} y2={b.y} className={cn("busline", !b.power && "off")} />
                {[sp.x1, sp.x2].map((x) => (
                  <rect key={x} x={x - 5} y={b.y - 9} width={10} height={18} rx={2.5} className="busterm" />
                ))}
              </g>
            );
          })}
        {view.snap.world.links.map((l) => (
          <LogicalLink key={l.id} scene={view} l={l} marks={marks.get(l.id)} selected={selection?.kind === "link" && selection.id === l.id} />
        ))}
      </g>
      <g>
        {devices.map((d) => {
          const T = scene.catalog.types[d.type];
          const ip = view.ipOf(d);
          const badge = view.badge(d);
          const isSel = selection?.kind === "dev" && selection.id === d.id;
          return (
            <g
              key={d.id}
              className={cn("node", isSel && "sel")}
              data-dev={d.id}
              transform={`translate(${d.x} ${d.y})`}
              tabIndex={0}
              role="button"
              aria-label={`${d.name}, ${T?.label ?? d.type}`}
            >
              {(() => {
                // The halo grows with a long name or address so its edge never cuts the text.
                const b = labelBox(view, d);
                const hx = Math.min(-44, b.x1 - 4);
                return (
                  <>
                    <rect x={hx} y={-36} width={-2 * hx} height={98} rx={10} className="halo" />
                    <g transform="translate(-32 -32)" opacity={d.power ? 1 : 0.45}>
                      <IconGlyph type={d.type} />
                    </g>
                    <rect x={b.x1} y={b.y1} width={b.x2 - b.x1} height={b.y2 - b.y1} rx={4} className="label-plate" />
                  </>
                );
              })()}
              <text y={38} textAnchor="middle" className="label">
                {d.name}
              </text>
              <text y={52} textAnchor="middle" className="sublabel">
                {!d.power ? "off" : ip ? ip : T?.endpoint ? "no address" : ""}
              </text>
              {badge && (
                <g transform="translate(22 -30)">
                  <circle r={8} className="badge-bg" />
                  <circle r={4.5} fill={`var(--${badge.tone})`} />
                  <title>{badge.title}</title>
                </g>
              )}
              {running.has(d.id) && (
                <g transform="translate(-40 -32)">
                  <rect width={26} height={14} rx={3} className="vm-tag" />
                  <text x={13} y={10.5} textAnchor="middle" className="vm-tag-text">
                    VM
                  </text>
                </g>
              )}
            </g>
          );
        })}
      </g>
    </>
  );
}

/** Where a cable end's LED and port name go, in canvas units; null when
 *  the cable is too short to keep that mark clear of the device. */
interface EndMarks {
  led: Pt | null;
  label: Pt | null;
}

/** The LED and port-name spots for every cable end. Each device collects the
 *  marks already placed round it, so two cables leaving a switch side by side
 *  never stack their port names. */
function placeMarks(scene: Scene): Map<string, [EndMarks, EndMarks]> {
  const taken = new Map<string, Box[]>();
  const out = new Map<string, [EndMarks, EndMarks]>();
  const free = (id: string, x: number, y: number, rx: number, ry: number): boolean =>
    !(taken.get(id) ?? []).some((o) => hits(o, x, y, rx, ry));
  const take = (id: string, x: number, y: number, rx: number, ry: number): void => {
    const list = taken.get(id) ?? [];
    list.push({ x1: x - rx, x2: x + rx, y1: y - ry, y2: y + ry });
    taken.set(id, list);
  };
  for (const l of scene.snap.world.links) {
    const da = scene.dev(l.a.dev);
    const db = scene.dev(l.b.dev);
    if (!da || !db) continue;
    const A = scene.anchor(da, db);
    const B = scene.anchor(db, da);
    const len = Math.hypot(B.x - A.x, B.y - A.y) || 1;
    const ux = (B.x - A.x) / len;
    const uy = (B.y - A.y) / len;
    const half = len / 2 - 6;
    const ends = ([
      [A, l.a, 1, da],
      [B, l.b, -1, db],
    ] as const).map(([P, end, sg, d]): EndMarks => {
      if (d.type === "bus") return { led: { x: P.x, y: P.y }, label: null };
      const box = labelBox(scene, d);
      const vx = ux * sg;
      const vy = uy * sg;
      const clear = (x: number, y: number, rx: number, ry: number): boolean =>
        !hits(box, x, y, rx, ry) && !hits(ICON_BOX, x, y, rx, ry) && free(d.id, x, y, rx, ry);
      let led: number | null = null;
      for (let t = 40; t <= half; t += 4) {
        if (clear(vx * t, vy * t, 5, 5)) {
          led = t;
          break;
        }
      }
      if (led === null) return { led: null, label: null };
      take(d.id, vx * led, vy * led, 5, 5);
      // A port name is about 6px a letter and one line of 9.5px mono. It sits
      // beside the cable, as far off it as its box reaches across the cable's
      // normal, so it never covers the line, the LED or a packet on it.
      const tx = end.port.length * 3 + 3;
      const off = Math.abs(uy) * tx + Math.abs(ux) * 6 + 6;
      let label: Pt | null = null;
      search: for (let t = led; t <= half; t += 4) {
        for (const side of [1, -1]) {
          const x = vx * t - vy * off * side;
          const y = vy * t + vx * off * side;
          if (clear(x, y, tx + 2, 7)) {
            take(d.id, x, y, tx + 2, 7);
            label = { x: P.x + x, y: P.y + y };
            break search;
          }
        }
      }
      return { led: { x: P.x + vx * led, y: P.y + vy * led }, label };
    });
    out.set(l.id, [ends[0]!, ends[1]!]);
  }
  return out;
}

function LogicalLink({ scene, l, marks, selected }: { scene: Scene; l: Link; marks: [EndMarks, EndMarks] | undefined; selected: boolean }) {
  const da = scene.dev(l.a.dev);
  const db = scene.dev(l.b.dev);
  if (!da || !db || !marks) return null;
  const A = scene.anchor(da, db);
  const B = scene.anchor(db, da);
  const st = scene.linkState(l);
  return (
    <g>
      <line x1={A.x} y1={A.y} x2={B.x} y2={B.y} className={cn("link", l.kind, selected && "sel")} />
      <line x1={A.x} y1={A.y} x2={B.x} y2={B.y} className="link-hit" data-link={l.id}>
        <title>{`${da.name} ${l.a.port} ↔ ${db.name} ${l.b.port} (${l.kind})`}</title>
      </line>
      {marks.map((m, i) => (
        <g key={i}>
          {m.led && <circle cx={m.led.x} cy={m.led.y} r={(i === 0 ? da : db).type === "bus" ? 4 : 4.5} className={ledClass(st)} />}
          {m.label && (
            <text x={m.label.x} y={m.label.y + 3} className="portlabel" textAnchor="middle">
              {(i === 0 ? l.a : l.b).port}
            </text>
          )}
        </g>
      ))}
    </g>
  );
}

type Box = { x1: number; x2: number; y1: number; y2: number };
type Pt = { x: number; y: number };
const overlaps = (p: Box, q: Box): boolean => p.x1 < q.x2 && q.x1 < p.x2 && p.y1 < q.y2 && q.y1 < p.y2;
/** True when the box crosses the room's wall: partly in, partly out. */
function straddles(R: { x: number; y: number; w: number; h: number }, b: Box): boolean {
  const room = { x1: R.x, x2: R.x + R.w, y1: R.y, y2: R.y + R.h };
  const inside = b.x1 >= room.x1 && b.x2 <= room.x2 && b.y1 >= room.y1 && b.y2 <= room.y2;
  return overlaps(room, b) && !inside;
}

/** A port's position on the floor, from the device's hardware drawing. */
function portAbs(d: Device, port: string): { x: number; y: number } {
  const p = roomPos(d);
  const hw = hwDrawing(d, () => "none");
  const pp = hw.ports[port] ?? [hw.w / 2, hw.h];
  return { x: p.x + pp[0] * HWK, y: p.y + pp[1] * HWK };
}

function Physical({
  scene,
  selection,
  pos,
}: {
  scene: Scene;
  selection: CanvasTarget | null;
  pos: { id: string; px: number; py: number } | null;
}) {
  const devices = scene.snap.world.devices.map((d) => (pos && pos.id === d.id ? { ...d, px: pos.px, py: pos.py } : d));
  const byId = new Map(devices.map((d) => [d.id, d]));
  const placed = devices.filter((d) => d.px !== null && d.py !== null);
  // What a cable's length label must not cover: every device, its name, and
  // the labels already placed. Rooms are checked separately (no straddling).
  const taken: Box[] = placed.flatMap((d) => {
    const p = roomPos(d);
    const hw = hwDrawing(d, () => "none");
    const w = hw.w * HWK;
    const h = hw.h * HWK;
    const side = scene.isGear(d) || ((d.site === "office" || d.site === "dc") && (d.px ?? 0) < 190);
    const nameW = d.name.length * 7.4;
    const name: Box = side
      ? { x1: p.x + w + 4, x2: p.x + w + 12 + nameW, y1: p.y + h / 2 - 10, y2: p.y + h / 2 + 8 }
      : { x1: p.x + w / 2 - nameW / 2 - 4, x2: p.x + w / 2 + nameW / 2 + 4, y1: p.y + h + 2, y2: p.y + h + 20 };
    return [{ x1: p.x - 6, x2: p.x + w + 6, y1: p.y - 6, y2: p.y + h + 6 }, name];
  }).concat(Object.values(ROOMS).map((R) => ({ x1: R.x + 8, x2: R.x + 190, y1: R.y + 6, y2: R.y + 38 })));
  const meterAt = (a: Pt, b: Pt, sag: number, text: string): Pt => {
    const half = text.length * 3.1 + 3;
    const at = (t: number, dy: number): Pt => {
      const u = 1 - t;
      const x = u * u * u * a.x + 3 * u * u * t * a.x + 3 * u * t * t * b.x + t * t * t * b.x;
      const y = u * u * u * a.y + 3 * u * u * t * (a.y + sag) + 3 * u * t * t * (b.y + sag) + t * t * t * b.y;
      return { x, y: y + dy };
    };
    for (const t of [0.5, 0.42, 0.58, 0.34, 0.66, 0.26, 0.74, 0.18, 0.82]) {
      for (const dy of [12, -5]) {
        const p = at(t, dy);
        const box = { x1: p.x - half, x2: p.x + half, y1: p.y - 9, y2: p.y + 3 };
        if (taken.some((o) => overlaps(o, box))) continue;
        if (Object.values(ROOMS).some((R) => straddles(R, box))) continue;
        taken.push(box);
        return p;
      }
    }
    return at(0.5, 12);
  };
  return (
    <>
      <rect x={-4000} y={-4000} width={8000} height={8000} className="ground" />
      {Object.entries(ROOMS).map(([k, R]) => (
        <g key={k}>
          <rect x={R.x} y={R.y} width={R.w} height={R.h} rx={6} className="room" data-room={k} />
          <text x={R.x + 14} y={R.y + 20} className="room-title">
            {scene.catalog.sites[k]?.name}
          </text>
          <text x={R.x + 14} y={R.y + 33} className="room-sub">
            {scene.catalog.sites[k]?.sub}
          </text>
          <Furniture k={k} R={R} />
        </g>
      ))}
      <g>
        {placed
          .filter((d) => d.type === "ap" && d.power)
          .map((d) => {
            const p = roomPos(d);
            return <circle key={d.id} cx={p.x + 30 * HWK} cy={p.y + 22 * HWK} r={125} className="range" />;
          })}
        {scene.snap.world.links.map((l) => {
          const A = byId.get(l.a.dev);
          const B = byId.get(l.b.dev);
          if (!A || !B || A.px === null || B.px === null) return null;
          const a = portAbs(A, l.a.port);
          const b = portAbs(B, l.b.port);
          const isSel = selection?.kind === "link" && selection.id === l.id;
          if (l.kind === "wifi") {
            return <path key={l.id} d={`M${a.x} ${a.y}L${b.x} ${b.y}`} className={cn("link wifi", isSel && "sel")} fill="none" />;
          }
          const dist = Math.hypot(b.x - a.x, b.y - a.y);
          const same = A.site === B.site;
          const sag = same ? 30 + dist * 0.18 : 40;
          const d = `M${a.x} ${a.y}C${a.x} ${a.y + sag} ${b.x} ${b.y + sag} ${b.x} ${b.y}`;
          const meters = same
            ? `${Math.max(0.5, dist * 0.02).toFixed(1)} m ${l.kind === "fiber" ? "fiber" : "Cat6"}`
            : A.site === "dc" || B.site === "dc"
              ? "datacenter uplink"
              : "ISP line";
          const m = meterAt(a, b, sag, meters);
          return (
            <g key={l.id}>
              <path d={d} className={cn("cable", same ? (l.kind === "wan" ? "copper" : l.kind) : "wan", isSel && "sel")} />
              <path d={d} className="link-hit" data-link={l.id}>
                <title>{`${A.name} ${l.a.port} ↔ ${B.name} ${l.b.port}`}</title>
              </path>
              <text x={m.x} y={m.y} className="meters" textAnchor="middle">
                {meters}
              </text>
            </g>
          );
        })}
      </g>
      <g>
        {placed.map((d) => {
          const p = roomPos(d);
          const hw = hwDrawing(d, (port) => scene.portState(d.id, port));
          const isSel = selection?.kind === "dev" && selection.id === d.id;
          const T = scene.catalog.types[d.type];
          const gear = scene.isGear(d);
          const racked = (d.site === "office" || d.site === "dc") && (d.px ?? 0) < 190;
          const side = racked || gear;
          const labelX = racked && !gear ? 184 - (d.px ?? 0) + 30 : side ? hw.w * HWK + 8 : (hw.w * HWK) / 2;
          return (
            <g
              key={d.id}
              className={cn("hw", isSel && "sel")}
              data-dev={d.id}
              transform={`translate(${p.x} ${p.y})`}
              tabIndex={0}
              role="button"
              aria-label={`${d.name}, ${T?.label ?? d.type}`}
            >
              <rect x={-5} y={-5} width={hw.w * HWK + 10} height={hw.h * HWK + 10} rx={6} className="hw-halo" />
              <g transform={`scale(${HWK})`}>{hw.node}</g>
              <text x={labelX} y={side ? (hw.h * HWK) / 2 + 4 : hw.h * HWK + 15} textAnchor={side ? undefined : "middle"} className="label">
                {d.name}
              </text>
            </g>
          );
        })}
      </g>
    </>
  );
}

function Furniture({ k, R }: { k: string; R: { x: number; y: number; w: number; h: number } }) {
  if (k === "home") {
    return (
      <>
        <rect x={R.x + 20} y={R.y + 108} width={300} height={8} rx={2} className="furn" />
        <rect x={R.x + 20} y={R.y + R.h - 46} width={R.w - 40} height={12} rx={3} className="furn" />
        <rect x={R.x + 40} y={R.y + R.h - 34} width={8} height={30} className="furn" />
        <rect x={R.x + R.w - 48} y={R.y + R.h - 34} width={8} height={30} className="furn" />
        <text x={R.x + 24} y={R.y + 130} className="room-sub">
          hallway shelf
        </text>
        <text x={R.x + 24} y={R.y + R.h - 52} className="room-sub">
          desk
        </text>
      </>
    );
  }
  if (k === "office" || k === "dc") {
    const units = k === "office" ? 12 : 15;
    const h = k === "office" ? 196 : 250;
    return (
      <>
        <rect x={R.x + 22} y={R.y + 38} width={184} height={h} rx={3} className="rack" />
        {Array.from({ length: units }, (_, u) => (
          <line key={u} x1={R.x + 28} x2={R.x + 200} y1={R.y + 46 + u * 15.5} y2={R.y + 46 + u * 15.5} className="rack-u" />
        ))}
        {k === "office" ? (
          <>
            <text x={R.x + 26} y={R.y + 250} className="room-sub">
              network cabinet, 12U
            </text>
            <rect x={R.x + 220} y={R.y + R.h - 46} width={R.w - 240} height={12} rx={3} className="furn" />
            <text x={R.x + 224} y={R.y + R.h - 52} className="room-sub">
              desks
            </text>
          </>
        ) : (
          <text x={R.x + 220} y={R.y + 60} className="room-sub">
            rented VPS, public IPv4, 1 Gbit/s
          </text>
        )}
      </>
    );
  }
  return null;
}

/* Envelopes riding the logical links: the one layer that changes per frame. */
function Packets({ packets }: { packets: readonly Packet[] }) {
  return (
    <g>
      {packets.map((p, i) => (
        <g key={`${p.flow}-${i}`} className="pdu" transform={`translate(${p.x - 11} ${p.y - 8})`} data-proto={p.proto}>
          <rect width={22} height={15} rx={2.5} className="pdu-body" strokeWidth={1.5} />
          <path d="M1.5 2L11 9l9.5-7" className="pdu-flap" fill="none" strokeWidth={1.4} />
          {p.real && <circle cx={20} cy={1} r={3} fill="var(--ok)" />}
        </g>
      ))}
    </g>
  );
}

interface Drag {
  kind: "node" | "pan";
  id?: string;
  sx: number;
  sy: number;
  ox: number;
  oy: number;
  moved: boolean;
}

/** The port of `d` under a floor point, if the pointer is on one. */
function portAt(d: Device, x: number, y: number): string | undefined {
  const hw = hwDrawing(d, () => "none");
  let best: string | undefined;
  let bestD = 9;
  for (const port of Object.keys(hw.ports)) {
    const p = portAbs(d, port);
    const dist = Math.hypot(p.x - x, p.y - y);
    if (dist < bestD) {
      best = port;
      bestD = dist;
    }
  }
  return best;
}

export function SvgCanvas(props: LabCanvasProps) {
  const { snapshot, catalog, linkBorn, view, tool, selection, camera: cam, running, packets } = props;
  const t = useT();
  const wrap = React.useRef<HTMLDivElement>(null);
  const svgRef = React.useRef<SVGSVGElement>(null);
  const rubber = React.useRef<SVGPathElement>(null);
  const drag = React.useRef<Drag | null>(null);
  const [dragPos, setDragPos] = React.useState<{ id: string; x: number; y: number } | null>(null);
  // The latest props, for the listeners attached once below.
  const live = React.useRef(props);
  live.current = props;

  const scene = React.useMemo(() => new Scene(snapshot, catalog, linkBorn), [snapshot, catalog, linkBorn]);

  // The canvas's size, reported on every change; the host fits the camera.
  React.useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      live.current.onViewport({ width: r.width, height: r.height });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Wheel zoom wants a non-passive listener, which React's onWheel is not.
  React.useEffect(() => {
    const el = svgRef.current;
    if (!el) return;
    const onWheel = (ev: WheelEvent) => {
      ev.preventDefault();
      const r = el.getBoundingClientRect();
      const p = live.current;
      p.onCamera(zoomCam(p.camera, ev.clientX - r.left, ev.clientY - r.top, Math.exp(-ev.deltaY * 0.0015)));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  const toWorld = (ev: { clientX: number; clientY: number }) => {
    const r = svgRef.current!.getBoundingClientRect();
    return { x: (ev.clientX - r.left - cam.x) / cam.k, y: (ev.clientY - r.top - cam.y) / cam.k };
  };

  const onDown = (ev: React.PointerEvent<SVGSVGElement>) => {
    if (ev.button !== 0) return;
    const target = ev.target as Element;
    const pw = target.closest("[data-power]");
    if (pw) {
      props.onPower(pw.getAttribute("data-power") ?? "");
      return;
    }
    const nodeEl = target.closest("[data-dev]");
    const linkEl = target.closest("[data-link]");
    const w = toWorld(ev);
    if (tool.kind === "place") {
      if (view === "physical") {
        const site = roomAt(w.x, w.y) ?? "home";
        const R = ROOMS[site]!;
        props.onPlace({ view, site, px: Math.round(w.x - R.x - 40), py: Math.round(w.y - R.y - 20) }, ev.shiftKey);
      } else props.onPlace({ view, x: Math.round(w.x), y: Math.round(w.y) }, ev.shiftKey);
      return;
    }
    if (nodeEl) {
      const id = nodeEl.getAttribute("data-dev") ?? "";
      if (tool.kind === "delete") {
        props.onDelete({ kind: "dev", id });
        return;
      }
      const d = scene.dev(id);
      if (!d) return;
      if (tool.kind === "connect") {
        props.onConnect(id, { port: view === "physical" ? portAt(d, w.x, w.y) : undefined, keep: ev.shiftKey });
        return;
      }
      props.onSelect({ kind: "dev", id }, { tap: false });
      const p = view === "logical" ? { x: d.x, y: d.y } : roomPos(d);
      drag.current = { kind: "node", id, sx: ev.clientX, sy: ev.clientY, ox: p.x, oy: p.y, moved: false };
      svgRef.current?.setPointerCapture(ev.pointerId);
      return;
    }
    if (linkEl) {
      const id = linkEl.getAttribute("data-link") ?? "";
      if (tool.kind === "delete") props.onDelete({ kind: "link", id });
      else props.onSelect({ kind: "link", id }, { tap: true });
      return;
    }
    drag.current = { kind: "pan", sx: ev.clientX, sy: ev.clientY, ox: cam.x, oy: cam.y, moved: false };
    svgRef.current?.setPointerCapture(ev.pointerId);
  };

  const onMove = (ev: React.PointerEvent<SVGSVGElement>) => {
    if (tool.kind === "connect" && tool.from && view === "logical" && rubber.current) {
      const a = scene.dev(tool.from);
      const w = toWorld(ev);
      if (a) rubber.current.setAttribute("d", `M${a.x} ${a.y}L${w.x} ${w.y}`);
    }
    const dr = drag.current;
    if (!dr) return;
    const dx = ev.clientX - dr.sx;
    const dy = ev.clientY - dr.sy;
    if (Math.abs(dx) + Math.abs(dy) > 3) dr.moved = true;
    if (dr.kind === "pan") {
      props.onCamera({ ...cam, x: dr.ox + dx, y: dr.oy + dy });
      return;
    }
    if (!dr.moved || !dr.id) return;
    const x = Math.round(dr.ox + dx / cam.k);
    const y = Math.round(dr.oy + dy / cam.k);
    setDragPos({ id: dr.id, x, y });
    const d = scene.dev(dr.id);
    if (!d) return;
    if (view === "logical") props.onMove(dr.id, { view, x, y }, false);
    else props.onMove(dr.id, { view, site: d.site, px: x - roomPos({ ...d, px: 0, py: 0 }).x, py: y - roomPos({ ...d, px: 0, py: 0 }).y }, false);
  };

  const onUp = () => {
    const dr = drag.current;
    drag.current = null;
    if (!dr) return;
    if (dr.kind === "node" && dr.id) {
      const d = scene.dev(dr.id);
      if (dr.moved && d && dragPos && dragPos.id === dr.id) {
        if (view === "logical") props.onMove(dr.id, { view, x: dragPos.x, y: dragPos.y }, true);
        else props.onMove(dr.id, { view, ...dropSpot(d, dragPos.x, dragPos.y) }, true);
      } else if (!dr.moved) props.onSelect({ kind: "dev", id: dr.id }, { tap: true });
      setDragPos(null);
    }
    if (dr.kind === "pan" && !dr.moved && tool.kind === "select") props.onSelect(null, { tap: false });
  };

  const onKeyDown = (ev: React.KeyboardEvent<SVGSVGElement>) => {
    const n = (ev.target as Element).closest("[data-dev]");
    if (n && (ev.key === "Enter" || ev.key === " ")) {
      props.onSelect({ kind: "dev", id: n.getAttribute("data-dev") ?? "" }, { tap: true });
      ev.preventDefault();
    }
  };

  const onOver = (ev: React.PointerEvent<SVGSVGElement>) => {
    if (!props.onHover) return;
    const el = (ev.target as Element).closest("[data-dev],[data-link]");
    const dev = el?.getAttribute("data-dev");
    const link = el?.getAttribute("data-link");
    props.onHover(dev ? { kind: "dev", id: dev } : link ? { kind: "link", id: link } : null);
  };

  // The world redraws when the world, the selection or a drag changes, not per frame.
  const world = React.useMemo(() => {
    if (view === "logical") return <Logical scene={scene} selection={selection} running={running} pos={dragPos} />;
    const d = dragPos ? scene.dev(dragPos.id) : undefined;
    const R = d ? (ROOMS[d.site] ?? ROOMS["home"]!) : null;
    return (
      <Physical scene={scene} selection={selection} pos={dragPos && R ? { id: dragPos.id, px: dragPos.x - R.x, py: dragPos.y - R.y } : null} />
    );
  }, [view, scene, selection, running, dragPos]);

  const showRubber = tool.kind === "connect" && !!tool.from && view === "logical";
  return (
    <div ref={wrap} className="relative h-full min-h-0 overflow-hidden" data-testid="lab-canvas-wrap" data-theme-canvas={props.theme}>
      <svg
        ref={svgRef}
        className={cn("lab-canvas", "tool-" + tool.kind)}
        aria-label={t("lab.canvas.label")}
        data-view={view}
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerCancel={onUp}
        onPointerOver={onOver}
        onKeyDown={onKeyDown}
      >
        <defs>
          <pattern id="lab-grid" width={40} height={40} patternUnits="userSpaceOnUse">
            <path d="M40 0H0V40" fill="none" className="gridline" />
          </pattern>
        </defs>
        <g transform={`translate(${cam.x} ${cam.y}) scale(${cam.k})`}>
          {world}
          {view === "logical" && <Packets packets={packets} />}
          {showRubber && <path ref={rubber} className="rubber" />}
        </g>
      </svg>
    </div>
  );
}
