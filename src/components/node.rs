use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct Node {
    pub node_text: String,
    pub timers: Vec<(Timer, String)>,
}

#[derive(Component, Debug, Clone)]
pub struct NodeTimers {
    pub timers: Vec<(Timer, String)>,
}

impl Default for Node {
    fn default() -> Self {
        Node {
            node_text: "ANODE".to_string(),
            timers: vec![],
        }
    }
}
