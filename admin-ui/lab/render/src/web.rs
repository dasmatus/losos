//! The page side: the `LabCanvas` JavaScript API, the DOM listeners on the
//! canvas, and the one state a page has. Everything runs on the page's main
//! thread; no borrow of the state or of the core is held while a JavaScript
//! callback runs, so a callback may call straight back into `Lab` or
//! `LabCanvas`.

use crate::interact::{self, Event, Mods, Pick, Pointer};
use crate::scene::{Cam, Catalog, Target, Tool, Ui, View};
use crate::theme::Theme;
use losos_lab_core::{Lab, LabCore};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, MouseEvent, PointerEvent, WheelEvent};

#[derive(Default)]
pub(crate) struct Stats {
    pub frames: u64,
    pub rebuilds: u64,
    pub update_ms: VecDeque<f64>,
    pub interval_ms: VecDeque<f64>,
    pub frame_start: f64,
    pub last_start: f64,
    pub vertices: usize,
    pub labels: usize,
    pub first_frame_at: Option<f64>,
    /// The adapter Bevy got: wgpu's backend name and the adapter's name.
    pub adapter: Option<(String, String)>,
    pub entities: usize,
    /// The render error that stopped drawing, if one did.
    pub render_error: Option<String>,
}

/// A DOM pointer event, queued for the next frame: picking is a ray cast
/// into the scene, which only a Bevy system can make.
pub(crate) enum Input {
    Down {
        pid: i32,
        x: f32,
        y: f32,
        shift: bool,
    },
    Move {
        pid: i32,
        x: f32,
        y: f32,
    },
    Up {
        pid: i32,
    },
    Leave,
    Context {
        x: f32,
        y: f32,
        cx: f32,
        cy: f32,
    },
}

pub(crate) struct Page {
    pub core: Option<Rc<RefCell<LabCore>>>,
    pub cat: Catalog,
    pub ui: Ui,
    pub pointer: Pointer,
    pub inputs: Vec<Input>,
    pub theme: Theme,
    pub theme_gen: u64,
    pub callbacks: HashMap<String, js_sys::Function>,
    pub attached: bool,
    pub started: bool,
    pub canvas: Option<HtmlCanvasElement>,
    /// Views to fit once the canvas has its real size.
    pub fit_pending: [bool; 2],
    /// Forces a rebuild of the static layer next frame.
    pub force: bool,
    pub known_links: Option<HashSet<String>>,
    pub known_devs: HashSet<String>,
    pub stats: Stats,
    /// The last camera and canvas size told to the page ("camera",
    /// "viewport"), so each is sent once per change.
    pub told_cam: Option<(View, Cam)>,
    pub told_size: (f32, f32),
    /// The page keeps the camera (`set_camera` was called): no fit of its
    /// own on attach or on a new setup, only when `fit` asks.
    pub host_camera: bool,
}

impl Default for Page {
    fn default() -> Self {
        Page {
            core: None,
            cat: Catalog::default(),
            ui: Ui::default(),
            pointer: Pointer::default(),
            inputs: Vec::new(),
            theme: Theme::light(),
            theme_gen: 1,
            callbacks: HashMap::new(),
            attached: false,
            started: false,
            canvas: None,
            fit_pending: [true, true],
            force: true,
            known_links: None,
            known_devs: HashSet::new(),
            stats: Stats::default(),
            told_cam: None,
            told_size: (0.0, 0.0),
            host_camera: false,
        }
    }
}

thread_local! {
    pub(crate) static PAGE: RefCell<Page> = RefCell::new(Page::default());
}

pub(crate) fn with_page<R>(f: impl FnOnce(&mut Page) -> R) -> R {
    PAGE.with(|p| f(&mut p.borrow_mut()))
}

pub(crate) fn now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

/// Calls the page's callbacks, with no borrow held.
pub(crate) fn dispatch(events: Vec<Event>) {
    for (name, payload) in events {
        let cb = with_page(|p| p.callbacks.get(name).cloned());
        if let Some(cb) = cb {
            let _ = cb.call1(&JsValue::NULL, &JsValue::from_str(&payload.to_string()));
        }
    }
}

