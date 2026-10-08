/* The Lab's guests: which backends run them, which devices have one, and the
 * fabric between them. The UI asks three things of it: is a backend there
 * (and what to say on the badge), start or stop the guest behind a device,
 * and mount a running guest's console.
 *
 * Backends are probed in the order given (app.tsx: libvirt through the
 * helper's virsh, then libvirt through the page's WebAssembly client and
 * the helper's relay, then qemu-wasm) and every one that answers is kept.
 * A guest goes to the first that can run its device; when that one
 * refuses, it falls through to the next, and the Lab says so once. A
 * device whose start every backend refused keeps its simulated console.
 *
 * Guests start one after another: two emscripten instances initialising at
 * the same moment raced each other and one never printed. At most `max` of
 * one backend at once (3 for qemu-wasm: with a fourth the tab ran out of
 * cores and every guest stalled; libvirt counts for itself). */

import type { LabStore } from "../store";
import { Fabric, routerMac } from "./fabric";
import type { GuestBackend, GuestDevice, GuestHandle, Probe } from "./types";

export type { GuestBackend, Probe } from "./types";

interface Running {
  backend: GuestBackend;
  handle: GuestHandle | null;
  mac: string;
  ready: boolean;
  poll?: number;
  off?: () => void;
}

interface Ready {
  backend: GuestBackend;
  probe: Probe;
}

export class Engine {
  /** What the badge says: the backends that answered, or why none did. */
  probe: Probe = { available: false, label: "", reason: "" };
  private backends: Ready[] = [];
  /** Device → backend kinds that already refused it, so a retry falls through. */
  private refused = new Map<string, Set<string>>();
  private toldFallback = false;
  private running = new Map<string, Running>();
  private chain: Promise<unknown> = Promise.resolve();
  private fabric: Fabric;
  private listeners = new Set<() => void>();
  version = 0;

  constructor(private readonly store: LabStore) {
    this.fabric = new Fabric({
      net: () => store.snap.net,
      dev: (id) => store.dev(id),
      endpoint: (type) => !!store.catalog.types[type]?.endpoint,
      name: (id) => store.name(id),
      guests: () => {
        const m = new Map<string, { mac: string; ready: boolean }>();
        for (const [id, r] of this.running) if (r.handle) m.set(id, { mac: r.mac, ready: r.ready });
        return m;
      },
      deliver: (id, frame) => this.running.get(id)?.handle?.send(frame),
      flow: (stages) => store.core.startFlow(stages, ""),
    });
  }

  /** Probe every backend in order; each one that answers runs guests. */
  async init(backends: GuestBackend[]): Promise<void> {
    const reasons: string[] = [];
    for (const b of backends) {
      const p = await b.probe();
      if (p.available) this.backends.push({ backend: b, probe: p });
      else if (p.reason) reasons.push(b.kind === "qemu-wasm" ? p.reason : `${b.kind}: ${p.reason}.`);
    }
    // The tab's own engine first in the tooltip: it is the one most copies lack.
    reasons.sort((x, y) => Number(/^\w+: /.test(x)) - Number(/^\w+: /.test(y)));
    const [first, ...rest] = this.backends;
    this.probe = first
      ? {
          available: true,
          label: first.probe.label + (rest.length ? ", " + rest.map((r) => r.probe.label).join(", ") + " as the fallback" : ""),
          reason: "",
        }
      : { available: false, label: "", reason: reasons.join(" ") || "No guest engine is built into this copy." };
    this.changed();
  }

  get available(): boolean {
    return this.backends.length > 0;
  }
  /** The kinds that answered, first first ("libvirt", "virt-rpc", "qemu-wasm"). */
  get kinds(): string[] {
    return this.backends.map((b) => b.backend.kind);
  }
  /** qemu-wasm's cap, for the "N guests are running" line. */
  get max(): number {
    const capped = this.backends.find((b) => Number.isFinite(b.backend.max));
    return capped?.backend.max ?? 0;
  }
  isRunning(id: string): boolean {
    return this.running.has(id);
  }
  private count(b: GuestBackend): number {
    let n = 0;
    for (const r of this.running.values()) if (r.backend === b) n++;
    return n;
  }

  private guestOf(id: string): GuestDevice | null {
    const d = this.store.dev(id);
    const role = d ? this.store.catalog.types[d.type]?.emulate : undefined;
    if (!d || !role) return null;
    return { id, name: d.name, type: d.type, image: this.store.isGear(d) ? "gear" : "losos", role };
  }
  /** The backends that would take this device, in order, skipping refusals. */
  private candidates(id: string): GuestBackend[] {
    const g = this.guestOf(id);
    if (!g) return [];
    const no = this.refused.get(id);
    return this.backends.map((b) => b.backend).filter((b) => !no?.has(b.kind) && (b.canRun?.(g) ?? true));
  }
  /** Whether any backend could boot this device; if not, its console stays simulated. */
  canBoot(id: string): boolean {
    return this.running.has(id) || this.candidates(id).length > 0;
  }
  /** Every backend that would take it is at its cap. */
  full(id?: string): boolean {
    const list = id ? this.candidates(id) : this.backends.map((b) => b.backend);
    return list.length > 0 && list.every((b) => this.count(b) >= b.max);
  }
  /** What runs (or would run) a device's guest, for the console header. */
  backendLabel(id: string): string {
    const r = this.running.get(id);
    if (r) return r.handle?.label ?? this.labelOf(r.backend);
    const next = this.candidates(id).find((b) => this.count(b) < b.max) ?? this.candidates(id)[0];
    return next ? this.labelOf(next) : "";
  }
  private labelOf(b: GuestBackend): string {
    return this.backends.find((x) => x.backend === b)?.probe.label ?? b.kind;
  }

