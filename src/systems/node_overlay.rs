//! Renders each node's custom `draw()` overlay (see `parser::draw`).
//!
//! A node's Rhai handler sets `Node::overlay` via the `draw([...])` host function
//! (handled in `rhai_engine`), which also raises `Node::overlay_dirty`. This system
//! picks up dirty nodes, despawns their existing overlay shape entities, and
//! respawns them from the current command list as children of the node entity -
//! the same despawn-and-respawn-all approach used by `create_nodes` /
//! `update_connectors`. Being children of the node entity, overlay shapes track
//! the node under drag, pan and zoom for free.
//!
//! `spawn_shape` (below) builds a shape entity from one `DrawCmd` without opinion
//! about what marks or owns it - callers attach their own marker component and
//! parent it wherever makes sense. `render_node_overlays` uses it for the
//! always-on per-node overlay; `explain_bubble` reuses it for narration bubbles.

use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;

use crate::components::node::{NodeMarker, NodeOverlayShape, ProgressKind, TickProgressFill};
use crate::parser::draw::{DrawCmd, DrawProgressStyle, Paint};
use crate::resources::common_assets::{CommonAssets, ResourceType};
use crate::resources::graph_def::GraphDefinitionRes;

/// Local z of overlay shapes, relative to the node entity. Above the icon/label
/// children (which sit at 100/101).
const OVERLAY_Z: f32 = 120.0;
/// Fallback fill for a shape that specifies neither `fill` nor `stroke`.
const DEFAULT_FILL: Color = Color::srgb(0.6, 0.6, 0.6);

/// A `DrawCmd::Text` is rasterized at `size * TEXT_SUPERSAMPLE` px and the
/// entity's `Transform` scaled down by the same factor to compensate, so it
/// still occupies exactly `size` world units - but the glyph atlas (and the
/// anti-aliasing baked into it) is generated at twice the resolution before
/// being shrunk back down by the GPU's (linearly-filtered, by this app's
/// default `ImagePlugin`) texture sampling. Bevy's text layout rasterizes a
/// glyph atlas at exactly the requested `font_size` with no supersampling of
/// its own, so a small `draw()`/template label (the overlay/badge/narration
/// text this module renders - the default per-node name label, spawned
/// separately by `node_system::spawn_node` at a much larger fixed size, never
/// went through this path and isn't affected) reads visibly softer than
/// larger text at the same anti-aliasing quality; this halves that softness
/// for free with no schema/author-visible change. Any caller that also
/// constrains a `spawn_shape`-built `Text2dBounds` (word-wrap) itself, as
/// `explain_bubble` does, must scale that bounds size by this same factor -
/// wrapping is computed in the *unscaled* glyph-layout space, before this
/// function's compensating `Transform` scale is applied.
pub const TEXT_SUPERSAMPLE: f32 = 2.0;

pub fn render_node_overlays(
    mut commands: Commands,
    mut gd: ResMut<GraphDefinitionRes>,
    ca: Res<CommonAssets>,
    nodes: Query<(Entity, &NodeMarker)>,
    existing: Query<(Entity, &NodeOverlayShape)>,
    // TEMPORARY - see systems::profiling's doc comment.
    mut prof: ResMut<crate::systems::profiling::ProfilingStats>,
) {
    let __prof_t0 = web_time::Instant::now(); // TEMPORARY
    (|| {
        let dirty: Vec<String> = gd
            .graph_defn
            .node_instances
            .iter()
            .filter(|n| n.overlay_dirty)
            .map(|n| n.name.clone())
            .collect();
        if dirty.is_empty() {
            return;
        }

        let mut font: Handle<Font> = Default::default();
        if let Some(ResourceType::FontHandle(f)) = ca.resource_map.get("default_font") {
            font = f.clone();
        }

        // Only the names this pass actually rendered get their `overlay_dirty`
        // cleared below - a node whose entity hasn't been spawned yet this
        // frame (e.g. a template-seeded overlay - see `graphv2::instantiate_node`
        // - racing `node_system::create_nodes` on the very same frame a graph
        // loads) would otherwise have its flag cleared here having never been
        // drawn at all, silently dropping the overlay for good. Left dirty, it
        // simply tries again next frame once the entity exists.
        let mut rendered = Vec::with_capacity(dirty.len());

        for name in &dirty {
            for (entity, marker) in existing.iter() {
                if &marker.node_name == name {
                    commands.entity(entity).despawn_recursive();
                }
            }

            let Some((node_entity, _)) = nodes.iter().find(|(_, m)| &m.node_name == name) else {
                continue;
            };
            let Some(node) = gd
                .graph_defn
                .node_instances
                .iter()
                .find(|n| &n.name == name)
            else {
                continue;
            };

            for (i, cmd) in node.overlay.iter().enumerate() {
                let z = OVERLAY_Z + i as f32 * 0.01;
                let child = spawn_shape(&mut commands, cmd, z, &font, &ca, name)
                    .insert(NodeOverlayShape {
                        node_name: name.clone(),
                    })
                    .id();
                commands.entity(node_entity).add_child(child);
            }
            rendered.push(name.clone());
        }

        for node in gd.graph_defn.node_instances.iter_mut() {
            if rendered.contains(&node.name) {
                node.overlay_dirty = false;
            }
        }
    })(); // TEMPORARY
    prof.overlays_ms += __prof_t0.elapsed().as_secs_f64() * 1000.0; // TEMPORARY
}

