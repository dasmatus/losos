# losos-lab-render

LosOS Lab's canvas on the GPU. Bevy draws the logical and the physical view
into a `<canvas>` the page owns, through WebGPU where the browser has it and
WebGL2 where it does not. React keeps everything around the canvas: the top
bar, the tray, the inspector, the consoles and the toasts. The React page's
SVG canvas draws first and stays when neither GPU path works.

The views are a 3D scene. Every room, Wi-Fi range, cable and device is a
mesh on a ground plane, and an orthographic camera looks straight down at
it. Each build is one WebAssembly module that also carries the Lab's core
(`../core`, contract in `../core/README.md`), so a page has one `Lab`. The
page drives that `Lab` as it drives the core alone, and the canvas renders
the same instance every frame without copying the world.

The canvas draws what the React Lab's SVG canvas draws
(`admin-ui/app/src/lab/svg-canvas.tsx`, `icons.tsx`, `lab.css`), from the
same coordinates:

- the grid, the devices with their icons, names, addresses and badges;
- cables by kind, with link LEDs and port names placed clear of each other;
- buses, and packets riding the logical links;
- the four rooms with their furniture, front panels with port LEDs, Wi-Fi
  ranges, and sagging cables with their lengths placed clear of the
  hardware in the physical view;
- selection, hover and the connect tool's rubber band.

Every shape is tessellated in Rust. There is no bitmap art.

## Controls

The canvas reads the pointer and the wheel. It never reads the keyboard.

| Input | Does |
|---|---|
| Click a device or a cable | selects it (`select`, then `tap` if the pointer did not move) |
| Drag a device | moves it; in the physical view, into another room too |
| Drag the empty canvas | pans |
| Click the empty canvas | clears the selection |
| Wheel | zooms about the pointer, by e^0.15 per 100 px of wheel |
| Two fingers | pinch to zoom, about the midpoint |
| Click a power LED (physical view) | asks the page to switch the device (`power`) |
| Click with a place tool | asks the page to add the device there (`place`) |
| Click with the connect tool | picks the ends of a cable (`connect` or `request-port-pick`) |
| Click with the delete tool | asks the page to remove what it hit (`delete`) |
| Right click | reports the target and the point (`context`) |
| Hover | reports the target and its tooltip (`hover`) |

Mouse, pen and touch all arrive as pointer events. Only the primary mouse
button acts. Zoom stays between 0.3 and 2.5 (`Tunables`). Esc and Delete
belong to the page. The React Lab cancels the tool on Esc and removes the
selection on Delete or Backspace.

## Run

The demo page in `demo/` drives the canvas the way the React Lab does, on
its own. Build both backends into `demo/pkg/` (see "Building", with
`--out-dir demo/pkg/$backend`), then serve the folder under the admin
page's content security policy, which `demo/serve.json` sets:

```sh
npx --yes serve@14.2.4 demo   # then open http://localhost:3000/
```

Query parameters: `setup=two-sites|web|star|bus|office|three`,
`view=physical`, `theme=dark`, `defer=click|idle|<ms>`, `msaa=1|4`,
`backend=webgpu|webgl2`, `labels=0`.

In the admin UI, `npm run lab:render` in `admin-ui/app` builds both
backends into `src/lab/render-pkg/` and `npm run dev` serves the Lab at
`/lab/`.

## Building

Bevy chooses its WebGPU or WebGL2 code paths at compile time. With both
features on, the WebGPU paths win and that module cannot run on WebGL2. So
the crate has two exclusive features, `webgl2` (the default) and `webgpu`,
and there are two modules:

```sh
for backend in webgpu webgl2; do
  cargo build --release --target wasm32-unknown-unknown \
    --no-default-features --features $backend
  wasm-bindgen --target web --out-dir pkg/$backend --out-name losos_lab_render \
    target/wasm32-unknown-unknown/release/losos_lab_render.wasm
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types \
    --enable-multivalue pkg/$backend/losos_lab_render_bg.wasm \
    -o pkg/$backend/losos_lab_render_bg.wasm
  rm -rf target
done
```

`nix build .#losos-lab-render` does the same and leaves `webgpu/` and
`webgl2/` in `result/`, each with `losos_lab_render.js`, its `.d.ts` and
`losos_lab_render_bg.wasm`. `losos-admin-ui` copies both into the page.
Run wasm-opt after wasm-bindgen, never before, because wasm-bindgen reads
custom sections that wasm-opt rewrites. The two builds share no Bevy crate
and each release target directory takes about 3.5 GB, so the loop removes
the first before it starts the second. A release build of one backend
takes about 6 minutes on 4 cores.

