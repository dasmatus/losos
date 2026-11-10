/* The web bundle's babel transformer, wrapping Expo's default
 * (@expo/metro-config/build/babel-transformer) with the one exception the
 * dependency tree carries: onnxruntime-web's register file is a single
 * self-contained ESM output whose loader opens its worker script through a
 * computed `import(url)` (the `webpackIgnore`/`vite-ignore` comments are
 * what two other bundlers require). Metro's import rewriting is the thing
 * that refuses a computed call — so the file goes to the bundle as it is,
 * same code the browser module graph would load.
 */
const path = require("path");

let defaultTransform = null;

async function transform({ filename, options, src }) {
  if (!defaultTransform) {
    defaultTransform = require("@expo/metro-config/build/babel-transformer");
  }
  if (
    options?.type !== "asset" &&
    path
      .normalize(filename)
      .split(path.sep)
      .join("/")
      .includes("onnxruntime-web/dist/ort.wasm.bundle.min.mjs")
  ) {
    return { code: src, map: null };
  }
  return defaultTransform.transform({ filename, options, src });
}

module.exports = { transform };
