use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use crate::components::node::Node;

pub fn drag(e: Listener<Pointer<Drag>>,
        mut q: Query<(&mut Transform, &mut Children, &Node)>) {

  // for (mut transform, children, _) in q.iter_mut() {
  //   for child in children.iter() {
  //     if *child == e.target {
  //       transform.translation.x += e.event.delta.x;
  //       transform.translation.y -= e.event.delta.y;
  //       return;
  //     }
  //   }
  // }
}
