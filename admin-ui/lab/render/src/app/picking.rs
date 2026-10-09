//! What lies under the pointer. Each part's hit regions become hidden pick
//! meshes, children of the part, and the queued pointer events are resolved
//! by casting a ray from the scene camera through the pointer against them,
//! with `MeshRayCast` as in Bevy's `mesh_ray_cast` example. Bevy's picking
//! plugins stay off: the canvas listens to the DOM itself (src/web.rs), and
//! a hidden mesh is not something Bevy's pointer picking would report.

use super::camera::SceneCam;
use super::scene::PartHits;
use super::{new_mesh, Step};
use crate::paint::Painter;
use crate::scene::HitTarget;
use crate::web::with_page;
use bevy::ecs as bevy_ecs;
use bevy::picking::mesh_picking::ray_cast::{
    MeshRayCast, MeshRayCastSettings, RayCastBackfaces, RayCastVisibility,
};
use bevy::prelude::*;

pub(super) struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, pump_input.in_set(Step::Input))
            .add_systems(Update, sync_pick_meshes.in_set(Step::Picking));
    }
}

/// A pick mesh: what a ray that hits it has hit, and the tooltip.
#[derive(Component)]
struct PickTarget(HitTarget, String);

/// A part's pick meshes, to replace when its hit regions change.
#[derive(Component)]
struct PickMeshes(Vec<Entity>);

/// The queued pointer events, with picking as a ray cast from the scene
/// camera through the pointer. The camera's transform is the one the last
/// frame was drawn with, which is what the pointer was aimed at.
fn pump_input(
    mut ray_cast: MeshRayCast,
    camera: Single<(&Camera, &GlobalTransform), With<SceneCam>>,
    picks: Query<&PickTarget>,
) {
    let (camera, eye) = *camera;
    let filter = |e: Entity| picks.contains(e);
    let settings = MeshRayCastSettings::default()
        .with_filter(&filter)
        // The pick meshes are hidden: they are hit regions, not drawings.
        .with_visibility(RayCastVisibility::Any);
    let mut pick = |sx: f32, sy: f32| {
        let ray = camera.viewport_to_world(eye, Vec2::new(sx, sy)).ok()?;
        let (e, _) = ray_cast.cast_ray(ray, &settings).first()?;
        picks.get(*e).ok().map(|p| (p.0.clone(), p.1.clone()))
    };
    crate::web::pump(&mut pick);
}

/// New pick meshes for every part whose hit regions changed. Later hits
/// win: each sits a little nearer the camera than the one before.
fn sync_pick_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Query<(Entity, Ref<PartHits>, Option<&mut PickMeshes>)>,
) {
    // One query for the changed parts and the count: a second query reading
    // `PickMeshes` would conflict with this one's write.
    let mut entities = 0;
    for (part, hits, old) in &mut parts {
        entities += 1 + old.as_ref().map_or(0, |m| m.0.len());
        if !hits.is_changed() {
            continue;
        }
        let mut made = Vec::with_capacity(hits.hits.len());
        for (j, h) in hits.hits.iter().enumerate() {
            let mut p = Painter::new();
            h.shape.offset(-hits.at.0, -hits.at.1).paint(&mut p);
            let proxy = commands
                .spawn((
                    Mesh3d(meshes.add(new_mesh(&p))),
                    Transform::from_xyz(0.0, 0.001 * (j + 1) as f32, 0.0),
                    Visibility::Hidden,
                    PickTarget(h.target.clone(), h.title.clone()),
                    RayCastBackfaces,
                    ChildOf(part),
                ))
                .id();
            made.push(proxy);
        }
        match old {
            Some(mut old) => {
                for e in std::mem::replace(&mut old.0, made) {
                    commands.entity(e).despawn();
                }
            }
            None => {
                commands.entity(part).insert(PickMeshes(made));
            }
        }
    }
    with_page(|p| p.stats.entities = entities);
}
