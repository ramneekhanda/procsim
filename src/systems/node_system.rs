use crate::components::node::{DragState, NodeMarker, SelectedNodeMarker, TickProgressFill};
use crate::parser::graphv2::{GraphAttrs, Node};
use crate::resources::common_assets::CommonAssets;
use crate::resources::common_assets::ResourceType;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::systems::drag;
use bevy::text::TextLayoutInfo;
use bevy_prototype_lyon::prelude::*;

use crate::resources::graph_def::{GraphChange, NodeAdded, NodeRemoved};
use crate::resources::narration::PendingExplain;
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_tweening::{lens::*, *};
use rand::Rng;
use std::time::Duration;

const ICON_WIDTH: f32 = 64.0;
const ICON_HEIGHT: f32 = 64.0;
const TEXT_DISTANCE_FROM_BOTTOM: f32 = 8.0;
const FONT_SIZE: f32 = 24.0;
const BOUNDING_BOX_PADDING: f32 = 8.0;
pub const TICK_BAR_WIDTH: f32 = 32.0;
const TICK_BAR_HEIGHT: f32 = 3.0;
const TICK_BAR_Y: f32 = -(ICON_HEIGHT / 2.0 + TEXT_DISTANCE_FROM_BOTTOM / 2.0);

/// `GraphChange` (a YAML edit) still does a full despawn/respawn of every node - the
/// whole graph may have changed shape and there's no live simulation state worth
/// preserving across an edit. `NodeAdded`/`NodeRemoved` (a script's runtime `spawn()`/
/// `despawn()`) are handled as a single-entity add/remove instead, so every other
/// node's position, drag state and overlay survive untouched.
pub fn create_nodes(
    mut commands: Commands,
    ca: Res<CommonAssets>,
    g: Res<GraphDefinitionRes>,
    query: Query<(Entity, &NodeMarker)>,
    mut change_reader: EventReader<GraphChange>,
    mut added_reader: EventReader<NodeAdded>,
    mut removed_reader: EventReader<NodeRemoved>,
    mut pending_explain: ResMut<PendingExplain>,
    mut sim_time: ResMut<Time<Virtual>>,
) {
    if change_reader.read().count() > 0 {
        for (entity, _) in query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        let mut z = 0.;
        let g_attrs: &GraphAttrs = &g.graph_defn.graph_attrs;
        let node_count = g.graph_defn.node_instances.len();
        for node in g.graph_defn.node_instances.iter() {
            spawn_node(z, node, &mut commands, &ca, g_attrs, node_count);
            z += 1.;
        }
        // a full rebuild already reflects any adds/removes queued this same frame
        added_reader.clear();
        removed_reader.clear();
        // A fresh run should get to re-introduce itself, and a reload mid-dialog
        // shouldn't leave the sim stuck paused (the despawn above already took the
        // narration bubble with it, as a child of whichever node held it).
        pending_explain.reset();
        sim_time.unpause();
        return;
    }

    for removed in removed_reader.read() {
        if let Some((entity, _)) = query.iter().find(|(_, m)| m.node_name == removed.name) {
            commands.entity(entity).despawn_recursive();
        }
    }

    if added_reader.is_empty() {
        return;
    }
    let g_attrs: &GraphAttrs = &g.graph_defn.graph_attrs;
    let mut z = query.iter().count() as f32;
    let node_count = g.graph_defn.node_instances.len();
    for added in added_reader.read() {
        if let Some(node) = g
            .graph_defn
            .node_instances
            .iter()
            .find(|n| n.name == added.name)
        {
            spawn_node(z, node, &mut commands, &ca, g_attrs, node_count);
            z += 1.;
        }
    }
}

