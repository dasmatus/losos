/* Metro's resolver, with the two things the web build of this SPA needs
 * beyond Expo's defaults (@expo/metro-config):
 *
 *  - the `.wasm?url` imports the wasm loaders carry (src/mdx/wasm.ts,
 *    src/lab/*): Bun's bundler turns the suffix into the asset's URL in the
 *    bundle it makes; Metro has no suffix protocol, and its asset-module
 *    import hands the caller a runtime handle, not a URL. scripts/
 *    lab-core.ts (and nix/packages.nix's preBuild for the sandbox copies)
 *    lay a generated twin beside the glue, `<glue>_bg.wasm.url.ts`,
 *    exporting the public path the export copies from public/wasm/, and
 *    the resolver points those imports at the twin. Bun's bundler never
 *    reads it.
 *
 * type: the config object Metro expects. */
const path = require("path");

module.exports = (async () => {
  const { getDefaultConfig } = require("@expo/metro-config");
  const config = await getDefaultConfig(__dirname);

  const fallback = config.resolver.resolveRequest;
  config.resolver.resolveRequest = (context, moduleName, platform) => {
    if (moduleName.endsWith(".wasm?url")) {
      const twin =
        context.originModulePath &&
        path.resolve(
          path.dirname(context.originModulePath),
          `${moduleName.slice(0, -4)}.url.ts`,
        );
      return { type: "sourceFile", filePath: twin };
    }
    if (fallback) return fallback(context, moduleName, platform);
    return context.resolveRequest(context, moduleName, platform);
  };

  return config;
})();
