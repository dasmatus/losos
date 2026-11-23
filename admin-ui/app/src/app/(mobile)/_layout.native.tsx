/* The (mobile) group's root, native-only: the companion's shell kept whole
 * (companion/app/_layout.tsx, the port of MainActivity.kt). The theme wraps
 * the routes — the mode the config carries — and StatusBar follows the
 * resolved mode the Compose Surface's colour did. It is named
 * _layout.native.tsx because the web bundle has no routes in this group:
 * every file here is a platform extension, so Metro never bundles the shell
 * with its config store, zeroconf and the whole pane set into the browser.
 */

import * as React from "react";
import { StatusBar as ExpoStatusBar } from "expo-status-bar";
import { Stack } from "expo-router";
import { View } from "react-native";

import { loadConfig } from "../../store/config";
import { useTokens } from "../../mobile/theme/useTokens";

/* The (mobile)/ route files, so the layout knows the set and the bar's
 * first address (the dashboard behind index). */
const PANES = [
  "dashboard",
  "storage",
  "mesh",
  "apps",
  "network",
  "security",
  "advanced",
  "history",
  "backup",
  "cloud",
  "settings",
] as const;

export default function MobileLayout(): React.JSX.Element {
  const [ready, setReady] = React.useState(false);
  React.useEffect(() => {
    let live = true;
    void loadConfig().finally(() => {
      if (live) setReady(true);
    });
    return () => {
      live = false;
    };
  }, []);

  const tokens = useTokens();

  if (!ready) return <View style={{ flex: 1, backgroundColor: "#DCBDAA" }} />;

  return (
    <View style={{ flex: 1, backgroundColor: tokens.palette.background }}>
      {/* userInterfaceStyle is automatic in app.config.ts, so a system
       * preference drives ThemeMode.System; a pinned mode is enforced in
       * the theme layer (src/mobile/theme/useTokens.ts). */}
      <ExpoStatusBar style={tokens.dark ? "light" : "dark"} />
      <Stack screenOptions={{ headerShown: false }}>
        <Stack.Screen name="index" />
        {PANES.map((pane) => (
          <Stack.Screen key={pane} name={pane} />
        ))}
      </Stack>
    </View>
  );
}
