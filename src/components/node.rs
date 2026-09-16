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
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgressKind {
    Bar {
        origin_x: f32,
        w: f32,
    },
    Ring {
        center: Vec2,
        r: f32,
        start_angle: f32,
        clockwise: bool,
    },
    Pie {
        center: Vec2,
        r: f32,
        start_angle: f32,
        clockwise: bool,
    },
    Segmented {
        segment_index: usize,
        total_segments: usize,
        active_color: Color,
        inactive_color: Color,
    },
}

#[derive(Component, Clone, Debug)]
pub struct TickProgressFill {
    pub node_name: String,
    pub kind: ProgressKind,
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

/// Marks a node whose position is managed by the layout system and cannot be manually dragged.
#[derive(Component, Debug, Default)]
pub struct LayoutLocked;