/// Runs `f` with the page state and the core both borrowed, unless the
/// canvas is detached or the core is busy (a call into it from a callback
/// that is still on the stack): then the event is dropped.
fn with_core(f: impl FnOnce(&mut Page, &mut LabCore) -> Vec<Event>) -> Vec<Event> {
    with_page(|p| {
        if !p.attached {
            return vec![];
        }
        let Some(core) = p.core.clone() else {
            return vec![];
        };
        let Ok(mut c) = core.try_borrow_mut() else {
            return vec![];
        };
        f(p, &mut c)
    })
}

fn local(canvas: &HtmlCanvasElement, e: &MouseEvent) -> (f32, f32) {
    let r = canvas.get_bounding_client_rect();
    (
        (e.client_x() as f64 - r.left()) as f32,
        (e.client_y() as f64 - r.top()) as f32,
    )
}

fn listen<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(
    canvas: &HtmlCanvasElement,
    name: &str,
    passive: bool,
    f: impl FnMut(E) + 'static,
) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(passive);
    let _ = canvas.add_event_listener_with_callback_and_add_event_listener_options(
        name,
        cb.as_ref().unchecked_ref(),
        &opts,
    );
    // The canvas lives as long as the page: so does the listener.
    cb.forget();
}

fn install_listeners(canvas: &HtmlCanvasElement) {
    let style = canvas.style();
    // CSSOM writes, which `style-src 'self'` allows: no `style` attribute.
    let _ = style.set_property("touch-action", "none");
    let _ = style.set_property("user-select", "none");
    let _ = style.set_property("-webkit-user-select", "none");
    let _ = style.set_property("outline", "none");

    let c = canvas.clone();
    listen(canvas, "pointerdown", true, move |e: PointerEvent| {
        if e.pointer_type() == "mouse" && e.button() != 0 {
            return;
        }
        let _ = c.set_pointer_capture(e.pointer_id());
        let (x, y) = local(&c, &e);
        queue(Input::Down {
            pid: e.pointer_id(),
            x,
            y,
            shift: e.shift_key(),
        });
    });
    let c = canvas.clone();
    listen(canvas, "pointermove", true, move |e: PointerEvent| {
        let (x, y) = local(&c, &e);
        queue(Input::Move {
            pid: e.pointer_id(),
            x,
            y,
        });
    });
    for name in ["pointerup", "pointercancel"] {
        listen(canvas, name, true, move |e: PointerEvent| {
            queue(Input::Up {
                pid: e.pointer_id(),
            });
        });
    }
    listen(canvas, "pointerleave", true, move |_e: PointerEvent| {
        queue(Input::Leave);
    });
    let c = canvas.clone();
    listen(canvas, "wheel", false, move |e: WheelEvent| {
        e.prevent_default();
        let (x, y) = local(&c, &e);
        // Lines and pages to pixels, roughly as browsers scroll them.
        let dy = match e.delta_mode() {
            1 => e.delta_y() * 16.0,
            2 => e.delta_y() * 400.0,
            _ => e.delta_y(),
        } as f32;
        with_page(|p| {
            if p.attached {
                interact::wheel(&mut p.ui, x, y, dy);
            }
        });
    });
    let c = canvas.clone();
    listen(canvas, "contextmenu", false, move |e: MouseEvent| {
        e.prevent_default();
        let (x, y) = local(&c, &e);
        queue(Input::Context {
            x,
            y,
            cx: e.client_x() as f32,
            cy: e.client_y() as f32,
        });
    });
}

fn queue(i: Input) {
    with_page(|p| {
        if !p.attached {
            return;
        }
        // Moves only matter where they end: keep the last one per pointer.
        if let (Input::Move { pid, .. }, Some(Input::Move { pid: last, .. })) =
            (&i, p.inputs.last())
        {
            if pid == last {
                p.inputs.pop();
            }
        }
        p.inputs.push(i);
    });
}

