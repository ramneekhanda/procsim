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

/// Fired when a script's `spawn()` call is applied. Handled by `node_system::create_nodes`
/// as a targeted single-entity spawn, distinct from `GraphChange`'s full despawn/respawn -
/// a running node's position, state and overlay must survive a peer being added.
#[derive(Event)]
pub struct NodeAdded {
    pub name: String,
}

/// Fired when a script's `despawn()` call is applied. See `NodeAdded`.
#[derive(Event)]
pub struct NodeRemoved {
    pub name: String,
}
