/* The GPU canvas behind <LabCanvas> (lab-canvas.ts): an adapter from the
 * boundary's props and intents to the Bevy module's imperative LabCanvas
 * (admin-ui/lab/render/README.md).
 *
 * Props go in through setters, and only when they change. The canvas reads
 * the world, the packets and new cables from the core itself, because it
 * renders the same `Lab` the store drives (render.ts moved the store onto
 * it), so `snapshot`, `packets` and `linkBorn` need no setter. The canvas's
 * events come out as the boundary's intents. Two of them differ in shape
 * from the SVG canvas: `select` arrives on pointer down and `tap` on a
 * click without a drag, and a drag has already called move_device by the
 * time `move` arrives. The host's onMove repeats that call, which the core
 * treats as the same position.
 *
 * The page keeps the camera. A pan or zoom inside the canvas comes back as
 * onCamera, and the camera prop that follows is not set again: setting a
 * camera the canvas already moved past would pull it back a frame. */

import * as React from "react";
import { useT } from "@/lib/i18n-react";
import type { LabApi } from "./core";
import type { CanvasTarget, LabCanvasProps, MoveTo } from "./lab-canvas";
import type { GpuCanvas, RenderModule } from "./render";
import type { Cam } from "./scene";

export interface BevyCanvasProps extends LabCanvasProps {
  mod: RenderModule;
  lab: LabApi;
  /** The first frame is on screen; `adapter` names what wgpu got. */
  onDrew(adapter: string): void;
  /** wgpu lost its device or refused a call; the canvas has stopped. */
  onLost(why: string): void;
}

const CANVAS_ID = "lab-gpu-canvas";

type Wire = { kind: "device" | "link"; id: string } | null;

function fromWire(t: Wire): CanvasTarget | null {
  return t ? { kind: t.kind === "device" ? "dev" : "link", id: t.id } : null;
}

function toWire(t: CanvasTarget | null): string {
  return JSON.stringify(t ? { kind: t.kind === "dev" ? "device" : "link", id: t.id } : null);
}

function toolString(p: LabCanvasProps): string {
  const t = p.tool;
  if (t.kind === "place") return `place:${t.type}`;
  if (t.kind === "connect") return `connect:${t.linkKind}`;
  return t.kind;
}

const camKey = (c: Cam): string => `${c.x.toFixed(2)},${c.y.toFixed(2)},${c.k.toFixed(4)}`;

export function BevyCanvas(props: BevyCanvasProps) {
  const t = useT();
  const live = React.useRef(props);
  live.current = props;
  const canvas = React.useRef<GpuCanvas | null>(null);
  const el = React.useRef<HTMLCanvasElement>(null);
  /** Cameras the canvas reported lately, newest last. */
  const told = React.useRef<string[]>([]);
  const { mod, lab } = props;

  React.useEffect(() => {
    const c = mod.LabCanvas.attach(lab as never, `#${CANVAS_ID}`);
    canvas.current = c;
    const on = <T,>(event: string, fn: (payload: T) => void) => c.on(event, (json: string) => fn(JSON.parse(json) as T));
    on<Wire>("select", (w) => live.current.onSelect(fromWire(w), { tap: false }));
    on<Wire>("tap", (w) => live.current.onSelect(fromWire(w), { tap: true }));
    on<(Wire & { title?: string }) | null>("hover", (w) => {
      if (el.current) el.current.title = w?.title ?? "";
      live.current.onHover?.(fromWire(w));
    });
    on<{ id: string; to: MoveTo; done: boolean }>("move", (m) => {
      if (m.to) live.current.onMove(m.id, m.to, m.done);
    });
    on<{ type: string; x: number; y: number; opts: { site: string; px?: number; py?: number }; shift: boolean }>("place", (p) => {
      if (p.opts.px !== undefined && p.opts.py !== undefined) {
        live.current.onPlace({ view: "physical", site: p.opts.site, px: p.opts.px, py: p.opts.py }, p.shift);
      } else live.current.onPlace({ view: "logical", x: p.x, y: p.y }, p.shift);
    });
    on<{ id: string; port: string | null; shift: boolean }>("connect", (p) =>
      live.current.onConnect(p.id, { port: p.port ?? undefined, keep: p.shift }),
    );
    on<Wire>("delete", (w) => {
      const target = fromWire(w);
      if (target) live.current.onDelete(target);
    });
    on<{ id: string }>("power", (p) => live.current.onPower(p.id));
    on<Cam>("camera", (cam) => {
      told.current = [...told.current.slice(-15), camKey(cam)];
      live.current.onCamera(cam);
    });
    on<{ width: number; height: number }>("viewport", (v) => live.current.onViewport(v));
    const drew = () => {
      const stats = JSON.parse(c.stats()) as { adapter: { backend: string; name: string } | null };
      live.current.onDrew(stats.adapter ? `${stats.adapter.backend} "${stats.adapter.name}"` : "unknown");
    };
    on<unknown>("frame", drew);
    on<{ type?: string; description?: string }>("render-error", (e) => live.current.onLost(`${e.type ?? "error"}: ${e.description ?? ""}`));
    // The page owns the half-made connection; the canvas only draws it.
    c.set_connect_from("null");
    // A remount (StrictMode, or the canvas coming back) finds the app drawing already.
    if ((JSON.parse(c.stats()) as { firstFrameAt: number | null }).firstFrameAt !== null) drew();
    return () => {
      c.detach();
      c.free();
      canvas.current = null;
    };
  }, [mod, lab]);

  const { theme, palette, running, selection, view, camera } = props;
  const tool = toolString(props);
  const from = props.tool.kind === "connect" ? props.tool.from : null;

  React.useEffect(() => {
    canvas.current?.set_theme(JSON.stringify({ theme, palette }));
  }, [theme, palette, mod, lab]);

  React.useEffect(() => {
    canvas.current?.set_running(JSON.stringify([...running]));
  }, [running, mod, lab]);

  React.useEffect(() => {
    canvas.current?.set_selection(toWire(selection));
  }, [selection, mod, lab]);

  // The view, the tool and the first end go together: set_view and
  // set_tool clear the canvas's copy of the first end, and a new view
  // needs its own camera.
  React.useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    c.set_view(view);
    c.set_tool(tool);
    c.set_connect_from(JSON.stringify(from));
    c.set_camera(JSON.stringify(live.current.camera));
  }, [view, tool, from, mod, lab]);

  React.useEffect(() => {
    if (told.current.includes(camKey(camera))) return;
    canvas.current?.set_camera(JSON.stringify(camera));
  }, [camera, mod, lab]);

  return (
    <div className="absolute inset-0" data-testid="lab-gpu-wrap" data-theme-canvas={theme}>
      <canvas
        ref={el}
        id={CANVAS_ID}
        className={`lab-gpu-canvas tool-${props.tool.kind}`}
        aria-label={t("lab.canvas.label")}
        role="img"
        data-view={view}
      />
    </div>
  );
}
