/* How the Lab boots a real guest behind a device on the canvas.
 *
 * A GuestBackend is one way of running guests. The engine (index.ts) probes
 * them in order and keeps every one that answers, so a guest one backend
 * cannot start falls through to the next:
 *
 *   1. libvirt through the helper `losos-registrar lab` and its virsh
 *      (libvirt.ts): KVM on the viewer's machine, or on the box through
 *      lososd's /api/lab
 *   2. libvirt spoken directly by the page's WASM client (virt-rpc.ts,
 *      admin-ui/lab/virt-rpc) through the helper's relay route: the same
 *      helper with no virsh behind it
 *   3. qemu-wasm in this tab (qemu.ts)
 *
 * The two libvirt backends find the helper the same way (helper.ts).
 *
 * The rest of the Lab never sees which: it starts a guest per powered device
 * with the kernel command line the model implies, and gets back the guest's
 * console and its network card as a stream of Ethernet frames. The fabric
 * (fabric.ts) switches those frames along the cables of the diagram, so
 * guests of two backends share a segment.
 *
 * Keep this narrow: probe, start, stop, and a handle with the console and the
 * frames. Anything a backend needs beyond that (a helper's address, a token)
 * it finds for itself in probe(). */

export interface Probe {
  available: boolean;
  /** Names the backend on the engine badge and in the console header,
   *  e.g. "QEMU in this tab", "KVM via libvirt". */
  label: string;
  /** Why it is not available, a sentence for the badge's tooltip. */
  reason: string;
}

/** What a guest is booted as. */
export interface GuestDevice {
  id: string;
  name: string;
  type: string;
  /** "gear" boots Netzgeräte Betriebssystem, "losos" the LosOS stand-in. */
  image: "losos" | "gear";
  /** The catalogue's `emulate` for the type (box, edge, router, …). */
  role: string;
}

export interface GuestNic {
  mac: string;
}

/** The guest's serial console, drawn into whatever element the inspector gives it. */
export interface GuestConsole {
  /** Mount (or move) the terminal into `host`. */
  attach(host: HTMLElement): void;
  /** The last `lines` lines it printed, as text, for the readiness check. */
  tail(lines: number): string;
}

export interface GuestHandle {
  /** What runs this guest, for the console header ("KVM via libvirt"). */
  label: string;
  console: GuestConsole;
  /** A frame from the fabric into the guest's first NIC. */
  send(frame: Uint8Array): void;
  /** Frames the guest sends out of its first NIC. */
  onFrame(listener: (frame: Uint8Array) => void): () => void;
  /** Called once if the guest goes away by itself (the helper dropped it). */
  onClose?(listener: () => void): void;
  stop(): void;
}

export interface GuestBackend {
  readonly kind: string;
  /** How many guests of this backend the Lab starts at once. Infinity when
   *  the backend refuses for itself (the libvirt helper's --max-guests), so
   *  the cap of three counts qemu-wasm guests only. */
  readonly max: number;
  probe(): Promise<Probe>;
  /** Whether this backend can boot this device at all (libvirt needs
   *  gear.bin for network gear). Asked only after a successful probe. */
  canRun?(device: GuestDevice): boolean;
  start(device: GuestDevice, cmdline: string[], nics: GuestNic[]): Promise<GuestHandle>;
  stop(id: string): void;
}
