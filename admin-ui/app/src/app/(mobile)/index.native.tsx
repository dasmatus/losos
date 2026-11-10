/* The (mobile) home route: the companion's single document
 * (companion/app/index.tsx, MainActivity.kt's setContent). The connect
 * screen stays until the demo is on or a box is linked, then the
 * dashboard takes the routes over - `needsConnect` is that switch,
 * the words the Compose shell ended with.
 */

import * as React from "react";
import { View } from "react-native";

import { needsConnect, useBoxConfig } from "../../store/config";
import { ConnectScreen } from "../../mobile/screens/ConnectScreen";
import { DashboardScreen } from "../../mobile/screens/DashboardScreen";

export default function MobileIndex(): React.JSX.Element {
  const config = useBoxConfig();
  if (config === null) return <View />;
  return needsConnect(config) ? <ConnectScreen /> : <DashboardScreen />;
}
