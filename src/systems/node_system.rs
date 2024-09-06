use crate::components::node::{SelectedNodeMarker,NodeMarker};
use crate::parser::graphv2::{GraphAttrs, Node};
use crate::resources::common_assets::CommonAssets;
use crate::resources::common_assets::ResourceType;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::systems::drag;
use crate::c_log;
use bevy::text::TextLayoutInfo;
use bevy::transform::commands;
use bevy_egui::egui::Shape;
use bevy_prototype_lyon::prelude::*;

use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_tweening::{lens::*, *};
use rand::Rng;
use std::time::Duration;
use crate::resources::graph_def::GraphChange;

const ICON_WIDTH: f32 = 64.0;
const ICON_HEIGHT: f32 = 64.0;
const TEXT_DISTANCE_FROM_BOTTOM: f32 = 8.0;
const FONT_SIZE: f32 = 24.0;
const BOUNDING_BOX_PADDING: f32 = 8.0;

pub fn create_nodes(
    mut commands: Commands,
    ca: Res<CommonAssets>,
    g: Res<GraphDefinitionRes>,
    query: Query<Entity, With<NodeMarker>>,
    mut event_reader: EventReader<GraphChange>,
) {
  if event_reader.read().count() > 0 {
    for entity in query.iter() {
      commands.entity(entity).despawn_recursive();
    }
    let mut z = 0.;
    let g_attrs: &GraphAttrs = &g.graph_defn.graph_attrs;
    for node in g.graph_defn.node_instances.iter() {
        spawn_node(z, node, &mut commands, &ca, g_attrs);
        z += 1.;
    }
  }
}

pub fn on_click(
    e: Listener<Pointer<Click>>,
    mut commands: Commands,
    mut q_selected: Query<(Entity, &SelectedNodeMarker)>,
    mut q: Query<(Entity, &mut Transform, &mut Children, &NodeMarker)>,
    text_query: Query<&TextLayoutInfo>,
    sprite: Query<&Sprite>,
) {
    if q.iter().count() == 0 {
        return;
    }
    for (entity, _) in q_selected.iter_mut() {
        commands.entity(entity).despawn_recursive();
    }
    
    for (mut entity, mut transform, children, node) in q.iter_mut() {
        let mut selected: bool = false;
        let mut scale = 1.0;
        for child in children.iter() {
            if *child == e.target() {
                if selected == false {
                    selected = true;
                }
            }
        }

        // find bounding box and create a shape around it
        if selected {
            scale = 1.25;
            let mut text_rect = Vec2::default();
            for child in children.iter() {
                if let Ok(text) = text_query.get(*child) {
                    text_rect = text.logical_size;
                } 
            }
            let x = f32::max(ICON_WIDTH, text_rect.x) + BOUNDING_BOX_PADDING;
            let y = ICON_HEIGHT + FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM + BOUNDING_BOX_PADDING;
            let y_transform = -1.0 * (FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM)/2.0;    

            let rect = Vec2::new(x, y);
            let mut vec: Vec<Vec2> = Vec::new();
            vec.push(Vec2::new(-rect.x/2.0, -rect.y/2.0));
            vec.push(Vec2::new(rect.x/2.0, -rect.y/2.0));
            vec.push(Vec2::new(rect.x/2.0, rect.y/2.0));
            vec.push(Vec2::new(-rect.x/2.0, rect.y/2.0));
            let shape = shapes::RoundedPolygon {
                points:vec,
                radius: 4.0,
                ..shapes::RoundedPolygon::default()
            };
            let id_shape = commands.spawn((
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
                }
            )).id();
            commands.entity(entity).push_children(&[id_shape]);
        }
        //transform.scale = Vec3::new(scale, scale, 1.0);
    }
    
}

fn spawn_node(
    z: f32,
    node: &Node,
    commands: &mut Commands,
    ca: &Res<CommonAssets>,
    g_attrs: &GraphAttrs,
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
    let x = rng.gen_range(-250.0..250.0);
    let y = rng.gen_range(-250.0..250.0);

    let tween: Tween<Transform> = Tween::new(
        EaseFunction::QuadraticInOut,
        Duration::from_millis(500),
        TransformPositionLens {
            start: Vec3::ZERO,
            end: Vec3::new(x, y, z),
        },
    );

    let shape = shapes::RegularPolygon {
        sides: 4,
        feature: shapes::RegularPolygonFeature::Radius(22.0),
        ..shapes::RegularPolygon::default()
    };

    let text_y = -1.0 * (ICON_HEIGHT/2.0 + TEXT_DISTANCE_FROM_BOTTOM + FONT_SIZE/2.0);

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

    commands
        .entity(parent)
        .push_children(&[icon_child, text_child]);
}

