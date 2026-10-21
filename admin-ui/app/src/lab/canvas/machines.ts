/* The box's machines on the mesh (the Machines pane's /api/vms), for the
 * machine cards on the canvas: what each ordered machine is called, its
 * image, and what its replicas are doing, read again every 20 seconds while
 * the page is open. Only the copy a box serves asks; machines are offered
 * only to a box that shares its disk, so most boxes get a reason instead. */

import { getVms, isUnauthorized, type VmUnavailableReason } from "@/lib/api";
import { BOX_COPY } from "../shape";
import type { LabStore } from "../store";

export interface MachineInfo {
  order: string;
  name: string;
  image: string;
  imageName: string;
  /** awaiting payment, lapsed, or running as ordered */
  state: "pending" | "lapsed" | "paid";
  replicas: { name: string; status: string }[];
  address: string | null;
}

export type MachinesView =
  | { state: "off" }
  | { state: "loading" }
  | { state: "unavailable"; reason: VmUnavailableReason | "signedOut" }
  | { state: "failed" }
  | { state: "ready"; images: { id: string; name: string }[]; machines: MachineInfo[] };

const EVERY_MS = 20_000;

async function read(): Promise<MachinesView> {
  try {
    const r = await getVms();
    if (!r.available) return { state: "unavailable", reason: r.reason };
    const images = [
      ...r.catalogue.images.map((i) => ({ id: i.id, name: i.name })),
      ...(r.account.uploads ?? []).filter((u) => u.stored).map((u) => ({ id: u.id, name: u.name })),
    ];
    const machines: MachineInfo[] = r.account.purchases
      .filter((o) => o.kind === "vm" && o.vm != null)
      .map((o) => {
        const vm = o.vm!;
        return {
          order: o.id,
          name: vm.name,
          image: vm.image,
          imageName: images.find((i) => i.id === vm.image)?.name ?? vm.image,
          state: o.status === "pending" ? "pending" : vm.halted || o.expired || o.status === "expired" ? "lapsed" : "paid",
          replicas: r.machines.find((m) => m.order_id === o.id)?.replicas ?? [],
          address: vm.halted ? null : vm.address,
        };
      });
    return { state: "ready", images, machines };
  } catch (e) {
    return isUnauthorized(e) ? { state: "unavailable", reason: "signedOut" } : { state: "failed" };
  }
}

/** Starts the reads; once per page. */
export function watchMachines(s: LabStore): void {
  if (!BOX_COPY) return;
  s.machines = { state: "loading" };
  const tick = async () => {
    if (!document.hidden) {
      const next = await read();
      if (JSON.stringify(next) !== JSON.stringify(s.machines)) {
        s.machines = next;
        s.emit("model");
      }
    }
  };
  void tick();
  window.setInterval(() => void tick(), EVERY_MS);
}

/** The machine a card stands for, from its `losos.machine`. */
export function machineOf(losos: Record<string, unknown> | undefined): { order?: string; image?: string; name?: string } | null {
  const m = losos?.["machine"];
  if (typeof m !== "object" || m === null || Array.isArray(m)) return null;
  const o = m as Record<string, unknown>;
  const str = (v: unknown) => (typeof v === "string" && v ? v : undefined);
  return { order: str(o["order"]), image: str(o["image"]), name: str(o["name"]) };
}

/** The dot of a replica status, in the Machines pane's words. */
export function replicaTone(status: string): "ok" | "warn" | "crit" | "idle" {
  return status === "Running" ? "ok" : status === "Failed" ? "crit" : status === "Stopped" || status === "Paused" ? "idle" : "warn";
}
