/* The fabric: Ethernet frames from the guests, switched along the cables of
 * the diagram. Ported from admin-ui/lab/src/engine.js.
 *
 * A frame a guest sends reaches every other running guest on the same
 * segment (switches, buses and access points flood); a router that has no
 * guest of its own, and the simulated hosts on the segment, are answered
 * here: ARP, ICMP echo, and DHCP from the lease the model gave the guest.
 * What crosses is also handed to the core with start_flow(real: true), so it
 * shows on the canvas and in the event list as a guest frame. */

import type { Device, Net, Pdu } from "../core";

export interface FabricHost {
  net(): Net;
  dev(id: string): Device | undefined;
  endpoint(type: string): boolean;
  name(id: string): string;
  /** Running guests: id → its NIC's MAC and whether its console said it is ready. */
  guests(): Map<string, { mac: string; ready: boolean }>;
  deliver(id: string, frame: Uint8Array): void;
  flow(stages: Pdu[][]): void;
}

const macStr = (b: Uint8Array, o: number): string =>
  Array.from(b.slice(o, o + 6), (x) => x.toString(16).padStart(2, "0")).join(":");
const macBytes = (s: string): number[] => s.split(":").map((h) => parseInt(h, 16));
const ipStr = (b: Uint8Array, o: number): string => Array.from(b.slice(o, o + 4)).join(".");
const ipBytes = (s: string): number[] => s.split(".").map(Number);
const at = (b: Uint8Array, i: number): number => b[i] ?? 0;

export const routerMac = (rid: string): string =>
  "52:54:00:0a:" + rid.replace(/\D/g, "").padStart(2, "0").slice(-2) + ":01";

function csum(b: Uint8Array, o: number, n: number): number {
  let s = 0;
  for (let i = 0; i < n; i += 2) s += (at(b, o + i) << 8) + (i + 1 < n ? at(b, o + i + 1) : 0);
  while (s >> 16) s = (s & 0xffff) + (s >> 16);
  return ~s & 0xffff;
}

const pdu = (proto: string, from: string, to: string, info: string): Pdu => ({ proto, from, to, info, real: true });

export class Fabric {
  private lastLog = new Map<string, number>();
  constructor(private readonly h: FabricHost) {}

  private seg(id: string): string | undefined {
    const n = this.h.net();
    return n.gearSeg[id] ?? n.iface[id]?.seg;
  }
  private guestUp(id: string | undefined): boolean {
    return !!id && !!this.h.guests().get(id)?.ready;
  }

  in(src: string, f: Uint8Array): void {
    const net = this.h.net();
    const dst = macStr(f, 0);
    const type = (at(f, 12) << 8) | at(f, 13);
    const seg = this.seg(src);
    const port = (o: number) => (at(f, o) << 8) | at(f, o + 1);
    const isDhcp = type === 0x0800 && at(f, 23) === 17 && (port(34) === 68 || port(36) === 67);
    const proto =
      type === 0x0806
        ? "ARP"
        : type === 0x0800
          ? at(f, 23) === 1
            ? "ICMP"
            : isDhcp
              ? "DHCP"
              : at(f, 23) === 6 && (port(34) === 80 || port(36) === 80)
                ? "HTTP"
                : null
          : null;
    const s = seg ? net.segs[seg] : undefined;
    let toDev: string | null = null;
    if (seg) {
      for (const [id, g] of this.h.guests()) {
        if (id === src || this.seg(id) !== seg) continue;
        if (dst === "ff:ff:ff:ff:ff:ff" || at(f, 0) & 1 || dst === g.mac) {
          this.h.deliver(id, f);
          if (dst === g.mac) toDev = id;
        }
      }
    }
    if (s) this.answer(src, s, f);
    if (!proto || (proto === "DHCP" && !(s?.router && this.guestUp(s.router)))) return;
    const k = src + proto + (toDev ?? "");
    if (performance.now() - (this.lastLog.get(k) ?? 0) < 25) return;
    this.lastLog.set(k, performance.now());
    const info =
      proto === "ARP"
        ? `ARP ${at(f, 21) === 1 ? "who-has " + ipStr(f, 38) : ipStr(f, 28) + " is-at " + macStr(f, 22)}`
        : proto === "ICMP"
          ? `ICMP ${at(f, 34) === 8 ? "echo request" : at(f, 34) === 0 ? "echo reply" : "type " + at(f, 34)} ${ipStr(f, 26)} → ${ipStr(f, 30)}`
          : proto === "DHCP"
            ? `DHCP ${port(36) === 67 ? "from client " + macStr(f, 6) : "from udhcpd on " + this.h.name(src)}`
            : `TCP :80 ${ipStr(f, 26)} → ${ipStr(f, 30)}`;
    const target = toDev ?? s?.router ?? null;
    if (target && target !== src) this.h.flow([[pdu(proto, src, target, info)]]);
  }

