/* Expo config plugin: everything the mDNS browse of the boxes needs, in the
 * one place (the port of Discovery.kt's host app wiring). `react-native-zeroconf`
 * documents the prerequisites in its README and a wiki; an Expo app carries
 * its own manifest, so a plugin has to write them at `expo prebuild` time.
 *
 * Android - four permissions. NSD never asked for more than INTERNET, but the
 * library's Android implementations (NsdManager and the embedded DNS-SD
 * responder) want the network and WiFi state, and multicast joins want
 * CHANGE_WIFI_MULTICAST_STATE. `usesCleartextTraffic` and the network
 * security config (the port of res/xml/network.xml: cleartext allowed, user
 * CAs trusted) are written by expo-build-properties in app.config.ts.
 *
 * iOS 14+ - a browse over Bonjour only runs when the service types it may
 * touch are listed in NSBonjourServices and the local network permission has
 * a plain-language reason to show. Listing both the bare type and the
 * `.local.` spelled one is belt and braces: Apple's docs name the bare type,
 * several Bonjour browser guides name the qualified one.
 */

import { bonjourServices } from "./services.js";


export default function withLososDiscovery(config) {
  const info = config.ios?.infoPlist ?? {};

  const bonjour = new Set([
    ...(Array.isArray(info.NSBonjourServices) ? info.NSBonjourServices : []),
    ...bonjourServices,
  ]);

  return {
    ...config,
    ios: {
      ...config.ios,
      infoPlist: {
        ...info,
        NSBonjourServices: [...bonjour].sort(),
        NSLocalNetworkUsageDescription:
          info.NSLocalNetworkUsageDescription ??
          "LosOS Companion looks for your LosOS box on the home network.",
      },
    },
    android: {
      ...config.android,
      permissions: [
        ...new Set([
          ...(config.android?.permissions ?? []),
          "android.permission.INTERNET",
          "android.permission.ACCESS_NETWORK_STATE",
          "android.permission.ACCESS_WIFI_STATE",
          "android.permission.CHANGE_WIFI_MULTICAST_STATE",
        ]),
      ],
    },
  };
};