  subscribe(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
  private changed(): void {
    this.version += 1;
    for (const fn of this.listeners) fn();
  }

  /** Queue a guest for `id`; resolves once it was started or refused. */
  start(id: string): Promise<void> {
    const p = this.chain.then(() => this.startNow(id)).then(() => new Promise((r) => setTimeout(r, 400)));
    this.chain = p.catch(() => {});
    return p.then(() => {});
  }

  private async startNow(id: string): Promise<void> {
    const s = this.store;
    if (this.running.has(id)) return;
    const d = s.dev(id);
    const g = this.guestOf(id);
    if (!d || !g) return;
    const net = s.snap.net;
    const a = net.addr[id];
    const segId = net.iface[id]?.seg;
    const seg = segId ? net.segs[segId] : undefined;
    const append = [
      "console=ttyS0", "root=/dev/vda", "ro", "rootwait", "loglevel=4", "no_timer_check",
      `losos.host=${d.name}`, `losos.role=${d.type === "box" ? "box" : d.type}`,
    ];
    if (g.image === "gear") {
      const m = net.mgmt[id];
      if (m) append.push(`losos.ip=${m.ip}/${m.mask}`);
      if (m?.gw) append.push(`losos.gw=${m.gw}`);
      if (d.type === "router" && m) {
        const sn = net.subnet[id];
        append.push(`losos.dhcp=${sn}.100-${sn}.199`);
        const leases = Object.entries(net.addr)
          .filter(([k, v]) => v.router === id && v.src === "dhcp" && s.dev(k))
          .map(([k, v]) => `${s.dev(k)?.mac}@${v.ip}`);
        if (leases.length) append.push("losos.leases=" + leases.join(","));
      }
    } else if (a && !(seg && seg.kind === "lan")) append.push(`losos.ip=${a.ip}/${a.mask}`);
    const mac = d.type === "router" ? routerMac(id) : d.mac;

    const list = this.candidates(id);
    for (const [i, backend] of list.entries()) {
      const last = i === list.length - 1;
      if (this.count(backend) >= backend.max) {
        if (last) s.toast(`${backend.max} guests are running, as many as one tab runs well. Power one off to boot this one.`);
        continue;
      }
      const r: Running = { backend, handle: null, mac, ready: false };
      this.running.set(id, r);
      this.changed();
      try {
        const handle = await backend.start(g, append, [{ mac }]);
        if (this.running.get(id) !== r) {
          handle.stop();
          return;
        }
        r.handle = handle;
        r.off = handle.onFrame((frame) => this.fabric.in(id, frame));
        handle.onClose?.(() => {
          if (this.running.get(id) === r) this.stop(id);
        });
        // The guest is ready once its console says so and shows a prompt; a
        // router's guest then takes over DHCP, ARP and ping from the fabric.
        r.poll = window.setInterval(() => {
          const txt = handle.console.tail(40);
          if (/is ready/.test(txt) && /# *$/m.test(txt)) {
            r.ready = true;
            window.clearInterval(r.poll);
            this.changed();
          }
        }, 1000);
        this.changed();
        return;
      } catch (e) {
        if (this.running.get(id) === r) this.running.delete(id);
        const why = e instanceof Error ? e.message : String(e);
        const no = this.refused.get(id) ?? new Set<string>();
        no.add(backend.kind);
        this.refused.set(id, no);
        const next = list.slice(i + 1).find((b) => this.count(b) < b.max);
        if (!this.toldFallback || !next) {
          this.toldFallback = true;
          s.toast(
            `${this.labelOf(backend)} could not start ${d.name}: ${why}. ` +
              (next ? `It runs as ${this.labelOf(next)} instead.` : "Its console stays simulated."),
          );
        }
        this.changed();
      }
    }
  }

  stop(id: string): void {
    const r = this.running.get(id);
    if (!r) return;
    window.clearInterval(r.poll);
    r.off?.();
    r.handle?.stop();
    r.backend.stop(id);
    this.running.delete(id);
    this.changed();
  }

  stopAll(): void {
    for (const id of [...this.running.keys()]) this.stop(id);
  }

  attach(id: string, host: HTMLElement): boolean {
    const h = this.running.get(id)?.handle;
    if (!h) return false;
    h.console.attach(host);
    return true;
  }

  /** Called after every snapshot: a guest whose device is gone or off stops. */
  reconcile(): void {
    for (const id of [...this.running.keys()]) {
      const d = this.store.dev(id);
      if (!d || !d.power) this.stop(id);
    }
  }
}
