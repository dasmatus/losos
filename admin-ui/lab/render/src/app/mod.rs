//! The Bevy app: a 3D scene seen from straight above. Every room, Wi-Fi
//! range, cable and device is a `Mesh3d` entity of its own on the ground
//! plane (world XZ, Y up), drawn unlit with its vertex colours, so the two
//! views look as the SVG canvas does; tilting the camera is all it takes to
//! see the same entities in 3D.
//!
//! One plugin per concern, all added in `start`:
//! - `theme`: the clear colour and the logical view's grid.
//! - `camera`: the scene camera, the label camera over it, and the page's
//!   `Cam` and `Tunables` applied to both.
//! - `scene`: one entity per part of the scene, the connect tool's rubber
//!   band, and the packets.
//! - `labels`: text in screen space, `Text2d` under the label camera,
//!   placed each frame where the scene camera projects its anchor.
//! - `picking`: hidden pick meshes under each part's hit regions, and the
//!   ray cast from the camera through the pointer that the queued pointer
//!   events are resolved with.
//!
//! Everything runs in `Update`, in the order of `Step`, except the label
//! placement, which needs this frame's transforms (`PostUpdate`), and the
//! frame statistics (`First`, `Last`). Nothing steps at a fixed rate here:
//! the core's clock is driven by the page (`tick`), so there is no
//! `FixedUpdate` work.
//!
//! On wasm `App::run` hands the app to the browser's event loop
//! (`EventLoop::spawn_app`) and returns at once; from then on every frame
//! is a `requestAnimationFrame` callback.

mod camera;
mod labels;
mod picking;
mod scene;
mod theme;

use crate::paint::Painter;
use crate::scene::Layer;
use crate::tunables::Tunables;
use crate::web::{dispatch, now, with_page};
// Bevy's derives name `bevy_ecs`; with Bevy a target-specific dependency
// their manifest lookup does not see `bevy`, so name it for them.
use bevy::ecs as bevy_ecs;

use bevy::asset::{AssetMetaCheck, RenderAssetUsages};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::picking::input::PointerInputPlugin;
use bevy::picking::{InteractionPlugin, PickingPlugin as BevyPickingPlugin};
use bevy::prelude::*;
use bevy::render::error_handler::{RenderErrorHandler, RenderErrorPolicy};
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::WindowResolution;
use bevy::winit::WinitSettings;
use serde_json::json;

/// The scene camera's distance from the ground point it looks at.
const EYE_HEIGHT: f32 = 1000.0;

/// The order of a frame's work in `Update`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Step {
    /// The page's tunables, then the queued pointer events.
    Input,
    Theme,
    Scene,
    Picking,
    Labels,
    Camera,
}

/// The one material: white times the vertex colour, unlit, blended back to
/// front. Both faces, because the painter's fans wind either way.
#[derive(Resource)]
struct Unlit(Handle<StandardMaterial>);

impl FromWorld for Unlit {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Unlit(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        }))
    }
}

pub fn start(selector: String) {
    // With panic = "abort" and no log subscriber a panic would end the app
    // without a word; say what it was on the console.
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("LosOS Lab canvas: {info}").into());
    }));
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "LosOS Lab".into(),
                    canvas: Some(selector),
                    fit_canvas_to_parent: true,
                    // Keys and the browser's own gestures stay the page's:
                    // Esc and Delete come from React, and the canvas's own
                    // listeners cancel only the wheel and the context menu.
                    prevent_default_event_handling: false,
                    resolution: WindowResolution::new(800, 600),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                // Nothing is fetched: the shaders are embedded and the fonts
                // are compiled in. Never ask the server for .meta files.
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            // Only the ray cast is used; pointer events come from the
            // canvas's own listeners (src/web.rs).
            .disable::<PointerInputPlugin>()
            .disable::<BevyPickingPlugin>()
            .disable::<InteractionPlugin>(),
    )
    .insert_resource(WinitSettings::continuous())
    // A WebGPU validation error or a lost device: say so on the console and
    // to the page ("render-error"), and stop drawing instead of quitting
    // the app, so the page can fall back to its SVG canvas.
    .insert_resource(RenderErrorHandler(|e, _, _| {
        // Asked again every frame while the error stands: tell it once.
        let msg = format!("{:?}: {}", e.ty, e.description);
        let first = with_page(|p| p.stats.render_error.replace(msg.clone()).is_none());
        if !first {
            return RenderErrorPolicy::StopRendering;
        }
        web_sys::console::error_1(&format!("LosOS Lab canvas: render error {msg}").into());
        dispatch(vec![(
            "render-error",
            json!({ "type": format!("{:?}", e.ty), "description": e.description }),
        )]);
        RenderErrorPolicy::StopRendering
    }))
    .init_resource::<Unlit>()
    .insert_resource(with_page(|p| p.ui.tune))
    .configure_sets(
        Update,
        (
            Step::Input,
            Step::Theme,
            Step::Scene,
            Step::Picking,
            Step::Labels,
            Step::Camera,
        )
            .chain(),
    )
    .add_systems(Update, sync_tunables.in_set(Step::Input))
    .add_plugins((
        StatsPlugin,
        theme::ThemePlugin,
        camera::CameraPlugin,
        scene::ScenePlugin,
        picking::PickingPlugin,
        labels::LabelsPlugin,
    ));
    app.run();
}

