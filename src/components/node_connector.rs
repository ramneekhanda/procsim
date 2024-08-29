use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct NodeConnector {
  pub id1: String,
  pub id2: String,
  pub path: lyon_algorithms::path::Path,
}
