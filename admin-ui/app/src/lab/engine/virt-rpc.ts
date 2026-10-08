/* Guests under libvirt, driven by this page itself.
 *
 * The WebAssembly libvirt client (admin-ui/lab/virt-rpc, `losos-lab-virt`)
 * speaks libvirt's own remote protocol, over the helper's byte relay to
 * libvirt's socket (`GET virt`, admitted with a single-use ticket from
 * `POST virt-ticket`). So this path needs `losos-registrar lab` and a
 * running libvirt daemon, but no `virsh`: it is what runs the guests when
 * the helper's own virsh path cannot.
 *
 * Per guest: a ticket, one relayed connection, a transient domain created
 * paused with AUTODESTROY (it dies with the connection, so a closed tab ends
 * it), its console opened before it is resumed (so the first byte is seen),
 * then resumed. The domain XML is the helper's (backend-registrar/src/lab/
 * spec.rs) with the serial port as a pty, read through DOMAIN_OPEN_CONSOLE.
 *
 * The network card: the ticket comes with a UDP tunnel the helper owns and
 * bridges at `guests/<key>/nic/0`, exactly as for its own guests, so these
 * guests are on the fabric too. A helper that is full hands out no card;
 * the guest then boots with no network and its console header says so.
 *
 * The client's module (about 220 KB) is imported only when the first guest
 * starts here, not when the page loads. */

import { guestTerminal, helperFor, stream, type Hello, type Helper } from "./helper";
import type { GuestBackend, GuestDevice, GuestHandle, GuestNic, Probe } from "./types";
import type { VirtClient, VirtConsole } from "../virt-pkg/losos_lab_virt.js";

type VirtModule = typeof import("../virt-pkg/losos_lab_virt.js");

let loading: Promise<VirtModule> | null = null;
/** The client, compiled and initialised once per page. */
function loadClient(): Promise<VirtModule> {
  loading ??= (async () => {
    const [mod, wasm] = await Promise.all([
      import("../virt-pkg/losos_lab_virt.js"),
      import("../virt-pkg/losos_lab_virt_bg.wasm?url"),
    ]);
    await mod.default({ module_or_path: wasm.default });
    return mod;
  })();
  loading.catch(() => {
    loading = null;
  });
  return loading;
}

/** What `POST virt-ticket` answers. */
interface Ticket {
  ticket?: string;
  uri?: string;
  nic?: { guest: string; domain: string; ticket: string; helperPort: number; qemuPort: number } | null;
  error?: string;
}