/// Runs the queued pointer events through `interact`, with `pick` saying
/// what lies under a canvas point, and hands the results to the page.
pub(crate) fn pump(pick: &mut Pick) {
    let inputs = with_page(|p| std::mem::take(&mut p.inputs));
    for input in inputs {
        let ev = match input {
            Input::Down { pid, x, y, shift } => with_core(|p, core| {
                let Page {
                    ui, pointer, cat, ..
                } = p;
                pointer.down(core, ui, pick, &cat.ports, pid, x, y, Mods { shift })
            }),
            Input::Move { pid, x, y } => with_core(|p, core| {
                let Page { ui, pointer, .. } = p;
                pointer.moved(core, ui, pick, pid, x, y)
            }),
            Input::Up { pid } => with_core(|p, core| {
                let Page { ui, pointer, .. } = p;
                pointer.up(core, ui, pid)
            }),
            Input::Leave => with_page(|p| {
                if p.pointer.dragging() {
                    return vec![];
                }
                let Page { ui, pointer, .. } = p;
                pointer.leave(ui)
            }),
            Input::Context { x, y, cx, cy } => with_page(|p| {
                if !p.attached {
                    return vec![];
                }
                let Page { ui, pointer, .. } = p;
                pointer.context(ui, pick, x, y, cx, cy)
            }),
        };
        dispatch(ev);
    }
}

/// The canvas a page renders the Lab into. One per page: the Bevy app it
/// starts lives as long as the page does (a wasm event loop cannot be
/// stopped), so `attach` after `detach`, a second `attach`, or React's
/// StrictMode double mount all land on the same app.
#[wasm_bindgen]
pub struct LabCanvas {
    _priv: (),
}

fn parse_tool(s: &str) -> Tool {
    match s.split_once(':') {
        Some(("place", t)) => Tool::Place(t.to_string()),
        Some(("connect", k)) => Tool::Connect(k.to_string()),
        _ if s == "delete" => Tool::Delete,
        _ => Tool::Select,
    }
}

#[wasm_bindgen]
impl LabCanvas {
    /// Renders `lab` into the `<canvas>` that `selector` finds. The first
    /// call on a page starts Bevy on that element; later calls rebind the
    /// same app to `lab` and, when the selector now finds a different
    /// element (the component was remounted), move the original canvas into
    /// its place, WebGL context and all.
    pub fn attach(lab: &Lab, selector: &str) -> Result<LabCanvas, JsValue> {
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let el: HtmlCanvasElement = doc
            .query_selector(selector)?
            .ok_or_else(|| JsValue::from_str("no element matches the selector"))?
            .dyn_into()
            .map_err(|_| JsValue::from_str("the selector does not name a <canvas>"))?;
        let core = lab.shared();
        let cat = Catalog::from_core(&core.borrow());
        let start = with_page(|p| {
            p.core = Some(core);
            p.cat = cat;
            p.attached = true;
            p.force = true;
            if !p.host_camera {
                p.fit_pending = [true, true];
            }
            p.known_links = None;
            match &p.canvas {
                Some(old) if *old != el => {
                    if let Some(class) = el.get_attribute("class") {
                        let _ = old.set_attribute("class", &class);
                    }
                    let _ = el.replace_with_with_node_1(old);
                    false
                }
                Some(_) => false,
                None => {
                    p.canvas = Some(el.clone());
                    p.started = true;
                    true
                }
            }
        });
        if start {
            install_listeners(&el);
            crate::app::start(selector.to_string());
        }
        Ok(LabCanvas { _priv: () })
    }

    /// Stops drawing and drops the callbacks. The app idles until the next
    /// `attach`.
    pub fn detach(&self) {
        with_page(|p| {
            p.attached = false;
            p.callbacks.clear();
            p.pointer = Pointer::default();
            p.inputs.clear();
            p.ui.hover = None;
            p.ui.pointer = None;
        });
    }

    /// "logical" or "physical". Each view keeps its own camera.
    pub fn set_view(&self, view: &str) {
        with_page(|p| {
            p.ui.view = if view == "physical" {
                View::Physical
            } else {
                View::Logical
            };
            p.ui.connect_from = None;
        });
    }

    /// "select", "delete", "place:<type>" or "connect:<kind>". Any change
    /// drops a half-made connection.
    pub fn set_tool(&self, tool: &str) {
        with_page(|p| {
            p.ui.tool = parse_tool(tool);
            p.ui.connect_from = None;
        });
    }