Debug builds compile this crate at `opt-level = 1` and every dependency at
`opt-level = 3`, so a debug Bevy draws at a usable rate.

## Crates

Pinned exactly, and moved together:

- `bevy = "=0.19.1"`, with default features off and only the ones listed
  under "Sizes". Bevy 0.19.1 brings wgpu 29.0.4, naga 29.0.4 and winit
  0.30.13.
- `wasm-bindgen = "=0.2.127"`, exactly nixpkgs' `wasm-bindgen-cli`, as in
  `../core`. `Cargo.lock` resolves `web-sys` and `js-sys` to 0.3.104 to
  match.

Not pinned beyond `Cargo.lock`: `serde_json` 1 and the path dependency
`losos-lab-core`. Bevy, `js-sys` and `web-sys` are wasm32-only
dependencies; the native build has none of them.

## Using it

```ts
const lab = new mod.Lab(seed, Date.now(), -new Date().getTimezoneOffset());
lab.load_scenario("two-sites");
const canvas = mod.LabCanvas.attach(lab, "#lab-canvas");
canvas.set_theme(JSON.stringify({ theme, palette }));
canvas.on("select", (json) => store.select(JSON.parse(json)));
// The page keeps calling lab.tick(dt) every frame, lab.add_device(),
// lab.connect() and so on, as it does with the core alone.
```

`Lab` is the core's class, unchanged (`../core/README.md`). A page that
uses the GPU canvas must hand it the `Lab` from the same module, because
two modules hold two separate cores. Both backend modules export the same
API.

The React Lab starts on the core's own small module and moves to this one
later. `admin-ui/app/src/lab/core.ts` records every call that changes the
`Lab`, makes a new `Lab` from this module with the same seed, clock and
time zone, replays the calls, and switches only if both snapshots then
match. The core is deterministic, so the replay rebuilds the same state.

### `LabCanvas`

```ts
type CanvasEvent =
  | "select" | "tap" | "hover" | "request-port-pick" | "connect" | "place"
  | "delete" | "power" | "move" | "moved" | "context" | "camera"
  | "viewport" | "frame" | "render-error";

class LabCanvas {
  // Starts Bevy on the <canvas> the selector finds, the first time on a
  // page; later calls rebind the running app (see "Lifecycle").
  static attach(lab: Lab, selector: string): LabCanvas;
  detach(): void;                       // stops drawing, drops the callbacks
  free(): void;                         // the JS handle only; the app stays

  set_view(view: "logical" | "physical"): void;
  set_tool(tool: "select" | "delete" | `place:${string}` | `connect:${string}`): void;
  set_connect_from(id: string): void;   // JSON device id or "null"; the page keeps the first end
  set_selection(sel: string): void;     // JSON {kind: "device"|"link", id} or "null"
  set_theme(theme: string): void;       // JSON, see "Theme"
  set_running(ids: string): void;       // JSON string[]: devices with a guest ("VM" tag)
  set_msaa(samples: number): void;      // 4 (default) or 1
  set_labels(on: boolean): void;        // all labels on or off; off skips a pass
  set_pitch(deg: number): void;         // camera angle, 20 to 90 (default, straight down)

  on(event: CanvasEvent, cb: (payloadJson: string) => void): void;  // one per event
  off(event: CanvasEvent): void;

  fit(): void;                          // fits the current view
  zoom_by(factor: number): void;        // about the canvas centre
  camera(): string;                     // {x, y, k}: canvas px = world × k + (x, y)
  set_camera(cam: string): void;        // the page keeps the camera from now on
  redraw(): void;                       // forces a rebuild of the scene
  stats(): string;                      // see "Measuring"
  backend(): string;                    // "webgpu" | "webgl2": this module
}

function webgl2_supported(): boolean;
function backend(): string;             // the same, without a canvas
```

`set_tool` takes the tray's names: `place:box`, `place:router` and so on,
and `connect:copper`, `connect:fiber`, `connect:wan`, `connect:wifi`. Any
`set_tool` or `set_view` drops a half-made connection, which is what Esc
should call.

Two calls hand a piece of state to the page:

