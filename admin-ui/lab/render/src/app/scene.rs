//! The scene's entities: one per part (`crate::scene::build`), the connect
//! tool's rubber band, and the packets. A frame rebuilds the parts only
//! when the scene's fingerprint changes, and then writes a mesh or a
//! transform only where it changed. The new labels and each part's hit
//! regions are handed on to `labels` and `picking`.

use super::{layer_entity, new_mesh, write_mesh, Step, Unlit};
use crate::paint::Painter;
use crate::scene::{self as geom, Hit, Label, Layer, View};
use crate::web::{now, with_page};
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

pub(super) struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Parts>()
            .init_resource::<Packets>()
            .init_resource::<NewLabels>()
            .add_systems(Startup, spawn_band)
            .add_systems(
                Update,
                (sync_scene, sync_band, sync_packets)
                    .chain()
                    .in_set(Step::Scene),
            );
    }
}

/// The device type a device part draws: where a per-type 3D model will
/// replace the generated geometry.
#[derive(Component)]
#[allow(dead_code)]
pub struct DeviceModel(pub String);

/// A part's hit regions, in canvas units, and where the part sits. Written
/// only when they change, so `picking` rebuilds its pick meshes on
/// `Changed<PartHits>`.
#[derive(Component)]
pub(super) struct PartHits {
    pub at: (f32, f32),
    pub hits: Vec<Hit>,
}

/// The labels of the last rebuild, for `labels` to take: `Some` only on a
/// frame that rebuilt the scene (or cleared it).
#[derive(Resource, Default)]
pub(super) struct NewLabels(pub Option<Vec<Label>>);

/// A part on screen: its entity, mesh, what was last written to the mesh,
/// and a hash of its hit regions.
struct PartEnt {
    entity: Entity,
    mesh: Handle<Mesh>,
    last: Painter,
    hits_sig: u64,
}

#[derive(Resource, Default)]
struct Parts {
    fp: u64,
    parts: HashMap<String, PartEnt>,
}

#[derive(Resource)]
struct Band {
    mesh: Handle<Mesh>,
    painter: Painter,
    was_empty: bool,
}

#[derive(Resource, Default)]
struct Packets {
    entities: Vec<Entity>,
    /// Envelope meshes by colour and "real" flag, for the current theme.
    envelopes: HashMap<([u32; 4], bool), Handle<Mesh>>,
    theme_gen: u64,
}

fn spawn_band(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, unlit: Res<Unlit>) {
    let (_, mesh) = layer_entity(&mut commands, &mut meshes, &unlit.0, Layer::Band);
    commands.insert_resource(Band {
        mesh,
        painter: Painter::new(),
        was_empty: false,
    });
}

fn hits_sig(hits: &[&Hit]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for hit in hits {
        format!("{:?}{:?}{}", hit.shape, hit.target, hit.title).hash(&mut h);
    }
    h.finish()
}

fn same_mesh(a: &Painter, b: &Painter) -> bool {
    a.idx == b.idx && a.pos == b.pos && a.col == b.col
}

