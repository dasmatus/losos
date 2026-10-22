/* The diagram as a picture: an SVG file that stands on its own (colours
 * resolved from the theme, words set as SVG text, no foreignObject, so any
 * viewer and any vector editor draws it), and a PNG drawn from that SVG at
 * twice its size. What is on the page and what is in the file come from the
 * same core snapshot and the same shapes (shapes.ts). */

import type { CanvasEdge, CanvasNode } from "../core";
import type { LabStore } from "../store";
import { colorValue } from "./colors";
import { machineOf } from "./machines";
import { type Block, type Inline, parse, plain } from "./markdown";
import { centred, dash, head, outline, textBox } from "./shapes";

const PAD = 40;
const BASE = 14;

function esc(s: string): string {
  return s.replace(/[<>&"']/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** Every var(--x) in a value, resolved against the page's theme. */
function resolver(): (value: string) => string {
  const cs = getComputedStyle(document.documentElement);
  const cache = new Map<string, string>();
  const one = (name: string): string => {
    let v = cache.get(name);
    if (v === undefined) {
      v = cs.getPropertyValue(name).trim();
      cache.set(name, v);
    }
    return v;
  };
  const resolve = (value: string): string => {
    let out = value;
    for (let i = 0; i < 6 && out.includes("var("); i++) out = out.replace(/var\((--[\w-]+)\)/g, (_, n: string) => one(n) || "currentColor");
    return out;
  };
  return resolve;
}

interface Line {
  text: string;
  size: number;
  bold: boolean;
  italic?: boolean;
  mono?: boolean;
  muted?: boolean;
  /** Indent in px. */
  indent: number;
  gap: number;
  rule?: boolean;
}

let measureCtx: CanvasRenderingContext2D | null = null;
function measure(text: string, size: number, bold: boolean, mono: boolean, family: string): number {
  measureCtx ??= document.createElement("canvas").getContext("2d");
  if (!measureCtx) return text.length * size * 0.55;
  measureCtx.font = `${bold ? 600 : 400} ${size}px ${mono ? "ui-monospace, monospace" : family}`;
  return measureCtx.measureText(text).width;
}

/** Words broken into lines no wider than `width`. */
function wrap(text: string, width: number, size: number, bold: boolean, mono: boolean, family: string): string[] {
  const out: string[] = [];
  for (const para of text.split("\n")) {
    const words = para.split(/(\s+)/);
    let line = "";
    for (const w of words) {
      const next = line + w;
      if (line && measure(next.trimEnd(), size, bold, mono, family) > width) {
        out.push(line.trimEnd());
        line = w.trimStart();
        // A word longer than the line is cut where it must be.
        while (measure(line, size, bold, mono, family) > width && line.length > 1) {
          let cut = line.length - 1;
          while (cut > 1 && measure(line.slice(0, cut), size, bold, mono, family) > width) cut--;
          out.push(line.slice(0, cut));
          line = line.slice(cut);
        }
      } else line = next;
    }
    out.push(line.trimEnd());
  }
  return out;
}

function linesOf(blocks: Block[], width: number, family: string, depth = 0): Line[] {
  const out: Line[] = [];
  const push = (text: string, o: Partial<Line> & { size?: number }) => {
    const size = o.size ?? BASE;
    const ind = (o.indent ?? 0) + depth * 14;
    const rows = wrap(text, Math.max(20, width - ind), size, !!o.bold, !!o.mono, family);
    rows.forEach((r, i) => out.push({ text: r, size, bold: !!o.bold, italic: o.italic, mono: o.mono, muted: o.muted, indent: ind, gap: i === 0 ? (o.gap ?? 6) : 0 }));
  };
  const allBold = (kids: Inline[]) => kids.length > 0 && kids.every((k) => k.k === "b");
  for (const b of blocks) {
    switch (b.k) {
      case "h":
        push(plain(b.kids), { size: [0, 24, 20, 17, 15, 14, 13][b.level]!, bold: true, gap: 8 });
        break;
      case "p":
        push(plain(b.kids), { bold: allBold(b.kids) });
        break;
      case "hr":
        out.push({ text: "", size: BASE, bold: false, indent: 0, gap: 8, rule: true });
        break;
      case "code":
        push(b.text, { mono: true, size: 12.5 });
        break;
      case "quote": {
        if (b.title) push(plain(b.title), { bold: true, indent: 10 });
        for (const l of linesOf(b.blocks, width - 10, family, depth)) out.push({ ...l, indent: l.indent + 10, muted: true });
        break;
      }
      case "ul":
      case "ol":
        b.items.forEach((it, i) => {
          const mark = it.task !== null ? (it.task ? "☑ " : "☐ ") : b.k === "ol" ? `${b.start + i}. ` : "• ";
          push(mark + plain(it.kids), { indent: it.depth * 16, gap: i === 0 ? 6 : 2, muted: it.task === true });
        });
        break;
    }
  }
  return out;
}

/** Lines set in `box`: `middle` centres them both ways (a shape's words),
 *  `vmiddle` only up and down (a device's). */
function textSvg(
  lines: Line[],
  box: { x: number; y: number; w: number; h: number },
  middle: boolean,
  align: string | undefined,
  ink: string,
  muted: string,
  vmiddle = middle,
): string {
  const height = lines.reduce((a, l, i) => a + (i === 0 ? 0 : l.gap) + l.size * 1.4, 0);
  let y = box.y + (vmiddle ? Math.max(0, (box.h - height) / 2) : 0);
  const anchor = middle || align === "center" ? "middle" : align === "right" ? "end" : "start";
  const out: string[] = [];
  lines.forEach((l, i) => {
    y += (i === 0 ? 0 : l.gap) + l.size * 1.4;
    if (l.rule) {
      out.push(`<line x1="${box.x}" x2="${box.x + box.w}" y1="${y - l.size * 0.6}" y2="${y - l.size * 0.6}" stroke="${muted}" stroke-width="1"/>`);
      return;
    }
    const x = anchor === "middle" ? box.x + box.w / 2 : anchor === "end" ? box.x + box.w : box.x + l.indent;
    out.push(
      `<text x="${x.toFixed(1)}" y="${(y - l.size * 0.35).toFixed(1)}" font-size="${l.size}"${l.bold ? ' font-weight="600"' : ""}${l.italic ? ' font-style="italic"' : ""}${l.mono ? ' font-family="ui-monospace, monospace"' : ""} fill="${l.muted ? muted : ink}" text-anchor="${anchor}">${esc(l.text)}</text>`,
    );
  });
  return out.join("");
}

/** The diagram, as an SVG document. */
export function exportSvg(s: LabStore): string {
  const { nodes, edges } = s.canvas;
  const r = resolver();
  const family = getComputedStyle(document.body).fontFamily || "system-ui, sans-serif";
  const ink = r("var(--ink)");
  const muted = r("var(--muted)");
  const surface = r("var(--surface)");
  const ground = r("var(--ground)");
  const line = r("var(--line)");
  const accent = r("var(--accent)");
  const minX = Math.min(...nodes.map((n) => n.x), ...edges.map((e) => e.labelAt[0])) - PAD;
  const minY = Math.min(...nodes.map((n) => n.y - (n.type === "group" && n.label ? 30 : 0))) - PAD;
  const maxX = Math.max(...nodes.map((n) => n.x + n.width)) + PAD;
  const maxY = Math.max(...nodes.map((n) => n.y + n.height)) + PAD;
  const W = Math.ceil(maxX - minX);
  const H = Math.ceil(maxY - minY);
  const colorOf = (c: string | undefined) => {
    const v = colorValue(c);
    return v ? r(v) : undefined;
  };
  const parts: string[] = [];
  parts.push(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="${minX} ${minY} ${W} ${H}" font-family="${esc(family)}">`,
    `<title>${esc(s.core.exportSetup().name)}</title>`,
    `<rect x="${minX}" y="${minY}" width="${W}" height="${H}" fill="${ground}"/>`,
  );
  for (const n of nodes.filter((x) => x.type === "group")) {
    const c = colorOf(n.color) ?? line;
    parts.push(
      `<rect x="${n.x}" y="${n.y}" width="${n.width}" height="${n.height}" rx="12" fill="${c}" fill-opacity="0.07" stroke="${c}" stroke-width="2"/>`,
    );
    if (n.label) parts.push(`<text x="${n.x + 4}" y="${n.y - 10}" font-size="16" font-weight="600" fill="${ink}">${esc(n.label)}</text>`);
  }
  for (const e of edges) parts.push(edgeSvg(e, s, colorOf, { ink, muted, ground, line }));
  nodes
    .filter((x) => x.type !== "group")
    .forEach((n, i) => parts.push(cardSvg(n, i, s, r, colorOf, family, { ink, muted, surface, line, accent })));
  parts.push("</svg>");
  return parts.join("\n");
}

function edgeSvg(
  e: CanvasEdge,
  s: LabStore,
  colorOf: (c: string | undefined) => string | undefined,
  p: { ink: string; muted: string; ground: string; line: string },
): string {
  const cableColor = e.cable === "fiber" ? "var(--lab-fiber)" : e.cable === "wifi" ? "var(--lab-wifi)" : e.cable === "wan" ? "var(--lab-wan)" : e.cable ? "var(--lab-copper)" : null;
  const c = colorOf(e.color) ?? (cableColor ? (getComputedStyle(document.documentElement).getPropertyValue(cableColor.slice(4, -1)).trim() || p.muted) : p.muted);
  const d = dash(e.styleAttributes?.["path"]) ?? (e.cable === "wifi" ? "2 6" : undefined);
  const out = [`<path d="${e.path}" fill="none" stroke="${c}" stroke-width="${e.cable ? 3 : 2.5}" stroke-linecap="round"${d ? ` stroke-dasharray="${d}"` : ""}/>`];
  for (const h of e.heads) {
    const hd = head(e.styleAttributes?.["arrow"]);
    out.push(
      `<path d="${hd.d}" transform="translate(${h.x} ${h.y}) rotate(${h.angle})" fill="${hd.fill ? c : p.ground}" stroke="${c}" stroke-width="2" stroke-linejoin="round"/>`,
    );
  }
  const link = e.link ? s.link(e.link) : undefined;
  const words = e.label || (link ? `${link.a.port} – ${link.b.port}` : "");
  if (words) {
    out.push(
      `<text x="${e.labelAt[0]}" y="${e.labelAt[1]}" font-size="${e.label ? 13 : 11}" fill="${e.label ? p.ink : p.muted}" text-anchor="middle" dominant-baseline="middle" stroke="${p.ground}" stroke-width="5" paint-order="stroke" stroke-linejoin="round">${esc(words)}</text>`,
    );
  }
  return out.join("");
}

function cardSvg(
  n: CanvasNode,
  i: number,
  s: LabStore,
  r: (v: string) => string,
  colorOf: (c: string | undefined) => string | undefined,
  family: string,
  p: { ink: string; muted: string; surface: string; line: string; accent: string },
): string {
  const shape = n.type === "text" ? n.styleAttributes?.["shape"] : undefined;
  const o = outline(shape, n.width, n.height);
  const c = colorOf(n.color);
  const border = n.styleAttributes?.["border"];
  const stroke = border === "invisible" ? "none" : (c ?? p.line);
  const dashAttr = border === "dashed" ? ' stroke-dasharray="8 6"' : border === "dotted" ? ' stroke-dasharray="1.5 5" stroke-linecap="round"' : "";
  const out = [`<g transform="translate(${n.x} ${n.y})">`];
  out.push(`<path d="${o.d}" fill="${p.surface}" stroke="${stroke}" stroke-width="2"${dashAttr}/>`);
  if (c) out.push(`<path d="${o.d}" fill="${c}" fill-opacity="0.1"/>`);
  if (o.inner) out.push(`<path d="${o.inner}" fill="none" stroke="${stroke}" stroke-width="2"/>`);
  const tb = textBox(shape, n.width, n.height);
  const inner = { x: tb.x + (shape ? 0 : 14), y: tb.y + (shape ? 0 : 10), w: tb.w - (shape ? 0 : 28), h: tb.h - (shape ? 0 : 20) };
  const clip = `dg-clip-${i}`;
  out.push(`<clipPath id="${clip}"><rect x="${tb.x}" y="${tb.y}" width="${tb.w}" height="${tb.h}"/></clipPath><g clip-path="url(#${clip})">`);
  const line = (text: string, o2: Partial<Line> = {}): Line => ({ text, size: BASE, bold: false, indent: 0, gap: 4, ...o2 });
  const machine = machineOf(n.losos);
  if (n.dev) {
    const d = s.dev(n.dev);
    const icon = document.querySelector(`[data-node="${CSS.escape(n.id)}"] .dg-device-icon`);
    const size = Math.min(56, n.height - 20, n.width / 2.6);
    if (icon) {
      const glyph = r(icon.innerHTML);
      out.push(`<svg x="12" y="${(n.height - size) / 2}" width="${size}" height="${size}" viewBox="0 0 64 64">${glyph}</svg>`);
    }
    if (d) {
      const T = s.catalog.types[d.type];
      const ip = s.scene.ipOf(d);
      const box = { x: 20 + size, y: 0, w: n.width - 32 - size, h: n.height };
      out.push(
        textSvg(
          [line(d.name, { bold: true }), line(T?.short ?? d.type, { muted: true, size: 12 }), line(!d.power ? "off" : (ip ?? ""), { mono: true, size: 11.5, muted: true })].filter(
            (l) => l.text,
          ),
          box,
          false,
          undefined,
          p.ink,
          p.muted,
          true,
        ),
      );
    }
  } else if (machine) {
    const info = s.machines.state === "ready" && machine.order ? s.machines.machines.find((m) => m.order === machine.order) : undefined;
    const ls = [line(info?.name ?? machine.name ?? "", { bold: true }), line(info?.imageName ?? machine.image ?? "", { muted: true, size: 12 })];
    if (info) ls.push(line(info.replicas.map((x) => `${x.name}: ${x.status}`).join(", "), { size: 12 }));
    out.push(textSvg(ls.filter((l) => l.text), inner, false, undefined, p.ink, p.muted));
  } else if (n.type === "link") {
    out.push(textSvg([line(hostOf(n.url ?? ""), { bold: true }), ...wrap(n.url ?? "", inner.w, 12, false, false, family).map((t) => line(t, { size: 12, muted: true, gap: 0 }))], inner, false, undefined, p.ink, p.muted));
  } else if (n.type === "file") {
    const f = n.file ?? "";
    out.push(textSvg([line(f.slice(f.lastIndexOf("/") + 1), { bold: true }), line(f, { size: 12, muted: true })], inner, false, undefined, p.ink, p.muted));
  } else {
    const align = n.styleAttributes?.["textAlign"];
    out.push(textSvg(linesOf(parse(n.text ?? ""), inner.w, family), inner, centred(shape), align, p.ink, p.muted));
  }
  out.push("</g></g>");
  return out.join("");
}

function hostOf(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

/** The SVG drawn twice its size into a PNG; null where the browser cannot. */
export async function exportPng(svg: string): Promise<Blob | null> {
  const m = /width="(\d+)" height="(\d+)"/.exec(svg);
  if (!m) return null;
  const scale = 2;
  const w = Number(m[1]) * scale;
  const h = Number(m[2]) * scale;
  if (w * h > 16384 * 16384 || w > 32767 || h > 32767) return null;
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  try {
    const img = new Image();
    img.decoding = "async";
    img.src = url;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = w;
    c.height = h;
    const g = c.getContext("2d");
    if (!g) return null;
    g.drawImage(img, 0, 0, w, h);
    return await new Promise<Blob | null>((ok) => c.toBlob((b) => ok(b), "image/png"));
  } catch {
    return null;
  } finally {
    URL.revokeObjectURL(url);
  }
}