- `set_connect_from` puts the connect tool in the page's hands. From then
  on a click with it sends `connect` and changes nothing in the canvas,
  which only draws the rubber band from the end the page names. The React
  Lab does this, because its tray, inspector and port picker own the
  half-made cable.
- `set_camera` makes the page the camera's owner. The canvas then fits a
  view only when `fit()` asks, not on its own when a new setup arrives. It
  still pans and zooms under the pointer and reports each change as
  `camera`.

### Events

Every callback gets one argument, the payload as JSON text, which is the
core's convention too. A click changes nothing in the `Lab` except by
dragging. The canvas asks, and the page makes the call, so the page keeps
its toasts, its dirty flag and its undo in one place.

| Event | Payload | The page then |
|---|---|---|
| `select` | `{kind: "device"\|"link", id}` or `null`, on pointer down | shows the inspector; the canvas already highlights it |
| `tap` | the same, on a click that did not drag | treats it as a click, as the SVG canvas's `onSelect(…, {tap: true})` |
| `hover` | `{kind, id, title}` or `null` | may show `title` as a tooltip |
| `place` | `{type, x, y, opts: {site, px?, py?}, shift}` | `lab.add_device(type, x, y, JSON.stringify(opts))`; without shift, `set_tool("select")` |
| `connect` | `{id, port, shift}`, only after `set_connect_from`; `port` is the port under the pointer in the physical view, else `null` | takes it as the first or second end, as its own connect flow does |
| `request-port-pick` | `{a, b, kind, shift, aPorts, bPorts}`, free ports in catalogue order; without `set_connect_from` | asks which ports, or takes the first, then `lab.connect(a, b, kind, aPort, bPort)` |
| `delete` | `{kind: "device"\|"link", id}` | `lab.remove_device` or `lab.remove_link` |
| `power` | `{id}`, a power LED in the physical view | `lab.set_power(id, !power)` |
| `move` | `{id, to, done}` while dragging, and once more with `done: true` on the drop; `to` is `LabCanvasProps`' `MoveTo` | nothing, because the core is already updated; a host that keeps its own copy follows it |
| `moved` | `{id, view, site}` after a drag; `site` is the new room or `null` | toasts a room change |
| `context` | `{target, clientX, clientY, x, y}` on a right click | opens a menu at the client point |
| `camera` | `{x, y, k}` after a pan, zoom or fit, not after `set_camera` | keeps it, to restore the view later |
| `viewport` | `{width, height}` in CSS px, when the canvas is sized | keeps it, for its own fit |
| `frame` | `{first: true, at}` once, on the first frame drawn | swaps the SVG canvas out |
| `render-error` | `{type, description}` once, when wgpu reports a lost device or a validation error; drawing stops | falls back to SVG (see "Loading") |

The canvas notices world changes by itself. It checks a fingerprint of the
devices, links, addresses and LosOS states every frame, and draws a cable
that just appeared with amber LEDs for 1.4 s, as the SVG canvas does. A
whole new set of device ids is fitted automatically unless the page owns
the camera.

### Who calls `tick`

The page calls `lab.tick(dt)` in its `requestAnimationFrame` loop, for
events, consoles and the clock. The canvas reads the packets from the same
core each frame (`LabCore::packets_now`, the same positions `tick`
returns), so it never advances the simulation. Packet speed is therefore
the page's setting (`set_sim_speed`, `set_clock_speed`), not the canvas's.

## Code layout

The plain Rust is unit-tested natively with no GPU and no Bevy build:

- `paint`: triangles from shapes;
- `theme`: the palette from the page's tokens;
- `icons`: the device drawings;
- `scene`: the two views as parts, labels and hit regions;
- `interact`: pointer handling;
- `tunables`: the settings the canvas owns.

`web.rs` is the JavaScript API, the DOM listeners and the one per-page
state. `src/app/` is the Bevy app, one plugin per concern, all added in
`app::start`:

- `ThemePlugin`: the clear colour and the logical view's grid.
- `CameraPlugin`: the scene camera, the label camera over it, and the
  page's camera and tunables applied to both.
- `ScenePlugin`: an entity per part, the rubber band, the packets.
- `PickingPlugin`: pick meshes under each part's hit regions, and the ray
  cast that resolves the queued pointer events.
- `LabelsPlugin`: the text.

All of it runs in `Update`, in a fixed order of system sets (`Step`): input,
theme, scene, picking, labels, camera. Label placement runs in `PostUpdate`
because it needs this frame's transforms. Nothing steps at a fixed rate in
the canvas, because the core's clock is the page's, so there is no
`FixedUpdate` work.

