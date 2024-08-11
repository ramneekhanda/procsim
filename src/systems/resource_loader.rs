use bevy::prelude::*;

use crate::resources::common_assets::{CommonAssets, LoadingState, ResourceType, LoadingStateOpt};


fn load_default_fonts(mut ca: ResMut<CommonAssets>, asset_server: Res<AssetServer>) {
  ca.resource_map.insert("default_font".to_string(), ResourceType::FontHandle( asset_server.load("http://fonts.gstatic.com/s/abeezee/v9/mE5BOuZKGln_Ex0uYKpIaw.ttf")));
}

pub fn load_assets(
  ca: ResMut<CommonAssets>,
  mut ls: ResMut<LoadingState>,
  asset_server: Res<AssetServer>) {
  load_default_fonts(ca, asset_server);
  ls.state = LoadingStateOpt::Ready;
}

