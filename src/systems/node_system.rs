use crate::components::camera::BubbleCamera;
use crate::components::node::{
    DragState, NodeMarker, NodeOverlayShape, SelectedNodeMarker, TickProgressFill,
};
use crate::parser::graphv2::{GraphAttrs, Node};
use crate::resources::common_assets::CommonAssets;
use crate::resources::common_assets::ResourceType;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::systems::drag;
use bevy::text::TextLayoutInfo;
use bevy_prototype_lyon::prelude::*;

use crate::resources::graph_def::{GraphChange, NodeAdded, NodeRemoved};
use crate::resources::narration::PendingExplain;
use crate::resources::ui_state::{LastNodeClick, NodePropertiesPopup};
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_tweening::{lens::*, *};
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
    query: Query<(Entity, &NodeMarker, &Transform)>,
    mut change_reader: EventReader<GraphChange>,
    mut added_reader: EventReader<NodeAdded>,
    mut removed_reader: EventReader<NodeRemoved>,
    mut pending_explain: ResMut<PendingExplain>,
    mut sim_time: ResMut<Time<Virtual>>,
    mut node_props: ResMut<crate::resources::ui_state::NodePropertiesPopup>,
) {
    if change_reader.read().count() > 0 {
        for (entity, _, _) in query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        let mut z = 0.;
        let g_attrs: &GraphAttrs = &g.graph_defn.graph_attrs;
        let layout = crate::systems::layout::compute_graph_layout(&g.graph_defn);
        for node in g.graph_defn.node_instances.iter() {
            let pos = layout.node_positions.get(&node.name).copied().unwrap_or(Vec2::ZERO);
            let locked = !layout.draggable;
            spawn_node(z, node, &mut commands, &ca, g_attrs, pos, locked);
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
        // A fresh YAML load may have removed/renamed the node the popup refers
        // to (or none at all, but there's no state worth preserving across a
        // full rebuild either way).
        *node_props = crate::resources::ui_state::NodePropertiesPopup::default();
        return;
    }

    for removed in removed_reader.read() {
        if let Some((entity, _, _)) = query.iter().find(|(_, m, _)| m.node_name == removed.name) {
            commands.entity(entity).despawn_recursive();
        }
        // A script-driven despawn() of the node the popup is currently
        // showing shouldn't leave it open pointing at nothing.
        if node_props.node_name.as_deref() == Some(removed.name.as_str()) {
            *node_props = crate::resources::ui_state::NodePropertiesPopup::default();
        }
    }

    if added_reader.is_empty() {
        return;
    }
    let g_attrs: &GraphAttrs = &g.graph_defn.graph_attrs;
    // Ground truth for "where is everyone right now" - not the freshly
    // recomputed `layout` below, which assumes every node's position gets
    // updated to match it. We deliberately only spawn the new node(s) here
    // (see this fn's doc comment: existing nodes' positions/drag state
    // survive untouched), so an existing node may already be sitting
    // somewhere the fresh layout no longer agrees with (a manual drag, or
    // simply because adding this node reshuffled its rank layer's spacing).
    // Steering the new node away from these actual on-screen positions -
    // instead of blindly trusting the recomputed one - is what keeps it
    // from landing on top of a node that isn't going to move to make room.
    let mut existing_positions: Vec<Vec2> =
        query.iter().map(|(_, _, t)| t.translation.truncate()).collect();
    let mut z = existing_positions.len() as f32;
    let layout = crate::systems::layout::compute_graph_layout(&g.graph_defn);
    for added in added_reader.read() {
        if let Some(node) = g
            .graph_defn
            .node_instances
            .iter()
            .find(|n| n.name == added.name)
        {
            let desired = layout.node_positions.get(&node.name).copied().unwrap_or(Vec2::ZERO);
            let pos = resolve_spawn_position(desired, &existing_positions, existing_positions.len());
            // Also steer any further nodes added this same frame away from
            // this one, not just from what was already on screen.
            existing_positions.push(pos);
            let locked = !layout.draggable;
            spawn_node(z, node, &mut commands, &ca, g_attrs, pos, locked);
            z += 1.;
        }
    }
}

/// A freshly-spawned node's layout-computed position can coincide with (or
/// sit too close to) an already-placed node's *actual* on-screen position -
/// see `create_nodes`' `NodeAdded` branch for why those can disagree. If
/// `desired` is too close to anything in `existing_positions`, nudge it
/// outward along a golden-angle spiral (evenly distributes points without
/// clustering, unlike a plain grid or random jitter) until it clears every
/// existing node, or give up after a bounded number of attempts and return
/// the last candidate rather than looping forever.
fn resolve_spawn_position(desired: Vec2, existing_positions: &[Vec2], node_count: usize) -> Vec2 {
    const MIN_SEPARATION: f32 = 160.0;
    const GOLDEN_ANGLE: f32 = 2.399963; // radians; ~137.5 degrees
    const MAX_ATTEMPTS: u32 = 24;

    let collides = |p: Vec2| existing_positions.iter().any(|&e| e.distance(p) < MIN_SEPARATION);
    if !collides(desired) {
        return desired;
    }

    let base_radius = spawn_spread_radius(node_count);
    let mut candidate = desired;
    for attempt in 1..=MAX_ATTEMPTS {
        let angle = attempt as f32 * GOLDEN_ANGLE;
        let radius = base_radius * (1.0 + attempt as f32 * 0.15);
        candidate = desired + Vec2::new(angle.cos(), angle.sin()) * radius;
        if !collides(candidate) {
            return candidate;
        }
    }
    candidate
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

/// Two clicks on empty background within this many (real-time) seconds opens
/// `GraphPropertiesOpen` - the background equivalent of a node's
/// `DOUBLE_CLICK_SECS` double-click (see `on_click`), since there's no gear
/// button or other summon control any more. (Closing is exclusively via
/// `ui::graph_properties_viewer`'s click-elsewhere handling, not a second
/// double-click - see that function's doc comment for why toggling here
/// was flaky.)
const BACKGROUND_DOUBLE_CLICK_SECS: f32 = 0.4;

pub fn open_graph_properties_on_background_double_click(
    mut contexts: bevy_egui::EguiContexts,
    mouse: Res<ButtonInput<MouseButton>>,
    hover_map: Res<bevy_mod_picking::focus::HoverMap>,
    parents: Query<&Parent>,
    is_node: Query<(), With<NodeMarker>>,
    real_time: Res<Time<Real>>,
    mut last_click_at: Local<f32>,
    mut open: ResMut<crate::resources::ui_state::GraphPropertiesOpen>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    // Don't count a click that landed on an egui window (Graph Properties
    // itself, a Node Properties popup, ...) as a background click - only
    // bevy_mod_picking's own sprite hit-testing is checked below, which
    // can't see egui at all, so without this a click on top of any open
    // window would look identical to a background click from here.
    if contexts.ctx_mut().is_pointer_over_area() {
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
    if hit_node {
        return;
    }

    let now = real_time.elapsed_seconds();
    let is_double_click = now - *last_click_at <= BACKGROUND_DOUBLE_CLICK_SECS;
    if is_double_click {
        // Always open (never toggle-close from here) - a real double-click
        // gesture can easily register as *three* button-up events rather
        // than two (mouse hardware/driver bounce, or the browser's own
        // click coalescing), which with a toggle meant the third click
        // flipped it straight back closed - the window would flash open
        // and disappear in what looked like one interaction. Resetting the
        // timer here (rather than to `now`) also means that spurious third
        // click can't chain into a second match against this same pair.
        open.open = true;
        open.click_pos = windows.get_single().ok().and_then(Window::cursor_position);
        *last_click_at = f32::NEG_INFINITY;
    } else {
        *last_click_at = now;
    }
}

/// Two clicks on the same node within this many (real-time) seconds count as
/// a double-click - opens `NodePropertiesPopup` (see that resource's doc
/// comment; `bevy_mod_picking`'s `Pointer<Click>` has no built-in click-count
/// to detect this from directly).
const DOUBLE_CLICK_SECS: f32 = 0.4;

pub fn on_click(
    e: Listener<Pointer<Click>>,
    mut commands: Commands,
    mut q_selected: Query<(Entity, &SelectedNodeMarker)>,
    mut q: Query<(Entity, &mut Transform, &mut Children, &NodeMarker)>,
    text_query: Query<(&Transform, &TextLayoutInfo, Option<&NodeOverlayShape>), Without<NodeMarker>>,
    real_time: Res<Time<Real>>,
    mut last_click: ResMut<LastNodeClick>,
    mut node_props: ResMut<NodePropertiesPopup>,
    query_camera: Query<(&Camera, &GlobalTransform), (With<Camera2d>, Without<BubbleCamera>)>,
    g: Res<GraphDefinitionRes>,
) {
    if q.iter().count() == 0 {
        return;
    }
    for (entity, _) in q_selected.iter_mut() {
        commands.entity(entity).despawn_recursive();
    }

    for (entity, transform, children, node) in q.iter_mut() {
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
            let now = real_time.elapsed_seconds();
            let is_double_click = last_click.node_name.as_deref() == Some(node.node_name.as_str())
                && now - last_click.at <= DOUBLE_CLICK_SECS;
            last_click.node_name = Some(node.node_name.clone());
            last_click.at = now;
            if is_double_click {
                // Screen position of the node *right now*, so the popup opens
                // anchored just above wherever the node actually is (it may
                // have drifted since spawn - drag, pulse, pan/zoom all move
                // it in screen space independent of world position).
                let screen_pos = query_camera.get_single().ok().and_then(|(cam, cam_transform)| {
                    cam.world_to_viewport(cam_transform, transform.translation)
                });
                node_props.node_name = Some(node.node_name.clone());
                node_props.anchor_screen_pos = screen_pos;
            }

            // Only the node's own default name-label text child (never an
            // overlay's own Text shape - see the query's other caller below
            // for why that distinction matters) contributes here.
            let mut text_rect = Vec2::default();
            for child in children.iter() {
                if let Ok((_, text, None)) = text_query.get(*child) {
                    text_rect = text.logical_size;
                }
            }

            // The default icon+label footprint only actually applies while
            // it's actually on screen - a node type with a template (see
            // Attrs::template's doc comment) hides both in favor of its own
            // overlay entirely, so including this box for one would highlight
            // empty space no shape is actually drawn in.
            let this_node = g
                .graph_defn
                .node_instances
                .iter()
                .find(|n| n.name == node.node_name);
            let has_template = this_node.is_some_and(|n| n.node_data.attrs.template.is_some());
            let icon_label_box = (!has_template).then(|| {
                let half_x = f32::max(ICON_WIDTH, text_rect.x) / 2.0;
                let y_transform = -1.0 * (FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM) / 2.0;
                let half_y = (ICON_HEIGHT + FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM) / 2.0;
                (
                    Vec2::new(-half_x, y_transform - half_y),
                    Vec2::new(half_x, y_transform + half_y),
                )
            });

            // Whatever the node's template/draw() overlay actually occupies -
            // a template or a live draw() call can paint well outside the
            // default icon+label footprint (a wide badge, a shape gallery,
            // ...), so the highlight needs to grow to fit it rather than
            // clip it.
            //
            // `parser::draw::bounds()` has no font metrics to work with, so for a Text
            // shape specifically it can only guess a width from the string's
            // character count - a guess that overestimates badly for longer
            // strings (comfortably wide-enough for a short label balloons
            // into a needlessly huge box for a long one), which is exactly
            // what actually spawning the overlay and measuring its real,
            // already-computed `TextLayoutInfo` avoids. `spawn_shape` gives
            // a text child's `Transform.translation` the exact (x, y) its
            // `DrawCmd::Text` was authored with, so matching on that finds
            // the right child without needing any extra id/index to do it -
            // and its `logical_size` has to be scaled back down by the same
            // `TEXT_SUPERSAMPLE` factor `spawn_shape` shrank the entity's own
            // `Transform.scale` by, or this would overestimate too, just by
            // a fixed 2x instead of a growing one.
            let overlay_box = this_node.and_then(|n| {
                n.overlay
                    .iter()
                    .map(|cmd| {
                        if let crate::parser::draw::DrawCmd::Text { x, y, .. } = cmd {
                            let measured = children.iter().find_map(|c| {
                                let (t, info, overlay_marker) = text_query.get(*c).ok()?;
                                let close = overlay_marker.is_some()
                                    && (t.translation.x - x).abs() < 0.01
                                    && (t.translation.y - y).abs() < 0.01;
                                close.then(|| {
                                    let half = (info.logical_size * t.scale.truncate()) / 2.0;
                                    (
                                        t.translation.truncate() - half,
                                        t.translation.truncate() + half,
                                    )
                                })
                            });
                            measured.unwrap_or_else(|| crate::parser::draw::bounds(cmd))
                        } else {
                            crate::parser::draw::bounds(cmd)
                        }
                    })
                    .reduce(|(min1, max1), (min2, max2)| (min1.min(min2), max1.max(max2)))
            });

            let (min, max) = match (icon_label_box, overlay_box) {
                (Some((min1, max1)), Some((min2, max2))) => (min1.min(min2), max1.max(max2)),
                (Some(b), None) | (None, Some(b)) => b,
                // Neither a visible default look nor any overlay shape at
                // all (a template that resolved to zero shapes, say) - fall
                // back to a small fixed box rather than a degenerate
                // zero-size highlight.
                (None, None) => (Vec2::splat(-ICON_WIDTH / 2.0), Vec2::splat(ICON_WIDTH / 2.0)),
            };
            let padding = Vec2::splat(BOUNDING_BOX_PADDING);
            let min = min - padding;
            let max = max + padding;
            let center = (min + max) / 2.0;
            let size = max - min;

            let vec: Vec<Vec2> = vec![
                Vec2::new(-size.x / 2.0, -size.y / 2.0),
                Vec2::new(size.x / 2.0, -size.y / 2.0),
                Vec2::new(size.x / 2.0, size.y / 2.0),
                Vec2::new(-size.x / 2.0, size.y / 2.0),
            ];
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
                                translation: center.extend(0.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    Fill::color(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                    Stroke::new(Color::BLACK, 2.0),
                    SelectedNodeMarker {
                        node_name: node.node_name.clone(),
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
    target_pos: Vec2,
    locked: bool,
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

    let x = target_pos.x;
    let y = target_pos.y;

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

    let mut entity_cmds = commands.spawn((
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
    ));

    if locked {
        entity_cmds.insert(crate::components::node::LayoutLocked);
    }

    let parent = entity_cmds.id();

    // A node type with a template (`attrs.template`, set directly or via
    // `template_ref` - already resolved to one flat list by `parse_graph2`)
    // owns its whole on-canvas look: `systems::node_overlay` draws it (icon
    // included, via a `TemplateShape::Icon`/`DrawCmd::Icon` if the template
    // has one) as overlay children of this same node entity, one tick after
    // this spawn. So the default icon sprite/name label built here are
    // skipped for it - except the icon sprite still gets spawned fully
    // transparent (`Color::NONE`), keeping the node clickable/draggable
    // (bevy_mod_picking's sprite backend needs *some* Sprite to hit-test;
    // the lyon shapes a template is usually made of aren't pickable, see
    // `node_overlay`'s module doc) even before the overlay renders, and
    // regardless of whether the template includes its own icon shape at all.
    let has_template = node_attrs.template.is_some() && !node.overlay.is_empty();

    let icon_child = commands
        .spawn((SpriteBundle {
            texture: node_icon.clone(),
            transform: Transform::from_translation(Vec3::new(0., 0., 100.)),
            sprite: Sprite {
                custom_size: Vec2::new(ICON_WIDTH, ICON_HEIGHT).into(),
                color: if has_template { Color::NONE } else { Color::WHITE },
                ..Default::default()
            },
            ..default()
        },))
        .id();

    let mut children = vec![icon_child];
    if !has_template {
        children.push(commands.spawn(txt_bndl).id());

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
                    kind: crate::components::node::ProgressKind::Bar {
                        origin_x: 0.0,
                        w: TICK_BAR_WIDTH,
                    },
                },
            ))
            .id();

        children.push(track_child);
        children.push(fill_child);
    }
    commands.entity(parent).push_children(&children);
}
