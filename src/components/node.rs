use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct Node {
  pub node_id: String,
  pub node_text: String,
}

impl Default for Node {
  fn default() -> Self {
      Node {
          node_text: "name".to_string(),
          node_id: "id".to_string(),
      }
  }
}
