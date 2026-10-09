//! Text. Labels stay in screen space: a `Text2d` each under the label
//! camera, placed every frame where the scene camera projects its anchor,
//! so names stay upright and sharp whatever the scene camera does.
//!
//! Glyphs are rasterised at the label's size times the zoom. While the
//! zoom moves, that zoom is rounded to a quarter step of a power of two and
//! the text is scaled the rest of the way, so a wheel turn re-rasterises a
//! few sizes rather than one per frame; once the zoom has stayed put for
//! `Tunables::label_settle_ms` the labels are drawn at exactly that zoom,
//! unscaled and on whole device pixels.

use super::camera::SceneCam;
use super::scene::NewLabels;
use super::{lin, Step};
use crate::scene::{Face, Label, LABEL_Y};
use crate::tunables::Tunables;
use crate::web::{now, with_page};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera::CameraUpdateSystems;
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSize, FontSource};
use bevy::transform::TransformSystems;
use std::collections::{HashMap, HashSet};

const SANS: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf");
const MONO: &[u8] = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");

pub(super) struct LabelsPlugin;

impl Plugin for LabelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Fonts>()
            .init_resource::<Labels>()
            .add_systems(Update, sync_labels.in_set(Step::Labels))
            .add_systems(
                PostUpdate,
                place_labels
                    .after(TransformSystems::Propagate)
                    .after(CameraUpdateSystems),
            );
    }
}

/// The two faces, compiled in: nothing is fetched.
#[derive(Resource)]
struct Fonts {
    sans: Handle<Font>,
    mono: Handle<Font>,
}

impl FromWorld for Fonts {
    fn from_world(world: &mut World) -> Self {
        let mut fonts = world.resource_mut::<Assets<Font>>();
        Fonts {
            sans: fonts.add(Font::from_bytes(SANS.to_vec())),
            mono: fonts.add(Font::from_bytes(MONO.to_vec())),
        }
    }
}

#[derive(Component)]
struct LabelText;

#[derive(Resource, Default)]
struct Labels {
    on_screen: HashMap<String, (Entity, Label)>,
    /// The zoom the glyphs were last rasterised for.
    kq: f32,
    /// The zoom last seen, and since when.
    k_seen: f32,
    k_seen_at: f64,
}

/// Quarter steps of a power of two.
fn quant(k: f32) -> f32 {
    2f32.powf((k.log2() * 4.0).round() / 4.0)
}

fn sync_labels(
    mut commands: Commands,
    mut state: ResMut<Labels>,
    mut new: ResMut<NewLabels>,
    fonts: Res<Fonts>,
    tune: Res<Tunables>,
    mut texts: Query<(&mut Text2d, &mut TextFont, &mut TextColor, &mut Anchor), With<LabelText>>,
) {
    let state = &mut *state;
    let t = now();
    let k = with_page(|p| p.ui.cam().k);
    if k != state.k_seen {
        state.k_seen = k;
        state.k_seen_at = t;
    }
    let kq = if t - state.k_seen_at >= tune.label_settle_ms {
        k
    } else {
        quant(k)
    };
    let zoomed = kq != state.kq;
    state.kq = kq;
    let font = |f: Face| -> FontSource {
        match f {
            Face::Sans => fonts.sans.clone().into(),
            Face::Mono => fonts.mono.clone().into(),
        }
    };
    let components = |l: &Label| {
        (
            TextFont {
                font: font(l.face),
                font_size: FontSize::Px(l.size * kq),
                ..default()
            },
            TextColor(lin(l.color)),
            Anchor(Vec2::new(l.ax, 0.0)),
        )
    };
    let Some(list) = new.0.take() else {
        if zoomed {
            // Only the zoom moved: rasterise the glyphs at the new size.
            for (e, l) in state.on_screen.values() {
                if let Ok((_, mut tf, _, _)) = texts.get_mut(*e) {
                    tf.font_size = FontSize::Px(l.size * kq);
                }
            }
        }
        return;
    };
    let mut keep = HashSet::new();
    for l in list {
        keep.insert(l.key.clone());
        match state.on_screen.get_mut(&l.key) {
            Some((e, old)) => {
                if let Ok((mut text, mut tf, mut col, mut anchor)) = texts.get_mut(*e) {
                    if text.0 != l.text {
                        text.0 = l.text.clone();
                    }
                    let (f, c, a) = components(&l);
                    if tf.font_size != f.font_size || tf.font != f.font {
                        *tf = f;
                    }
                    if col.0 != c.0 {
                        *col = c;
                    }
                    if *anchor != a {
                        *anchor = a;
                    }
                }
                *old = l;
            }
            None => {
                let (f, c, a) = components(&l);
                let e = commands
                    .spawn((
                        Text2d::new(l.text.clone()),
                        f,
                        c,
                        a,
                        LabelText,
                        // Placed after culling has run (see `place_labels`):
                        // never cull them.
                        NoFrustumCulling,
                    ))
                    .id();
                state.on_screen.insert(l.key.clone(), (e, l));
            }
        }
    }
    state.on_screen.retain(|k, (e, _)| {
        if keep.contains(k) {
            true
        } else {
            commands.entity(*e).despawn();
            false
        }
    });
}

/// Puts each label where the scene camera projects its anchor this frame.
/// Runs after transforms and cameras are updated, so it writes the global
/// transform too. Unscaled text goes on whole device pixels: a glyph drawn
/// between two pixels is a blurred glyph.
#[allow(clippy::type_complexity)]
fn place_labels(
    camera: Single<(&Camera, &GlobalTransform), With<SceneCam>>,
    window: Single<&Window>,
    state: Res<Labels>,
    mut q: Query<(&mut Transform, &mut GlobalTransform), (With<LabelText>, Without<SceneCam>)>,
) {
    if state.on_screen.is_empty() {
        return;
    }
    let (camera, eye) = *camera;
    let (w, h) = (window.resolution.width(), window.resolution.height());
    let dpr = window.resolution.scale_factor();
    let k = with_page(|p| p.ui.cam().k);
    let s = if state.kq > 0.0 { k / state.kq } else { 1.0 };
    let snap = |v: f32| {
        if s == 1.0 {
            (v * dpr).round() / dpr
        } else {
            v
        }
    };
    for (e, l) in state.on_screen.values() {
        let Ok((mut tr, mut gt)) = q.get_mut(*e) else {
            continue;
        };
        let Ok(v) = camera.world_to_viewport(eye, Vec3::new(l.x, LABEL_Y, l.y)) else {
            continue;
        };
        let want = Transform::from_xyz(snap(v.x) - w / 2.0, h / 2.0 - snap(v.y), 0.0)
            .with_scale(Vec3::splat(s));
        if *tr != want {
            *tr = want;
            *gt = GlobalTransform::from(want);
        }
    }
}
