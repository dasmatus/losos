/* Babel for both targets. babel-preset-expo carries Metro's transforms and,
 * since SDK 53, wires the expo-router plugin in automatically — and since
 * SDK 50 spelling it out prints a deprecation warning on every dev-server
 * bundling pass, ten lines of noise a pane load. The preset is the one
 * source of the routing transform now.
 *.mjs because package.json is "type": "module" — a CommonJS .js config gets
 * Node's "not defined in ES module scope" from the loader that walks it */
export default function (api) {
  api.cache(true);
  return {
    presets: ["babel-preset-expo"],
  };
};
