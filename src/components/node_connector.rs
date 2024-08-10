use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct NodeConnector {
    pub node1: String,
    pub node2: String,
    pub path: lyon_algorithms::path::Path,
}
