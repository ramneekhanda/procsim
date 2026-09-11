use crate::components::node::NodeMarker;
use crate::resources::graph_def::NodeTicked;
use bevy::prelude::*;
use bevy_tweening::{lens::TransformScaleLens, Animator, EaseFunction, Tween};
use std::time::Duration;

const PULSE_SCALE: f32 = 1.3;
const PULSE_HALF_DURATION_MS: u64 = 150;

pub fn pulse_on_tick(
    mut commands: Commands,
    mut ticked_reader: EventReader<NodeTicked>,
    query: Query<(Entity, &NodeMarker)>,
) {
    for ticked in ticked_reader.read() {
        for (entity, marker) in query.iter() {
            if marker.node_name != ticked.name {
                continue;
            }

            let scale_up = Tween::new(
                EaseFunction::QuadraticOut,
                Duration::from_millis(PULSE_HALF_DURATION_MS),
                TransformScaleLens {
                    start: Vec3::ONE,
                    end: Vec3::splat(PULSE_SCALE),
                },
            );
            let scale_down = Tween::new(
                EaseFunction::QuadraticIn,
                Duration::from_millis(PULSE_HALF_DURATION_MS),
                TransformScaleLens {
                    start: Vec3::splat(PULSE_SCALE),
                    end: Vec3::ONE,
                },
            );

            // `try_insert`, not `insert`: a node can `despawn()` itself on the very
            // tick that fires this `NodeTicked` (its last), and whether that
            // despawn's command or this insert's command lands first when the
            // schedule flushes them isn't guaranteed - `insert` on an
            // already-gone entity panics (bevy error B0003), `try_insert` no-ops.
            commands
                .entity(entity)
                .try_insert(Animator::new(scale_up.then(scale_down)));
        }
    }
}
