/* The mode-aware theme hook: LosTheme.kt's LocalTokens, as useTokens().
 *
 * The panes and components read the current mode's palette from here
 * instead of receiving it as props, the way the Compose app read
 * LocalTokens; the mode lives with the config (BoxStore), which owns the
 * owner's ThemeMode choice. Until loadConfig() has answered the store is
 * null and the palette reads light - the app shell renders nothing before
 * then, so that resolution is never actually seen on screen.
 */

import { useColorScheme } from "react-native";

import { useBoxConfig } from "../../store/config";
import { tokensOf, type LososTokens } from "./tokens";

export function useTokens(): LososTokens {
  const config = useBoxConfig();
  const systemPrefersDark = useColorScheme() === "dark";
  return tokensOf(config?.themeMode ?? "system", systemPrefersDark);
}
