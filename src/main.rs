mod shimmer;
mod ui;

mod components;
mod parser;
mod systems;
use bevy_egui::EguiPlugin;
//use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_prototype_lyon::prelude::*;
use bevy_tweening::TweeningPlugin;
use parser::graphv2::{parse_graph2, GraphDefinition};
use std::time::Duration;
use ui::{CodeStorage, GraphDefinitionRes};

#[cfg(target_arch = "wasm32")]
use systems::browser_resize::handle_browser_resize;

fn main() {
    let mut app = App::new();

    app.insert_resource(ClearColor(Color::rgb(0.9, 0.9, 0.9)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
          primary_window: Some(Window {
              canvas: Some("#bevy-canvas".into()),
              ..default()
          }),
          ..default()
        }))
        //.add_plugins(WorldInspectorPlugin::default())
        .add_plugins(EguiPlugin)
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPickingPlugins)
        .add_plugins(ShapePlugin)
        .add_plugins(TweeningPlugin)
        .insert_resource(DebugPickingMode::Noisy)
        .insert_resource(systems::demo_message::DemoTimer {
            timer: Timer::new(Duration::from_secs(4), TimerMode::Repeating),
        })
        .insert_resource(CodeStorage::default())
        .insert_resource(GraphDefinitionRes {
            graph_defn: GraphDefinition::default(),
        })
        .add_systems(Startup, setup_camera)
        .add_systems(
            Update,
            (
                systems::demo_message::demo_send_message,
                systems::update_message::update_message_path,
            ),
        )
        .add_systems(
            Update,
            (
                systems::background::update_background,
                ui::draw_codeviewer,
                systems::update_connectors::update_connectors,
                systems::update_node::update_nodes,
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
