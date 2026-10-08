// Loads the canvas module the way the Bevy cheat book's web page does,
// as a file because the box's CSP has no 'unsafe-inline': import the
// wasm-bindgen glue, run its init(), and swallow only the exception winit's
// web event loop may throw on purpose to leave the start-up call stack.
// Bevy 0.19 starts winit with spawn_app, which does not throw; the catch
// stays so an older or newer winit cannot break the page.
// Resolves to { mod, wasm }: the module's exports (Lab, LabCanvas, …) and
// the instance's raw exports (wasm.memory is its linear memory).
export async function loadRender(url) {
  const mod = await import(url);
  let wasm = null;
  try {
    wasm = await mod.default();
  } catch (e) {
    if (!(e instanceof Error) || !e.message.startsWith('Using exceptions for control flow')) throw e;
  }
  return { mod, wasm };
}

// Which renderer this browser gets: "webgpu" when navigator.gpu hands out
// an adapter, else "webgl2" when a scratch canvas gives a WebGL2 context,
// else "svg" (the React Lab's own SVG canvas, no module at all). Bevy picks
// its WebGPU or WebGL2 code paths at compile time, so the two are separate
// modules, pkg/webgpu/ and pkg/webgl2/. `force` ("webgpu" or "webgl2")
// skips the probe, for tests.
export async function pickBackend(force) {
  if (force === 'webgpu' || force === 'webgl2') return force;
  if (navigator.gpu && !webgpuFailedHere()) {
    try {
      if (await navigator.gpu.requestAdapter()) return 'webgpu';
    } catch {
      // No adapter: fall through to WebGL2.
    }
  }
  try {
    if (document.createElement('canvas').getContext('webgl2')) return 'webgl2';
  } catch {
    // No WebGL2 either.
  }
  return 'svg';
}

// A WebGPU adapter can still fail once Bevy uses it (a lost device, a
// validation error): the module then sends "render-error" and stops
// drawing. A canvas that had a WebGPU context cannot take a WebGL2 one and
// the module cannot be unloaded, so the page remembers it and reloads;
// pickBackend() then skips WebGPU in this browser.
const FAILED = 'losos-lab-webgpu-failed';
export function webgpuFailed() {
  try {
    localStorage.setItem(FAILED, '1');
  } catch {
    // Storage blocked: the reload picks WebGPU again, and fails again.
  }
}
function webgpuFailedHere() {
  try {
    return localStorage.getItem(FAILED) === '1';
  } catch {
    return false;
  }
}

// Waits before loading, so a page can show its SVG canvas first:
// "click" waits for #load to be pressed, "idle" for an idle period,
// a number for that many milliseconds; anything else loads at once.
export function delay(how, button) {
  if (how === 'click' && button) {
    button.hidden = false;
    return new Promise((ok) => button.addEventListener('click', () => { button.hidden = true; ok(); }, { once: true }));
  }
  if (how === 'idle') return new Promise((ok) => (window.requestIdleCallback || setTimeout)(() => ok(), { timeout: 2000 }));
  const ms = Number(how);
  if (ms > 0) return new Promise((ok) => setTimeout(ok, ms));
  return Promise.resolve();
}
