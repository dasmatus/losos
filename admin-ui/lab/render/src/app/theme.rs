//! The page's palette on the canvas: the clear colour, and the logical
//! view's grid, one mesh redrawn when the theme changes and hidden on the
//! physical view.

use super::{layer_entity, lin, write_mesh, Step, Unlit};
use crate::paint::Painter;
use crate::scene::{self as geom, Layer, View};
use crate::theme::Theme;
use crate::web::with_page;
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;

pub(super) struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(lin(Theme::light().ground)))
            .add_systems(Startup, spawn_grid)
            .add_systems(Update, sync_theme.in_set(Step::Theme));
    }
}

#[derive(Resource)]
struct Grid(Entity, Handle<Mesh>);

fn spawn_grid(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, unlit: Res<Unlit>) {
    let (e, h) = layer_entity(&mut commands, &mut meshes, &unlit.0, Layer::Grid);
    commands.insert_resource(Grid(e, h));
}

/// The theme generation last drawn; the page bumps it on `set_theme`.
fn sync_theme(
    mut drawn: Local<u64>,
    grid: Res<Grid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut clear: ResMut<ClearColor>,
    mut vis: Query<&mut Visibility>,
) {
    let (gen, theme, view) = with_page(|p| (p.theme_gen, p.theme.clone(), p.ui.view));
    if let Ok(mut v) = vis.get_mut(grid.0) {
        let want = if view == View::Logical {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    if gen == *drawn {
        return;
    }
    *drawn = gen;
    clear.0 = lin(theme.ground);
    let mut p = Painter::new();
    geom::grid(&theme, &mut p);
    if let Some(mut m) = meshes.get_mut(&grid.1) {
        write_mesh(&mut m, &p);
    }
}
