use bevy::prelude::*;
use rhai::Dynamic;

#[derive(Debug, Clone)]
pub struct Message {
    pub timer: Timer,
    pub str: String,
    pub node_from: String,
    pub node_to: String,
    pub obj: rhai::Dynamic,
}

#[derive(Component, Debug, Clone, Default)]
pub struct Messages {
    pub msg_inflight: Vec<Message>,
    pub msg_delivered: Vec<Message>,
}

#[derive(Component, Debug, Clone)]
pub struct MessageMarker {}

