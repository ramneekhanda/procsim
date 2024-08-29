use bevy::prelude::*;
use crate::{parser::graphv2::{parse_graph2, File}, resources::graph_def::GraphDefinitionRes, ui::CodeStorage};

pub fn clear_color(
    mut graph_defn_r: ResMut<GraphDefinitionRes>,
    mut clear_color: ResMut<ClearColor>,
) {
    if graph_defn_r.is_changed() {
        let bgcolor = graph_defn_r.graph_defn.graph_attrs.background;
        clear_color.0 = bgcolor;
    }
}
