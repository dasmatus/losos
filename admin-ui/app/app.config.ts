/* LosOS' one front end: the admin web UI (this folder's SPA) and the
 * phone's panes, in a single Expo project.
 *
 * Until this file arrived the SPA built with Bun's bundler (build.ts) and
 * the phone was a second project beside it (admin-ui/companion, whose Expo
 * client mirrored the terminal's panes); the two now live in one codebase:
 * the web target serves the SPA through Expo Router's web route
 * (src/app/index.web.tsx, react-dom tree untouched), iOS and Android mount
 * the companion's panes as (mobile) routes, and the same Metro build makes
 * both. Explanatory comments that survive the merge keep their own history:
 * every "Kotlin" name names the Compose app the companion was ported from,
 * not a live file.
 *
 * Expo Go cannot run the native targets: the mDNS browse and the box's TLS
 * trust live in native code the Go runtime does not ship, so the build is
 * prebuild + development client (`npx expo prebuild`, then `npx expo run:ios`
 * / `run:android`), with android/ and ios/ CNG output kept out of git.
 */

import type { ExpoConfig } from "expo/config";

import { lososSemver } from "./scripts/losos-version.cjs";
import { serviceType, bonjourServices } from "./plugins/services";

const config: ExpoConfig = {
  name: "LosOS Admin",
  owner: "thanks-lads",
  // The EAS project's slug: extra.eas.projectId points at
  // @thanks-lads/losos, and EAS reads the two as one identity.
  slug: "losos",
  // Parity with the Compose app's versionName (app/build.gradle.kts): the
  // tag the tree is, not a counter anyone bumps. Resolution mirrors
  // backend/build.rs's stamp_version exactly — LOSOS_VERSION first (nix's
  // losos-admin-ui package exports it, because this fileset is
  // admin-ui/app alone and its nix build cannot walk ../nix), then
  // nix/version.nix — the sync contract version.yml keeps (its header
  // explains), for a checkout that carries the nix tree — then the plain
  // counter, the escape for a source tree without nix/ (nix/tests anchor
  // the tagged spelling). tag() is build.rs's tag(); Expo's version field
  // wants semver, so the display convention's leading v (the
  // `losos --version` word) is not what the store metadata carries;
  // buildNumber stays 1 on both platforms: that field is device-upgrade
  // ordered, not tag-shaped, and deriving it would reshuffle on a bump.
  version: lososSemver(),
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
  web: {
    // Metro on web, like the other two targets: one bundler, one Babel
    // config, one dependency set (react-native-web + react-dom, the SDK 54
    // pair `expo install` pinned). "static" pre-renders each router route
    // to HTML at `expo export` time - the companion's screens are the box's
    // panes, all behind the connect gate, so static is what a shareable
    // page needs and the SPA fallback (single-file output) is not.
    bundler: "metro",
    output: "static",
    // The brand salmon again, downscaled for the tab (assets/images/.
    // favicon.png is salmon.png box-filtered to 48x48).
    favicon: "./assets/images/favicon.png",
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
  // The EAS project slug links the development-client cloud builds
  // (extra.eas.projectId); `eas init --id` writes it here.
  extra: {
    eas: {
      projectId: "ec8f631b-a125-4735-af1c-7563f896525e",
    },
    // The browse type names the DNS-SD record the plugin wires the platform
    // to (reference the plugin + src/api/discovery.ts, which browse the
    // same one); the EAS link sits beside it (extra.eas.projectId above).
    lososServiceType: `${serviceType}._tcp`,
  },
  experiments: {
    typedRoutes: false,
  },
};

export default config;
