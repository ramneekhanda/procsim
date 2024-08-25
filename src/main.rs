mod ui;
mod resources;
mod components;
mod parser;
mod systems;
mod conditions;

use std::{collections::HashMap, sync::{Arc, Mutex}};
use bevy::{asset::AssetMetaCheck, prelude::*};
use bevy_egui::EguiPlugin;
use bevy_mod_picking::prelude::*;
use bevy_prototype_lyon::prelude::*;
use bevy_tweening::TweeningPlugin;
use parser::graphv2::GraphDefinition;
use std::time::Duration;
use ui::{CodeStorage, GraphDefinitionRes};
use resources::common_assets::{CommonAssets, LoadingState, LoadingStateOpt};
#[cfg(target_arch = "wasm32")]
use systems::browser_resize::handle_browser_resize;
use bevy_web_asset::WebAssetPlugin;

fn setup_app(app: &mut App) {

    app.insert_resource(ClearColor(Color::rgb(0.9, 0.9, 0.9)))
        .add_plugins(WebAssetPlugin)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#bevy-canvas".into()),
                ..default()
            })
            ,..Default::default()
        }).set(
            AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            }))
        //.add_plugins(WorldInspectorPlugin::default())
        .insert_resource(CommonAssets {
            resource_map: HashMap::new(),
        })
        .insert_resource(Msaa::Sample4)
        .insert_resource(systems::demo_message::DemoTimer {
            timer: Timer::new(Duration::from_secs(4), TimerMode::Repeating),
        })
        .insert_resource(CodeStorage::default())
        .insert_resource(LoadingState {state: LoadingStateOpt::Ready})
        .insert_resource(GraphDefinitionRes {
            graph_defn: GraphDefinition::default(),
        })
        .add_plugins(ShapePlugin)
        .add_plugins(TweeningPlugin)
        .add_plugins(EguiPlugin)
        .add_plugins(DefaultPickingPlugins)
        
        .add_systems(Startup, 
            setup_camera,
        )
        .add_systems(
            Update,
            (
                systems::resource_loader::load_assets.run_if(resource_equals(LoadingState{state: LoadingStateOpt::Loading})),
                systems::demo_message::demo_send_message.run_if(resource_equals(LoadingState{state: LoadingStateOpt::Ready})),
                systems::update_message::update_message_path.run_if(resource_equals(LoadingState{state: LoadingStateOpt::Ready})),
            ),
        )
        .add_systems(
            Update,
            (
                systems::background::update_background.run_if(resource_equals(LoadingState{state: LoadingStateOpt::Ready})),
                ui::ingest_codechange.run_if(resource_equals(LoadingState{state: LoadingStateOpt::Ready})),
                #[cfg(not(target_arch = "wasm32"))]
                ui::draw_codeviewer.run_if(resource_equals(LoadingState{state: LoadingStateOpt::Ready})),
                systems::update_connectors::update_connectors.run_if(resource_equals(LoadingState{state: LoadingStateOpt::Ready})),
                systems::update_node::update_nodes.run_if(resource_equals(LoadingState{state: LoadingStateOpt::Ready})),
            ),
        );
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, handle_browser_resize);
    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        ..Default::default()
    });
}

fn main() {
    let mut app = App::new();
    setup_app(&mut app);
    app.run();
}
