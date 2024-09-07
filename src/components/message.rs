use bevy::prelude::*;
#[derive(Debug, Clone)]
pub struct Message {
    pub timer: Timer,
    pub str: String,
    pub node_from: String,
    pub node_to: String,
}

#[derive(Component, Debug, Clone, Default)]
pub struct Messages {
    pub msg_inbox: Vec<Message>,
}

#[derive(Component, Debug, Clone)]
pub struct MessageMarker {}

