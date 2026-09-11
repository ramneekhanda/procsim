use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;

use crate::components::message::*;
use crate::components::node_connector::*;
use crate::resources::common_assets::CommonAssets;
use crate::resources::common_assets::ResourceType;
use crate::resources::graph_def::GraphDefinitionRes;

const BUBBLE_FONT_SIZE: f32 = 16.0;
const BUBBLE_PADDING_X: f32 = 10.0;
const BUBBLE_PADDING_Y: f32 = 6.0;
const BUBBLE_CHAR_WIDTH_FACTOR: f32 = 0.55;
const BUBBLE_MIN_WIDTH: f32 = 28.0;
const BUBBLE_CORNER_RADIUS: f32 = 8.0;
const BUBBLE_ICON_SIZE: f32 = 20.0;
const BUBBLE_ICON_TEXT_GAP: f32 = 4.0;

/// Samples points at a fixed arc-length interval along a path.
fn walk_message(path: &lyon_algorithms::path::Path) -> Vec<[f32; 2]> {
    use lyon_algorithms::walk::{walk_along_path, RegularPattern, WalkerEvent};

    let mut x: Vec<[f32; 2]> = vec![];
    let mut pattern = RegularPattern {
        callback: &mut |event: WalkerEvent| {
            x.push(event.position.to_array());
            true // Return true to continue walking the path.
        },
        interval: 0.1,
    };

    let tolerance = 0.1; // The path flattening tolerance.
    let start_offset = 0.0; // Start walking at the beginning of the path.

    walk_along_path(path.iter(), start_offset, tolerance, &mut pattern);
    x
}

pub fn update_message_path(
    mut query_conn: Query<(&mut Messages, &mut NodeConnector)>,
    mut query_marker: Query<(Entity, &mut MessageMarker)>,
    ca: Res<CommonAssets>,
    gd: Res<GraphDefinitionRes>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let mut font: Handle<Font> = Default::default();
    if let Some(ResourceType::FontHandle(f1)) = ca.resource_map.get("default_font") {
        font = f1.clone();
    };

    let text_style = TextStyle {
        font: font.clone(),
        font_size: BUBBLE_FONT_SIZE,
        color: gd.graph_defn.graph_attrs.text_color,
    };

    for (entity, _marker) in query_marker.iter_mut() {
        commands.entity(entity).despawn_recursive();
    }

    for (mut msgs, mut nc) in query_conn.iter_mut() {
        let v_points = walk_message(&nc.path); //TODO: cache this - put it in NodeConnector
        let mut msg_finished = Vec::<Message>::new();

        msgs.msg_inflight.retain(|m: &Message| {
            if m.timer
                .duration()
                .as_millis()
                .abs_diff(m.timer.elapsed().as_millis())
                > 10
            {
                return true;
            } else {
                msg_finished.push(m.clone());
                return false;
            }
        });

        if !msg_finished.is_empty() {
            // A brief highlight on the connector itself when a message lands,
            // decayed back down by `update_connectors::update_connector_style`
            // - reinforces delivery on the edge, not just the arriving icon.
            nc.flash = 1.0;
        }
        msgs.msg_delivered.append(&mut msg_finished);

        for mesg in msgs.msg_inflight.iter_mut() {
            mesg.timer.tick(time.delta());

            let mut loc = ((v_points.len() as f32 * mesg.timer.elapsed().as_millis() as f32)
                / mesg.timer.duration().as_millis() as f32) as usize;

            let reversed = mesg.node_from == nc.id2;
            if reversed {
                loc = v_points.len() - loc;
            }
            if loc >= v_points.len() {
                continue;
            }
            let parent = commands
                .spawn((
                    SpatialBundle {
                        transform: Transform::from_translation(Vec3::new(
                            v_points[loc][0],
                            v_points[loc][1],
                            100.,
                        )),
                        ..Default::default()
                    },
                    MessageMarker {},
                ))
                .id();

            let msg_icon: Option<Handle<Image>> = mesg.icon.as_ref().and_then(|icon_id| {
                if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get(icon_id) {
                    Some(img.clone())
                } else {
                    None
                }
            });

            let text_width =
                (mesg.str.chars().count() as f32) * BUBBLE_FONT_SIZE * BUBBLE_CHAR_WIDTH_FACTOR;
            let content_width = if msg_icon.is_some() {
                BUBBLE_ICON_SIZE + BUBBLE_ICON_TEXT_GAP + text_width
            } else {
                text_width
            };
            let bubble_width = (content_width + 2.0 * BUBBLE_PADDING_X).max(BUBBLE_MIN_WIDTH);
            let bubble_height = BUBBLE_FONT_SIZE.max(BUBBLE_ICON_SIZE) + 2.0 * BUBBLE_PADDING_Y;
            let half = Vec2::new(bubble_width / 2.0, bubble_height / 2.0);
            let bubble_shape = shapes::RoundedPolygon {
                points: vec![
                    Vec2::new(-half.x, -half.y),
                    Vec2::new(half.x, -half.y),
                    Vec2::new(half.x, half.y),
                    Vec2::new(-half.x, half.y),
                ],
                radius: BUBBLE_CORNER_RADIUS,
                ..shapes::RoundedPolygon::default()
            };
            let bubble_child = commands
                .spawn((
                    ShapeBundle {
                        path: GeometryBuilder::build_as(&bubble_shape),
                        ..default()
                    },
                    Fill::color(gd.graph_defn.graph_attrs.background),
                    Stroke::new(gd.graph_defn.graph_attrs.text_color, 1.5),
                ))
                .id();

            let content_left = -content_width / 2.0;
            let text_x = if let Some(icon) = &msg_icon {
                let icon_x = content_left + BUBBLE_ICON_SIZE / 2.0;
                commands.entity(parent).with_children(|p| {
                    p.spawn(SpriteBundle {
                        texture: icon.clone(),
                        transform: Transform::from_translation(Vec3::new(icon_x, 0.0, 1.0)),
                        sprite: Sprite {
                            custom_size: Vec2::new(BUBBLE_ICON_SIZE, BUBBLE_ICON_SIZE).into(),
                            ..Default::default()
                        },
                        ..default()
                    });
                });
                content_left + BUBBLE_ICON_SIZE + BUBBLE_ICON_TEXT_GAP + text_width / 2.0
            } else {
                0.0
            };

            let text_child = commands
                .spawn(Text2dBundle {
                    text: Text::from_section(mesg.str.clone(), text_style.clone())
                        .with_justify(JustifyText::Center),
                    transform: Transform::from_translation(Vec3::new(text_x, 0.0, 1.0)),
                    ..default()
                })
                .id();

            commands
                .entity(parent)
                .push_children(&[bubble_child, text_child]);
        }
    }
}
