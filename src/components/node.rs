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

#[derive(Component, Debug)]
pub struct TickProgressFill {
    pub node_name: String,
}

/// Tracks a node's true (unsnapped) position while dragging, so drag deltas
/// accumulate smoothly even though the displayed `Transform` snaps to the grid.
#[derive(Component, Debug)]
pub struct DragState {
    pub raw: Vec2,
}
