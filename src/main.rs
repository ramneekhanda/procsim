mod components;
mod parser;
mod resources;
mod stdlib;
mod systems;
mod ui;
mod wasm;

use bevy::{asset::AssetMetaCheck, prelude::*, render::view::RenderLayers};
use bevy_egui::EguiPlugin;
use bevy_mod_picking::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use bevy_prototype_lyon::prelude::*;
use bevy_tweening::TweeningPlugin;
use bevy_web_asset::WebAssetPlugin;
use components::camera::BubbleCamera;
use parser::graphv2::GraphDefinition;
use resources::common_assets::{CommonAssets, LoadingState, LoadingStateOpt};
use resources::graph_def::{GraphChange, GraphDefinitionRes, NodeAdded, NodeRemoved, NodeTicked};
use resources::narration::PendingExplain;
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use systems::browser_resize::handle_browser_resize;

use ui::CodeStorage;

fn setup_app(app: &mut App) {
    app.insert_resource(ClearColor(Color::srgb(0.9, 0.9, 0.9)))
        .add_plugins(WebAssetPlugin)
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some("#bevy-canvas".into()),
                        ..default()
                    }),
                    ..Default::default()
                })
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        );
    app.insert_resource(CommonAssets {
        resource_map: HashMap::new(),
    })
    .insert_resource(Msaa::Sample4)
    .insert_resource(CodeStorage::default())
    .insert_resource(LoadingState {
        state: LoadingStateOpt::Ready,
    })
    .insert_resource(GraphDefinitionRes {
        graph_defn: GraphDefinition::default(),
    })
    .insert_resource(PendingExplain::default())
    .add_event::<GraphChange>()
    .add_event::<NodeTicked>()
    .add_event::<NodeAdded>()
    .add_event::<NodeRemoved>()
    .add_plugins(ShapePlugin)
    .add_plugins(TweeningPlugin)
    .add_plugins(EguiPlugin)
    .add_plugins(DefaultPickingPlugins)
    .add_plugins(PanCamPlugin)
    .add_plugins(systems::radial_blur::RadialBlurPlugin)
    .add_systems(
        Startup,
        (setup_camera, systems::background_grid::setup_grid),
    )
    .add_systems(
        Update,
        (
            systems::rhai_engine::execute_rhai_engine.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::clearcolor::clear_color.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::resource_loader::load_assets.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Loading,
            })),
            systems::update_message::update_message_path.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::node_pulse::pulse_on_tick.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
        ),
    )
    .add_systems(
        Update,
        (
            systems::ingest_code::ingest_codechange.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            ui::graph_properties_viewer.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::update_connectors::update_connectors.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::update_connectors::update_connector_style.run_if(resource_equals(
                LoadingState {
                    state: LoadingStateOpt::Ready,
                },
            )),
            systems::node_system::create_nodes.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::node_progress::update_tick_progress.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::node_system::deselect_on_background_click.run_if(resource_equals(
                LoadingState {
                    state: LoadingStateOpt::Ready,
                },
            )),
            systems::node_overlay::render_node_overlays.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::explain_bubble::show_next_explain.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::explain_bubble::animate_bubble_pop.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::radial_blur::animate_radial_blur.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::radial_blur::sync_bubble_camera.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
        ),
    );
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, handle_browser_resize);
    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2dBundle {
            ..Default::default()
        },
        PanCam {
            grab_buttons: vec![MouseButton::Right],
            min_scale: 0.1,
            max_scale: Some(10.0),
            ..default()
        },
        systems::radial_blur::RadialBlurSettings::default(),
    ));

    // Second camera, drawn on top of the first: renders only `RenderLayers::layer(1)`
    // (narration bubbles - see `systems::explain_bubble`) with a transparent clear,
    // so it composites over the (possibly blurred) world without carrying
    // `RadialBlurSettings` itself, and therefore without being blurred. Kept in sync
    // with the main camera's pan/zoom by `systems::radial_blur::sync_bubble_camera`.
    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            ..default()
        },
        RenderLayers::layer(1),
        BubbleCamera,
    ));
}

fn main() {
    let mut app = App::new();
    setup_app(&mut app);
    app.run();
}
