use crate::components::node::TickProgressFill;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::systems::node_system::TICK_BAR_WIDTH;
use bevy::prelude::*;

pub fn update_tick_progress(
    gd: Res<GraphDefinitionRes>,
    mut bars: Query<(&TickProgressFill, &mut Sprite, &mut Transform)>,
) {
    for (bar, mut sprite, mut transform) in bars.iter_mut() {
        let Some(node) = gd
            .graph_defn
            .node_instances
            .iter()
            .find(|n| n.name == bar.node_name)
        else {
            continue;
        };

        let remaining = node.timer.fraction_remaining().clamp(0.0, 1.0);
        let width = TICK_BAR_WIDTH * remaining;
        sprite.custom_size = Some(Vec2::new(width, sprite.custom_size.unwrap_or_default().y));
        transform.translation.x = -TICK_BAR_WIDTH / 2.0 + width / 2.0;
    }
}
