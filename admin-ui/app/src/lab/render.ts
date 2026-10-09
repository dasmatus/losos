/* The GPU canvas (admin-ui/lab/render, README.md there): which backend this
 * browser gets, loading that module, and moving the page onto it.
 *
 * The page paints with the SVG canvas and core-pkg's small module first.
 * Once it is up, `startGpu` picks WebGPU (navigator.gpu and an adapter),
 * else WebGL2, else nothing, loads that one module (about 3.4 to 3.6 MB
 * brotli), moves the core's state into the module's own `Lab` (core.ts,
 * `adopt`), and only then lets canvas.tsx put the Bevy canvas in place. A
 * failure anywhere leaves the page on SVG and says so once, in a toast. A
 * device lost later (render-error) also goes back to SVG, keeping the new
 * module's `Lab`, which needs no canvas; a lost WebGPU device is remembered
 * so the next visit starts on WebGL2.
 *
 * A browser that offers WebGPU but has no adapter for it (Chromium on many
 * Linux GPUs) prints "No available adapters." on the console when asked,
 * and the page cannot silence that. Such a browser is remembered by its
 * user agent, so the probe runs once per browser version, not on every
 * visit. `?canvas=webgpu` still asks.
 *
 * `?canvas=svg|gpu|webgpu|webgl2` overrides the choice for one load, and
 * the localStorage key `losos-lab-canvas` (same values) for this browser. */

import { t } from "@/lib/i18n";
import type { LabApi } from "./core";
import type { LabStore } from "./store";

export type Backend = "webgpu" | "webgl2";
export type RenderModule = typeof import("./render-pkg/webgl2/losos_lab_render.js");
export type GpuCanvas = import("./render-pkg/webgl2/losos_lab_render.js").LabCanvas;

export interface GpuState {
  /** svg: no GPU canvas (none wanted, none possible, or it failed);
   *  loading: the module is on its way; starting: the Bevy canvas is
   *  mounted under the SVG one and has not drawn yet; gpu: it has. */
  state: "svg" | "loading" | "starting" | "gpu";
  backend: Backend | null;
  mod: RenderModule | null;
  lab: LabApi | null;
  /** Why the page is on SVG after trying, for the badge's tooltip. */
  failed: string | null;
}

const CHOICE_KEY = "losos-lab-canvas";
const FAILED_KEY = "losos-lab-webgpu-failed";
/** The user agent of a browser whose WebGPU had no adapter. */
const NO_ADAPTER_KEY = "losos-lab-webgpu-none";
/** How long the Bevy canvas may take to draw once it is mounted. */
const FIRST_FRAME_MS = 20_000;

type Wish = "svg" | "auto" | Backend;

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

/** The canvas the URL, then this browser, asks for. */
export function canvasWish(): Wish {
  const read = (v: string | null | undefined): Wish | null =>
    v === "svg" ? "svg" : v === "gpu" || v === "auto" ? "auto" : v === "webgpu" || v === "webgl2" ? v : null;
  const fromUrl = read(new URLSearchParams(window.location.search).get("canvas"));
  if (fromUrl) return fromUrl;
  let stored: string | null = null;
  try {
    stored = storage()?.getItem(CHOICE_KEY) ?? null;
  } catch {
    stored = null;
  }
  return read(stored) ?? "auto";
}

function webgpuFailedBefore(): boolean {
  try {
    return storage()?.getItem(FAILED_KEY) === "1";
  } catch {
    return false;
  }
}

function rememberWebgpuFailed(): void {
  try {
    storage()?.setItem(FAILED_KEY, "1");
  } catch {
    /* private window: the next visit tries WebGPU again */
  }
}

function noAdapterBefore(): boolean {
  try {
    return storage()?.getItem(NO_ADAPTER_KEY) === navigator.userAgent;
  } catch {
    return false;
  }
}

function rememberNoAdapter(): void {
  try {
    storage()?.setItem(NO_ADAPTER_KEY, navigator.userAgent);
  } catch {
    /* private window: the next visit asks again */
  }
}

