/* qemu-wasm: one qemu-system-x86_64 (ktock/qemu-wasm, emscripten, pthreads)
 * per guest, in this tab. Ported from admin-ui/lab/src/engine.js.
 *
 * Each guest's virtio-net is a QEMU "socket" netdev; emscripten turns its TCP
 * connect into a WebSocket, which the shim below intercepts: the stream it
 * carries (4-byte big-endian length, then the frame) is cut into frames and
 * handed to the fabric, and frames from the fabric go back the same way.
 *
 * It needs a cross-origin isolated page (COOP + COEP), because QEMU's
 * threads are Web Workers sharing memory, and the files engine/build.sh
 * puts beside the page: qemu/out.js and its worker, pc-bios, the guests,
 * and xterm + xterm-pty under vendor/. */

import type { GuestBackend, GuestConsole, GuestDevice, GuestHandle, GuestNic, Probe } from "./types";

/* xterm and xterm-pty are classic scripts from vendor/; they leave globals. */
export interface XTerm {
  open(el: HTMLElement): void;
  loadAddon(addon: unknown): void;
  write(s: string | Uint8Array): void;
  onData(cb: (s: string) => void): void;
  resize(cols: number, rows: number): void;
  focus(): void;
  dispose(): void;
  cols: number;
  rows: number;
  element?: HTMLElement;
  buffer: {
    active: { length: number; baseY: number; cursorY: number; getLine(i: number): { translateToString(trim: boolean): string } | undefined };
  };
  _core?: { _renderService?: { dimensions?: { css?: { cell?: { width: number; height: number } } } } };
}
export interface VendorGlobals {
  Terminal?: new (opts: Record<string, unknown>) => XTerm;
  openpty?: () => { master: unknown; slave: { readable: boolean; writable: boolean } };
}

type EmModule = Record<string, unknown> & {
  FS?: { mkdir(p: string): void; writeFile(p: string, b: Uint8Array): void };
  TTY?: { stream_ops: { poll: (stream: unknown, timeout: number) => number } };
  pty?: { readable: boolean; writable: boolean };
};

const FILES: Record<string, string> = {
  "bzImage": "guest/bzImage",
  "rootfs.bin": "guest/rootfs.bin",
  "bios-256k.bin": "qemu/pc-bios/bios-256k.bin",
  "kvmvapic.bin": "qemu/pc-bios/kvmvapic.bin",
  "linuxboot_dma.bin": "qemu/pc-bios/linuxboot_dma.bin",
  "vgabios-stdvga.bin": "qemu/pc-bios/vgabios-stdvga.bin",
  "efi-virtio.rom": "qemu/pc-bios/efi-virtio.rom",
  "gear.bin": "guest/gear.bin",
};

const here = (path: string): string => new URL(path, document.baseURI).href;

function loadScript(src: string): Promise<void> {
  return new Promise((ok, bad) => {
    const s = document.createElement("script");
    s.src = src;
    s.onload = () => ok();
    s.onerror = () => bad(new Error("could not load " + src));
    document.head.appendChild(s);
  });
}

interface Port {
  sock: FabricSocket;
  buf: Uint8Array;
  listeners: Set<(frame: Uint8Array) => void>;
}

/* The sockets emscripten opens, by guest id. */
const ports = new Map<string, Port>();

class FabricSocket extends EventTarget {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  readonly CONNECTING = 0;
  readonly OPEN = 1;
  readonly CLOSING = 2;
  readonly CLOSED = 3;
  readyState = 0;
  binaryType = "arraybuffer";
  protocol = "binary";
  bufferedAmount = 0;
  readonly devId: string;
  [handler: `on${string}`]: unknown;

  constructor(readonly url: string) {
    super();
    this.devId = url.split("/").pop() ?? "";
    const old = ports.get(this.devId);
    ports.set(this.devId, { sock: this, buf: new Uint8Array(0), listeners: old?.listeners ?? new Set() });
    setTimeout(() => {
      this.readyState = 1;
      this.fire("open");
    }, 0);
  }

  fire(type: string, data?: ArrayBuffer): void {
    const ev = type === "message" ? new MessageEvent("message", { data }) : new Event(type);
    const handler = this[`on${type}`];
    if (typeof handler === "function") (handler as (e: Event) => void).call(this, ev);
    this.dispatchEvent(ev);
  }

  send(data: ArrayBuffer | ArrayBufferView): void {
    const p = ports.get(this.devId);
    if (!p) return;
    const chunk =
      data instanceof ArrayBuffer
        ? new Uint8Array(data)
        : new Uint8Array(data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength));
    const nb = new Uint8Array(p.buf.length + chunk.length);
    nb.set(p.buf);
    nb.set(chunk, p.buf.length);
    p.buf = nb;
    while (p.buf.length >= 4) {
      const len = ((p.buf[0]! << 24) | (p.buf[1]! << 16) | (p.buf[2]! << 8) | p.buf[3]!) >>> 0;
      if (p.buf.length < 4 + len) break;
      const frame = p.buf.slice(4, 4 + len);
      p.buf = p.buf.slice(4 + len);
      for (const fn of p.listeners) {
        try {
          fn(frame);
        } catch (e) {
          console.warn("fabric", e);
        }
      }
    }
  }

  close(): void {
    this.readyState = 3;
    ports.delete(this.devId);
    this.fire("close");
  }
}

