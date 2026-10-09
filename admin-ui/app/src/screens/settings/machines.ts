import * as React from "react";
import {
  ApiError,
  getVms,
  isAbort,
  isUnauthorized,
  postMarketOnboard,
  postVmImageRemove,
  postVmListing,
  postVmListingClose,
  postVmOrder,
  putVmImage,
  type MarketAccount,
  type MarketActionResponse,
  type MarketShelfListing,
  type VmCatalogue,
  type VmMachine,
  type VmUnavailableReason,
} from "@/lib/api";
import { toast } from "@/components/ui/toast";
import { intlTag, t } from "@/lib/i18n";

/* The Machines pane's data: virtual machines on the mesh, sold by the
 * replica through the market (lososd's /api/vms, the registrar's vms.rs).
 *
 * Like the market, most boxes meet `unavailable`, and it comes with a reason
 * the pane turns into a sentence: this box does not share its disk (the
 * offer is for sharing boxes only), no official edge is in reach, or the
 * edge runs no machines. None of them is an error. */

export type MachinesState =
  | { kind: "loading" }
  | { kind: "unavailable"; reason: VmUnavailableReason }
  | { kind: "failed"; message: string }
  | {
      kind: "ready";
      catalogue: VmCatalogue;
      listings: MarketShelfListing[];
      account: MarketAccount;
      machines: VmMachine[];
    };

export interface MachineOrder {
  listingId: string;
  replicas: number;
  image: string;
  name: string;
  userData: string;
}

export interface Uploading {
  name: string;
  /** 0 to 100. */
  percent: number;
}

export interface MachinesData {
  state: MachinesState;
  /** True while an action is in flight. */
  busy: boolean;
  /** The image being sent to the edge, if one is. */
  uploading: Uploading | null;
  refresh: () => void;
  onboard: () => Promise<MarketActionResponse | null>;
  order: (order: MachineOrder) => Promise<MarketActionResponse | null>;
  list: (priceMinor: number, capacity: number) => Promise<boolean>;
  close: (listingId: string) => Promise<boolean>;
  upload: (file: File, name: string, efi: boolean) => Promise<boolean>;
  remove: (uploadId: string) => Promise<boolean>;
}

/* A machine that is still being made changes by the minute, so the pane
 * asks again while one is. Nothing else on it moves by itself. */
const SETTLING_POLL_MS = 15_000;

function settling(state: MachinesState): boolean {
  if (state.kind !== "ready") return false;
  return state.machines.some((machine) =>
    machine.replicas.some((replica) => replica.status !== "Running" && replica.status !== "Stopped"),
  );
}

export function useMachines(enabled: boolean): MachinesData {
  const [state, setState] = React.useState<MachinesState>({ kind: "loading" });
  const [busy, setBusy] = React.useState(false);
  const [uploading, setUploading] = React.useState<Uploading | null>(null);
  const [tick, setTick] = React.useState(0);

  React.useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void (async () => {
      try {
        const reply = await getVms({ signal: controller.signal });
        if (controller.signal.aborted) return;
        setState(
          reply.available
            ? {
                kind: "ready",
                catalogue: reply.catalogue,
                listings: reply.listings,
                account: reply.account,
                machines: reply.machines,
              }
            : { kind: "unavailable", reason: reply.reason },
        );
      } catch (error) {
        if (isAbort(error) || isUnauthorized(error)) return;
        setState({ kind: "failed", message: error instanceof Error ? error.message : "" });
      }
    })();
    return () => controller.abort();
  }, [enabled, tick]);

  const refresh = React.useCallback(() => setTick((n) => n + 1), []);

  const moving = settling(state);
  React.useEffect(() => {
    if (!enabled || !moving) return;
    const timer = window.setTimeout(refresh, SETTLING_POLL_MS);
    return () => window.clearTimeout(timer);
  }, [enabled, moving, refresh, state]);

  /* One action at a time, as on the market: each one moves money or
   * changes what is for sale, and a double click must not be two orders. */
  const act = React.useCallback(
    async <T,>(fn: () => Promise<T>, done?: () => void): Promise<T | null> => {
      setBusy(true);
      try {
        const out = await fn();
        done?.();
        refresh();
        return out;
      } catch (error) {
        if (!isUnauthorized(error) && !isAbort(error)) {
          const said = error instanceof Error ? error.message : "";
          toast.error(
            t("machines.actionFailedTitle"),
            said.length > 0 ? said : t("machines.actionFailed"),
            { help: error instanceof ApiError && error.status === 503 ? "edge-not-found" : "index" },
          );
        }
        return null;
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  return {
    state,
    busy,
    uploading,
    refresh,
    onboard: () => act(() => postMarketOnboard()),
    order: (o) =>
      act(() =>
        postVmOrder({
          listing_id: o.listingId,
          quantity: o.replicas,
          image: o.image,
          name: o.name,
          ...(o.userData.trim().length > 0 ? { user_data: o.userData } : {}),
        }),
      ),
    list: async (priceMinor, capacity) =>
      (await act(
        () => postVmListing({ unit_price: priceMinor, capacity }),
        () => toast.success(t("machines.host.listedTitle"), t("machines.host.listedBody")),
      )) !== null,
    close: async (id) =>
      (await act(
        () => postVmListingClose(id),
        () => toast.done(t("machines.host.closedTitle")),
      )) !== null,
    upload: async (file, name, efi) => {
      setUploading({ name, percent: 0 });
      try {
        return (
          (await act(
            () =>
              putVmImage(file, { name, efi }, {
                onProgress: (percent) => setUploading({ name, percent }),
              }),
            () => toast.success(t("machines.images.storedTitle"), t("machines.images.storedBody", { name })),
          )) !== null
        );
      } finally {
        setUploading(null);
      }
    },
    remove: async (id) =>
      (await act(
        () => postVmImageRemove(id),
        () => toast.done(t("machines.images.removedTitle")),
      )) !== null,
  };
}

/** "3.2 GiB", with the number in the language on screen. */
export function formatBytes(bytes: number): string {
  const units = ["B", "KiB", "MiB", "GiB", "TiB"] as const;
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const number = new Intl.NumberFormat(intlTag(), { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(value);
  return `${number} ${units[unit]}`;
}