async function hasWebgpu(): Promise<boolean> {
  const gpu = (navigator as Navigator & { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
  if (!gpu) return false;
  try {
    const timeout = Symbol("timeout");
    const adapter = await Promise.race([gpu.requestAdapter(), new Promise((r) => window.setTimeout(() => r(timeout), 2000))]);
    // A browser that answered "none" answers the same next time; a slow
    // one may not.
    if (adapter == null) rememberNoAdapter();
    return adapter != null && adapter !== timeout;
  } catch {
    return false;
  }
}

function hasWebgl2(): boolean {
  try {
    const c = document.createElement("canvas");
    const gl = c.getContext("webgl2");
    if (!gl) return false;
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return true;
  } catch {
    return false;
  }
}

/** The backend to load, or null for none (the SVG canvas stays). Decided
 *  before anything heavy is fetched. */
export async function pickBackend(wish: Wish): Promise<Backend | null> {
  if (wish === "svg") return null;
  if (wish === "webgpu") return (await hasWebgpu()) ? "webgpu" : null;
  if (wish === "webgl2") return hasWebgl2() ? "webgl2" : null;
  if (!webgpuFailedBefore() && !noAdapterBefore() && (await hasWebgpu())) return "webgpu";
  return hasWebgl2() ? "webgl2" : null;
}

/** Imports one backend's glue and module and starts it. winit's web loop
 *  may throw "Using exceptions for control flow" on purpose; that one error
 *  is not a failure. */
export async function loadRender(backend: Backend): Promise<RenderModule> {
  const [mod, url] =
    backend === "webgpu"
      ? ((await Promise.all([
          import("./render-pkg/webgpu/losos_lab_render.js"),
          import("./render-pkg/webgpu/losos_lab_render_bg.wasm?url"),
        ])) as unknown as [RenderModule, { default: string }])
      : await Promise.all([import("./render-pkg/webgl2/losos_lab_render.js"), import("./render-pkg/webgl2/losos_lab_render_bg.wasm?url")]);
  try {
    await mod.default({ module_or_path: url.default });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    if (!msg.startsWith("Using exceptions for control flow")) throw e;
  }
  return mod;
}

function note(text: string): void {
  console.info(`LosOS Lab canvas: ${text}`);
}

/** Tries the GPU canvas once per page. Runs after the page has painted. */
export async function startGpu(s: LabStore): Promise<void> {
  const wish = canvasWish();
  const backend = await pickBackend(wish);
  note(backend ?? "svg");
  if (!backend) return;
  s.setGpu({ state: "loading", backend });
  let mod: RenderModule;
  try {
    mod = await loadRender(backend);
  } catch (e) {
    gpuFailed(s, `the ${backend} module did not load: ${e instanceof Error ? e.message : String(e)}`);
    return;
  }
  const lab = s.core.adopt((seed, now, tz) => new mod.Lab(seed, now, tz));
  if (!lab) {
    gpuFailed(s, "the setup could not be moved into the GPU canvas's module");
    return;
  }
  s.setGpu({ state: "starting", mod, lab });
  window.setTimeout(() => {
    if (s.gpu.state === "starting") gpuFailed(s, "the GPU canvas drew nothing");
  }, FIRST_FRAME_MS);
}

/** The Bevy canvas drew its first frame: it replaces the SVG one. */
export function gpuDrew(s: LabStore, adapter: string): void {
  if (s.gpu.state !== "starting") return;
  note(`Bevy on ${s.gpu.backend}, adapter ${adapter}`);
  s.setGpu({ state: "gpu" });
}

/** Back to (or stay on) SVG, and say so once. */
export function gpuFailed(s: LabStore, why: string, lost = false): void {
  if (s.gpu.state === "svg") return;
  note(why);
  const webgpuLost = lost && s.gpu.backend === "webgpu";
  if (webgpuLost) rememberWebgpuFailed();
  s.setGpu({ state: "svg", failed: why });
  s.toast(t(webgpuLost ? "lab.canvas.lost" : "lab.canvas.fallback"));
}
