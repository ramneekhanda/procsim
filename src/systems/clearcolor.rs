use bevy::prelude::*;
use crate::{parser::graphv2::{parse_graph2, File}, resources::{common_assets::{LoadingState, LoadingStateOpt}, graph_def::GraphDefinitionRes}, ui::CodeStorage};

pub fn clear_color(
    mut graph_defn_r: ResMut<GraphDefinitionRes>,
    mut clear_color: ResMut<ClearColor>,
) {
    if graph_defn_r.is_changed() {
        match &graph_defn_r.graph_defn.graph_attrs {
            Some(ga) => {
                ga.background.is_some_and(|bgcolor| {
                    if bgcolor.len() < 3 { return false; }
                    clear_color.0 = Color::srgb(bgcolor[0], bgcolor[1], bgcolor[2]);
                    true
                });
            },
            None => {

            }
        }

    }
}
