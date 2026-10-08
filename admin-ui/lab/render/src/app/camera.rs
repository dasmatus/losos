//! The two cameras. The scene camera is a `Camera3d` with an orthographic
//! projection (`OrthographicProjection::default_3d`, as in Bevy's
//! `projection_zoom` example) hanging over the ground; the label camera is
//! a `Camera2d` drawn after it into the same target, with no clear.
//!
//! The page's `Cam` maps world to canvas: canvas = world × k + (x, y), in
//! CSS pixels with y down. The scene camera looks at the ground point under
//! the canvas centre with -Z as up, so canvas y runs along +Z, and its
//! orthographic scale is 1/k, which gives one CSS pixel per world unit at
//! k = 1. Pan and zoom happen in `interact` on `Cam`; this only applies it.

use super::{Step, EYE_HEIGHT};
use crate::tunables::Tunables;
use crate::web::with_page;
use bevy::camera::{ClearColorConfig, ScalingMode};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;

pub(super) struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_cameras)
            .add_systems(Update, sync_camera.in_set(Step::Camera));
    }
}

/// The camera the scene is seen through.
#[derive(Component)]
pub(super) struct SceneCam;

/// The 2D camera the labels are drawn by, over the scene.
#[derive(Component)]
pub(super) struct LabelCam;

fn spawn_cameras(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 0,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            near: 0.0,
            far: 2.0 * EYE_HEIGHT,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, EYE_HEIGHT, 0.0).looking_to(Vec3::NEG_Y, Vec3::NEG_Z),
        // The theme's colours, exactly: no tone curve, no dither.
        Tonemapping::None,
        DebandDither::Disabled,
        SceneCam,
    ));
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Tonemapping::None,
        DebandDither::Disabled,
        LabelCam,
    ));
}

/// Where the scene camera hangs for a ground point and a pitch: `EYE_HEIGHT`
/// away from the point, `pitch` degrees above the ground, on the canvas's
/// bottom side, so a lower pitch tips the far side of the scene away.
fn eye(ground: Vec3, pitch_deg: f32) -> Transform {
    let a = pitch_deg.to_radians();
    if (pitch_deg - 90.0).abs() < f32::EPSILON {
        return Transform::from_translation(ground + Vec3::Y * EYE_HEIGHT)
            .looking_to(Vec3::NEG_Y, Vec3::NEG_Z);
    }
    let at = ground + Vec3::new(0.0, a.sin(), a.cos()) * EYE_HEIGHT;
    Transform::from_translation(at).looking_at(ground, Vec3::NEG_Z)
}

#[allow(clippy::type_complexity)]
fn sync_camera(
    window: Single<&Window>,
    tune: Res<Tunables>,
    camera: Single<(&mut Transform, &mut Projection, &mut Msaa), With<SceneCam>>,
    labels: Single<(&mut Msaa, &mut Camera), (With<LabelCam>, Without<SceneCam>)>,
) {
    let (w, h) = (window.resolution.width(), window.resolution.height());
    crate::web::tell_camera((w, h));
    let c = with_page(|p| p.ui.cam());
    let (mut label_msaa, mut label_cam) = labels.into_inner();
    // No labels, no second pass over the canvas.
    if label_cam.is_active != tune.labels {
        label_cam.is_active = tune.labels;
    }
    let (mut tr, mut proj, mut msaa) = camera.into_inner();
    let want = if tune.msaa >= 4 {
        Msaa::Sample4
    } else {
        Msaa::Off
    };
    // Both cameras draw into one target: they must agree.
    if *msaa != want {
        *msaa = want;
    }
    if *label_msaa != want {
        *label_msaa = want;
    }
    let ground = Vec3::new((w / 2.0 - c.x) / c.k, 0.0, (h / 2.0 - c.y) / c.k);
    let place = eye(ground, tune.pitch_deg);
    if *tr != place {
        *tr = place;
    }
    if let Projection::Orthographic(o) = &mut *proj {
        let s = 1.0 / c.k;
        if o.scale != s {
            o.scale = s;
        }
    }
}
