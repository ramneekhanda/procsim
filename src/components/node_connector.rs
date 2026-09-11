use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct NodeConnector {
    pub id1: String,
    pub id2: String,
    pub path: lyon_algorithms::path::Path,
    /// The invisible `Sprite` hit-region entity (child of this connector) that
    /// actually receives pointer events - `bevy_mod_picking` here only
    /// hit-tests `Sprite`s, not this connector's own lyon `Mesh2d`. See
    /// `systems::update_connectors`.
    pub hit_region: Entity,
    /// True while the pointer is over `hit_region`. Drives the hover
    /// highlight in `systems::update_connectors::update_connector_style`.
    pub hovered: bool,
    /// 0..1, set to 1 when a message is delivered on this connector and
    /// decays back to 0 over `FLASH_DECAY_SECS` - a brief highlight so
    /// delivery reads on the edge itself, not only on the traveling message
    /// icon.
    pub flash: f32,
}

/// Marker on a connector's hit-region child sprite, so the transform-sync
/// system in `update_connectors` can find and reposition just those
/// entities (not every other `Sprite` in the scene) each frame.
#[derive(Component, Debug)]
pub struct ConnectorHitRegion;
