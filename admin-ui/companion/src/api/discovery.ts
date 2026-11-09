/* Boxes on the LAN, through DNS-SD: every box announces `_losos._tcp` on
 * the front vhost's port with its release tag in TXT (modules/
 * configuration.nix, modules/mdns.nix) - the one browse type LosOS Desktop
 * lists boxes by, so the companion browses the same record instead of
 * knocking on workstations.
 *
 * The Compose app walked Android's NsdManager by hand (Discovery.kt);
 * react-native-zeroconf wraps the same NSD on Android and Bonjour on iOS,
 * and its platform differences stay behind `scan()` here. The Android
 * implementation stays NSD (the library default, the system DNS-SD) -
 * parity with the port; the library's DNSSD responder is the fallback if a
 * vendor ships a broken NsdManager (recorded in README).
 */

import Zeroconf from "react-native-zeroconf";

import { bonjourServices, serviceType } from "../../plugins/services";

const SERVICE_TYPE = `${serviceType}._tcp`;
const DOMAIN = "local.";

/** How long to hold the browse window open (Discovery.kt's 3s), plus the
 *  per-service resolve grace the loop below gives itself. */
const BROWSE_WINDOW_MS = 3_000;

/** One answer to the browse. `name` is the box's host name - its
 *  `<name>.local` on this LAN - and `port` the front vhost's as announced
 *  (Discovery.kt FoundBox). */
export interface FoundBox {
  name: string;
  ip: string | null;
  port: number;
  version: string | null;
  /** How to reach it: the `.local` name, which every mDNS-speaking phone
   *  resolves; `ip` is the fallback when it will not. */
  readonly address: string;
}

interface ZeroconfService {
  name?: string;
  addresses?: string[];
  port?: number;
  txt?: Record<string, string | undefined>;
}

/** Browse `_losos._tcp` for the window, resolve what answered, stop. The
 *  name needs no lookup (it is the host name), but resolution carries the
 *  announced port and the TXT version tag, and an IP for networks whose
 *  phone-side `.local` lookup is broken (Discovery.kt's resolve loop,
 *  which historical Android made walk one service at a time). */
export async function browseBoxes(timeoutMs = BROWSE_WINDOW_MS): Promise<FoundBox[]> {
  const zeroconf = new Zeroconf();
  const answered = new Map<string, FoundBox>();

  const onResolved = (svc: ZeroconfService) => {
    if (typeof svc.name !== "string" || svc.name === "") return;
    if (answered.has(svc.name)) return;
    answered.set(svc.name, {
      name: svc.name,
      ip: typeof svc.addresses?.[0] === "string" ? svc.addresses[0] : null,
      port: typeof svc.port === "number" ? svc.port : 443,
      version: typeof svc.txt?.version === "string" ? svc.txt.version : null,
      get address() {
        return `${svc.name}.local`;
      },
    });
  };
  zeroconf.on("resolved", onResolved);
  zeroconf.scan(SERVICE_TYPE, "tcp", DOMAIN);

  await new Promise<void>((resolve) => setTimeout(resolve, timeoutMs));

  zeroconf.stop();
  zeroconf.off("resolved", onResolved);
  zeroconf.removeDeviceListeners();

  return [...answered.values()];
}

export { SERVICE_TYPE, DOMAIN, bonjourServices };
