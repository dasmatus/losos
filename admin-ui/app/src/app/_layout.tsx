/* The router's root, one file for every platform. Nothing lives here but
 * the slot (expo-router/README): the web target's only route is the SPA
 * mount (index.web.tsx, DOM code react-dom bundles), the native targets'
 * routes are the (mobile) group, whose layout carries the theme the
 * companion's shell did. Keeping this root free of both is what keeps one
 * backend-offending direction out of the other bundle: Metro resolves
 * Platform-Specific Routes per target, so the two trees never see each
 * other's imports — react-dom and @base-ui stay out of the phone, and
 * react-native's native modules stay out of the browser bundle.
 */

import { Slot } from "expo-router";

export default function RootLayout(): React.JSX.Element {
  return <Slot />;
}
