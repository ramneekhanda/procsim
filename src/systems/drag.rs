use crate::components::node::{DragState, NodeMarker};
use crate::systems::background_grid::GRID_SPACING;
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;

fn snap(value: f32) -> f32 {
    (value / GRID_SPACING).round() * GRID_SPACING
}

pub fn drag(
    e: Listener<Pointer<Drag>>,
    mut q: Query<(&mut Transform, &mut DragState, &Children, &NodeMarker)>,
    mut query_camera: Query<&mut OrthographicProjection, With<Camera2d>>,
) {
    let projection = query_camera.single();
    if q.iter().count() == 0 {
        return;
    }
    for (mut transform, mut drag_state, children, _) in q.iter_mut() {
        for child in children.iter() {
            if *child == e.target {
                drag_state.raw.x += e.event.delta.x * projection.scale;
                drag_state.raw.y -= e.event.delta.y * projection.scale;
                transform.translation.x = snap(drag_state.raw.x);
                transform.translation.y = snap(drag_state.raw.y);
                return;
            }
        }
    }
}
