use crate::components::node::NodeMarker;
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;

pub fn drag(
    e: Listener<Pointer<Drag>>,
    mut q: Query<(&mut Transform, &mut Children, &NodeMarker)>,
    mut query_camera: Query<&mut OrthographicProjection, With<Camera2d>>,
) {
    let projection = query_camera.single();
    if q.iter().count() == 0 {
        return;
    }
    for (mut transform, children, _) in q.iter_mut() {
        for child in children.iter() {
            if *child == e.target {
                transform.translation.x += e.event.delta.x * projection.scale;
                transform.translation.y -= e.event.delta.y * projection.scale;
                return;
            }
        }
    }
}
