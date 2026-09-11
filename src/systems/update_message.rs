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

/// Animates each in-flight message's bubble along its connector's cached
/// walk points (`NodeConnector::walk_cache`).
///
/// Each message's bubble entity tree (rounded-rect + icon + text) is spawned
/// *once*, the first frame the message exists, and its `Entity` cached on
/// `Message::bubble_entity`; every later frame just updates that entity's
/// `Transform` in place. This used to despawn and fully respawn every
/// in-flight message's bubble - a fresh lyon shape tessellation, text
/// layout, and sprite - on *every* frame, which was a real, measurable cost
/// that scaled with message-traffic volume (a message's bubble content never
/// changes once created; only its position along the path does).
pub fn update_message_path(
    mut query_conn: Query<(&mut Messages, &mut NodeConnector)>,
    mut query_bubble_transform: Query<&mut Transform, With<MessageMarker>>,
    ca: Res<CommonAssets>,
    gd: Res<GraphDefinitionRes>,
    time: Res<Time>,
    mut commands: Commands,
    // TEMPORARY - see systems::profiling's doc comment.
    mut prof: ResMut<crate::systems::profiling::ProfilingStats>,
) {
    let __prof_t0 = web_time::Instant::now(); // TEMPORARY
    let mut font: Handle<Font> = Default::default();
    if let Some(ResourceType::FontHandle(f1)) = ca.resource_map.get("default_font") {
        font = f1.clone();
    };

    let text_style = TextStyle {
        font: font.clone(),
        font_size: BUBBLE_FONT_SIZE,
        color: gd.graph_defn.graph_attrs.text_color,
    };

    for (mut msgs, mut nc) in query_conn.iter_mut() {
        // The overwhelming majority of connectors have no in-flight message at
        // any given instant, especially on a large/dense graph. Skip
        // everything below entirely when idle.
        if msgs.msg_inflight.is_empty() {
            continue;
        }
        let mut msg_finished = Vec::<Message>::new();

        msgs.msg_inflight.retain_mut(|m: &mut Message| {
            if m.timer
                .duration()
                .as_millis()
                .abs_diff(m.timer.elapsed().as_millis())
                > 10
            {
                return true;
            } else {
                // Retiring - despawn its bubble now rather than leaving it to
                // a "despawn everything" sweep (there no longer is one).
                if let Some(e) = m.bubble_entity.take() {
                    commands.entity(e).despawn_recursive();
                }
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

        // Borrowed after the `nc.flash` write above (not before) so this
        // shared borrow of `nc.walk_cache`/`nc.id2` doesn't overlap that
        // mutation - `nc` isn't written again for the rest of this connector.
        let v_points = &nc.walk_cache;

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
            let translation = Vec3::new(v_points[loc][0], v_points[loc][1], 100.);

            // Already spawned - just move it and move on, no respawn.
            if let Some(entity) = mesg.bubble_entity {
                if let Ok(mut transform) = query_bubble_transform.get_mut(entity) {
                    transform.translation = translation;
                    continue;
                }
                // Entity's gone missing somehow (shouldn't normally happen) -
                // fall through and treat this message as needing a fresh spawn.
                mesg.bubble_entity = None;
            }

            let parent = commands
                .spawn((
                    SpatialBundle {
                        transform: Transform::from_translation(translation),
                        ..Default::default()
                    },
                    MessageMarker {},
                ))
                .id();
            mesg.bubble_entity = Some(parent);

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
    prof.message_path_ms += __prof_t0.elapsed().as_secs_f64() * 1000.0; // TEMPORARY
}
