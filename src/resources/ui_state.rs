//! Small resources backing the egui panels in `ui.rs` - kept separate from
//! `ui.rs` itself so `systems::node_system::on_click` (which needs to open
//! `NodePropertiesPopup` on a double-click) doesn't have to depend on the ui
//! module.

use bevy::prelude::*;

/// Whether the "Graph Properties" window (`ui::graph_properties_viewer`) is
/// currently shown. Starts closed - opened by double-clicking empty canvas
/// background (`node_system::open_graph_properties_on_background_double_click`),
/// closed by clicking anywhere outside the window.
#[derive(Resource, Default)]
pub struct GraphPropertiesOpen {
    pub open: bool,
    /// Cursor position (window/logical pixels) at the moment it was opened,
    /// used to place the window right where the user double-clicked (see
    /// `ui::graph_properties_viewer`) instead of a fixed default spot -
    /// otherwise any click elsewhere on a large canvas would immediately
    /// register as "outside the window" and close it right back, since the
    /// window and the click would almost never be near each other.
    pub click_pos: Option<Vec2>,
}

/// The node currently shown in the "Node Properties" popup
/// (`ui::node_properties_popup`), or `None` if it's closed. Opened by
/// double-clicking a node (`node_system::on_click`'s double-click detection);
/// closed by clicking the popup's own close button or when the node it
/// refers to disappears from the graph (a respawn on `GraphChange`, or a
/// script `despawn()`).
#[derive(Resource, Default)]
pub struct NodePropertiesPopup {
    pub node_name: Option<String>,
    /// The node's on-screen position (viewport/logical pixels) at the moment
    /// the popup was opened, used to place the window just above the node
    /// (see `ui::node_properties_popup`) - a plain `Vec2` rather than an egui
    /// type so this resource (read by `node_system`, a non-UI module) doesn't
    /// need an egui dependency.
    pub anchor_screen_pos: Option<Vec2>,
}

/// Double-click detection has no built-in support in `bevy_mod_picking`
/// (`Pointer<Click>` carries no click-count) - this tracks the last node
/// clicked and when, so `node_system::on_click` can tell "two clicks on the
/// same node within `DOUBLE_CLICK_SECS`" apart from two unrelated clicks.
#[derive(Resource, Default)]
pub struct LastNodeClick {
    pub node_name: Option<String>,
    pub at: f32,
}
