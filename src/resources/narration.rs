use bevy::prelude::*;
use std::collections::{HashSet, VecDeque};

/// One `explain()` call that passed the once-per-run dedup and is waiting to be
/// (or currently being) shown as a narration bubble.
#[derive(Clone, Debug)]
pub struct ExplainEntry {
    pub node_name: String,
    pub key: String,
    pub text: String,
}

/// Backs the `explain(key, text)` host function (see `systems::rhai_engine`).
/// `seen` is the once-per-run dedup set - a call whose `key` is already in it is a
/// no-op. `queue` holds entries that passed dedup and haven't been shown yet;
/// `systems::explain_bubble` shows them one at a time, pausing the sim for each.
#[derive(Resource, Default)]
pub struct PendingExplain {
    pub seen: HashSet<String>,
    pub queue: VecDeque<ExplainEntry>,
    /// Name of the node the currently-shown bubble (if any) is anchored to -
    /// set by `explain_bubble::show_next_explain` when it spawns one, cleared
    /// by `dismiss_explain_bubble`. Lets `node_system::create_nodes` notice
    /// when a script despawns the very node its own bubble is anchored to
    /// (e.g. `explain(...)` followed by `despawn(node_name)` in the same
    /// handler) and unpause the sim itself - the despawn takes the bubble
    /// entity down with it (it's a child of the node), so there's no
    /// `dismiss_explain_bubble` click left to do that unpause otherwise, and
    /// the sim would stay frozen forever with nothing visible to dismiss.
    pub showing: Option<String>,
}

impl PendingExplain {
    /// Resets state for a fresh run (called on `GraphChange`) - a reloaded graph
    /// should get to re-introduce itself, and no stale queue should keep the sim
    /// paused waiting on a bubble anchored to a node that no longer exists.
    pub fn reset(&mut self) {
        self.seen.clear();
        self.queue.clear();
        self.showing = None;
    }
}