/** XML-escape for an element body or a single-quoted attribute. */
const esc = (s: string): string =>
  s.replace(/[&<>'"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&apos;", '"': "&quot;" })[c]!);

export interface DomainSpec {
  name: string;
  title: string;
  type: "kvm" | "qemu";
  memoryMiB: number;
  kernel: string;
  disk: string;
  cmdline: string;
  /** The NIC: its MAC and the two ends of the helper's UDP tunnel. */
  nic: { mac: string; helperPort: number; qemuPort: number } | null;
}

/** The transient domain, as the helper's spec::domain_xml writes it, but
 *  with the serial port a pty the client reads as the domain's console. */
export function domainXml(d: DomainSpec): string {
  const nic = d.nic
    ? [
        "    <interface type='udp'>",
        `      <mac address='${esc(d.nic.mac)}'/>`,
        `      <source address='127.0.0.1' port='${d.nic.helperPort | 0}'>`,
        `        <local address='127.0.0.1' port='${d.nic.qemuPort | 0}'/>`,
        "      </source>",
        "      <model type='virtio'/>",
        "    </interface>",
      ]
    : [];
  return [
    `<domain type='${d.type}'>`,
    `  <name>${esc(d.name)}</name>`,
    `  <title>${esc(d.title)}</title>`,
    `  <memory unit='MiB'>${d.memoryMiB | 0}</memory>`,
    "  <vcpu>1</vcpu>",
    "  <os>",
    "    <type arch='x86_64' machine='pc'>hvm</type>",
    `    <kernel>${esc(d.kernel)}</kernel>`,
    `    <cmdline>${esc(d.cmdline)}</cmdline>`,
    "  </os>",
    "  <features><acpi/></features>",
    "  <clock offset='utc'/>",
    "  <on_poweroff>destroy</on_poweroff>",
    "  <on_reboot>restart</on_reboot>",
    "  <on_crash>destroy</on_crash>",
    "  <devices>",
    "    <disk type='file' device='disk'>",
    "      <driver name='qemu' type='raw'/>",
    `      <source file='${esc(d.disk)}'/>`,
    "      <target dev='vda' bus='virtio'/>",
    "      <readonly/>",
    "    </disk>",
    ...nic,
    "    <serial type='pty'><target port='0'/></serial>",
    "    <console type='pty'><target type='serial' port='0'/></console>",
    "    <controller type='usb' model='none'/>",
    "    <memballoon model='none'/>",
    "  </devices>",
    "</domain>",
    "",
  ].join("\n");
}

/** CONNECT_OPEN wants the URI without the client-side transport parameters
 *  (`?socket=…` named the socket for the helper, not for the daemon). */
const daemonUri = (uri: string): string => uri.split("?")[0] || "qemu:///session";

export class VirtRpcBackend implements GuestBackend {
  readonly kind = "virt-rpc";
  /* The helper caps relayed sockets and network cards at its --max-guests
   * and a refusal falls through, as for the virsh path. */
  readonly max = Infinity;
  private hello: Hello | null = null;
  /** The domain type that worked, once one has. */
  private domainType: "kvm" | "qemu" | null = null;
  private handles = new Map<string, GuestHandle & { kill(): void }>();
  private readonly helper: Helper;

  constructor(boxCopy: boolean) {
    this.helper = helperFor(boxCopy);
    window.addEventListener("pagehide", () => {
      for (const h of [...this.handles.values()]) h.kill();
    });
  }

  async probe(): Promise<Probe> {
    const no = (reason: string): Probe => ({ available: false, label: "", reason });
    const r = await this.helper.hello();
    // No helper at all: the libvirt backend already says why, once.
    if (!r.ok) return no("");
    const h = r.hello;
    if (!h.virt) return no("");
    if (!h.virt.available) return no(h.virt.reason || "libvirt's socket did not answer the helper");
    const dir = h.images?.dir;
    if (!dir) return no("the helper does not say where its guest images are; update losos-registrar");
    if (!h.images?.kernel || !h.images.rootfs) return no(`${dir} has no bzImage and rootfs.bin`);
    this.hello = h;
    return { available: true, label: "libvirt via WebAssembly", reason: "" };
  }

  canRun(device: GuestDevice): boolean {
    return !!this.hello && (device.image !== "gear" || !!this.hello.images?.gear);
  }

  /** The domain types to try, in order: what virsh found, else KVM first. */
  private types(): ("kvm" | "qemu")[] {
    if (this.domainType) return [this.domainType];
    const h = this.hello;
    if (h?.virsh && (h.domainType === "kvm" || h.domainType === "qemu")) return [h.domainType];
    return ["kvm", "qemu"];
  }

  async start(device: GuestDevice, cmdline: string[], nics: GuestNic[]): Promise<GuestHandle> {
    const hello = this.hello;
    const dir = hello?.images?.dir;
    if (!hello || !dir) throw new Error("the helper did not answer");
    const mod = await loadClient();

    const r = await this.helper.call("virt-ticket", { method: "POST" }, 10000);
    const t: Ticket = await r.json().then(
      (j: unknown) => (j && typeof j === "object" ? (j as Ticket) : {}),
      () => ({}),
    );
    if (!r.ok || !t.ticket) throw new Error(t.error || "the helper answered " + r.status);
    const card = t.nic ?? null;
    const dropCard = () => {
      if (card) this.helper.call("guests/" + encodeURIComponent(card.guest), { method: "DELETE", keepalive: true }, 10000).catch(() => {});
    };

    let client: VirtClient;
    try {
      client = await mod.VirtClient.connect(this.helper.virtUrl(), ["losos-lab", "ticket." + t.ticket]);
    } catch (e) {
      dropCard();
      throw new Error("the relay did not open: " + String(e instanceof Error ? e.message : e));
    }
    const name = card?.domain ?? `losos-lab-page-${device.id}-${Math.floor(Math.random() * 0xffffff).toString(16)}`;
    const mac = nics[0]?.mac;
    let type: "kvm" | "qemu" = "qemu";
    let con: VirtConsole;
    try {
      await client.open(daemonUri(t.uri ?? ""));
      const spec = {
        name,
        title: `LosOS Lab: ${device.name}`,
        memoryMiB: hello.memoryMiB ?? 96,
        kernel: `${dir}/bzImage`,
        disk: `${dir}/${device.image === "gear" ? "gear.bin" : "rootfs.bin"}`,
        cmdline: cmdline.join(" "),
        nic: card && mac ? { mac, helperPort: card.helperPort, qemuPort: card.qemuPort } : null,
      };
      const flags = mod.VirtClient.PAUSED | mod.VirtClient.AUTODESTROY;
      let refused: unknown = null;
      for (const ty of this.types()) {
        try {
          await client.createDomain(domainXml({ ...spec, type: ty }), flags);
          type = ty;
          refused = null;
          break;
        } catch (e) {
          refused = e;
        }
      }
      if (refused) throw refused;
      this.domainType = type;
      con = await client.console(name);
    } catch (e) {
      client.close().catch(() => {});
      dropCard();
      throw new Error(e instanceof Error ? e.message : String(e));
    }

    let net: { socket: WebSocket; stream: ReturnType<typeof stream> } | null = null;
    if (card && mac) {
      const socket = this.helper.socket(this.helper.guestSocketUrl(encodeURIComponent(card.guest) + "/nic/0"), card.ticket);
      net = { socket, stream: stream(socket) };
    }
    const term = guestTerminal(device.name, (s) => {
      try {
        con.write(s);
      } catch {
        /* the console is closed */
      }
    });
    const notes = [type === "qemu" ? "no KVM" : "", net ? "" : "no network"].filter(Boolean);
    const label = `${type === "kvm" ? "KVM" : "QEMU"} via libvirt from WebAssembly` + (notes.length ? ` (${notes.join(", ")})` : "");

    let closed = null as (() => void) | null;
    let alive = true;
    const reader = con.readable.getReader() as ReadableStreamDefaultReader<Uint8Array>;
    const handle = {
      label,
      console: term.console,
      send: (frame: Uint8Array) => net?.stream.send(frame),
      onFrame: (fn: (frame: Uint8Array) => void) => {
        let live = true;
        net?.stream.on((f) => {
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
        alive = false;
        reader.cancel().catch(() => {});
        if (net) {
          net.socket.onclose = null;
          try {
            net.socket.close();
          } catch {
            /* closed */
          }
        }
        term.dispose();
        // Destroy now rather than when libvirt notices the connection went;
        // AUTODESTROY covers a tab that closes before this runs.
        client
          .destroy(name)
          .catch(() => {})
          .finally(() => client.close().catch(() => {}));
        dropCard();
      },
    };
    this.handles.set(device.id, handle);
    // What the guest prints, until the console ends: the guest powered off,
    // or the relay went away.
    void (async () => {
      try {
        for (;;) {
          const { value, done } = await reader.read();
          if (done) break;
          if (value) term.write(value);
        }
      } catch {
        /* the connection failed */
      }
      if (alive && this.handles.get(device.id) === handle) closed?.();
    })();
    await client.resume(name).catch((e: unknown) => {
      handle.kill();
      throw new Error(e instanceof Error ? e.message : String(e));
    });
    return handle;
  }

  stop(id: string): void {
    this.handles.get(id)?.kill();
  }
}
