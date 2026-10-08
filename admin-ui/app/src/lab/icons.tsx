/* Device icons for the logical view and the tray (viewBox 0 0 64 56), and the
 * hardware drawings for the physical view. Network gear uses the generic
 * shapes every topology diagram uses (cylinder router, flat switch, cloud);
 * the LosOS devices get their own: a mini-PC with a coloured stripe.
 * Ported from admin-ui/lab/src/icons.js. Colours are presentation
 * attributes, never style="", so the page's style-src 'self' has nothing to
 * refuse, and every one is a house token (styles/tokens.css) or a lab alias of
 * one (lab.css), so the drawings follow the light and dark themes. */

import type * as React from "react";
import { logoFor } from "@/lib/logo";
import type { Device } from "./core";

const surface = "var(--surface)";
const ink = "var(--ink)";

export function IconGlyph({ type }: { type: string }) {
  switch (type) {
    case "box":
      return (
        <g>
          <rect x="9" y="16" width="46" height="28" rx="5" fill={surface} stroke={ink} strokeWidth="2" />
          <rect x="9" y="16" width="46" height="6" rx="3" fill="var(--lab-mark)" />
          <circle cx="17" cy="35" r="2.6" fill="var(--ok)" />
          <rect x="24" y="33" width="24" height="3" rx="1.5" fill="var(--faint)" />
          <path d="M14 44v4M50 44v4" stroke={ink} strokeWidth="2" strokeLinecap="round" />
        </g>
      );
    case "edge-local":
      return (
        <g>
          <rect x="9" y="16" width="46" height="28" rx="5" fill={surface} stroke={ink} strokeWidth="2" />
          <rect x="9" y="16" width="46" height="6" rx="3" fill="var(--accent)" />
          <circle cx="17" cy="35" r="2.6" fill="var(--ok)" />
          <path d="M27 36h18m-5-5 5 5-5 5" fill="none" stroke="var(--accent)" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
          <path d="M32 8a12 12 0 0 1 0 8M28 6a16 16 0 0 1 0 12" fill="none" stroke="var(--accent)" strokeWidth="1.8" strokeLinecap="round" transform="rotate(-90 32 12)" />
        </g>
      );
    case "edge-official":
      return (
        <g>
          <rect x="6" y="18" width="52" height="11" rx="2.5" fill={surface} stroke={ink} strokeWidth="2" />
          <rect x="6" y="31" width="52" height="11" rx="2.5" fill={surface} stroke={ink} strokeWidth="2" />
          <circle cx="12" cy="23.5" r="1.8" fill="var(--ok)" />
          <circle cx="12" cy="36.5" r="1.8" fill="var(--ok)" />
          <path d="M18 23.5h20M18 36.5h20" stroke="var(--faint)" strokeWidth="2" strokeLinecap="round" />
          <path d="M47 9l8 3v6c0 5-3.5 8-8 9.5-4.5-1.5-8-4.5-8-9.5v-6z" fill="var(--accent)" stroke={surface} strokeWidth="1.5" />
          <path d="M43.5 17.5l2.5 2.5 4.5-5" fill="none" stroke={surface} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        </g>
      );
    case "laptop":
      return (
        <g>
          <rect x="13" y="11" width="38" height="26" rx="3" fill="var(--lab-screen)" stroke={ink} strokeWidth="2" />
          <rect x="17" y="15" width="12" height="8" rx="1.5" fill="none" stroke="var(--accent)" strokeWidth="1.4" />
          <rect x="31" y="15" width="16" height="18" rx="1.5" fill="var(--lab-screen-tile)" />
          <rect x="17" y="25" width="12" height="8" rx="1.5" fill="var(--lab-screen-tile)" />
          <path d="M6 41h52l-4 5H10z" fill={surface} stroke={ink} strokeWidth="2" strokeLinejoin="round" />
        </g>
      );
    case "router":
      return (
        <g>
          <ellipse cx="32" cy="35" rx="23" ry="8" fill={surface} stroke={ink} strokeWidth="2" />
          <path d="M9 27v8M55 27v8" stroke={ink} strokeWidth="2" />
          <ellipse cx="32" cy="27" rx="23" ry="8" fill={surface} stroke={ink} strokeWidth="2" />
          <g stroke="var(--accent)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" fill="none">
            <path d="M24 24l-6 3 6 3M40 24l6 3-6 3M29 22l3-3 3 3M29 32l3 3 3-3" />
          </g>
        </g>
      );
    case "switch":
      return (
        <g>
          <path d="M8 24l8-8h40l-8 8z" fill="var(--sunk)" stroke={ink} strokeWidth="2" strokeLinejoin="round" />
          <path d="M56 16v14l-8 8V24z" fill="var(--sunk)" stroke={ink} strokeWidth="2" strokeLinejoin="round" />
          <rect x="8" y="24" width="40" height="14" fill={surface} stroke={ink} strokeWidth="2" />
          <g stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" fill="none">
            <path d="M14 28h12l-3-2.5M26 34H14l3 2.5M30 28h12l-3-2.5M42 34H30l3 2.5" />
          </g>
        </g>
      );
    case "ap":
      return (
        <g>
          <ellipse cx="32" cy="38" rx="20" ry="7" fill={surface} stroke={ink} strokeWidth="2" />
          <circle cx="32" cy="38" r="2.2" fill="var(--accent)" />
          <g fill="none" stroke="var(--accent)" strokeWidth="2.2" strokeLinecap="round">
            <path d="M24 24a11 11 0 0 1 16 0M19 18a18 18 0 0 1 26 0M28.5 29a5 5 0 0 1 7 0" />
          </g>
        </g>
      );
    case "bus":
      return (
        <g>
          <path d="M6 30h52" stroke={ink} strokeWidth="4" strokeLinecap="round" />
          <path d="M6 30h52" stroke={surface} strokeWidth="1.6" strokeDasharray="3 3" />
          <rect x="2" y="24" width="6" height="12" rx="1.5" fill={ink} />
          <rect x="56" y="24" width="6" height="12" rx="1.5" fill={ink} />
          <g stroke="var(--accent)" strokeWidth="2.4" strokeLinecap="round">
            <path d="M18 30v-12M32 30v12M46 30v-12" />
          </g>
          <g fill="var(--accent)">
            <circle cx="18" cy="16" r="3" />
            <circle cx="32" cy="44" r="3" />
            <circle cx="46" cy="16" r="3" />
          </g>
        </g>
      );
    case "internet":
      return (
        <g>
          <path d="M17 44h31a10 10 0 0 0 1-20 14 14 0 0 0-27-3 10 10 0 0 0-5 23z" fill={surface} stroke={ink} strokeWidth="2" strokeLinejoin="round" />
          <g fill="none" stroke="var(--lab-wan)" strokeWidth="1.6">
            <ellipse cx="33" cy="33" rx="9" ry="9" />
            <path d="M24 33h18M33 24c-4 5-4 13 0 18M33 24c4 5 4 13 0 18" />
          </g>
        </g>
      );
    default:
      return null;
  }
}

