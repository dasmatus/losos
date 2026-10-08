/* Guests under libvirt, through `losos-registrar lab`.
 *
 * The helper starts each guest as a transient libvirt domain (KVM when the
 * machine has it) and bridges its serial console and network card to two
 * WebSockets; the Lab stays the switch fabric, so a libvirt guest's frames
 * cross the drawn cables exactly like a qemu-wasm guest's. Ported from
 * admin-ui/lab/src/engine-libvirt.js; wiki/Lab.md, "Guests under libvirt",
 * has the design.
 *
 * Where the helper is:
 *   box copy     /api/lab/ on this origin (lososd relays it, with the admin
 *                key this tab holds); sockets at /api/lab/ws/. Always asked.
 *   hosted copy  http://127.0.0.1:8095/lab/v1/ on the viewer's own machine.
 *                Asked only when the page itself is on loopback, or the
 *                viewer opted in with ?libvirt (remembered; ?libvirt=0
 *                forgets): Chrome asks every visitor of a public page for
 *                local network access the moment it touches 127.0.0.1.
 *
 * Each socket authenticates with the per-guest ticket the POST returned, as
 * the subprotocol `ticket.<hex>` beside `losos-lab`: a browser cannot set an
 * Authorization header on a WebSocket. */

import { ByteTerm } from "./byteterm";
import { houseTheme, xtermConsole, type VendorGlobals } from "./qemu";
import type { GuestBackend, GuestDevice, GuestHandle, GuestNic, Probe } from "./types";

const HOSTED = "http://127.0.0.1:8095/lab/v1/";
const OPT_IN = "losos-lab-libvirt";
const TOKEN_KEY = "losos-token";

interface Hello {
  available: boolean;
  reason?: string;
  label?: string;
  images?: { gear?: boolean; [k: string]: unknown };
  maxGuests?: number;
}

interface Stream {
  on(cb: (b: Uint8Array) => void): void;
  send(data: string | Uint8Array): void;
}

