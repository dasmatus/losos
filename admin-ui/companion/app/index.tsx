/* The app's single document (MainActivity.kt setContent): the connect
 * screen stays until the demo is on or a box is linked, then the
 * dashboard takes the routes over - `needsConnect` is that switch, the
 * words the Compose shell ended with.
 */

import * as React from "react";
import { View } from "react-native";

import { needsConnect, useBoxConfig } from "../src/store/config";
import { ConnectScreen } from "../src/screens/ConnectScreen";
import { DashboardScreen } from "../src/screens/DashboardScreen";

export default function Index(): React.JSX.Element {
  const config = useBoxConfig();
  if (config === null) return <View />;
  return needsConnect(config) ? <ConnectScreen /> : <DashboardScreen />;
}
