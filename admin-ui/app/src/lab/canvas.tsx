/* The canvas host: the store's state in as <LabCanvas> props, the canvas's
 * intents out as actions, and the chrome that sits over any canvas (the
 * tool hint with the port picker, the zoom buttons, the legend).
 *
 * The canvas itself is behind the boundary in lab-canvas.ts. Two take the
 * same props. The SVG one (svg-canvas.tsx) paints first and stays where no
 * GPU canvas can run. The GPU one (bevy-canvas.tsx, Bevy on WebGPU or
 * WebGL2) replaces it once render.ts has loaded its module and moved the
 * core onto it. While the GPU canvas starts, it sits under the SVG one, and
 * the SVG one leaves when the first GPU frame is drawn. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, MinusSignIcon, ArrowExpand01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/i18n-react";
import { getResolvedTheme, subscribeTheme } from "@/lib/theme";
import { actions } from "./actions";
import { useEngine } from "./engine-hook";
import type { CanvasTool, LabCanvasProps, Palette } from "./lab-canvas";
import { BevyCanvas } from "./bevy-canvas";
import { gpuDrew, gpuFailed } from "./render";
import { store, useLab } from "./store";
import { SvgCanvas } from "./svg-canvas";

/* The tokens a canvas draws with, resolved: a WebGL canvas cannot read CSS
 * variables, so it gets the values (the SVG one uses the variables). */
const PALETTE_TOKENS = [
  "ground", "surface", "sunk", "ink", "muted", "faint", "line", "hair", "accent", "accent-wash", "ok", "warn", "crit",
  "lab-copper", "lab-fiber", "lab-wan", "lab-wifi", "lab-grid", "lab-room", "lab-room-edge", "lab-mark", "lab-led-off",
  "lab-p-dhcp", "lab-p-arp", "lab-p-mdns", "lab-p-dns", "lab-p-http", "lab-p-icmp", "lab-p-rathole", "lab-p-rke2", "lab-p-lososd",
  "lab-kit", "lab-kit-face", "lab-kit-edge", "lab-hole", "lab-rack", "lab-rack-edge", "lab-rack-face", "lab-rack-bay", "lab-rack-ink",
  "lab-screen", "lab-screen-off", "lab-screen-tile",
];

function useThemeAndPalette(): { theme: "light" | "dark"; palette: Palette } {
  const theme = React.useSyncExternalStore(
    (fn) => {
      const m = window.matchMedia("(prefers-color-scheme: dark)");
      m.addEventListener("change", fn);
      const off = subscribeTheme(fn);
      return () => {
        m.removeEventListener("change", fn);
        off();
      };
    },
    getResolvedTheme,
  );
  const palette = React.useMemo(() => {
    const cs = getComputedStyle(document.documentElement);
    return Object.fromEntries(PALETTE_TOKENS.map((k) => [k, cs.getPropertyValue("--" + k).trim()]));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the theme is what changes the values
  }, [theme]);
  return { theme, palette };
}