/// Clears the current node selection when a left click lands on nothing pickable that
/// belongs to a node (e.g. empty canvas, the background grid) - `on_click` above only
/// runs when a node's own sprite is the click target, so it can't see misses.
pub fn deselect_on_background_click(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    hover_map: Res<bevy_mod_picking::focus::HoverMap>,
    parents: Query<&Parent>,
    is_node: Query<(), With<NodeMarker>>,
    q_selected: Query<Entity, With<SelectedNodeMarker>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if q_selected.iter().count() == 0 {
        return;
    }

    let hit_node = hover_map.values().any(|hits| {
        hits.keys().any(|entity| {
            is_node.contains(*entity)
                || parents
                    .get(*entity)
                    .is_ok_and(|parent| is_node.contains(parent.get()))
        })
    });

    if !hit_node {
        for entity in q_selected.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn on_click(
    e: Listener<Pointer<Click>>,
    mut commands: Commands,
    mut q_selected: Query<(Entity, &SelectedNodeMarker)>,
    mut q: Query<(Entity, &mut Transform, &mut Children, &NodeMarker)>,
    text_query: Query<&TextLayoutInfo>,
) {
    if q.iter().count() == 0 {
        return;
    }
    for (entity, _) in q_selected.iter_mut() {
        commands.entity(entity).despawn_recursive();
    }

    for (entity, _, children, node) in q.iter_mut() {
        let mut selected: bool = false;
        for child in children.iter() {
            if *child == e.target() {
                if selected == false {
                    selected = true;
                }
            }
        }

        // find bounding box and create a shape around it
        if selected {
            let mut text_rect = Vec2::default();
            for child in children.iter() {
                if let Ok(text) = text_query.get(*child) {
                    text_rect = text.logical_size;
                }
            }
            let x = f32::max(ICON_WIDTH, text_rect.x) + BOUNDING_BOX_PADDING;
            let y = ICON_HEIGHT + FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM + BOUNDING_BOX_PADDING;
            let y_transform = -1.0 * (FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM) / 2.0;

            let rect = Vec2::new(x, y);
            let mut vec: Vec<Vec2> = Vec::new();
            vec.push(Vec2::new(-rect.x / 2.0, -rect.y / 2.0));
            vec.push(Vec2::new(rect.x / 2.0, -rect.y / 2.0));
            vec.push(Vec2::new(rect.x / 2.0, rect.y / 2.0));
            vec.push(Vec2::new(-rect.x / 2.0, rect.y / 2.0));
            let shape = shapes::RoundedPolygon {
                points: vec,
                radius: 4.0,
                ..shapes::RoundedPolygon::default()
            };
            let id_shape = commands
                .spawn((
                    ShapeBundle {
                        path: GeometryBuilder::build_as(&shape),
                        spatial: SpatialBundle {
                            transform: Transform {
                                translation: Vec3::new(0.0, y_transform, 0.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    Fill::color(Color::rgba(1.0, 1.0, 1.0, 0.5)),
                    Stroke::new(Color::BLACK, 2.0),
                    SelectedNodeMarker {
                        node_name: node.node_name.clone(),
                        node_type: node.node_type.clone(),
                    },
                ))
                .id();
            commands.entity(entity).push_children(&[id_shape]);
        }
    }
}

/// Below this many nodes, spawn spread stays at the original fixed
/// `BASE_SPREAD` radius (tuned for the typical 5-20 node example graphs).
/// Above it, `spawn_spread_radius` grows the radius so larger graphs (e.g. a
/// several-hundred-node stress test) don't spawn every node crammed into the
/// same small area.
const BASE_SPREAD_NODE_COUNT: f32 = 20.0;
const BASE_SPREAD_RADIUS: f32 = 250.0;

/// Random spawn spread scales with `sqrt(node_count)` (not linearly) so
/// on-screen node *density* stays roughly constant as a graph grows - circle
/// area is proportional to radius squared, so radius needs to grow with the
/// square root of node count to keep area-per-node fixed.
fn spawn_spread_radius(node_count: usize) -> f32 {
    let count = (node_count as f32).max(1.0);
    BASE_SPREAD_RADIUS * (count / BASE_SPREAD_NODE_COUNT).sqrt().max(1.0)
}

fn spawn_node(
    z: f32,
    node: &Node,
    commands: &mut Commands,
    ca: &Res<CommonAssets>,
    g_attrs: &GraphAttrs,
    node_count: usize,
) {
    let mut font: Handle<Font> = Default::default();
    if let Some(ResourceType::FontHandle(f1)) = ca.resource_map.get("default_font") {
        font = f1.clone();
    };

    let mut def_icon: Handle<Image> = Default::default();
    if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get("default_system_icon") {
        def_icon = img.clone();
    };

    let text_style = TextStyle {
        font: font.clone(),
        font_size: FONT_SIZE,
        color: g_attrs.text_color,
    };
    let node_attrs = &node.node_data.attrs;
    let mut node_icon: Handle<Image> = def_icon;
    let _ = node_attrs.icon.as_ref().is_some_and(|icon_txt| {
        if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get(icon_txt) {
            node_icon = img.clone();
        };
        true
    });

    let mut rng = rand::thread_rng();
    let spread = spawn_spread_radius(node_count);
    let x = rng.gen_range(-spread..spread);
    let y = rng.gen_range(-spread..spread);

    let tween: Tween<Transform> = Tween::new(
        EaseFunction::QuadraticInOut,
        Duration::from_millis(500),
        TransformPositionLens {
            start: Vec3::ZERO,
            end: Vec3::new(x, y, z),
        },
    );

    let text_y = -1.0 * (ICON_HEIGHT / 2.0 + TEXT_DISTANCE_FROM_BOTTOM + FONT_SIZE / 2.0);

    let txt_bndl = Text2dBundle {
        text: Text::from_section(&node.name, text_style).with_justify(JustifyText::Center),
        transform: Transform::from_translation(Vec3::new(0.0, text_y, 100.)),
        ..default()
    };

    let parent = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(0., 0., 100.)),
                ..Default::default()
            },
            NodeMarker {
                node_type: node.node_data.id.clone(),
                node_name: node.name.clone(),
                ..Default::default()
            },
            DragState {
                raw: Vec2::new(x, y),
            },
            On::<Pointer<Click>>::run(on_click),
            On::<Pointer<Drag>>::run(drag::drag),
            Animator::new(tween),
        ))
        .id();

    let icon_child = commands
        .spawn((SpriteBundle {
            texture: node_icon.clone(),
            transform: Transform::from_translation(Vec3::new(0., 0., 100.)),
            sprite: Sprite {
                custom_size: Vec2::new(ICON_WIDTH, ICON_HEIGHT).into(),
                ..Default::default()
            },
            ..default()
        },))
        .id();

    let text_child = commands.spawn(txt_bndl).id();

    let track_child = commands
        .spawn(SpriteBundle {
            transform: Transform::from_translation(Vec3::new(0.0, TICK_BAR_Y, 100.)),
            sprite: Sprite {
                color: Color::srgba(0.0, 0.0, 0.0, 0.25),
                custom_size: Some(Vec2::new(TICK_BAR_WIDTH, TICK_BAR_HEIGHT)),
                ..Default::default()
            },
            ..default()
        })
        .id();

    let fill_child = commands
        .spawn((
            SpriteBundle {
                transform: Transform::from_translation(Vec3::new(0.0, TICK_BAR_Y, 101.)),
                sprite: Sprite {
                    color: g_attrs.connection_color,
                    custom_size: Some(Vec2::new(TICK_BAR_WIDTH, TICK_BAR_HEIGHT)),
                    ..Default::default()
                },
                ..default()
            },
            TickProgressFill {
                node_name: node.name.clone(),
            },
        ))
        .id();

    commands
        .entity(parent)
        .push_children(&[icon_child, text_child, track_child, fill_child]);
}
