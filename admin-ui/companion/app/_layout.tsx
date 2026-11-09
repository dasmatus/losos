/* The root layout, the port of MainActivity.kt's shell: the theme wraps
 * everything (the mode the config carries), and one route, index, holds
 * the connect screen and the dashboard. StatusBar's style follows the
 * resolved mode - the Compose Surface's colour did that for free.
 */

import * as React from "react";
import { StatusBar as ExpoStatusBar } from "expo-status-bar";
import { Stack } from "expo-router";
import { View } from "react-native";

import { loadConfig } from "../src/store/config";
import { useTokens } from "../src/theme/useTokens";

export default function RootLayout(): React.JSX.Element {
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
       * the theme layer (src/theme/useTokens.ts). */}
      <ExpoStatusBar style={tokens.dark ? "light" : "dark"} />
      <Stack screenOptions={{ headerShown: false }}>
        <Stack.Screen name="index" />
      </Stack>
    </View>
  );
}
