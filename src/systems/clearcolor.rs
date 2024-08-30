use crate::resources::graph_def::GraphDefinitionRes;
use bevy::prelude::*;

pub fn clear_color(graph_defn_r: ResMut<GraphDefinitionRes>, mut clear_color: ResMut<ClearColor>) {
    if graph_defn_r.is_changed() {
        let bgcolor = graph_defn_r.graph_defn.graph_attrs.background;
        clear_color.0 = bgcolor;
    }
}
