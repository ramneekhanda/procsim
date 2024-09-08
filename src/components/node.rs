use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct NodeMarker {
    pub node_name: String,
    pub node_type: String,
}

impl Default for NodeMarker {
    fn default() -> Self {
        NodeMarker {
            node_type: "name".to_string(),
            node_name: "id".to_string(),
        }
    }
}

#[derive(Component, Debug)]
pub struct SelectedNodeMarker {
    pub node_name: String,
    pub node_type: String,
}
