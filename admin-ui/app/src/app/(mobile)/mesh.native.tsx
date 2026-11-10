/* The (mobile) route /mesh: the dashboard focused on the Mesh pane.
 * Expo Router turns the tree into deep links - the phone's Push="losos:///mesh"
 * story (plugins/with-losos-discovery.js's scheme), the deep route the
 * notifications and the box's cards open - and the pane bar stays, the
 * pane arriving selected (DashboardScreen's initialTab). .native keeps the
 * web target out: the browser's Mesh stories are the SPA's pages, and the
 * panes' native modules are none of its business.
 */
import * as React from "react";

import { DashboardScreen } from "../../mobile/screens/DashboardScreen";

export default function MeshRoute(): React.JSX.Element {
  return <DashboardScreen initialTab="Mesh" />;
}
