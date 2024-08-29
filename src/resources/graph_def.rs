use bevy::prelude::*;
use crate::parser::graphv2::GraphDefinition;

#[derive(Resource, Default, Debug)]
pub struct GraphDefinitionRes {
  pub graph_defn: GraphDefinition,
}

