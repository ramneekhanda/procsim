use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;

use crate::components::message::*;
use crate::components::node_connector::*;
use crate::parser::graphv2::MessageBubbleShape;
use crate::resources::common_assets::CommonAssets;
use crate::resources::common_assets::ResourceType;
use crate::resources::graph_def::{GraphChange, GraphDefinitionRes};

const BUBBLE_FONT_SIZE: f32 = 16.0;
const BUBBLE_PADDING_X: f32 = 10.0;
const BUBBLE_PADDING_Y: f32 = 6.0;
const BUBBLE_CHAR_WIDTH_FACTOR: f32 = 0.55;
const BUBBLE_MIN_WIDTH: f32 = 28.0;
const BUBBLE_CORNER_RADIUS: f32 = 8.0;
const BUBBLE_ICON_SIZE: f32 = 20.0;
const BUBBLE_ICON_TEXT_GAP: f32 = 4.0;

/// The travelling message itself: a small dot walking the connector path.
/// The speech bubble (built below) floats above it at a fixed offset, with a
/// pointer aimed back down at the dot, comic-panel style.
const DOT_RADIUS: f32 = 5.0; // 10px-wide dot
/// Gap between the dot's edge and the pointer's apex.
const BUBBLE_GAP: f32 = 10.0;
const BUBBLE_POINTER_WIDTH: f32 = 10.0;
const BUBBLE_POINTER_HEIGHT: f32 = 10.0;
/// How far the pointer's fill triangle pushes up into the bubble body, to
/// mask the seam between the two shapes - same trick `explain_bubble` uses.
const BUBBLE_POINTER_OVERLAP: f32 = 2.0;