fn sync_scene(
    mut commands: Commands,
    mut parts: ResMut<Parts>,
    mut labels: ResMut<NewLabels>,
    unlit: Res<Unlit>,
    mut meshes: ResMut<Assets<Mesh>>,
    window: Single<&Window>,
    mut transforms: Query<&mut Transform, With<PartHits>>,
) {
    let size = (window.resolution.width(), window.resolution.height());
    let t = now();
    let fp_drawn = parts.fp;
    let result = with_page(|p| {
        if !p.attached {
            return None;
        }
        let core = p.core.clone()?;
        p.ui.size = size;
        let v = p.ui.view as usize;
        // Placing devices on the physical canvas mutates them, as the SVG
        // canvas's physPos does.
        if p.ui.view == View::Physical {
            let missing: Vec<String> = core
                .borrow()
                .world()
                .devices
                .iter()
                .filter(|d| d.px.is_none() || d.py.is_none())
                .map(|d| d.id.clone())
                .collect();
            if !missing.is_empty() {
                let Ok(mut c) = core.try_borrow_mut() else {
                    return None;
                };
                for id in missing {
                    c.phys_pos(&id);
                }
            }
        }
        let c = core.try_borrow().ok()?;
        let w = c.world();
        // A whole new set of devices is a new setup: fit both views to it,
        // unless the page keeps the camera.
        let devs: HashSet<String> = w.devices.iter().map(|d| d.id.clone()).collect();
        if !devs.is_empty() && p.known_devs.is_disjoint(&devs) {
            if !p.host_camera {
                p.fit_pending = [true, true];
            }
            p.known_links = None;
        }
        p.known_devs = devs;
        // Fit only once winit has sized the canvas to its parent: the window
        // starts at 800×600 and follows the element a frame or two later.
        let sized = p.canvas.as_ref().is_none_or(|el| {
            (el.client_width() as f32 - size.0).abs() <= 1.0
                && (el.client_height() as f32 - size.1).abs() <= 1.0
        });
        if p.fit_pending[v] && sized && size.0 > 1.0 && size.1 > 1.0 {
            p.ui.cams[v] = geom::fit(&c, p.ui.view, size);
            p.fit_pending[v] = false;
        }
        // A cable that just appeared negotiates for 1.4 s (amber LEDs).
        let links: HashSet<String> = w.links.iter().map(|l| l.id.clone()).collect();
        if let Some(known) = &p.known_links {
            let new: Vec<&String> = links.difference(known).collect();
            if new.len() == 1 {
                p.ui.link_born.insert(new[0].clone(), t);
            }
        }
        p.ui.link_born.retain(|id, _| links.contains(id));
        p.known_links = Some(links);

        let fp = geom::fingerprint(&c, &p.ui, p.theme_gen, t);
        if fp == fp_drawn && !p.force {
            return Some(None);
        }
        p.force = false;
        let b = geom::build(&c, &p.cat, &p.ui, &p.theme, t);
        p.stats.rebuilds += 1;
        p.stats.vertices = b.parts.iter().map(|x| x.mesh.pos.len()).sum();
        p.stats.labels = b.labels.len();
        Some(Some((fp, b)))
    });
    match result {
        // Detached: nothing on screen but the clear colour.
        None => {
            if parts.fp != 0 || !parts.parts.is_empty() {
                parts.fp = 0;
                for (_, part) in parts.parts.drain() {
                    commands.entity(part.entity).despawn();
                }
                labels.0 = Some(Vec::new());
            }
        }
        Some(None) => {}
        Some(Some((fp, b))) => {
            parts.fp = fp;
            sync_parts(
                &mut commands,
                &mut parts,
                &unlit.0,
                &mut meshes,
                &mut transforms,
                b.parts,
                &b.hits,
            );
            labels.0 = Some(b.labels);
        }
    }
}

/// Brings the part entities in line with a fresh build: new parts are
/// spawned, gone ones despawned, a moved part only moves, and a mesh or
/// the hit regions are written only when their contents changed.
fn sync_parts(
    commands: &mut Commands,
    built: &mut Parts,
    material: &Handle<StandardMaterial>,
    meshes: &mut Assets<Mesh>,
    transforms: &mut Query<&mut Transform, With<PartHits>>,
    parts: Vec<geom::Part>,
    hits: &[Hit],
) {
    let mut keep = HashSet::new();
    for (i, part) in parts.into_iter().enumerate() {
        keep.insert(part.key.clone());
        let own: Vec<&Hit> = hits.iter().filter(|h| h.part == i).collect();
        let sig = hits_sig(&own);
        let place = Transform::from_xyz(part.at.0, part.y, part.at.1);
        let ent = match built.parts.get_mut(&part.key) {
            Some(ent) => {
                if !same_mesh(&ent.last, &part.mesh) {
                    if let Some(mut m) = meshes.get_mut(&ent.mesh) {
                        write_mesh(&mut m, &part.mesh);
                    }
                    ent.last = part.mesh;
                }
                if let Ok(mut tr) = transforms.get_mut(ent.entity) {
                    if *tr != place {
                        *tr = place;
                    }
                }
                ent
            }
            None => {
                let mesh = meshes.add(new_mesh(&part.mesh));
                let mut e = commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    place,
                ));
                if let Some(kind) = &part.model {
                    e.insert(DeviceModel(kind.clone()));
                }
                let entity = e.id();
                built.parts.entry(part.key.clone()).or_insert(PartEnt {
                    entity,
                    mesh,
                    last: part.mesh,
                    // Never equal to `sig`: the hits are written below.
                    hits_sig: !sig,
                })
            }
        };
        if ent.hits_sig != sig {
            ent.hits_sig = sig;
            commands.entity(ent.entity).insert(PartHits {
                at: part.at,
                hits: own.into_iter().cloned().collect(),
            });
        }
    }
    built.parts.retain(|k, ent| {
        if keep.contains(k) {
            true
        } else {
            // Takes its pick meshes with it.
            commands.entity(ent.entity).despawn();
            false
        }
    });
}

