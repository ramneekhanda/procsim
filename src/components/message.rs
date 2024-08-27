use bevy::prelude::*;
#[derive(Component, Debug, Clone)]
pub struct Message {
  pub timer: Timer,
  pub str: String,
  pub node_from: String,
  pub node_to: String,
}

#[derive(Component, Debug, Clone)]
pub struct HotSpot {}

#[derive(Component)]
pub struct ColorText;
