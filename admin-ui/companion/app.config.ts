/* LosOS Companion - the box in a phone's pocket.
 *
 * An Expo (React Native) client for the LosOS appliance: it finds the box on
 * the LAN by its `_losos._tcp` announcement (modules/configuration.nix,
 * modules/mdns.nix) or takes `<name>.local` typing, links with a token from
 * the admin pages' Settings -> Access tokens, and mirrors the terminal
 * client's panes on a phone. The Compose app it replaces is the comment
 * record these decisions cross-reference (its files are gone, its README was
 * carried; every "Kotlin" name below names the port, not a live file.
 *
 * Expo Go cannot run this app: the mDNS browse and the box's TLS trust live
 * in native code the Go runtime does not ship, so the build is prebuild +
 * development client (`npx expo prebuild`, then `npx expo run:ios` /
 * `run:android`), with android/ and ios/ CNG output kept out of git.
 */

import type { ExpoConfig } from "expo/config";

import { serviceType, bonjourServices } from "./plugins/services";

const config: ExpoConfig = {
  name: "LosOS Companion",
  slug: "losos-companion",
  // Parity with the Compose app's versionName (app/build.gradle.kts).
  version: "0.1.0",
  scheme: "losos",
  orientation: "portrait",
  // userInterfaceStyle is "automatic" so ThemeMode.System reads the system
  // preference the box already does; a pinned mode is enforced in the theme
  // layer (src/theme/tokens.ts).
  userInterfaceStyle: "automatic",
  // The coho salmon of themes/brand/salmon.png (NOAA Fisheries cutout,
  // credits beside the original), the mark every front end carries. The
  // splash sits on the brand plate the Compose launcher sat on
  // (values/colors.xml: #DCBDAA, flattened from themes/brand/plate.png).
  icon: "./assets/images/salmon.png",
  assetBundlePatterns: ["**/*"],
  ios: {
    bundleIdentifier: "us.dasmat.losos.companion",
    buildNumber: "1",
    supportsTablet: true,
    // The box's admin pages serve a self-signed certificate on the LAN until
    // the owner enrols a CA (BoxTrust.kt and its res/xml/network.xml, which
    // already trusted user-added CAs). Arbitrary loads are also what lets a
    // discovered box be probed as plain `http://<ip>:<port>` the way the
    // connect screen's fallback wanted. Scoped exceptions cannot spell
    // "*.local", so the loads are allowed at large and the box-address
    // language lives in README; a release to the App Store would narrow this.
    infoPlist: {
      NSAppTransportSecurity: {
        NSAllowsArbitraryLoads: true,
        NSAllowsArbitraryLoadsInLocalNetwork: true,
      },
      // The same list plugins/with-losos-discovery writes is spelled out
      // here so prebuild reads it in one place; the plugin merges, and its
      // Set keeps a plain-list config from dropping a spelling.
      NSBonjourServices: [...bonjourServices],
      NSLocalNetworkUsageDescription:
        "LosOS Companion looks for your LosOS box on the home network.",
    },
    entitlements: {
      // The Bonjour browser needs no multicast entitlement; documented in
      // README (raw mDNS readers would, and that is Apple's own review).
      // Left out so a TestFlight build does not hinge on it.
    },
  },
  android: {
    package: "us.dasmat.losos.companion",
    versionCode: 1,
    edgeToEdgeEnabled: true,
    adaptiveIcon: {
      foregroundImage: "./assets/images/salmon.png",
      // The brand plate behind the salmon (values/colors.xml).
      backgroundColor: "#DCBDAA",
    },
    intentFilters: [],
  },
  plugins: [
    "expo-router",
    [
      "expo-splash-screen",
      {
        image: "./assets/images/salmon.png",
        imageWidth: 144,
        resizeMode: "contain",
        backgroundColor: "#DCBDAA",
      },
    ],
    [
      "expo-build-properties",
      {
        android: {
          // The old manifest's network.xml: cleartext to the LAN,
          // user-added CAs trusted (BoxStore.kt's BoxTrust behaviour is
          // carried by src/api/transport.ts for the self-signed case).
          usesCleartextTraffic: true,
          // react-native-zeroconf 0.17 and react-native-blob-util 1.x are
          // old-architecture modules; the interop layer carries them until
          // zeroconf's 1.0 C++ module (
          // https://github.com/balthazar/react-native-zeroconf#readme).
          newArchEnabled: false,
        },
        ios: {
          newArchEnabled: false,
        },
      },
    ],
    "./plugins/with-losos-discovery.js",
  ],
  // The browse type names the DNS-SD record the plugin wires the platform to
  // (reference the plugin + src/api/discovery.ts, which browse the same one).
  extra: {
    lososServiceType: `${serviceType}._tcp`,
  },
  experiments: {
    typedRoutes: false,
  },
};

export default config;