fn apply_paint(ec: &mut EntityCommands, paint: &Paint) {
    match (paint.fill, paint.stroke) {
        (None, None) => {
            ec.insert(Fill::color(DEFAULT_FILL));
        }
        (fill, stroke) => {
            if let Some(c) = fill {
                ec.insert(Fill::color(c));
            }
            if let Some(c) = stroke {
                ec.insert(Stroke::new(c, paint.stroke_width));
            }
        }
    }
}

/// Spawns one shape entity for `cmd` at local z `z`, with no marker or parent of
/// its own - the caller inserts whatever marker component ties it to their
/// lifecycle and parents it wherever it belongs. `ca` is only consulted for a
/// `DrawCmd::Icon` (to resolve its icon id to a texture); every other shape
/// ignores it.
pub(crate) fn spawn_shape<'a>(
    commands: &'a mut Commands,
    cmd: &DrawCmd,
    z: f32,
    font: &Handle<Font>,
    ca: &CommonAssets,
    node_name: &str,
) -> EntityCommands<'a> {
    match cmd {
        DrawCmd::Rect {
            x,
            y,
            w,
            h,
            radius,
            paint,
        } => {
            let at = Transform::from_xyz(*x, *y, z);
            let mut ec = if *radius > 0.0 {
                let (hw, hh) = (w / 2.0, h / 2.0);
                let shape = shapes::RoundedPolygon {
                    points: vec![
                        Vec2::new(-hw, -hh),
                        Vec2::new(hw, -hh),
                        Vec2::new(hw, hh),
                        Vec2::new(-hw, hh),
                    ],
                    radius: *radius,
                    ..shapes::RoundedPolygon::default()
                };
                commands.spawn(ShapeBundle {
                    path: GeometryBuilder::build_as(&shape),
                    spatial: SpatialBundle::from_transform(at),
                    ..default()
                })
            } else {
                let shape = shapes::Rectangle {
                    extents: Vec2::new(*w, *h),
                    origin: shapes::RectangleOrigin::Center,
                    ..shapes::Rectangle::default()
                };
                commands.spawn(ShapeBundle {
                    path: GeometryBuilder::build_as(&shape),
                    spatial: SpatialBundle::from_transform(at),
                    ..default()
                })
            };
            apply_paint(&mut ec, paint);
            ec
        }

        DrawCmd::Circle { x, y, r, paint } => {
            let shape = shapes::Circle {
                radius: *r,
                center: Vec2::ZERO,
            };
            let mut ec = commands.spawn(ShapeBundle {
                path: GeometryBuilder::build_as(&shape),
                spatial: SpatialBundle::from_transform(Transform::from_xyz(*x, *y, z)),
                ..default()
            });
            apply_paint(&mut ec, paint);
            ec
        }

        DrawCmd::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => {
            let shape = shapes::Line(Vec2::new(*x1, *y1), Vec2::new(*x2, *y2));
            commands.spawn((
                ShapeBundle {
                    path: GeometryBuilder::build_as(&shape),
                    spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, z)),
                    ..default()
                },
                Stroke::new(*color, *width),
            ))
        }

        DrawCmd::Polygon {
            points,
            closed,
            paint,
        } => {
            let shape = shapes::Polygon {
                points: points.clone(),
                closed: *closed,
            };
            let mut ec = commands.spawn(ShapeBundle {
                path: GeometryBuilder::build_as(&shape),
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, z)),
                ..default()
            });
            apply_paint(&mut ec, paint);
            ec
        }

        DrawCmd::Text {
            x,
            y,
            text,
            size,
            color,
        } => commands.spawn(Text2dBundle {
            text: Text::from_section(
                text.clone(),
                TextStyle {
                    font: font.clone(),
                    font_size: *size * TEXT_SUPERSAMPLE,
                    color: *color,
                },
            )
            .with_justify(JustifyText::Center),
            transform: Transform::from_xyz(*x, *y, z)
                .with_scale(Vec3::splat(1.0 / TEXT_SUPERSAMPLE)),
            ..default()
        }),

        DrawCmd::Icon { x, y, w, h, icon } => {
            // Same fallback-to-default-system-icon lookup
            // `node_system::spawn_node` uses for a node's built-in icon sprite.
            let mut texture: Handle<Image> = Default::default();
            if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get("default_system_icon")
            {
                texture = img.clone();
            }
            if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get(icon) {
                texture = img.clone();
            }
            commands.spawn(SpriteBundle {
                texture,
                transform: Transform::from_xyz(*x, *y, z),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(*w, *h)),
                    ..Default::default()
                },
                ..default()
            })
        }

        DrawCmd::Progress {
            x,
            y,
            style,
            track_color,
            fill_color,
        } => {
            let track_c = track_color.unwrap_or(Color::srgba(0.0, 0.0, 0.0, 0.25));
            let fill_c = fill_color.unwrap_or(Color::srgb(0.2, 0.6, 1.0));
            let mut children_ids = Vec::new();

            match style {
                DrawProgressStyle::Bar { w, h, radius } => {
                    if *radius > 0.0 {
                        let (hw, hh) = (w / 2.0, h / 2.0);
                        let shape = shapes::RoundedPolygon {
                            points: vec![
                                Vec2::new(-hw, -hh),
                                Vec2::new(hw, -hh),
                                Vec2::new(hw, hh),
                                Vec2::new(-hw, hh),
                            ],
                            radius: *radius,
                            ..shapes::RoundedPolygon::default()
                        };
                        let t = commands
                            .spawn((
                                ShapeBundle {
                                    path: GeometryBuilder::build_as(&shape),
                                    spatial: SpatialBundle::from_transform(Transform::from_xyz(
                                        *x, *y, 0.0,
                                    )),
                                    ..default()
                                },
                                Fill::color(track_c),
                            ))
                            .id();
                        let f = commands
                            .spawn((
                                SpriteBundle {
                                    transform: Transform::from_xyz(*x, *y, 0.01),
                                    sprite: Sprite {
                                        color: fill_c,
                                        custom_size: Some(Vec2::new(*w, *h)),
                                        ..default()
                                    },
                                    ..default()
                                },
                                TickProgressFill {
                                    node_name: node_name.to_string(),
                                    kind: ProgressKind::Bar {
                                        origin_x: *x,
                                        w: *w,
                                    },
                                },
                            ))
                            .id();
                        children_ids.push(t);
                        children_ids.push(f);
                    } else {
                        let t = commands
                            .spawn(SpriteBundle {
                                transform: Transform::from_xyz(*x, *y, 0.0),
                                sprite: Sprite {
                                    color: track_c,
                                    custom_size: Some(Vec2::new(*w, *h)),
                                    ..default()
                                },
                                ..default()
                            })
                            .id();
                        let f = commands
                            .spawn((
                                SpriteBundle {
                                    transform: Transform::from_xyz(*x, *y, 0.01),
                                    sprite: Sprite {
                                        color: fill_c,
                                        custom_size: Some(Vec2::new(*w, *h)),
                                        ..default()
                                    },
                                    ..default()
                                },
                                TickProgressFill {
                                    node_name: node_name.to_string(),
                                    kind: ProgressKind::Bar {
                                        origin_x: *x,
                                        w: *w,
                                    },
                                },
                            ))
                            .id();
                        children_ids.push(t);
                        children_ids.push(f);
                    }
                }
                DrawProgressStyle::Ring {
                    r,
                    thickness,
                    start_angle,
                    clockwise,
                } => {
                    let track_shape = shapes::Circle {
                        radius: *r,
                        center: Vec2::ZERO,
                    };
                    let t = commands
                        .spawn((
                            ShapeBundle {
                                path: GeometryBuilder::build_as(&track_shape),
                                spatial: SpatialBundle::from_transform(Transform::from_xyz(
                                    *x, *y, 0.0,
                                )),
                                ..default()
                            },
                            Stroke::new(track_c, *thickness),
                        ))
                        .id();

                    let mut pb = PathBuilder::new();
                    let start_rad = start_angle.to_radians();
                    let start_pt =
                        Vec2::new(*x, *y) + Vec2::new(r * start_rad.cos(), r * start_rad.sin());
                    let sweep_sign = if *clockwise { -1.0 } else { 1.0 };
                    let sweep_angle = sweep_sign * 360.0_f32.to_radians();
                    pb.move_to(start_pt);
                    pb.arc(Vec2::new(*x, *y), Vec2::splat(*r), sweep_angle, 0.0);

                    let f = commands
                        .spawn((
                            ShapeBundle {
                                path: pb.build(),
                                spatial: SpatialBundle::from_transform(Transform::from_xyz(
                                    0.0, 0.0, 0.01,
                                )),
                                ..default()
                            },
                            Stroke::new(fill_c, *thickness),
                            TickProgressFill {
                                node_name: node_name.to_string(),
                                kind: ProgressKind::Ring {
                                    center: Vec2::new(*x, *y),
                                    r: *r,
                                    start_angle: *start_angle,
                                    clockwise: *clockwise,
                                },
                            },
                        ))
                        .id();
                    children_ids.push(t);
                    children_ids.push(f);
                }
                DrawProgressStyle::Pie {
                    r,
                    start_angle,
                    clockwise,
                } => {
                    let track_shape = shapes::Circle {
                        radius: *r,
                        center: Vec2::ZERO,
                    };
                    let t = commands
                        .spawn((
                            ShapeBundle {
                                path: GeometryBuilder::build_as(&track_shape),
                                spatial: SpatialBundle::from_transform(Transform::from_xyz(
                                    *x, *y, 0.0,
                                )),
                                ..default()
                            },
                            Fill::color(track_c),
                        ))
                        .id();

                    let mut pb = PathBuilder::new();
                    let start_rad = start_angle.to_radians();
                    let start_pt =
                        Vec2::new(*x, *y) + Vec2::new(r * start_rad.cos(), r * start_rad.sin());
                    let sweep_sign = if *clockwise { -1.0 } else { 1.0 };
                    let sweep_angle = sweep_sign * 360.0_f32.to_radians();
                    pb.move_to(Vec2::new(*x, *y));
                    pb.line_to(start_pt);
                    pb.arc(Vec2::new(*x, *y), Vec2::splat(*r), sweep_angle, 0.0);
                    pb.close();

                    let f = commands
                        .spawn((
                            ShapeBundle {
                                path: pb.build(),
                                spatial: SpatialBundle::from_transform(Transform::from_xyz(
                                    0.0, 0.0, 0.01,
                                )),
                                ..default()
                            },
                            Fill::color(fill_c),
                            TickProgressFill {
                                node_name: node_name.to_string(),
                                kind: ProgressKind::Pie {
                                    center: Vec2::new(*x, *y),
                                    r: *r,
                                    start_angle: *start_angle,
                                    clockwise: *clockwise,
                                },
                            },
                        ))
                        .id();
                    children_ids.push(t);
                    children_ids.push(f);
                }
                DrawProgressStyle::Segmented {
                    w,
                    h,
                    segments,
                    gap,
                } => {
                    let count = (*segments).max(1);
                    let seg_w = (w - (count - 1) as f32 * gap) / count as f32;
                    let start_x = *x - w / 2.0 + seg_w / 2.0;

                    for i in 0..count {
                        let cur_x = start_x + i as f32 * (seg_w + gap);
                        let seg_id = commands
                            .spawn((
                                SpriteBundle {
                                    transform: Transform::from_xyz(cur_x, *y, 0.01),
                                    sprite: Sprite {
                                        color: fill_c,
                                        custom_size: Some(Vec2::new(seg_w, *h)),
                                        ..default()
                                    },
                                    ..default()
                                },
                                TickProgressFill {
                                    node_name: node_name.to_string(),
                                    kind: ProgressKind::Segmented {
                                        segment_index: i,
                                        total_segments: count,
                                        active_color: fill_c,
                                        inactive_color: track_c,
                                    },
                                },
                            ))
                            .id();
                        children_ids.push(seg_id);
                    }
                }
            }

            let mut parent = commands.spawn(SpatialBundle::from_transform(Transform::from_xyz(
                0.0, 0.0, z,
            )));
            parent.push_children(&children_ids);
            parent
        }
    }
}