  private answer(src: string, s: Net["segs"][string], f: Uint8Array): void {
    const net = this.h.net();
    const guests = this.h.guests();
    const type = (at(f, 12) << 8) | at(f, 13);
    const hosts: { id: string; ip: string; mac: string }[] = [];
    if (s.router && this.h.dev(s.router)?.power && !this.guestUp(s.router)) {
      const ip = net.addr[s.router + "#lan"]?.ip;
      if (ip) hosts.push({ id: s.router, ip, mac: routerMac(s.router) });
    }
    for (const id of s.members) {
      const d = this.h.dev(id);
      if (id === src || guests.has(id) || !d || !this.h.endpoint(d.type)) continue;
      const a = net.addr[id];
      if (a) hosts.push({ id, ip: a.ip, mac: d.mac });
    }
    for (const [id, sg] of Object.entries(net.gearSeg)) {
      const d = this.h.dev(id);
      if (sg !== s.id || id === src || guests.has(id) || !d || d.type === "router") continue;
      const m = net.mgmt[id];
      if (m) hosts.push({ id, ip: m.ip, mac: d.mac });
    }
    if (type === 0x0806 && at(f, 21) === 1) {
      const want = ipStr(f, 38);
      const h = hosts.find((x) => x.ip === want);
      if (!h) return;
      const r = new Uint8Array(42);
      r.set(f.slice(6, 12), 0);
      r.set(macBytes(h.mac), 6);
      r[12] = 8;
      r[13] = 6;
      r.set([0, 1, 8, 0, 6, 4, 0, 2], 14);
      r.set(macBytes(h.mac), 22);
      r.set(ipBytes(h.ip), 28);
      r.set(f.slice(22, 28), 32);
      r.set(f.slice(28, 32), 38);
      setTimeout(() => {
        this.h.deliver(src, r);
        this.h.flow([[pdu("ARP", h.id, src, `ARP ${h.ip} is-at ${h.mac}`)]]);
      }, 2);
      return;
    }
    if (type !== 0x0800) return;
    const ihl = (at(f, 14) & 15) * 4;
    const p = at(f, 23);
    const dstIp = ipStr(f, 30);
    if (p === 1 && at(f, 14 + ihl) === 8) {
      const h = hosts.find((x) => x.ip === dstIp);
      if (!h) return;
      const r = f.slice();
      r.set(f.slice(6, 12), 0);
      r.set(macBytes(h.mac), 6);
      r.set(f.slice(30, 34), 26);
      r.set(f.slice(26, 30), 30);
      r[22] = 64;
      r[24] = 0;
      r[25] = 0;
      const ic = csum(r, 14, ihl);
      r[24] = ic >> 8;
      r[25] = ic & 255;
      const o = 14 + ihl;
      r[o] = 0;
      r[o + 2] = 0;
      r[o + 3] = 0;
      const c2 = csum(r, o, r.length - o);
      r[o + 2] = c2 >> 8;
      r[o + 3] = c2 & 255;
      setTimeout(() => {
        this.h.deliver(src, r);
        this.h.flow([[pdu("ICMP", h.id, src, `ICMP echo reply ${h.ip} → ${ipStr(f, 26)} (answered by the simulator)`)]]);
      }, 3);
      return;
    }
    const rt = hosts[0];
    if (
      p === 17 &&
      ((at(f, 14 + ihl + 2) << 8) | at(f, 14 + ihl + 3)) === 67 &&
      s.router &&
      this.h.dev(s.router)?.power &&
      !this.guestUp(s.router) &&
      rt
    ) {
      this.dhcp(src, s.router, f, 14 + ihl + 8, rt);
    }
  }

  private dhcp(src: string, router: string, f: Uint8Array, o: number, rt: { ip: string; mac: string }): void {
    let mt = 0;
    for (let i = o + 240; i < f.length && at(f, i) !== 255; ) {
      const c = at(f, i);
      const l = at(f, i + 1);
      if (c === 53) mt = at(f, i + 2);
      i += c === 0 ? 1 : 2 + l;
    }
    if (mt !== 1 && mt !== 3) return;
    const lease = this.h.net().addr[src];
    if (!lease || lease.src !== "dhcp" || !lease.gw) return;
    const opts = [
      53, 1, mt === 1 ? 2 : 5, 54, 4, ...ipBytes(rt.ip), 51, 4, 0, 1, 81, 128, 1, 4, 255, 255, 255, 0, 3, 4,
      ...ipBytes(lease.gw), 6, 4, ...ipBytes(lease.gw), 255,
    ];
    const bootp = new Uint8Array(240 + opts.length);
    bootp[0] = 2;
    bootp[1] = 1;
    bootp[2] = 6;
    bootp.set(f.slice(o + 4, o + 8), 4);
    bootp.set(ipBytes(lease.ip), 16);
    bootp.set(ipBytes(rt.ip), 20);
    bootp.set(f.slice(o + 28, o + 44), 28);
    bootp.set([99, 130, 83, 99], 236);
    bootp.set(opts, 240);
    const udpLen = 8 + bootp.length;
    const ipLen = 20 + udpLen;
    const r = new Uint8Array(14 + ipLen);
    r.set([255, 255, 255, 255, 255, 255], 0);
    r.set(macBytes(rt.mac), 6);
    r[12] = 8;
    r[13] = 0;
    r.set([0x45, 0, ipLen >> 8, ipLen & 255, 0, 0, 0, 0, 64, 17, 0, 0], 14);
    r.set(ipBytes(rt.ip), 26);
    r.set([255, 255, 255, 255], 30);
    const c = csum(r, 14, 20);
    r[24] = c >> 8;
    r[25] = c & 255;
    r.set([0, 67, 0, 68, udpLen >> 8, udpLen & 255, 0, 0], 34);
    r.set(bootp, 42);
    setTimeout(() => this.h.deliver(src, r), 5);
    this.h.flow([
      [pdu("DHCP", src, router, `DHCP${mt === 1 ? "DISCOVER" : "REQUEST"} from the guest's udhcpc`)],
      [pdu("DHCP", router, src, `DHCP${mt === 1 ? "OFFER" : "ACK"} ${lease.ip} (router in the simulator)`)],
    ]);
  }
}
