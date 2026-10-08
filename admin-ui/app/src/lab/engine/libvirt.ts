/* Guests under libvirt, through `losos-registrar lab` and its `virsh`.
 *
 * The helper starts each guest as a transient libvirt domain (KVM when the
 * machine has it) and bridges its serial console and network card to two
 * WebSockets; the Lab stays the switch fabric, so a libvirt guest's frames
 * cross the drawn cables exactly like a qemu-wasm guest's. Ported from
 * admin-ui/lab/src/engine-libvirt.js; wiki/Lab.md, "Guests under libvirt",
 * has the design. Where the helper is, and who may ask it, is helper.ts. */

import { guestTerminal, helperFor, stream, type Hello, type Helper } from "./helper";
import type { GuestBackend, GuestDevice, GuestHandle, GuestNic, Probe } from "./types";

export class LibvirtBackend implements GuestBackend {
  readonly kind = "libvirt";
  /* The helper refuses past its own --max-guests, and a refusal falls back
   * to the next backend; the Lab's cap of three is qemu-wasm's alone. */
  readonly max = Infinity;
  private hello: Hello | null = null;
  private handles = new Map<string, GuestHandle & { kill(): void }>();
  private readonly helper: Helper;

  constructor(private readonly boxCopy: boolean) {
    this.helper = helperFor(boxCopy);
    window.addEventListener("pagehide", () => {
      for (const h of [...this.handles.values()]) h.kill();
    });
  }

  async probe(): Promise<Probe> {
    const no = (reason: string): Probe => ({ available: false, label: "", reason });
    const r = await this.helper.hello();
    if (!r.ok) {
      switch (r.why) {
        case "notAsked":
          return no("not asked: open the lab with ?libvirt to look for losos-registrar lab on this computer");
        case "signedOut":
          return no("sign in on the admin page first");
        case "status":
          return no("the helper answered " + r.status);
        default:
          return no(this.boxCopy ? "the box did not answer" : "no losos-registrar lab on 127.0.0.1:8095");
      }
    }
    const h = r.hello;
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
  }

  /** Network gear boots gear.bin, which the helper may not have. */
  canRun(device: GuestDevice): boolean {
    return !!this.hello && (device.image !== "gear" || !!this.hello.images?.gear);
  }

  async start(device: GuestDevice, cmdline: string[], nics: GuestNic[]): Promise<GuestHandle> {
    const macs = nics.map((n) => n.mac);
    const r = await this.helper.call(
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
    const base = encodeURIComponent(guest);
    const con = this.helper.socket(this.helper.guestSocketUrl(base + "/console"), ticket);
    const nicSockets = macs.map((_, i) => this.helper.socket(this.helper.guestSocketUrl(base + "/nic/" + i), ticket));
    const conStream = stream(con);
    const nicStreams = nicSockets.map(stream);
    const label = body.label || this.hello?.label || "libvirt";
    const term = guestTerminal(device.name, (s) => conStream.send(s));
    conStream.on((b) => term.write(b));

    let closed: (() => void) | null = null;
    const handle = {
      label,
      console: term.console,
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
        term.dispose();
        this.helper.call("guests/" + encodeURIComponent(guest), { method: "DELETE", keepalive: true }, 10000).catch(() => {});
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
