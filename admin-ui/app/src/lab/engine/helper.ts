/* `losos-registrar lab` as both libvirt backends see it: where it is, whether
 * this page may ask it at all, its hello, and the pieces both put around a
 * guest (a socket's messages as a stream, a console terminal).
 *
 * Where the helper is:
 *   box copy     /api/lab/ on this origin (lososd relays it, with the admin
 *                key this tab holds); sockets at /api/lab/ws/ and
 *                /api/lab/virt. Always asked.
 *   hosted copy  http://127.0.0.1:8095/lab/v1/ on the viewer's own machine.
 *                Asked only when the page itself is on loopback, or the
 *                viewer opted in with ?libvirt (remembered; ?libvirt=0
 *                forgets): Chrome asks every visitor of a public page for
 *                local network access the moment it touches 127.0.0.1.
 *
 * Each socket authenticates with a ticket the helper handed out, as the
 * subprotocol `ticket.<hex>` beside `losos-lab`: a browser cannot set an
 * Authorization header on a WebSocket. */

import { ByteTerm } from "./byteterm";
import { houseTheme, xtermConsole, type VendorGlobals } from "./qemu";
import type { GuestConsole } from "./types";

const HOSTED = "http://127.0.0.1:8095/lab/v1/";
const OPT_IN = "losos-lab-libvirt";
const TOKEN_KEY = "losos-token";
/** The WebSocket subprotocol the helper answers with. */
export const PROTOCOL = "losos-lab";

/** `GET hello`, as far as the Lab reads it. */
export interface Hello {
  /** The virsh path: libvirt answered `virsh` and the images are there. */
  available: boolean;
  reason?: string | null;
  label?: string;
  /** Whether `virsh` answered at all. */
  virsh?: boolean;
  /** "kvm" or "qemu", as the virsh path decided. */
  domainType?: string;
  images?: { kernel?: boolean; rootfs?: boolean; gear?: boolean; dir?: string };
  maxGuests?: number;
  memoryMiB?: number;
  /** The relay path: libvirt's socket accepts a connection now. */
  virt?: { available?: boolean; socket?: string | null; reason?: string | null };
}

export type HelloResult =
  | { ok: true; hello: Hello }
  | { ok: false; why: "notAsked" | "signedOut" | "noAnswer" }
  | { ok: false; why: "status"; status: number };

function token(): string | null {
  try {
    return window.sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

export class Helper {
  private hi: Promise<HelloResult> | null = null;

  constructor(readonly boxCopy: boolean) {}

  /** The helper's HTTP base, its WebSocket base and the bearer key, if any. */
  where(): { api: string; ws: string; auth: string | null } {
    if (this.boxCopy) {
      const scheme = location.protocol === "https:" ? "wss://" : "ws://";
      return { api: "/api/lab/", ws: scheme + location.host + "/api/lab/", auth: token() };
    }
    return { api: HOSTED, ws: HOSTED.replace(/^http/, "ws"), auth: null };
  }

  /** A guest socket: `<key>/console` or `<key>/nic/<n>`. */
  guestSocketUrl(rest: string): string {
    return this.where().ws + (this.boxCopy ? "ws/" : "guests/") + rest;
  }

  /** The libvirt relay socket. */
  virtUrl(): string {
    return this.where().ws + "virt";
  }

  optedIn(): boolean {
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

  async call(path: string, opts: RequestInit = {}, ms = 1500): Promise<Response> {
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

  /** One hello per page: both backends probe from the same answer. */
  hello(): Promise<HelloResult> {
    this.hi ??= this.askHello();
    return this.hi;
  }

  private async askHello(): Promise<HelloResult> {
    if (!this.optedIn()) return { ok: false, why: "notAsked" };
    if (this.boxCopy && !this.where().auth) return { ok: false, why: "signedOut" };
    try {
      const r = await this.call("hello");
      if (!r.ok) return { ok: false, why: "status", status: r.status };
      return { ok: true, hello: (await r.json()) as Hello };
    } catch {
      return { ok: false, why: "noAnswer" };
    }
  }

  /** A guest socket with the helper's ticket, binary. */
  socket(url: string, ticket: string): WebSocket {
    const s = new WebSocket(url, [PROTOCOL, "ticket." + ticket]);
    s.binaryType = "arraybuffer";
    return s;
  }
}

const helpers = new Map<boolean, Helper>();
/** The page's one Helper for this copy. */
export function helperFor(boxCopy: boolean): Helper {
  let h = helpers.get(boxCopy);
  if (!h) {
    h = new Helper(boxCopy);
    helpers.set(boxCopy, h);
  }
  return h;
}

export interface Stream {
  on(cb: (b: Uint8Array) => void): void;
  send(data: string | Uint8Array): void;
}

/** A socket's messages, held until someone listens (the banner arrives early). */
export function stream(s: WebSocket): Stream {
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

/** A real guest's serial console: xterm where the copy ships it (the hosted
 *  one), the byte terminal on the box. `send` takes what the viewer types. */
export function guestTerminal(
  name: string,
  send: (s: string) => void,
): { console: GuestConsole; write(b: Uint8Array): void; dispose(): void } {
  const g = window as unknown as VendorGlobals;
  if (g.Terminal) {
    const term = new g.Terminal({ fontSize: 12, fontFamily: "ui-monospace, Menlo, monospace", theme: houseTheme(), scrollback: 2000 });
    term.onData(send);
    return {
      console: xtermConsole(term),
      write: (b) => term.write(b),
      dispose: () => {
        try {
          term.dispose();
        } catch {
          /* already gone */
        }
      },
    };
  }
  const raw = new ByteTerm("Console of " + name);
  raw.send = send;
  return { console: raw, write: (b) => raw.write(b), dispose: () => {} };
}
