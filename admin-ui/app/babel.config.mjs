/* Babel for both targets. babel-preset-expo carries Metro's transforms and,
 * since SDK 53, wires the expo-router plugin in automatically; it is spelled
 * out anyway so the routing transform is independent of that convenience.
 *.mjs because package.json is "type": "module" — a CommonJS .js config gets
 * Node's "not defined in ES module scope" from the loader that walks it */
export default function (api) {
  api.cache(true);
  return {
    presets: ["babel-preset-expo"],
    plugins: ["expo-router/babel"],
  };
};
