/* The web bundle's babel transformer, wrapping Expo's default
 * (@expo/metro-config/build/babel-transformer) with the exceptions the
 * dependency tree carries:
 *
 *  - onnxruntime-web's register file is a single self-contained ESM
 *    output whose loader opens its worker script through a computed
 *    `import(url)`; Metro's import rewriting refuses a computed call, so
 *    the file goes to the bundle as it is.
 *  - Hermes does not support `import.meta`, and babel-preset-expo's
 *    `unstable_transformImportMeta` polyfill rewrites it. Metro's
 *    transformer does not pick the option up from babel.config.mjs, so it
 *    is applied here for any module that uses `import.meta`.
 */
const path = require("path");

let defaultTransform = null;
let babel = null;

async function transform({ filename, options, src }) {
  if (!defaultTransform) {
    defaultTransform = require("@expo/metro-config/build/babel-transformer");
  }
  const norm = path.normalize(filename).split(path.sep).join("/");
  if (options?.type !== "asset" && norm.includes("onnxruntime-web/dist/ort.wasm.bundle.min.mjs")) {
    return { code: src, map: null };
  }
  // Modules that use `import.meta` (openpgp, calls/session.ts, ...) need
  // the polyfill; everything else goes through Expo's default transform.
  if (typeof src === "string" && src.includes("import.meta")) {
    if (!babel) babel = require("@babel/core");
    const result = babel.transformSync(src, {
      filename,
      babelrc: false,
      configFile: false,
      presets: [["babel-preset-expo", { unstable_transformImportMeta: true }]],
      caller: { name: "metro", platform: options?.platform ?? "android", engine: "hermes" },
    });
    return { code: result.code, map: result.map };
  }
  return defaultTransform.transform({ filename, options, src });
}

module.exports = { transform };