function token(): string | null {
  try {
    return window.sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

/** A socket's messages, held until someone listens (the banner arrives early). */
function stream(s: WebSocket): Stream {
  const subs: ((b: Uint8Array) => void)[] = [];
  const early: Uint8Array[] = [];
  s.onmessage = (e: MessageEvent<ArrayBuffer>) => {
    const b = new Uint8Array(e.data);
    if (subs.length) for (const f of subs) f(b);
    else early.push(b);
  };
  return {
    on(cb) {
      subs.push(cb);
      while (early.length) cb(early.shift()!);
    },
    send(data) {
      if (s.readyState !== WebSocket.OPEN) return;
      // a copy, so a view of a shared buffer (qemu-wasm's memory) goes out too
      s.send(typeof data === "string" ? new TextEncoder().encode(data) : new Uint8Array(data));
    },
  };
}

export class LibvirtBackend implements GuestBackend {
  readonly kind = "libvirt";
  /* The helper refuses past its own --max-guests, and a refusal falls back
   * to the next backend; the Lab's cap of three is qemu-wasm's alone. */
  readonly max = Infinity;
  private hello: Hello | null = null;
  private handles = new Map<string, GuestHandle & { kill(): void }>();

  constructor(private readonly boxCopy: boolean) {
    window.addEventListener("pagehide", () => {
      for (const h of [...this.handles.values()]) h.kill();
    });
  }

  private where(): { api: string; ws: string; auth: string | null } {
    if (this.boxCopy) {
      const scheme = location.protocol === "https:" ? "wss://" : "ws://";
      return { api: "/api/lab/", ws: scheme + location.host + "/api/lab/ws/", auth: token() };
    }
    return { api: HOSTED, ws: HOSTED.replace(/^http/, "ws") + "guests/", auth: null };
  }

  private optedIn(): boolean {
    if (this.boxCopy) return true;
    if (/^(localhost|127\.\d+\.\d+\.\d+|\[::1\])$/.test(location.hostname)) return true;
    const q = new URLSearchParams(location.search).get("libvirt");
    try {
      if (q === "0") window.localStorage.removeItem(OPT_IN);
      else if (q !== null) window.localStorage.setItem(OPT_IN, "1");
      return window.localStorage.getItem(OPT_IN) === "1";
    } catch {
      return q !== null && q !== "0";
    }
  }

  private async call(path: string, opts: RequestInit = {}, ms = 1500): Promise<Response> {
    const w = this.where();
    const ctl = new AbortController();
    const timer = window.setTimeout(() => ctl.abort(), ms);
    const headers = new Headers(opts.headers);
    if (w.auth) headers.set("Authorization", "Bearer " + w.auth);
    try {
      return await fetch(w.api + path, { ...opts, headers, signal: ctl.signal, cache: "no-store" });
    } finally {
      window.clearTimeout(timer);
    }
  }

  async probe(): Promise<Probe> {
    const no = (reason: string): Probe => ({ available: false, label: "", reason });
    if (!this.optedIn()) return no("not asked: open the lab with ?libvirt to look for losos-registrar lab on this computer");
    if (this.boxCopy && !this.where().auth) return no("sign in on the admin page first");
    try {
      const r = await this.call("hello");
      if (!r.ok) return no("the helper answered " + r.status);
      const h = (await r.json()) as Hello;
      if (!h.available) {
        return no(
          h.reason === "off"
            ? "this box runs no libvirt helper (losos.lab.libvirt.enable)"
            : h.reason === "notRunning"
              ? "the libvirt helper is not running"
              : (h.reason ?? "not available"),
        );
      }
      this.hello = h;
      return { available: true, label: h.label || "libvirt", reason: "" };
    } catch {
      return no(this.boxCopy ? "the box did not answer" : "no losos-registrar lab on 127.0.0.1:8095");
    }
  }

  /** Network gear boots gear.bin, which the helper may not have. */
  canRun(device: GuestDevice): boolean {
    return !!this.hello && (device.image !== "gear" || !!this.hello.images?.gear);
  }

  private socket(url: string, ticket: string): WebSocket {
    const s = new WebSocket(url, ["losos-lab", "ticket." + ticket]);
    s.binaryType = "arraybuffer";
    return s;
  }

  async start(device: GuestDevice, cmdline: string[], nics: GuestNic[]): Promise<GuestHandle> {
    const macs = nics.map((n) => n.mac);
    const r = await this.call(
      "guests",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id: device.id, name: device.name, role: device.role, cmdline: cmdline.join(" "), macs }),
      },
      45000,
    );
    type Started = { guest?: string; ticket?: string; label?: string; error?: string };
    const body: Started = await r.json().then(
      (j: unknown) => (j && typeof j === "object" ? (j as Started) : {}),
      () => ({}),
    );
    if (!r.ok || !body.guest || !body.ticket) throw new Error(body.error || "the helper answered " + r.status);
    const guest = body.guest;
    const ticket = body.ticket;
    const w = this.where();
    const base = w.ws + encodeURIComponent(guest);
    const con = this.socket(base + "/console", ticket);
    const nicSockets = macs.map((_, i) => this.socket(base + "/nic/" + i, ticket));
    const conStream = stream(con);
    const nicStreams = nicSockets.map(stream);
    const label = body.label || this.hello?.label || "libvirt";

    // xterm where the copy ships it (the hosted one), the byte terminal on the box.
    const g = window as unknown as VendorGlobals;
    let console_: GuestHandle["console"];
    let disposeTerm = () => {};
    if (g.Terminal) {
      const term = new g.Terminal({ fontSize: 12, fontFamily: "ui-monospace, Menlo, monospace", theme: houseTheme(), scrollback: 2000 });
      term.onData((s) => conStream.send(s));
      conStream.on((b) => term.write(b));
      console_ = xtermConsole(term);
      disposeTerm = () => {
        try {
          term.dispose();
        } catch {
          /* already gone */
        }
      };
    } else {
      const raw = new ByteTerm("Console of " + device.name);
      raw.send = (s) => conStream.send(s);
      conStream.on((b) => raw.write(b));
      console_ = raw;
    }

    let closed: (() => void) | null = null;
    const handle = {
      label,
      console: console_,
      send: (frame: Uint8Array) => nicStreams[0]?.send(frame),
      onFrame: (fn: (frame: Uint8Array) => void) => {
        let live = true;
        nicStreams[0]?.on((f) => {
          if (live) fn(f);
        });
        return () => {
          live = false;
        };
      },
      onClose: (fn: () => void) => {
        closed = fn;
      },
      stop: () => this.stop(device.id),
      kill: () => {
        if (this.handles.get(device.id) !== handle) return;
        this.handles.delete(device.id);
        for (const s of [con, ...nicSockets]) {
          s.onclose = null;
          try {
            s.close();
          } catch {
            /* closed */
          }
        }
        disposeTerm();
        this.call("guests/" + encodeURIComponent(guest), { method: "DELETE", keepalive: true }, 10000).catch(() => {});
      },
    };
    con.onclose = () => {
      if (this.handles.get(device.id) === handle) closed?.();
    };
    this.handles.set(device.id, handle);
    return handle;
  }

  stop(id: string): void {
    this.handles.get(id)?.kill();
  }
}