/// Animates each in-flight message's bubble along its connector's cached
/// walk points (`NodeConnector::walk_cache`).
///
/// Each message's bubble entity tree (rounded-rect/pill/box/chamfered + icon + text) is spawned
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

    let msg_theme = gd.graph_defn.graph_attrs.message_theme.as_ref();
    let shape_kind = msg_theme.map(|t| t.shape).unwrap_or_default();
    let bg_color = msg_theme
        .and_then(|t| t.bg)
        .unwrap_or(gd.graph_defn.graph_attrs.background);
    let stroke_color = msg_theme
        .and_then(|t| t.stroke)
        .unwrap_or(gd.graph_defn.graph_attrs.text_color);
    // The dot's own fill: falls back to the graph's connector accent rather
    // than text_color, so a graph with no message_theme still gets a
    // travelling dot that reads as a distinct "message" marker instead of
    // blending into node/label text that happens to share the default
    // text color.
    let dot_color = msg_theme
        .and_then(|t| t.stroke)
        .unwrap_or(gd.graph_defn.graph_attrs.connection_color);
    let stroke_width = msg_theme.map(|t| t.stroke_width).unwrap_or(1.5);
    let text_color = msg_theme
        .and_then(|t| t.text_color)
        .unwrap_or(gd.graph_defn.graph_attrs.text_color);
    let font_size = msg_theme.map(|t| t.font_size).unwrap_or(BUBBLE_FONT_SIZE);
    let icon_size = msg_theme.map(|t| t.icon_size).unwrap_or(BUBBLE_ICON_SIZE);

    let text_style = TextStyle {
        font: font.clone(),
        font_size,
        color: text_color,
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
            // The retain check above tolerates elapsed running up to 10ms past
            // duration before retiring a message, so `loc` can land past the
            // last valid index here. Clamp before the reversed-direction
            // subtraction below - otherwise an overshot `loc` underflows
            // `usize` instead of just being caught by the bounds check that
            // already exists for the forward-direction case.
            loc = loc.min(v_points.len());

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
                (mesg.str.chars().count() as f32) * font_size * BUBBLE_CHAR_WIDTH_FACTOR;
            let content_width = if msg_icon.is_some() {
                icon_size + BUBBLE_ICON_TEXT_GAP + text_width
            } else {
                text_width
            };
            let bubble_width = (content_width + 2.0 * BUBBLE_PADDING_X).max(BUBBLE_MIN_WIDTH);
            let bubble_height = font_size.max(icon_size) + 2.0 * BUBBLE_PADDING_Y;
            let half = Vec2::new(bubble_width / 2.0, bubble_height / 2.0);

            let path = match shape_kind {
                MessageBubbleShape::Rounded => GeometryBuilder::build_as(&shapes::RoundedPolygon {
                    points: vec![
                        Vec2::new(-half.x, -half.y),
                        Vec2::new(half.x, -half.y),
                        Vec2::new(half.x, half.y),
                        Vec2::new(-half.x, half.y),
                    ],
                    radius: BUBBLE_CORNER_RADIUS,
                    ..shapes::RoundedPolygon::default()
                }),
                MessageBubbleShape::Pill => GeometryBuilder::build_as(&shapes::RoundedPolygon {
                    points: vec![
                        Vec2::new(-half.x, -half.y),
                        Vec2::new(half.x, -half.y),
                        Vec2::new(half.x, half.y),
                        Vec2::new(-half.x, half.y),
                    ],
                    radius: half.y.min(half.x),
                    ..shapes::RoundedPolygon::default()
                }),
                MessageBubbleShape::Box => GeometryBuilder::build_as(&shapes::RoundedPolygon {
                    points: vec![
                        Vec2::new(-half.x, -half.y),
                        Vec2::new(half.x, -half.y),
                        Vec2::new(half.x, half.y),
                        Vec2::new(-half.x, half.y),
                    ],
                    radius: 0.0,
                    ..shapes::RoundedPolygon::default()
                }),
                MessageBubbleShape::Chamfered => {
                    let cut = (half.y * 0.45).min(8.0);
                    GeometryBuilder::build_as(&shapes::Polygon {
                        points: vec![
                            Vec2::new(-half.x + cut, -half.y),
                            Vec2::new(half.x - cut, -half.y),
                            Vec2::new(half.x, -half.y + cut),
                            Vec2::new(half.x, half.y - cut),
                            Vec2::new(half.x - cut, half.y),
                            Vec2::new(-half.x + cut, half.y),
                            Vec2::new(-half.x, half.y - cut),
                            Vec2::new(-half.x, -half.y + cut),
                        ],
                        closed: true,
                    })
                }
            };

            // Dot sits at the parent's own origin (the actual walked
            // position); the bubble floats above it at a fixed offset, with
            // a pointer triangle aimed back down at the dot.
            let apex_y = DOT_RADIUS + BUBBLE_GAP;
            let bubble_bottom_y = apex_y + BUBBLE_POINTER_HEIGHT;
            let bubble_center_y = bubble_bottom_y + half.y;

            let dot_child = commands
                .spawn((
                    ShapeBundle {
                        path: GeometryBuilder::build_as(&shapes::Circle {
                            radius: DOT_RADIUS,
                            center: Vec2::ZERO,
                        }),
                        spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 0.5)),
                        ..default()
                    },
                    Fill::color(dot_color),
                    Stroke::new(bg_color, 1.5),
                ))
                .id();

            let bubble_child = commands
                .spawn((
                    ShapeBundle {
                        path,
                        spatial: SpatialBundle::from_transform(Transform::from_xyz(
                            0.0,
                            bubble_center_y,
                            1.0,
                        )),
                        ..default()
                    },
                    Fill::color(bg_color),
                    Stroke::new(stroke_color, stroke_width),
                ))
                .id();

            // Fill triangle overlapping into the bubble body masks the seam;
            // a separate open (unclosed) stroke polyline draws only the two
            // visible legs, so the pointer reads as part of the bubble's own
            // outline rather than a stitched-on shape.
            let pointer_fill_child = commands
                .spawn((
                    ShapeBundle {
                        path: GeometryBuilder::build_as(&shapes::Polygon {
                            points: vec![
                                Vec2::new(
                                    -BUBBLE_POINTER_WIDTH / 2.0,
                                    bubble_bottom_y + BUBBLE_POINTER_OVERLAP,
                                ),
                                Vec2::new(0.0, apex_y),
                                Vec2::new(
                                    BUBBLE_POINTER_WIDTH / 2.0,
                                    bubble_bottom_y + BUBBLE_POINTER_OVERLAP,
                                ),
                            ],
                            closed: true,
                        }),
                        spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 1.1)),
                        ..default()
                    },
                    Fill::color(bg_color),
                ))
                .id();

            let pointer_stroke_child = commands
                .spawn((
                    ShapeBundle {
                        path: GeometryBuilder::build_as(&shapes::Polygon {
                            points: vec![
                                Vec2::new(-BUBBLE_POINTER_WIDTH / 2.0, bubble_bottom_y),
                                Vec2::new(0.0, apex_y),
                                Vec2::new(BUBBLE_POINTER_WIDTH / 2.0, bubble_bottom_y),
                            ],
                            closed: false,
                        }),
                        spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 1.2)),
                        ..default()
                    },
                    Stroke::new(stroke_color, stroke_width),
                ))
                .id();

            let content_left = -content_width / 2.0;
            let text_x = if let Some(icon) = &msg_icon {
                let icon_x = content_left + icon_size / 2.0;
                commands.entity(parent).with_children(|p| {
                    p.spawn(SpriteBundle {
                        texture: icon.clone(),
                        transform: Transform::from_translation(Vec3::new(
                            icon_x,
                            bubble_center_y,
                            1.3,
                        )),
                        sprite: Sprite {
                            custom_size: Vec2::new(icon_size, icon_size).into(),
                            ..Default::default()
                        },
                        ..default()
                    });
                });
                content_left + icon_size + BUBBLE_ICON_TEXT_GAP + text_width / 2.0
            } else {
                0.0
            };

            let text_child = commands
                .spawn(Text2dBundle {
                    text: Text::from_section(mesg.str.clone(), text_style.clone())
                        .with_justify(JustifyText::Center),
                    transform: Transform::from_translation(Vec3::new(text_x, bubble_center_y, 1.3)),
                    ..default()
                })
                .id();

            commands.entity(parent).push_children(&[
                dot_child,
                bubble_child,
                pointer_fill_child,
                pointer_stroke_child,
                text_child,
            ]);
        }
    }
    prof.message_path_ms += __prof_t0.elapsed().as_secs_f64() * 1000.0; // TEMPORARY
}