/// The connect tool's rubber band: one mesh, rewritten while it shows.
fn sync_band(mut band: ResMut<Band>, mut meshes: ResMut<Assets<Mesh>>) {
    let band = &mut *band;
    band.painter.clear();
    with_page(|p| {
        if !p.attached {
            return;
        }
        let Some(core) = p.core.clone() else {
            return;
        };
        let guard = core.try_borrow();
        if let Ok(c) = guard.as_ref() {
            geom::rubber_band(c, &p.ui, &p.theme, &mut band.painter);
        }
    });
    let empty = band.painter.idx.is_empty();
    if empty && band.was_empty {
        return;
    }
    band.was_empty = empty;
    if let Some(mut m) = meshes.get_mut(&band.mesh) {
        write_mesh(&mut m, &band.painter);
    }
}

/// Packets on the logical view: an entity each, moved every frame, over a
/// mesh shared by every packet of the same colour. The core says where
/// each one is (`packets_now`).
fn sync_packets(
    mut commands: Commands,
    mut packets: ResMut<Packets>,
    unlit: Res<Unlit>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<(&mut Transform, &mut Visibility, &mut Mesh3d), Without<PartHits>>,
) {
    let packets = &mut *packets;
    let (marks, theme, gen) = with_page(|p| {
        let none = (Vec::new(), p.theme.clone(), p.theme_gen);
        if !p.attached || p.ui.view != View::Logical {
            return none;
        }
        let Some(core) = p.core.clone() else {
            return none;
        };
        let guard = core.try_borrow();
        let Ok(c) = guard.as_ref() else {
            return none;
        };
        (
            geom::marks(&c.packets_now(), &p.theme),
            p.theme.clone(),
            p.theme_gen,
        )
    });
    if packets.theme_gen != gen {
        packets.envelopes.clear();
        packets.theme_gen = gen;
    }
    if packets.entities.len() < marks.len() {
        for _ in packets.entities.len()..marks.len() {
            let e = commands
                .spawn((
                    Mesh3d(Handle::default()),
                    MeshMaterial3d(unlit.0.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    bevy::camera::visibility::NoFrustumCulling,
                ))
                .id();
            packets.entities.push(e);
        }
        // Spawned this frame; placed from the next.
        return;
    }
    for (i, e) in packets.entities.iter().enumerate() {
        let Ok((mut tr, mut vis, mut mesh)) = q.get_mut(*e) else {
            continue;
        };
        let Some(m) = marks.get(i) else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let key = (m.color.map(f32::to_bits), m.real);
        let handle = packets
            .envelopes
            .entry(key)
            .or_insert_with(|| {
                let mut p = Painter::new();
                geom::envelope(&mut p, m.color, m.real, &theme);
                meshes.add(new_mesh(&p))
            })
            .clone();
        if mesh.0 != handle {
            mesh.0 = handle;
        }
        let want = Transform::from_xyz(m.x, Layer::Packet.base() + i as f32 * 0.01, m.y);
        if *tr != want {
            *tr = want;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
}