    /// The first end of a half-made connection, as the page keeps it:
    /// `"d3"` (JSON) or `null`. Calling this hands the connection to the
    /// page for good: from then on a click with the connect tool sends a
    /// `connect` event (`{id, port, shift}`, `port` when the physical
    /// view's drawing shows which port was clicked) instead of
    /// `request-port-pick`, and the canvas only draws the rubber band from
    /// here. Call it after `set_tool` and `set_view`, which clear it.
    pub fn set_connect_from(&self, id: &str) {
        let v: Value = serde_json::from_str(id).unwrap_or(Value::Null);
        with_page(|p| {
            p.ui.host_connect = true;
            p.ui.connect_from = v.as_str().map(String::from);
        });
    }

    /// `{"kind": "device"|"link", "id": "…"}` or `null`.
    pub fn set_selection(&self, sel: &str) {
        let v: Value = serde_json::from_str(sel).unwrap_or(Value::Null);
        let id = v.get("id").and_then(Value::as_str).map(String::from);
        let t = match (v.get("kind").and_then(Value::as_str), id) {
            (Some("device"), Some(id)) | (Some("dev"), Some(id)) => Some(Target::Dev(id)),
            (Some("link"), Some(id)) => Some(Target::Link(id)),
            _ => None,
        };
        with_page(|p| p.ui.sel = t);
    }

    /// `{"dark": bool, "colors": {"--ground": "#…", …}}`: README.md lists
    /// the properties read.
    pub fn set_theme(&self, theme: &str) {
        let t = Theme::from_json(theme);
        with_page(|p| {
            if p.theme != t {
                p.theme = t;
                p.theme_gen += 1;
            }
        });
    }

    /// Device ids with a guest running (`["d3", …]`): the "VM" tag.
    pub fn set_running(&self, ids: &str) {
        let v: Vec<String> = serde_json::from_str(ids).unwrap_or_default();
        with_page(|p| p.ui.running = v.into_iter().collect());
    }

    /// Registers `cb(payloadJson)` for an event: "select", "hover",
    /// "tap", "request-port-pick", "connect", "place", "delete", "power", "move",
    /// "moved", "context", "camera", "viewport", "frame", "render-error". One callback per event; a
    /// second replaces the first.
    pub fn on(&self, event: &str, cb: js_sys::Function) {
        with_page(|p| {
            p.callbacks.insert(event.to_string(), cb);
        });
    }

    pub fn off(&self, event: &str) {
        with_page(|p| {
            p.callbacks.remove(event);
        });
    }

    /// Fits the current view to its devices (logical) or rooms (physical).
    pub fn fit(&self) {
        with_page(|p| {
            let v = p.ui.view as usize;
            p.fit_pending[v] = true;
        });
    }

    /// Zooms about the canvas centre, like the zoom buttons.
    pub fn zoom_by(&self, factor: f64) {
        with_page(|p| {
            let (w, h) = p.ui.size;
            let k2 = p.ui.tune.clamp_zoom(p.ui.cam().k * factor as f32);
            interact::zoom_to(&mut p.ui, w / 2.0, h / 2.0, k2);
        });
    }

    /// `{x, y, k}`: canvas = world × k + (x, y), in CSS pixels.
    pub fn camera(&self) -> String {
        with_page(|p| {
            let c = p.ui.cam();
            json!({ "x": c.x, "y": c.y, "k": c.k }).to_string()
        })
    }

    pub fn set_camera(&self, cam: &str) {
        let v: Value = serde_json::from_str(cam).unwrap_or(Value::Null);
        let g = |k: &str| v.get(k).and_then(Value::as_f64).map(|x| x as f32);
        if let (Some(x), Some(y), Some(k)) = (g("x"), g("y"), g("k")) {
            with_page(|p| {
                let c = Cam {
                    x,
                    y,
                    k: p.ui.tune.clamp_zoom(k),
                };
                *p.ui.cam_mut() = c;
                // The page knows: no echo.
                p.told_cam = Some((p.ui.view, c));
                p.host_camera = true;
                p.fit_pending = [false, false];
            });
        }
    }

    /// Antialiasing: 4 samples (the default) or 1, for a slow GPU or a
    /// software renderer, where four samples cost most of the frame.
    pub fn set_msaa(&self, samples: u32) {
        with_page(|p| p.ui.tune.set_msaa(samples));
    }

