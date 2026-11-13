/* Expo config plugin: the release build's signing configuration, read from
 * the environment so no credential ever lands in the tree (the same rule
 * eas.json's header carries — EXPO_TOKEN in CI keeps the secrets backend's,
 * credentials.json never lands here).
 *
 * android/app/build.gradle is generated output (CNG, the README's build
 * section): `expo prebuild` writes it fresh, with the template's release
 * buildType riding signingConfigs.debug. This plugin appends a
 * self-contained Groovy block that, when the environment carries a keystore,
 * registers it as a `lososRelease` signingConfig and points the release
 * buildType at it; without the environment (every local build) the block
 * evaluates to nothing and the generated file behaves exactly as before.
 * Appending after the `android { }` block re-configures the same extension,
 * which is the Gradle-sanctioned late-configuration route.
 *
 * The four variables (the CI's .github/workflows/mobile-release.yml decodes
 * the repo secrets into these before prebuild):
 *
 *   LOSOS_RELEASE_STORE_FILE      path to the .keystore (PKCS12)
 *   LOSOS_RELEASE_STORE_PASSWORD  the keystore's password
 *   LOSOS_RELEASE_KEY_ALIAS       the signing key's alias inside the store
 *   LOSOS_RELEASE_KEY_PASSWORD    the key's password (the same secret twice:
 *                                 a PKCS12 store carries one password, but
 *                                 Gradle's SigningConfig keeps the two apart)
 *
 * iOS has no twin here: Apple issues its signing material (the eas.json
 * header's TODO carries the account story), so the iOS lane stays unsigned
 * until a paid Apple Developer account exists.
 */

// "expo/config-plugins.js", not "expo/config-plugins": the package carries
// no exports map, so the ESM loader resolves the bare spelling as a literal
// (nonexistent) path — the file on disk is the .js one.
import { withDangerousMod } from "expo/config-plugins.js";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

/* The marker keeps a re-run of prebuild (which regenerates build.gradle
 * from scratch, so this really only guards against double application by
 * another config phase of the same run) from appending the block twice. */
const MARKER = "lososReleaseSigning";

const BLOCK = `
// LosOS release signing (${MARKER}): appended by
// plugins/with-release-signing.js at expo prebuild time, a no-op unless the
// environment carries a keystore (the CI's mobile-release.yml does).
def lososReleaseStoreFile = System.getenv("LOSOS_RELEASE_STORE_FILE")
if (lososReleaseStoreFile != null) {
    android {
        signingConfigs {
            create("lososRelease") {
                storeFile = file(lososReleaseStoreFile)
                storeType = "PKCS12"
                storePassword = System.getenv("LOSOS_RELEASE_STORE_PASSWORD")
                keyAlias = System.getenv("LOSOS_RELEASE_KEY_ALIAS")
                keyPassword = System.getenv("LOSOS_RELEASE_KEY_PASSWORD")
            }
        }
        buildTypes {
            release {
                signingConfig = signingConfigs.getByName("lososRelease")
            }
        }
    }
}
`;

export default function withReleaseSigning(config) {
  return withDangerousMod(config, [
    "android",
    async (modConfig) => {
      const path = resolve(
        modConfig.modRequest.platformProjectRoot,
        "app",
        "build.gradle",
      );
      let text = readFileSync(path, "utf8");
      if (!text.includes(MARKER)) {
        text += BLOCK;
        writeFileSync(path, text);
      }
      return modConfig;
    },
  ]);
};
