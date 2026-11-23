/* LosOS TUI wasm (admin-ui/tui, wasm32 build): the API/drive surface a page
 * can use to drive the box's JSON API without a local terminal binary.
 *
 * The wasm-bindgen "web" package is expected at /tui/losos.js (+ losos_bg.wasm)
 * in the built web bundle (copied into public/tui/ from the losos-tui-wasm
 * build output when available). Native targets can load the same assets from
 * their bundled resources when packaged.
 *
 * This module exposes a small loader; the drive API itself is whatever the
 * generated wasm-bindgen package exports. */
export interface TuiWasmModule {
  default?: (input?: unknown) => Promise<unknown>;
  [key: string]: unknown;
}

let loading: Promise<TuiWasmModule> | null = null;

export async function loadLososTuiWasm(): Promise<TuiWasmModule> {
  if (!loading) {
    loading = (async () => {
      // Served as a static asset from the web bundle / app package. The
      // specifier goes in a variable: a literal import would be resolved
      // ahead of time — tsc's typecheck is the shipped build's first step —
      // against a module that only exists once the bundle is served.
      const url = "/tui/losos.js";
      const mod = (await import(url)) as TuiWasmModule;
      if (typeof mod.default === "function") {
        await mod.default({ module_or_path: "/tui/losos_bg.wasm" });
      }
      return mod;
    })();
  }
  return loading;
}