    /// Shows or hides every label. They are drawn by a second camera over
    /// the scene, so hiding them also saves that pass: an overview at a
    /// small zoom, or a slow GPU, may want it.
    pub fn set_labels(&self, on: bool) {
        with_page(|p| p.ui.tune.labels = on);
    }

    /// The camera's angle above the ground in degrees, 20 to 90 (the
    /// default, straight down). Below 90 the same scene is seen tilted;
    /// pan, zoom and fit still assume 90, so this is for looking, not yet
    /// for working.
    pub fn set_pitch(&self, deg: f32) {
        with_page(|p| p.ui.tune.set_pitch(deg));
    }

    /// Redraws the static layer next frame (the canvas notices world
    /// changes by itself; this is for anything it cannot see).
    pub fn redraw(&self) {
        with_page(|p| p.force = true);
    }

    /// "webgpu" or "webgl2": which build this module is.
    pub fn backend(&self) -> String {
        backend()
    }

    /// Frame statistics for tests and the about box: frames drawn, Bevy
    /// update time and frame interval (mean and 95th percentile over the
    /// last 240 frames, ms), vertices in the scene's meshes, labels, scene
    /// entities, rebuilds of the scene, when the first frame was drawn
    /// (`performance.now()`), and the adapter Bevy runs on (wgpu's backend
    /// and the adapter's name; null until the renderer is up).
    pub fn stats(&self) -> String {
        with_page(|p| {
            let s = &p.stats;
            let summary = |q: &VecDeque<f64>| {
                if q.is_empty() {
                    return json!(null);
                }
                let mut v: Vec<f64> = q.iter().copied().collect();
                v.sort_by(|a, b| a.total_cmp(b));
                let mean = v.iter().sum::<f64>() / v.len() as f64;
                let p95 = v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)];
                json!({ "mean": mean, "p95": p95 })
            };
            json!({
                "frames": s.frames,
                "update": summary(&s.update_ms),
                "interval": summary(&s.interval_ms),
                "vertices": s.vertices,
                "labels": s.labels,
                "entities": s.entities,
                "backend": backend(),
                "renderError": s.render_error,
                "adapter": s.adapter.as_ref().map(|(b, n)| json!({ "backend": b, "name": n })),
                "rebuilds": s.rebuilds,
                "firstFrameAt": s.first_frame_at,
            })
            .to_string()
        })
    }
}

/// Whether this browser can give the canvas a WebGL2 context. Asks on a
/// scratch canvas, so the page can choose the SVG canvas before it loads
/// anything heavy.
#[wasm_bindgen]
pub fn webgl2_supported() -> bool {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    let Ok(Ok(c)) = doc
        .create_element("canvas")
        .map(|e| e.dyn_into::<HtmlCanvasElement>())
    else {
        return false;
    };
    matches!(c.get_context("webgl2"), Ok(Some(_)))
}

/// "webgpu" or "webgl2": the backend this module was built for. Bevy picks
/// its WebGPU or WebGL2 code paths at compile time, so a page loads the
/// module that fits the browser (see demo/loader.js).
#[wasm_bindgen]
pub fn backend() -> String {
    if cfg!(feature = "webgpu") {
        "webgpu"
    } else {
        "webgl2"
    }
    .into()
}

/// The camera and canvas-size events: sent when either changed since the
/// page last heard (pan, zoom, fit, a resize), never for a `set_camera`.
pub(crate) fn tell_camera(size: (f32, f32)) {
    let ev = with_page(|p| {
        let mut ev = Vec::new();
        if !p.attached {
            return ev;
        }
        // Only once winit has sized the canvas to its parent (it starts at
        // 800×600 and follows a frame or two later).
        let sized = p.canvas.as_ref().is_none_or(|el| {
            (el.client_width() as f32 - size.0).abs() <= 1.0
                && (el.client_height() as f32 - size.1).abs() <= 1.0
        });
        if sized && p.told_size != size && size.0 > 1.0 && size.1 > 1.0 {
            p.told_size = size;
            ev.push(("viewport", json!({ "width": size.0, "height": size.1 })));
        }
        let now = (p.ui.view, p.ui.cam());
        if p.told_cam != Some(now) {
            p.told_cam = Some(now);
            let c = now.1;
            ev.push(("camera", json!({ "x": c.x, "y": c.y, "k": c.k })));
        }
        ev
    });
    dispatch(ev);
}
