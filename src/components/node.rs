use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct Node {
    pub node_text: String,
}

impl Default for Node {
    fn default() -> Self {
        Node {
            node_text: "ANODE".to_string(),
        }
    }
}