`Tunables` holds what the canvas decides: multisampling, labels on or off,
the zoom range, the wheel rate, the camera pitch and how long the zoom has
to rest before labels are drawn again at exactly that zoom. The setters
write the copy in `Ui`, which the pointer code reads. On wasm the same
struct is a Bevy resource, refreshed from that copy at the start of each
frame, and the systems read the resource.

## Theme

The canvas draws with the house tokens (`admin-ui/app/src/styles/tokens.css`)
and the Lab's aliases of them in `admin-ui/app/src/lab/lab.css`, never a
colour of its own. It takes the palette in the shape the React Lab builds
for it (`canvas.tsx`, `PALETTE_TOKENS`, `LabCanvasProps.palette`):

```ts
canvas.set_theme(JSON.stringify({ theme, palette }));
// theme: "light" | "dark"; palette: {ground: "#f3f5f7", "lab-copper": "#0e6e7d",
//   "lab-p-dhcp": "oklch(from #0e6e7d l c 300)", ...}, keys with or without "--"
```

House tokens are read first, and the Lab's aliases are derived from them
the way lab.css does. Copper is the accent, fiber the warn hue, and the
hardware has its own dark arm. So a palette with only the house tokens still
draws right, and a `lab-*` key then overrides its alias. `lab-p-<proto>`
colours a packet by its protocol, and `faint` colours one not named.
Colours may be `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `oklch()`
or the relative `oklch(from <colour> l c H)` that lab.css uses. Other values
are ignored. `{dark, colors}` is accepted too. Without any `set_theme` the
canvas uses tokens.css's light values, compiled in. The switch is live, and
the clear colour follows `ground`.

## Lifecycle

- One app per page. `attach` starts Bevy on the first call. On wasm
  `App::run` hands the app to the browser through winit's `spawn_app` and
  returns at once, and every frame after that is a `requestAnimationFrame`
  callback. A wasm event loop cannot be stopped, so the app lives as long
  as the page.
- `detach` stops drawing and drops the callbacks, and the next `attach`
  resumes. React's StrictMode runs `attach`, `detach`, `attach`, which is
  safe.
- When `attach` finds a different element than the first time, it puts the
  original canvas back in the new one's place, with its WebGL context and
  listeners, and copies the new element's `class`. Keep one id for it.
- `attach` with another `Lab` switches to it, and the old one can be freed.
- Bevy fits the canvas to its parent (`fit_canvas_to_parent`) and follows
  the parent's size. Give the parent a size and `position: relative`.

### Loading

The admin UI's loader is `admin-ui/app/src/lab/render.ts`, and
`demo/loader.js` is the same pattern for the demo:

- The backend is chosen before anything heavy is fetched. It is WebGPU when
  `navigator.gpu` exists and `requestAdapter()` returns an adapter within
  2 s, else WebGL2 when a scratch canvas gives a WebGL2 context, else none,
  and the page keeps its SVG canvas.
- The loader imports the glue and the module and calls `init()`. It
  swallows only an error whose message starts with "Using exceptions for
  control flow", which some winit versions throw on purpose, and rethrows
  anything else.
- `?canvas=svg|webgpu|webgl2` in the admin page, and `?backend=` in the
  demo, choose the backend. The admin page also reads the local storage key
  `losos-lab-canvas`.

Each module is large (see "Sizes"), so the page shows the SVG canvas first
and swaps when the `frame` event arrives. If no frame arrives within 20 s
the page stays on SVG. Both loaders log which backend runs, as
`LosOS Lab canvas: …` lines, with the adapter Bevy got
(`stats().adapter`).

An adapter that `requestAdapter()` returns can still fail once Bevy uses
it. Headless Chromium with SwiftShader and its GL compositor loses the
device on the first frame, for example. The module then sends
`render-error` once, logs it, and stops drawing instead of quitting the
app. A canvas that had a WebGPU context cannot take a WebGL2 one, and a
module cannot be unloaded. The admin page therefore goes back to its SVG
canvas, which keeps working on the same `Lab`, and remembers the failure in
local storage so the next visit picks WebGL2. The demo reloads instead. A
panic is printed to the console too, since the module has no log
subscriber.

## CSP

The module runs under the admin page's policy with no violations:

```
default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self';
img-src 'self' data: blob:; font-src 'self'; connect-src 'self';
worker-src 'self' blob:
```

- `'wasm-unsafe-eval'` compiles the module. Nothing uses `eval`.
- winit and Bevy write canvas styles through the CSSOM
  (`style.setProperty`), which `style-src 'self'` allows. Nothing sets a
  `style` attribute or adds a `<style>` element. The canvas's own listeners
  set `touch-action`, `user-select` and `outline` the same way.
- winit's timer runs in a worker made from a `blob:` URL when it needs one.
- WebGPU needs nothing more from the policy. The adapter, device and shader
  modules are API calls, not fetches.
- Nothing is fetched besides the module. Bevy embeds its shaders, the fonts
  are compiled in, and the asset server never asks for `.meta` files
  (`AssetMetaCheck::Never`).

## Fonts

`assets/fonts/IBMPlexSans-SemiBold.ttf` and `IBMPlexMono-Regular.ttf` are the
Lab's own `--f-ui` and `--f-mono`, unmodified from IBM's releases
(`@ibm/plex-sans` 1.1.0 and `@ibm/plex-mono` 2.5.0), under the SIL Open
Font License 1.1, text in `assets/fonts/OFL.txt`. REUSE.toml annotates the
folder. They are not subset, because OFL reserves the name "Plex" for
modified versions.

## Sizes

Release build with `opt-level = "z"`, LTO, one codegen unit,
`panic = "abort"` and stripped symbols, then wasm-bindgen 0.2.127 and
`wasm-opt -Oz` from binaryen 132. Sizes of `losos_lab_render_bg.wasm` after
wasm-opt, in bytes divided by 10^6:

| Build | Raw | gzip -9 | brotli -q 11 |
|---|---|---|---|
| `webgl2` | 16.62 MB | 5.28 MB | 3.61 MB |
| `webgpu` | 15.81 MB | 4.95 MB | 3.38 MB |
| `losos_lab_render.js`, `webgl2` | 149 KB | 19.3 KB | 16.3 KB |
| `losos_lab_render.js`, `webgpu` | 150 KB | 21.7 KB | 18.3 KB |

A page loads one module, never both. The `webgpu` build is smaller because
it carries no WGSL to GLSL translation. The 3D parts, `bevy_pbr` for
`StandardMaterial` and `mesh_picking` for the ray cast, account for
3.37 MB raw, 0.98 MB gzip and 0.62 MB brotli of the `webgl2` build.

`opt-level = "s"` gave a larger module in all three columns and no faster
frames, so the crate stays on `"z"`. The core alone (`losos-lab-core`) is 384 KB raw,
138 KB gzip and 116 KB brotli. The two fonts are about 375 KB. Most of the
rest is Bevy's ECS, reflection, the renderer, the PBR material, naga and
the text stack.

Bevy is built with only these features: `std`, `bevy_winit`,
`bevy_window`, `bevy_render`, `bevy_core_pipeline`, `bevy_pbr`,
`mesh_picking`, `bevy_sprite`, `bevy_sprite_render` for the screen-space
labels, `bevy_text`, `bevy_asset`, `bevy_mesh`, `bevy_color`,
`bevy_camera`, `bevy_shader`, `bevy_image`, and one of `webgl2` or
`webgpu`. There is no audio, no gizmos, no UI, no light or shadow in use
and no log subscriber.

## Measuring

`stats()` returns `{frames, update: {mean, p95}, interval: {mean, p95},
vertices, labels, entities, rebuilds, firstFrameAt, backend, adapter,
renderError}`. That is Bevy's update time and the frame interval in ms over
the last 240 frames, the scene's vertex count, the labels, the part
entities with their pick meshes, how often the scene was rebuilt, the
`performance.now()` of the first frame, the module's backend, and
`{backend, name}` of the adapter wgpu got, `null` until the renderer is up.

## How it is drawn

A `Camera3d` hangs over the ground plane and looks straight down through an
orthographic projection (`OrthographicProjection::default_3d`, as in Bevy's
`projection_zoom` example). Canvas (x, y) is world (x, 0, z = y) with Y up,
and one world unit is one CSS pixel at zoom 1. Every room, Wi-Fi range,
bus, cable and device is a `Mesh3d` entity of its own, its triangles in its
own coordinates around its origin. One unlit `StandardMaterial` with vertex
colours draws them all, alpha-blended and without tone mapping, so the
theme's colours land exactly. Overlap is height. Each kind of part has a
band ten units above the last, and within a band later parts sit a
hundredth higher, in the SVG's document order. A dragged device moves its
entity and leaves its mesh alone. Packets are entities too, over one mesh
per colour, moved every frame. The grid and the rubber band are one mesh
each.

Picking casts a ray from the camera through the pointer with
`MeshRayCast`, as Bevy's `mesh_ray_cast` example does. The ray is tested
against hidden pick meshes, one per hit region, as children of the part
they belong to. A device's box, a cable's 7-unit band, a badge and a power
LED each have one. The nearest hit wins, and later regions sit a little
nearer. Pointer events come from the canvas's own DOM listeners, are
queued, and run through `interact.rs` in a Bevy system at the start of the
next frame, where the ray cast can see the scene. Bevy's own picking
plugins are off.

Labels are screen-space `Text2d` under a second, 2D camera drawn over the
scene. Each frame they are placed where the scene camera projects their
anchor (`Camera::world_to_viewport`). While the zoom moves, glyphs are
rasterised at quarter steps of a power of two and scaled the rest of the
way. Once the zoom has rested for 150 ms, they are rasterised at exactly
that zoom and placed on whole device pixels.

## Towards 3D

In place:

- The scene is 3D: a `Camera3d`, `Mesh3d` entities on the XZ plane,
  `StandardMaterial`, and picking by ray cast. The ray cast and the label
  placement follow any camera, because both go through `Camera`'s own
  projection.
- `set_pitch` tilts the camera already. The scene, picking and labels
  follow it, but pan, zoom about the pointer and fit still assume the
  camera looks straight down.
- Device geometry is generated in code per part (`scene::build`), in the
  part's own coordinates, and each device entity carries
  `DeviceModel(type)`, the key a per-type model would replace it by.
- Overlap is height, so a tilted camera sees a stack of thin slabs in the
  same order as the flat view.

The next three things:

1. An orbit camera for the physical view, with a perspective projection.
   Bevy's `camera/camera_orbit` example is the pattern to start from.
   `bevy_panorbit_camera` 0.35, which is built for Bevy 0.19, is the crate
   to take if the controls need more than the example. `ui.to_world` then becomes the pointer ray's intersection
   with the ground plane, so placing and dragging work under any camera.
2. A light and lit materials. The material is unlit only because the flat
   views want exact colours.
3. glTF models per device type, loaded from embedded bytes because the CSP
   allows no fetches, swapped in by `DeviceModel`. Pick meshes become the
   models' bounds, cables get a tube along `cubic_pts`, and packets a small
   mesh.

## Known gaps

- Touch goes through the same pointer events, pinch included, and has not
  been tried on a touch screen.
- The browser checks run WebGL2 on SwiftShader in headless Chromium. WebGPU
  has run on SwiftShader through Vulkan, slowly. Neither has been checked on
  a hardware GPU.
- Labels use one sans weight, SemiBold. The SVG canvas's room subtitles and
  notes are Regular, so they read a little heavier here.
- Bevy blends in linear colour, and the browser blends SVG in sRGB. So dark
  text on the light theme reads thinner than on the SVG canvas, and light
  text and the grid on the dark theme read heavier. Text under about 8 px,
  such as the physical view's notes at its fitted zoom, is noticeably
  fainter than the browser's hinted text.
- The `context` event has no menu in the React Lab yet, as on the SVG
  canvas.
- A tilted camera (`set_pitch` below 90) is for looking only, as "Towards
  3D" says.

## Tests and lints

- Native, with no GPU and no Bevy build: `cargo test` and `cargo clippy
  --all-targets -- -D warnings`. Bevy is a wasm32-only dependency, so this
  checks the drawing, hit testing, pointer handling and tunables against
  the real core.
- wasm32, once per backend: `cargo clippy --target wasm32-unknown-unknown
  --all-targets -- -D warnings`, and the same with `--no-default-features
  --features webgpu`. These cover `src/app/` and `web.rs` too, need no
  system libraries, and take about 2 GB of disk each. devenv's `lint-wasm`
  and CI's `clippy-wasm` job run both, one after the other.
- `admin-ui/app/tests/lab.browser.mjs` loads the page under the Lab's own
  policy and checks that the SVG canvas draws first. On WebGL2 it then
  checks that the setup, the selection and the camera survive the swap,
  that a click on the Bevy canvas picks a device, and that Delete removes
  it.