function deliver(devId: string, frame: Uint8Array): void {
  const p = ports.get(devId);
  if (!p || !p.sock || p.sock.readyState !== 1) return;
  const out = new Uint8Array(4 + frame.length);
  out[0] = frame.length >>> 24;
  out[1] = (frame.length >>> 16) & 255;
  out[2] = (frame.length >>> 8) & 255;
  out[3] = frame.length & 255;
  out.set(frame, 4);
  p.sock.fire("message", out.buffer);
}

function listen(devId: string, fn: (frame: Uint8Array) => void): () => void {
  let p = ports.get(devId);
  if (!p) {
    // The socket opens once QEMU starts; keep the listener for it.
    p = { sock: null as unknown as FabricSocket, buf: new Uint8Array(0), listeners: new Set() };
    ports.set(devId, p);
  }
  p.listeners.add(fn);
  return () => ports.get(devId)?.listeners.delete(fn);
}

let shimmed = false;
function installSocketShim(): void {
  if (shimmed) return;
  shimmed = true;
  const Real = window.WebSocket;
  const Shim = function (this: unknown, url: string | URL, protocols?: string | string[]) {
    if (String(url).startsWith("ws://losos-lab/")) return new FabricSocket(String(url));
    return new Real(url, protocols);
  } as unknown as typeof WebSocket;
  Object.assign(Shim, { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 });
  window.WebSocket = Shim;
}

/* Pthread workers carry the guest id in their URL, so a stop can end them. */
const workersByVm = new Map<string, Worker[]>();
let tracked = false;
function trackWorkers(): void {
  if (tracked) return;
  tracked = true;
  const W = window.Worker;
  const Tracked = function (url: string | URL, opts?: WorkerOptions) {
    const w = new W(url, opts);
    const m = String(url).match(/[?&]vm=([\w-]+)/);
    if (m?.[1]) {
      const list = workersByVm.get(m[1]) ?? [];
      list.push(w);
      workersByVm.set(m[1], list);
    }
    return w;
  } as unknown as typeof Worker;
  Tracked.prototype = W.prototype;
  window.Worker = Tracked;
}

/* xterm draws on a canvas, so it takes its colours as values, not CSS: the
 * house tokens as they resolve right now (the terminal keeps them if the
 * theme flips while it runs). */
export function houseTheme(): Record<string, string> {
  const cs = getComputedStyle(document.documentElement);
  const v = (name: string) => cs.getPropertyValue(name).trim();
  return {
    background: v("--sunk"),
    foreground: v("--ink"),
    cursor: v("--ink"),
    cursorAccent: v("--sunk"),
    selectionBackground: v("--accent-wash"),
    red: v("--crit"),
    green: v("--ok"),
    yellow: v("--warn"),
    blue: v("--accent"),
    cyan: v("--accent"),
    brightBlack: v("--faint"),
  };
}

interface Vm {
  term: XTerm;
}

/** An xterm as a GuestConsole: opened on first attach, moved after; fitted
 *  to the inspector so the guest's output wraps instead of being cut. */
export function xtermConsole(term: XTerm): GuestConsole {
  let el: HTMLElement | undefined;
  return {
    attach: (host) => {
      if (!el) {
        term.open(host);
        el = term.element;
      } else host.appendChild(el);
      requestAnimationFrame(() => {
        const cell = term._core?._renderService?.dimensions?.css?.cell;
        const cw = cell?.width ?? 7.3;
        const ch = cell?.height ?? 15;
        const cols = Math.max(40, Math.floor((host.clientWidth - 12) / cw));
        const rows = Math.max(10, Math.floor((host.clientHeight - 8) / ch));
        if (cols !== term.cols || rows !== term.rows) term.resize(cols, rows);
      });
      setTimeout(() => term.focus(), 30);
    },
    // Up to the cursor: an opened terminal has blank rows below it, and the
    // readiness check looks for the prompt on the last line.
    tail: (lines) => {
      const b = term.buffer.active;
      const end = b.baseY + b.cursorY + 1;
      let txt = "";
      for (let i = Math.max(0, end - lines); i < end; i++) txt += (b.getLine(i)?.translateToString(true) ?? "") + "\n";
      return txt;
    },
  };
}

export class QemuBackend implements GuestBackend {
  readonly kind = "qemu-wasm";
  readonly max = 3;
  private blobs: Record<string, Uint8Array> | null = null;
  private factory: ((m: EmModule) => Promise<unknown>) | null = null;
  private vms = new Map<string, Vm>();

  /** `boxCopy`: the copy the appliance serves, which ships without the engine. */
  constructor(
    private readonly boxCopy: boolean,
    private readonly say: (text: string) => void = () => {},
  ) {}

