/* The web bundle's babel transformer, wrapping Expo's default
 * (@expo/metro-config/build/babel-transformer) with the exceptions the
 * dependency tree carries:
 *
 *  - onnxruntime-web's register file is a single self-contained ESM
 *    output whose loader opens its worker script through a computed
 *    `import(url)`; Metro's import rewriting refuses a computed call, so
 *    the file goes to the bundle as it is.
 *  - Hermes does not support `import.meta`, and babel-preset-expo's
 *    `unstable_transformImportMeta` polyfill rewrites it to
 *    `globalThis.__ExpoImportMetaRegistry` (expo's winter runtime sets
 *    that up). Metro's transformer does not pick the option up from
 *    babel.config.mjs, so the rewrite is done here as a textual
 *    substitution before Expo's default transform handles the TypeScript.
 */
const path = require("path");

let defaultTransform = null;

async function transform({ filename, options, src }) {
  if (!defaultTransform) {
    defaultTransform = require("@expo/metro-config/build/babel-transformer");
  }
  const norm = path.normalize(filename).split(path.sep).join("/");
  if (options?.type !== "asset" && norm.includes("onnxruntime-web/dist/ort.wasm.bundle.min.mjs")) {
    return { code: src, map: null };
  }
  // Rewrite `import.meta` to the runtime registry before Expo's default
  // transform (which handles TypeScript) sees the file, so the Hermes
  // syntax error never arises.
  let source = src;
  if (typeof src === "string" && src.includes("import.meta")) {
    source = src.replace(/import\.meta\b/g, "globalThis.__ExpoImportMetaRegistry");
  }
  return defaultTransform.transform({ filename, options, src: source });
}

module.exports = { transform };