import { Platform } from "react-native";

/** The Apple TV and Android TV targets the `@react-native-tvos/config-tv`
 * plugin rewrites the native projects for (EXPO_TV=1). `Platform.isTV`
 * is set by the react-native-tvos fork; phone and tablet builds never
 * set it, so a check on it is a no-op on the default prebuild. */
export const isTV = Platform.isTV === true;

/** visionOS, as the @callstack/react-native-visionos fork reports it.
 * This fork is a separate React Native tree (0.79) from the tvos fork the
 * app's `react-native` alias resolves to (0.81), so a build that runs on
 * visionOS is generated from its own dependency set; see the app README's
 * "visionOS" section. The check is still the right guard for code that is
 * meant to run there. */
export const isVisionOS = Platform.OS === "visionos" as typeof Platform.OS;

/** Windows, the react-native-windows port (eject-time in this tree). */
export const isWindows = Platform.OS === "windows";

/** TV focus is pointer-free: a D-pad moves focus between pressables, so
 * every tappable surface needs a visible focus ring and generous hit
 * targets. Phone and tablet builds get the same components without the
 * TV-only focus affordances. */
export const tvFocusable = isTV
  ? ({ focusable: true, accessible: true } as const)
  : ({} as const);

/** Extra padding TV targets get so the D-pad has room to land; phones keep
 * the compact spacing they were laid out for. */
export const hitSlop = isTV ? 12 : 8;
