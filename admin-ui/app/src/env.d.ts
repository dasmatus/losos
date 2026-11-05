/* The build's ambient types — what `vite/client` declared for Vite, written
 * for the Bun build instead:
 *
 * - `*.png` etc. compile to the asset's URL at the page origin (here the
 *   import is a value Bun copies into assets/), so they carry the URL
 *   string type;
 * - `*.wasm?url` compiles to the URL of the copied WebAssembly module —
 *   src/mdx/wasm.ts and the Lab's three Rust cores load that way;
 * - `import.meta.env` is the build's client environment: BASE_URL (the page
 *   mount, "/" on the box and "/calls/guest/" for the guest page) and, set
 *   on a hosted Lab build only, VITE_LAB_HOSTED="1".
 */

declare module "*.png" {
  const url: string;
  export default url;
}
declare module "*.jpg" {
  const url: string;
  export default url;
}
declare module "*.jpeg" {
  const url: string;
  export default url;
}
declare module "*.svg" {
  const url: string;
  export default url;
}
declare module "*.webp" {
  const url: string;
  export default url;
}
declare module "*.woff2" {
  const url: string;
  export default url;
}
declare module "*.wasm?url" {
  const url: string;
  export default url;
}

/* Stylesheets: every import is a side-effect only import in src (the
 * registry turns it into a <link>), so these only need to exist. */
declare module "*.css" {
  const css: string;
  export default css;
}

interface ImportMetaEnv {
  readonly BASE_URL: string;
  readonly VITE_LAB_HOSTED?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
