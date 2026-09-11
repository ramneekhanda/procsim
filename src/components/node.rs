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

/// A single shape entity belonging to a node's custom `draw()` overlay. Child of
/// the node entity; despawned and rebuilt by `systems::node_overlay` whenever the
/// node's overlay changes.
#[derive(Component, Debug)]
pub struct NodeOverlayShape {
    pub node_name: String,
}

/// Marks the root entity of the currently-shown narration bubble (see
/// `systems::explain_bubble`). `despawn_recursive` on this one entity cleans up
/// the whole bubble (body, pointer, text, button), since they're all its children.
#[derive(Component, Debug)]
pub struct ExplainBubble;

/// Tracks a node's true (unsnapped) position while dragging, so drag deltas
/// accumulate smoothly even though the displayed `Transform` snaps to the grid.
#[derive(Component, Debug)]
pub struct DragState {
    pub raw: Vec2,
}
