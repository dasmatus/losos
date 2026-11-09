/* Babel for the companion. babel-preset-expo carries Metro's transforms and,
 * since SDK 53, wires the expo-router plugin in automatically; it is spelled
 * out anyway so the routing transform is independent of that convenience. */
module.exports = function (api) {
  api.cache(true);
  return {
    presets: ["babel-preset-expo"],
    plugins: ["expo-router/babel"],
  };
};
