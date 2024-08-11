use bevy::prelude::*;
use std::collections::HashMap;
use std::string::String;

#[derive(Clone)]
enum ResourceType {
    FontHandle(Handle<Font>),
}

#[derive(Resource, Default, Clone)]
pub struct CommonAssets {
    pub resource_map: HashMap<String, ResourceType>
}

#[derive(Resource, Default, Clone)]
pub enum LoadingState {
    #[default]
    Loading,
    Ready
}

