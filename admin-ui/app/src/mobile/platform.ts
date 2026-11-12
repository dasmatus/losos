import { Platform } from "react-native";
import { useColorScheme } from "react-native";

/** The Apple TV and Android TV targets the `@react-native-tvos/config-tv`
 *  plugin rewrites the native projects for (EXPO_TV=1). `Platform.isTV`
 *  is set by the react-native-tvos fork; phone and tablet builds never
 *  set it, so a check on it is a no-op on the default prebuild. */
export const isTV = Platform.isTV === true;

/** visionOS, as the @callstack/react-native-visionos fork reports it.
 *  This fork is a separate React Native tree (0.79) from the tvos fork the
 *  app's `react-native` alias resolves to (0.81), so a build that runs on
 *  visionOS is generated from its own dependency set; see the app README's
 *  "visionOS" section. The check is still the right guard for code that is
 *  meant to run there. */
export const isVisionOS = Platform.OS === ("visionos" as typeof Platform.OS);

/** Windows, the react-native-windows port (eject-time in this tree). */
export const isWindows = Platform.OS === "windows";

/** TV focus is pointer-free: a D-pad moves focus between pressables, so
 *  every tappable surface needs a visible focus ring and generous hit
 *  targets. Phone and tablet builds get the same components without the
 *  TV-only focus affordances. */
export const tvFocusable = isTV
  ? ({ focusable: true, accessible: true } as const)
  : ({} as const);

/** Extra padding TV targets get so the D-pad has room to land; phones keep
 *  the compact spacing they were laid out for. */
export const hitSlop = isTV ? 12 : 8;

/* ── Native look ─────────────────────────────────────────────────────────── */

export type NativePlatform = "ios" | "android" | "web" | "windows" | "macos";

export interface NativeLook {
  platform: NativePlatform;
  fontFamily: string | undefined;
  cornerRadiusScale: number;
  elevation: number;
  shadow: { shadowColor: string; shadowOffset: { width: number; height: number }; shadowOpacity: number; shadowRadius: number };
  ripple: { color: string; borderless: boolean } | null;
  focusRingColor: string;
}

const LOOKS: Record<NativePlatform, NativeLook> = {
  ios: {
    platform: "ios",
    fontFamily: "System",
    cornerRadiusScale: 1.0,
    elevation: 0,
    shadow: { shadowColor: "#000", shadowOffset: { width: 0, height: 2 }, shadowOpacity: 0.12, shadowRadius: 6 },
    ripple: null,
    focusRingColor: "#007AFF",
  },
  android: {
    platform: "android",
    fontFamily: undefined,
    cornerRadiusScale: 1.0,
    elevation: 2,
    shadow: { shadowColor: "#000", shadowOffset: { width: 0, height: 1 }, shadowOpacity: 0.1, shadowRadius: 2 },
    ripple: { color: "rgba(0,0,0,0.12)", borderless: false },
    focusRingColor: "#6750A4",
  },
  web: {
    platform: "web",
    fontFamily: undefined,
    cornerRadiusScale: 1.0,
    elevation: 0,
    shadow: { shadowColor: "#000", shadowOffset: { width: 0, height: 1 }, shadowOpacity: 0.08, shadowRadius: 3 },
    ripple: null,
    focusRingColor: "#2563eb",
  },
  windows: {
    platform: "windows",
    fontFamily: "Segoe UI",
    cornerRadiusScale: 0.8,
    elevation: 0,
    shadow: { shadowColor: "#000", shadowOffset: { width: 0, height: 1 }, shadowOpacity: 0.1, shadowRadius: 3 },
    ripple: null,
    focusRingColor: "#0067C0",
  },
  macos: {
    platform: "macos",
    fontFamily: "System",
    cornerRadiusScale: 1.0,
    elevation: 0,
    shadow: { shadowColor: "#000", shadowOffset: { width: 0, height: 2 }, shadowOpacity: 0.12, shadowRadius: 6 },
    ripple: null,
    focusRingColor: "#007AFF",
  },
};

function resolvedPlatform(): NativePlatform {
  // Dev-only override for testing alternate looks at runtime.
  if (typeof __DEV__ !== "undefined" && __DEV__) {
    let forced: string | undefined;
    try {
      forced = (globalThis as any).__LOSOS_FORCE_PLATFORM__;
    } catch {
      forced = undefined;
    }
    if (!forced) {
      try {
        forced = (process as any)?.env?.LOSOS_FORCE_PLATFORM;
      } catch {
        forced = undefined;
      }
    }
    if (forced && forced in LOOKS) return forced as NativePlatform;
  }
  const os = Platform.OS;
  if (os === "ios") return "ios";
  if (os === "android") return "android";
  if (os === "web") return "web";
  if (os === "windows") return "windows";
  if (os === "macos") return "macos";
  return "web";
}

import { useMemo } from "react";

export function useNativeLook(): NativeLook {
  // Keep hook stable; recompute only when forced override changes.
  return useMemo(() => LOOKS[resolvedPlatform()], []);
}
