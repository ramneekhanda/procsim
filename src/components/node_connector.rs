use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct NodeConnector {
    pub id1: String,
    pub id2: String,
    pub path: lyon_algorithms::path::Path,
    /// Points sampled along `path` at a fixed arc-length interval, used to
    /// place an in-flight message along the curve - recomputed only when
    /// `path` itself is rebuilt (see `update_connectors::walk_path`), not
    /// every frame like the code this replaced did (a real, measurable cost
    /// on any connector that happened to be carrying a message, since a
    /// message's connector rarely stays idle for its ~3s travel time).
    pub walk_cache: Vec<[f32; 2]>,
    /// 0..1, set to 1 when a message is delivered on this connector and
    /// decays back to 0 over `FLASH_DECAY_SECS` - a brief highlight so
    /// delivery reads on the edge itself, not only on the traveling message
    /// icon.
    pub flash: f32,
}