/// The page's copy of the tunables, into the resource the systems read.
fn sync_tunables(mut tune: ResMut<Tunables>) {
    let now = with_page(|p| p.ui.tune);
    if *tune != now {
        *tune = now;
    }
}

/// Frame times, the first frame, and the adapter, for `LabCanvas.stats`.
struct StatsPlugin;

impl Plugin for StatsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(First, frame_start)
            .add_systems(Update, note_adapter)
            .add_systems(Last, frame_end);
    }
}

fn frame_start() {
    let t = now();
    with_page(|p| {
        let s = &mut p.stats;
        if s.last_start > 0.0 {
            s.interval_ms.push_back(t - s.last_start);
            if s.interval_ms.len() > 240 {
                s.interval_ms.pop_front();
            }
        }
        s.last_start = t;
        s.frame_start = t;
    });
}

fn frame_end() {
    let t = now();
    let first = with_page(|p| {
        let s = &mut p.stats;
        s.frames += 1;
        s.update_ms.push_back(t - s.frame_start);
        if s.update_ms.len() > 240 {
            s.update_ms.pop_front();
        }
        if s.first_frame_at.is_none() && s.frames > 1 {
            s.first_frame_at = Some(t);
            return true;
        }
        false
    });
    if first {
        dispatch(vec![("frame", json!({ "first": true, "at": t }))]);
    }
}

fn note_adapter(info: Option<Res<RenderAdapterInfo>>) {
    let Some(info) = info else {
        return;
    };
    with_page(|p| {
        if p.stats.adapter.is_none() {
            p.stats.adapter = Some((format!("{:?}", info.backend), info.name.clone()));
        }
    });
}

fn lin(c: [f32; 4]) -> Color {
    Color::LinearRgba(LinearRgba::from_f32_array(c))
}

fn new_mesh(p: &Painter) -> Mesh {
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    write_mesh(&mut m, p);
    m
}

fn write_mesh(m: &mut Mesh, p: &Painter) {
    let (pos, col, idx) = if p.idx.is_empty() {
        // One degenerate triangle: an empty vertex buffer is not worth the
        // edge cases.
        (vec![[0.0f32; 3]; 3], vec![[0.0f32; 4]; 3], vec![0, 1, 2])
    } else {
        (p.pos.clone(), p.col.clone(), p.idx.clone())
    };
    // Flat on the ground, facing up: what a light would need, if one is
    // ever added.
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; pos.len()]);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    m.insert_indices(Indices::U32(idx));
}

/// One mesh for a whole layer (the grid, the rubber band), rewritten in
/// place when it changes.
fn layer_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    layer: Layer,
) -> (Entity, Handle<Mesh>) {
    let h = meshes.add(new_mesh(&Painter::new()));
    let e = commands
        .spawn((
            Mesh3d(h.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(0.0, layer.base(), 0.0),
            // These change size from frame to frame; their bounds would lag.
            NoFrustumCulling,
        ))
        .id();
    (e, h)
}
