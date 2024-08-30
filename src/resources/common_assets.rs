use bevy::prelude::*;
use std::collections::HashMap;
use std::string::String;

#[derive(Clone)]
pub enum ResourceType {
    FontHandle(Handle<Font>),
    ImageHandle(Handle<Image>),
}

#[derive(Resource, Default, Clone)]
pub struct CommonAssets {
    pub resource_map: HashMap<String, ResourceType>,
}

#[derive(Default, PartialEq, Clone)]
pub enum LoadingStateOpt {
    #[default]
    Loading,
    Ready,
}
#[derive(Resource, PartialEq, Default, Clone)]
pub struct LoadingState {
    pub state: LoadingStateOpt,
}