function Hint() {
  const s = useLab();
  const t = useT();
  const u = s.ui;
  if (u.tool === "select") return null;
  let text = "";
  if (u.tool === "place" && u.placeType) {
    text = t("lab.hint.place", { view: t(u.view === "logical" ? "lab.view.logical" : "lab.view.physical").toLowerCase(), type: s.catalog.types[u.placeType]?.label ?? u.placeType });
  } else if (u.tool === "connect") {
    text = u.connectFrom
      ? t("lab.hint.connectTo", { name: s.name(u.connectFrom) })
      : t("lab.hint.connectFrom", { kind: s.catalog.linkKinds[u.linkKind ?? ""]?.label ?? u.linkKind ?? "" });
  } else if (u.tool === "delete") text = t("lab.hint.delete");
  const free = u.tool === "connect" && u.connectFrom ? s.freePorts(u.connectFrom) : [];
  return (
    <div
      role="status"
      className="absolute top-3 left-4 z-10 flex max-w-[calc(100%-80px)] flex-wrap items-center gap-2 rounded-card border border-line bg-surface px-2.5 py-1.5 text-[12.5px] shadow-card"
    >
      <span>{text}</span>
      {u.tool === "connect" && u.connectFrom && free.length > 1 && (
        <label className="inline-flex items-center gap-1.5 text-muted">
          {t("lab.hint.port", { name: s.name(u.connectFrom) })}
          <select
            className="h-6 rounded-control border border-line bg-surface px-1 text-[12px] text-ink"
            value={u.connectPort}
            onChange={(e) => s.setUi({ connectPort: e.target.value })}
            data-testid="connect-port"
          >
            <option value="">{t("lab.hint.portAuto")}</option>
            {free.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        </label>
      )}
    </div>
  );
}

function Legend() {
  const s = useLab();
  const t = useT();
  return (
    <div className="absolute right-3 bottom-3 z-10 flex max-w-[calc(100%-32px)] flex-wrap gap-3 rounded-card border border-line bg-surface/90 px-2.5 py-1 text-[11.5px] text-muted">
      {s.ui.view === "logical" ? (
        <>
          <span className="inline-flex items-center gap-1.5">
            <i className="lab-swatch copper" />
            {t("lab.legend.copper")}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <i className="lab-swatch fiber" />
            {t("lab.legend.fiber")}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <i className="lab-swatch wan" />
            WAN
          </span>
          <span className="inline-flex items-center gap-1.5">
            <i className="lab-swatch wifi" />
            Wi-Fi
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="inline-block size-2 rounded-full bg-ok" />
            {t("lab.legend.up")}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="inline-block size-2 rounded-full bg-crit" />
            {t("lab.legend.down")}
          </span>
        </>
      ) : (
        <span>{t("lab.legend.physical")}</span>
      )}
    </div>
  );
}

export function Canvas() {
  const s = useLab();
  useLab("cam");
  useLab("frame");
  const engine = useEngine();
  const t = useT();
  const { theme, palette } = useThemeAndPalette();
  const fitted = React.useRef(false);
  const refit = React.useRef(0);
  const u = s.ui;

  const tool: CanvasTool =
    u.tool === "place" && u.placeType
      ? { kind: "place", type: u.placeType }
      : u.tool === "connect"
        ? { kind: "connect", linkKind: u.linkKind ?? "auto", from: u.connectFrom }
        : u.tool === "delete"
          ? { kind: "delete" }
          : { kind: "select" };
  const running = React.useMemo(
    () => new Set(s.snap.world.devices.filter((d) => engine.isRunning(d.id)).map((d) => d.id)),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- engine.version says when a guest came or went
    [s.snap, engine.version],
  );

  const props: LabCanvasProps = {
    snapshot: s.snap,
    catalog: s.catalog,
    packets: s.frame.packets,
    view: u.view,
    tool,
    selection: u.sel,
    camera: s.cams[u.view],
    theme,
    palette,
    running,
    linkBorn: s.linkBorn,
    onSelect: (target, how) => {
      actions().select(target);
      if (how.tap && s.phone) s.setUi({ sheet: true });
    },
    onMove: (id, to, done) => {
      const d = s.dev(id);
      if (!d) return;
      if (to.view === "logical") {
        // The core hears every step, so packets keep riding the cables under the pointer.
        s.core.moveDevice(id, { x: to.x, y: to.y });
        if (done) s.refresh();
        return;
      }
      if (!done) {
        s.core.moveDevice(id, { px: to.px, py: to.py });
        return;
      }
      s.core.moveDevice(id, { px: to.px, py: to.py, site: to.site });
      if (to.site !== d.site) s.toast(t("lab.toast.moved", { name: d.name, room: s.catalog.sites[to.site]?.name ?? to.site }));
      s.refresh();
    },
    onPlace: (at, keep) => {
      const type = u.placeType;
      if (!type) return;
      if (at.view === "physical") {
        // Its logical spot: along the bottom of the diagram, out of the way.
        actions().place(200 + s.snap.world.devices.length * 60, 300, { site: at.site, px: at.px, py: at.py }, keep);
      } else {
        actions().place(at.x, at.y, { site: type === "edge-official" ? "dc" : "home" }, keep);
      }
    },
    onConnect: (id, { port, keep }) => actions().connectTo(id, keep, port),
    onDelete: (target) => (target.kind === "dev" ? actions().removeDevice(target.id) : actions().removeLink(target.id)),
    onPower: (id) => actions().togglePower(id),
    onCamera: (cam) => s.setCam(cam),
    onViewport: ({ width, height }) => {
      // The other canvas reporting the same size (the swap) is no resize.
      if (fitted.current && width === s.viewport.width && height === s.viewport.height) return;
      s.viewport = { width, height };
      if (!width) return;
      if (!fitted.current) {
        fitted.current = true;
        s.fit();
        return;
      }
      window.clearTimeout(refit.current);
      refit.current = window.setTimeout(() => store().fit(), 120);
    },
  };

  const gpu = s.gpu;
  const zoomBy = (f: number) => s.zoomAt(s.viewport.width / 2, s.viewport.height / 2, f);
  return (
    <div className="relative min-h-0 overflow-hidden" data-canvas={gpu.state === "gpu" ? (gpu.backend ?? "gpu") : "svg"}>
      {gpu.mod && gpu.lab && (gpu.state === "starting" || gpu.state === "gpu") && (
        <BevyCanvas
          {...props}
          mod={gpu.mod}
          lab={gpu.lab}
          onDrew={(adapter) => gpuDrew(s, adapter)}
          onLost={(why) => gpuFailed(s, why, true)}
        />
      )}
      {gpu.state !== "gpu" && <SvgCanvas {...props} />}
      <Hint />
      <div className="absolute top-3 right-3 z-10 flex flex-col gap-1">
        <Button variant="secondary" size="icon-sm" aria-label={t("lab.zoom.in")} onClick={() => zoomBy(1.2)}>
          <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={1.8} color="currentColor" />
        </Button>
        <Button variant="secondary" size="icon-sm" aria-label={t("lab.zoom.out")} onClick={() => zoomBy(1 / 1.2)}>
          <HugeiconsIcon icon={MinusSignIcon} size={16} strokeWidth={1.8} color="currentColor" />
        </Button>
        <Button variant="secondary" size="icon-sm" aria-label={t("lab.zoom.fit")} onClick={() => s.fit()}>
          <HugeiconsIcon icon={ArrowExpand01Icon} size={15} strokeWidth={1.8} color="currentColor" />
        </Button>
      </div>
      <Legend />
    </div>
  );
}
