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
}

impl PendingExplain {
    /// Resets state for a fresh run (called on `GraphChange`) - a reloaded graph
    /// should get to re-introduce itself, and no stale queue should keep the sim
    /// paused waiting on a bubble anchored to a node that no longer exists.
    pub fn reset(&mut self) {
        self.seen.clear();
        self.queue.clear();
    }
}
