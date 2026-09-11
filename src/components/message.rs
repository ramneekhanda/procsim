use bevy::prelude::*;
use rhai::Dynamic;

#[derive(Debug, Clone)]
pub struct Message {
    pub timer: Timer,
    pub str: String,
    pub node_from: String,
    pub node_to: String,
    pub obj: rhai::Dynamic,
    pub icon: Option<String>,
    /// The message bubble's root entity, once spawned - lets
    /// `update_message::update_message_path` update its `Transform` in place
    /// across frames instead of despawning and respawning the whole bubble
    /// (lyon shape + text layout + icon sprite) from scratch every single
    /// frame for every in-flight message, which was a real, measurable cost.
    /// `None` until the first frame this message is animated.
    pub bubble_entity: Option<Entity>,
}

#[derive(Component, Debug, Clone, Default)]
pub struct Messages {
    pub msg_inflight: Vec<Message>,
    pub msg_delivered: Vec<Message>,
}

#[derive(Component, Debug, Clone)]
pub struct MessageMarker {}