/// Despawns every in-flight message's bubble and clears both message queues
/// on every connector when the graph reloads (`GraphChange`).
///
/// A message bubble is a free-standing entity (spawned in
/// `update_message_path` above, parented to nothing) - it's tracked only via
/// `Message::bubble_entity` inside a connector's `Messages` component, not
/// via Bevy's entity hierarchy. `node_system::create_nodes` despawns node
/// entities on `GraphChange` and `update_connectors` diffs connector entities
/// against the newly-loaded graph's edges, but neither of those touches a
/// bubble entity directly - so without this, a bubble left in flight at
/// reload time was simply orphaned (never despawned, since nothing else's
/// cleanup reaches it) rather than cleared, and a connector that happened to
/// share both endpoint names with an edge in the new graph would silently
/// carry over its old `Messages` state (including now-meaningless
/// `bubble_entity` references) instead of starting fresh.
pub fn cleanup_messages_on_graph_change(
    mut commands: Commands,
    mut change_reader: EventReader<GraphChange>,
    mut connectors: Query<&mut Messages>,
    bubbles: Query<Entity, With<MessageMarker>>,
) {
    if change_reader.read().count() == 0 {
        return;
    }
    for entity in bubbles.iter() {
        commands.entity(entity).despawn_recursive();
    }
    for mut messages in connectors.iter_mut() {
        messages.msg_inflight.clear();
        messages.msg_delivered.clear();
    }
}
