use crate::parser::graphv2::GraphDefinition;
use bevy::prelude::*;

#[derive(Resource, Default, Debug)]
pub struct GraphDefinitionRes {
    pub graph_defn: GraphDefinition,
}

#[derive(Event)]
pub struct GraphChange {}

#[derive(Event)]
pub struct NodeTicked {
    pub name: String,
}