export function DeviceIcon({ type, size = 48, className }: { type: string; size?: number; className?: string }) {
  return (
    <svg viewBox="0 0 64 56" width={size} height={Math.round((size * 56) / 64)} aria-hidden="true" className={className}>
      <IconGlyph type={type} />
    </svg>
  );
}

export type PortState = "up" | "wait" | "down" | "none";

export interface Hw {
  w: number;
  h: number;
  /** port id → [x, y] relative to the drawing's top-left */
  ports: Record<string, [number, number]>;
  node: React.ReactNode;
}

/* The hardware as it stands in the room. A power LED carries data-power, so
 * a click on it switches the device. */
export function hwDrawing(dev: Device, portState: (port: string) => PortState): Hw {
  const T = dev.type;
  const led = (id: string): string => {
    const s = portState(id);
    return s === "up" ? "var(--ok)" : s === "wait" ? "var(--warn)" : "var(--lab-led-off)";
  };
  const pwr = dev.power ? "var(--ok)" : "var(--lab-led-off)";
  const pw = { className: "pwr", "data-power": dev.id } as const;
  if (T === "box" || T === "edge-local") {
    const stripe = T === "box" ? "var(--lab-mark)" : "var(--accent)";
    return {
      w: 92,
      h: 50,
      ports: { eth0: [80, 40] },
      node: (
        <>
          <rect x="0" y="6" width="92" height="40" rx="7" fill="var(--lab-kit)" stroke="var(--lab-kit-edge)" />
          <rect x="0" y="6" width="92" height="7" rx="3.5" fill={stripe} />
          <rect x="6" y="18" width="40" height="22" rx="3" fill="var(--lab-kit-face)" />
          <circle {...pw} cx="14" cy="29" r="4.2" fill={pwr} stroke="var(--lab-kit-edge)" />
          <rect x="52" y="24" width="14" height="9" rx="1.5" fill="var(--lab-hole)" />
          <rect x="74" y="34" width="12" height="9" rx="1.5" fill="var(--lab-hole)" />
          <circle cx="77" cy="32" r="1.6" fill={led("eth0")} />
          <circle cx="83" cy="32" r="1.6" fill={led("eth0")} />
          <rect x="8" y="46" width="10" height="3" fill="var(--lab-kit-edge)" />
          <rect x="74" y="46" width="10" height="3" fill="var(--lab-kit-edge)" />
        </>
      ),
    };
  }
  if (T === "edge-official") {
    return {
      w: 132,
      h: 30,
      ports: { eth0: [118, 15] },
      node: (
        <>
          <rect x="0" y="2" width="132" height="26" rx="2" fill="var(--lab-rack)" stroke="var(--lab-rack-edge)" />
          <rect x="6" y="7" width="70" height="16" rx="1.5" fill="var(--lab-rack-face)" />
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <rect key={i} x={9 + i * 11} y="10" width="8" height="10" rx="1" fill="var(--lab-rack-bay)" />
          ))}
          <circle {...pw} cx="88" cy="15" r="3.6" fill={pwr} />
          <rect x="111" y="10" width="14" height="10" rx="1.5" fill="var(--lab-hole)" />
          <circle cx="114" cy="8" r="1.4" fill={led("eth0")} />
          <path d="M98 9l5 1.8v3.6c0 3-2.2 5-5 6-2.8-1-5-3-5-6v-3.6z" fill="var(--accent)" />
        </>
      ),
    };
  }
  if (T === "laptop") {
    return {
      w: 96,
      h: 66,
      ports: { eth0: [6, 52], wlan0: [90, 52] },
      node: (
        <>
          <rect x="10" y="0" width="76" height="48" rx="4" fill="var(--lab-rack)" stroke="var(--lab-rack-edge)" />
          <rect x="14" y="4" width="68" height="40" rx="2" fill={dev.power ? "var(--lab-screen)" : "var(--lab-screen-off)"} />
          {dev.power && (
            <>
              <rect x="18" y="8" width="20" height="12" rx="2" fill="none" stroke="var(--accent)" />
              <rect x="41" y="8" width="37" height="32" rx="2" fill="var(--lab-screen-tile)" />
              <rect x="18" y="23" width="20" height="17" rx="2" fill="var(--lab-screen-tile)" />
            </>
          )}
          <path d="M0 50h96l-6 10H6z" fill="var(--lab-kit)" stroke="var(--lab-kit-edge)" />
          <rect x="2" y="50" width="9" height="6" rx="1" fill="var(--lab-hole)" />
          <circle {...pw} cx="48" cy="55" r="3" fill={pwr} />
          <circle cx="90" cy="55" r="1.8" fill={led("wlan0")} />
        </>
      ),
    };
  }
  if (T === "router") {
    const lanX = [44, 56, 68, 80];
    const ports: Record<string, [number, number]> = { wan: [28, 36] };
    lanX.forEach((x, i) => (ports["lan" + (i + 1)] = [x, 36]));
    return {
      w: 100,
      h: 46,
      ports,
      node: (
        <>
          <path d="M14 0v10M86 0v10" stroke="var(--lab-kit-edge)" strokeWidth="3" strokeLinecap="round" />
          <rect x="0" y="10" width="100" height="34" rx="6" fill="var(--lab-kit)" stroke="var(--lab-kit-edge)" />
          <circle {...pw} cx="10" cy="22" r="3.4" fill={pwr} />
          <rect x="22" y="31" width="12" height="9" rx="1.5" fill="var(--lab-wan)" />
          {lanX.map((x) => (
            <rect key={x} x={x - 5} y="31" width="10" height="9" rx="1.5" fill="var(--lab-hole)" />
          ))}
          <circle cx="28" cy="20" r="1.8" fill={led("wan")} />
          {lanX.map((x, i) => (
            <circle key={x} cx={x} cy="20" r="1.8" fill={led("lan" + (i + 1))} />
          ))}
        </>
      ),
    };
  }
  if (T === "switch") {
    const ports: Record<string, [number, number]> = {};
    const xs: number[] = [];
    for (let i = 0; i < 8; i++) {
      const x = 34 + i * 12;
      xs.push(x);
      ports["p" + (i + 1)] = [x, 17];
    }
    return {
      w: 140,
      h: 30,
      ports,
      node: (
        <>
          <rect x="0" y="2" width="140" height="26" rx="2.5" fill="var(--lab-rack)" stroke="var(--lab-rack-edge)" />
          <text x="8" y="19" fontSize="8" fill="var(--lab-rack-ink)" className="hw-mono">
            SW
          </text>
          <circle {...pw} cx="24" cy="15" r="2.6" fill={pwr} />
          {xs.map((x, i) => (
            <g key={x}>
              <rect x={x - 4.5} y="12" width="9" height="9" rx="1" fill="var(--lab-hole)" />
              <circle cx={x} cy="8" r="1.4" fill={led("p" + (i + 1))} />
            </g>
          ))}
        </>
      ),
    };
  }
  if (T === "bus") {
    // a length of thin coax along the wall: a T-tap per port, a terminator at each end
    const ports: Record<string, [number, number]> = {};
    const xs: number[] = [];
    for (let i = 0; i < 8; i++) {
      const x = 22 + i * 26;
      xs.push(x);
      ports["t" + (i + 1)] = [x, 10];
    }
    return {
      w: 228,
      h: 26,
      ports,
      node: (
        <>
          <rect {...pw} x="0" y="12" width="10" height="10" rx="2" fill="var(--lab-rack)" />
          <rect x="218" y="12" width="10" height="10" rx="2" fill="var(--lab-rack)" />
          <path d="M10 17h208" stroke="var(--lab-rack-edge)" strokeWidth="4" />
          <path d="M10 17h208" stroke={dev.power ? "var(--warn)" : "var(--lab-kit-edge)"} strokeWidth="1.4" />
          {xs.map((x, i) => (
            <g key={x}>
              <path d={`M${x} 17v-6`} stroke="var(--lab-rack)" strokeWidth="4" />
              <circle cx={x} cy="10" r="3" fill="var(--lab-rack)" />
              <circle cx={x} cy="22.5" r="1.4" fill={led("t" + (i + 1))} />
            </g>
          ))}
        </>
      ),
    };
  }
  if (T === "ap") {
    return {
      w: 60,
      h: 40,
      ports: { eth0: [30, 36] },
      node: (
        <>
          <ellipse cx="30" cy="22" rx="28" ry="13" fill="var(--lab-kit)" stroke="var(--lab-kit-edge)" />
          <circle {...pw} cx="30" cy="22" r="3.4" fill={dev.power ? "var(--accent)" : "var(--lab-led-off)"} />
          <rect x="25" y="33" width="10" height="6" rx="1" fill="var(--lab-hole)" />
        </>
      ),
    };
  }
  if (T === "internet") {
    const ports: Record<string, [number, number]> = {};
    for (let i = 0; i < 8; i++) ports["wan" + (i + 1)] = [14 + i * 12, 58];
    return {
      w: 120,
      h: 64,
      ports,
      node: (
        <>
          <path d="M26 54h70a16 16 0 0 0 2-32 24 24 0 0 0-46-6 18 18 0 0 0-26 38z" fill={surface} stroke="var(--lab-wan)" strokeWidth="2" />
          <text x="60" y="40" textAnchor="middle" fontSize="11" fill="var(--muted)">
            ISP / Internet
          </text>
        </>
      ),
    };
  }
  return { w: 60, h: 40, ports: {}, node: null };
}

/* The logo, drawn on the splash, the top bar and the simulated pages:
 * today's, as on the admin pages (a live salmon, a plate of it on
 * Halloween; see lib/logo.ts). */
export const logoUrl: string = logoFor().url;