  async probe(): Promise<Probe> {
    const label = "QEMU in this tab";
    if (!self.crossOriginIsolated) {
      return {
        available: false,
        label,
        reason: this.boxCopy
          ? "This copy of the lab ships without the qemu-wasm engine, so consoles are simulated. The hosted copy boots real guests."
          : "This page is served without COOP/COEP headers, so qemu-wasm cannot start its threads. Serve the hosted copy with its serve.json (npx serve) or vercel.json.",
      };
    }
    try {
      const r = await fetch(here("qemu/out.js"), { method: "HEAD" });
      if (!r.ok) throw new Error("missing");
    } catch {
      return { available: false, label, reason: "The qemu-wasm build is not next to this page." };
    }
    try {
      this.say("qemu");
      await loadScript(here("vendor/xterm.js"));
      await loadScript(here("vendor/xterm-pty.js"));
      const css = document.createElement("link");
      css.rel = "stylesheet";
      css.href = here("vendor/xterm.css");
      document.head.appendChild(css);
      const mod = (await import(/* @vite-ignore */ here("qemu/out.js"))) as { default: (m: EmModule) => Promise<unknown> };
      this.factory = mod.default;
      const blobs: Record<string, Uint8Array> = {};
      await Promise.all(
        Object.entries(FILES).map(async ([name, url]) => {
          const r = await fetch(here(url));
          if (!r.ok) throw new Error(url);
          blobs[name] = new Uint8Array(await r.arrayBuffer());
        }),
      );
      this.blobs = blobs;
      installSocketShim();
      trackWorkers();
      return { available: true, label, reason: "" };
    } catch (e) {
      return { available: false, label, reason: "qemu-wasm failed to load: " + (e instanceof Error ? e.message : String(e)) };
    }
  }

  async start(device: GuestDevice, cmdline: string[], nics: GuestNic[]): Promise<GuestHandle> {
    const g = window as unknown as VendorGlobals;
    if (!this.factory || !this.blobs || !g.Terminal || !g.openpty) throw new Error("qemu-wasm is not loaded");
    const id = device.id;
    const mac = nics[0]?.mac ?? "52:54:00:12:34:56";
    const { master, slave } = g.openpty();
    const term = new g.Terminal({
      fontSize: 12,
      fontFamily: "ui-monospace, Menlo, monospace",
      theme: houseTheme(),
      convertEol: false,
      scrollback: 2000,
    });
    term.loadAddon(master);
    const vm: Vm = { term };
    this.vms.set(id, vm);
    const blobs = this.blobs;
    const Module: EmModule = {
      arguments: [
        "-nographic", "-M", "pc", "-m", "96M", "-accel", "tcg,tb-size=64", "-L", "/pack/", "-vga", "none", "-nic", "none",
        "-netdev", "socket,id=n0,connect=localhost:8888", "-device", `virtio-net-pci,netdev=n0,mac=${mac},romfile=`,
        "-drive", `if=virtio,format=raw,file=/pack/${device.image === "gear" ? "gear.bin" : "rootfs.bin"},readonly=on`,
        "-kernel", "/pack/bzImage", "-append", cmdline.join(" "),
      ],
      pty: slave,
      websocket: { url: "ws://losos-lab/" + id },
      locateFile: (p: string) => here("qemu/" + p) + (p.endsWith(".worker.js") ? "?vm=" + id : ""),
      mainScriptUrlOrBlob: here("qemu/out.js"),
      preRun: [
        (mod: EmModule) => {
          mod.FS?.mkdir("/pack");
          for (const [n, b] of Object.entries(blobs)) mod.FS?.writeFile("/pack/" + n, b);
        },
      ],
      print: (t: string) => console.log(`[${device.name}]`, t),
      printErr: (t: string) => console.warn(`[${device.name}]`, t),
    };
    try {
      await this.factory(Module);
      // emscripten's TTY poll blocks while the pty has nothing to read.
      const pty = Module.pty;
      const tty = Module.TTY;
      if (pty && tty) {
        const oldPoll = tty.stream_ops.poll;
        tty.stream_ops.poll = function (stream: unknown, timeout: number) {
          if (!pty.readable) return (pty.readable ? 1 : 0) | (pty.writable ? 4 : 0);
          return oldPoll.call(this, stream, timeout);
        };
      }
    } catch (e) {
      term.write("\r\nqemu-wasm failed: " + (e instanceof Error ? e.message : String(e)) + "\r\n");
    }
    const console_ = xtermConsole(term);
    return {
      label: "QEMU in this tab",
      console: console_,
      send: (frame) => deliver(id, frame),
      onFrame: (fn) => listen(id, fn),
      stop: () => this.stop(id),
    };
  }

  stop(id: string): void {
    const vm = this.vms.get(id);
    if (!vm) return;
    for (const w of workersByVm.get(id) ?? []) w.terminate();
    workersByVm.delete(id);
    const p = ports.get(id);
    if (p?.sock) p.sock.readyState = 3;
    ports.delete(id);
    try {
      vm.term.dispose();
    } catch {
      /* already gone */
    }
    this.vms.delete(id);
  }
}
