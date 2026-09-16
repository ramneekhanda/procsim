mod components;
mod parser;
mod resources;
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
use resources::graph_def::{GraphChange, GraphDefinitionRes, NodeAdded, NodeRemoved};
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
        theme_font_url: None,
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
    .insert_resource(resources::ui_state::GraphPropertiesOpen::default())
    .insert_resource(resources::ui_state::NodePropertiesPopup::default())
    .insert_resource(resources::ui_state::LastNodeClick::default())
    // TEMPORARY - see systems::profiling's doc comment.
    .insert_resource(systems::profiling::ProfilingStats::default())
    .add_event::<GraphChange>()
    .add_event::<NodeAdded>()
    .add_event::<NodeRemoved>()
    .add_plugins(ShapePlugin)
    .add_plugins(TweeningPlugin)
    .add_plugins(EguiPlugin)
    .add_plugins(DefaultPickingPlugins)
    .add_plugins(PanCamPlugin)
    .add_plugins(systems::radial_blur::RadialBlurPlugin)
    .add_plugins(systems::background_grid::BackgroundGridPlugin)
    .add_systems(
        Startup,
        (setup_camera, systems::background_grid::setup_grid),
    );
    // Native-only: see `ingest_code::load_native_demo_on_startup`'s doc comment -
    // there's no browser/Monaco editor in a native `cargo run` to ever call
    // `compile_code()`, so without this a native window shows an empty grid forever.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Startup, systems::ingest_code::load_native_demo_on_startup);
    // Native-only "Examples" picker window - see native_examples.rs's doc comment.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        Update,
        systems::native_examples::examples_picker.run_if(resource_equals(LoadingState {
            state: LoadingStateOpt::Ready,
        })),
    );
    app.add_systems(
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
            systems::update_message::cleanup_messages_on_graph_change.run_if(resource_equals(
                LoadingState {
                    state: LoadingStateOpt::Ready,
                },
            )),
            // TEMPORARY - see systems::profiling's doc comment.
            systems::profiling::report_profiling_stats.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
        ),
    )
    .add_systems(
        Update,
        (
            // Deliberately NOT gated on LoadingState::Ready (unlike everything
            // else in this block) - this is the system that detects an edit and
            // kicks off a new load in the first place. Gating it to Ready meant
            // an edit made while a previous graph was still mid-load (asset
            // fetches in flight) was silently ignored until that load finished -
            // easy to hit by editing/reloading in quick succession. Running it
            // every frame is safe: re-setting LoadingState to Loading while
            // already Loading is a no-op in effect, and replacing graph_defn
            // mid-load correctly means the newest edit wins rather than being
            // dropped - the old graph's in-flight asset handles just become
            // irrelevant leftovers, already handled by load_assets pruning
            // CommonAssets.resource_map down to what the new graph needs.
            systems::ingest_code::ingest_codechange,
            ui::graph_properties_viewer.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            ui::node_properties_popup.run_if(resource_equals(LoadingState {
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
            systems::node_progress::update_tick_progress.run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
            systems::node_system::deselect_on_background_click.run_if(resource_equals(
                LoadingState {
                    state: LoadingStateOpt::Ready,
                },
            )),
            systems::node_system::open_graph_properties_on_background_double_click.run_if(
                resource_equals(LoadingState {
                    state: LoadingStateOpt::Ready,
                }),
            ),
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
    )
    // `create_nodes` reacts to a `GraphChange` with a full despawn-and-respawn-all of
    // every node entity, via `Commands` - deferred, so without an explicit sync point
    // here `render_node_overlays`, if it happened to run first in the same frame
    // (systems in an unordered tuple have no guaranteed relative order - can vary
    // frame to frame, run to run), would see the *old*, about-to-despawn entities
    // still present under the same node names, attach that frame's overlay to them,
    // and mark it clean - only for those entities (overlay children included) to be
    // despawned a moment later when the queued commands finally apply, leaving the
    // *new* entities that actually replace them with `overlay_dirty` already false
    // and so no overlay ever attached. This is the "node/icon sometimes doesn't
    // render" bug - `apply_deferred` forces `create_nodes`' despawn/respawn to fully
    // land before `render_node_overlays` ever looks at the world, every frame, making
    // the outcome deterministic instead of a race.
    .add_systems(
        Update,
        (
            systems::node_system::create_nodes,
            apply_deferred,
            systems::node_overlay::render_node_overlays,
            systems::group_overlay::update_group_boxes,
        )
            .chain()
            .run_if(resource_equals(LoadingState {
                state: LoadingStateOpt::Ready,
            })),
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
