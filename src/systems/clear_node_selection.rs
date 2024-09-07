use bevy_mod_picking::prelude::*;
use bevy::prelude::*;

use crate::components::node::SelectedNodeMarker;

pub fn clear_node_selection(
    mut q_selected: Query<(Entity, &SelectedNodeMarker)>,
    mut ev: EventReader<Pointer<Click>>,
    mut commands: Commands,
) {
    if ev.len() > 0 {
      ev.clear();
      for (entity, _) in q_selected.iter_mut() {
          commands.entity(entity).despawn_recursive();
      }
    }
}
