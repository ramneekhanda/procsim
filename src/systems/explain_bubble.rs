//! Renders `explain()` calls as an in-canvas, pausing narration bubble - see
//! `rhai_engine`'s `explain` host fn and `resources::narration::PendingExplain`.
//!
//! At most one bubble is ever shown at once: while `PendingExplain.queue` is
//! non-empty and no bubble is currently active, `show_next_explain` pops the
//! front entry, pauses `Time<Virtual>` (which freezes ticks, in-flight messages
//! and tweened animations together, since they all read the generic `Time`
//! resource outside `FixedUpdate`), and spawns the bubble as a child of the node
//! that called `explain()` - it inherits that node's transform for free, so it
//! tracks drag/pan/zoom exactly like a `draw()` overlay. Clicking the Continue
//! button (`dismiss_explain_bubble`) despawns it and either shows the next queued
//! entry (still paused) or unpauses once the queue is empty.
//!
//! Every entity spawned here carries `RenderLayers::layer(1)`, so only the second
//! camera (`components::camera::BubbleCamera`, composited on top of the main one -
//! see `main::setup_camera`) renders it. That's what keeps the bubble and its text
//! sharp while `systems::radial_blur` blurs the world camera's output underneath -
//! the blur is a post-process on the main camera's pass only, so anything the
//! bubble camera draws afterward is untouched by it.
//!
//! The bubble's own pop-in animation deliberately does *not* go through
//! `bevy_tweening`'s `Animator` (used elsewhere, e.g. the tick pulse) - that
//! reads the generic `Time` resource, which mirrors the now-paused
//! `Time<Virtual>`, so a tween started the same frame we pause would freeze at
//! its start value forever (an invisible, scale-0.01 bubble - the bug this
//! comment is here so nobody reintroduces). `animate_bubble_pop` instead reads
//! `Time<Real>`, which keeps advancing regardless of the virtual-time pause -
//! correct anyway, since UI chrome narrating a frozen world should itself stay
//! alive.

use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::text::Text2dBounds;
use bevy_mod_picking::prelude::*;
use bevy_prototype_lyon::prelude::*;

use crate::components::node::{ExplainBubble, NodeMarker};
use crate::parser::draw::{DrawCmd, Paint};
use crate::parser::graphv2::MessageBubbleShape;
use crate::resources::common_assets::{CommonAssets, ResourceType};
use crate::resources::graph_def::GraphDefinitionRes;
use crate::resources::narration::PendingExplain;
use crate::systems::node_overlay::{spawn_shape, TEXT_SUPERSAMPLE};

/// Local z of the bubble root, relative to its anchor node. Comfortably above
/// the node's own icon/label/overlay children (which top out around 121) and
/// above every other node's overlay too, for any graph size we'd realistically
/// see (other nodes' root z is their spawn index, a handful at most).
const BUBBLE_Z: f32 = 500.0;
const BUBBLE_W: f32 = 320.0;
const BUBBLE_H: f32 = 172.0;
/// Local y of the bubble body's center; the pointer triangle bridges the gap
/// down to the node.
const BUBBLE_CENTER_Y: f32 = 168.0;
/// Local y of the bubble's bottom edge - the pointer's base sits here.
const BUBBLE_BOTTOM: f32 = BUBBLE_CENTER_Y - BUBBLE_H / 2.0;
const POP_IN_SECS: f32 = 0.22;
/// Matches the visible rounded-rect button's footprint (see `button_shape`).
const BUTTON_W: f32 = 104.0;
const BUTTON_H: f32 = 30.0;
/// Offset of the fake drop-shadow rect behind the body/pointer, in node-local
/// (y-up) units - centered with subtle downward drop.
const SHADOW_OFFSET: Vec2 = Vec2::new(0.0, -4.0);
/// Scale the bubble starts at before popping in - not exactly 0, so the shape
/// builder never has to deal with a zero-size geometry.
const POP_START_SCALE: f32 = 0.05;

/// Local z of the dim backdrop, relative to the anchor node - above every
/// node's own content (overlays top out around 121) but below the bubble
/// itself (500), so it reads as "behind the dialog, in front of the world".
const BACKDROP_Z: f32 = 300.0;
/// Comfortably covers the viewport at any pan/zoom a user would realistically
/// reach (PanCam here allows zooming out to 0.1x) without needing a per-frame
/// system to track the camera - it's centered on the bubble's own anchor node,
/// which is necessarily in view (the bubble is pointing at it).
const BACKDROP_SIZE: f32 = 50_000.0;
const BACKDROP_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);

const BODY_COLOR: Color = Color::srgb(0.098, 0.106, 0.137);
const BORDER_COLOR: Color = Color::srgb(0.42, 0.47, 0.78);
const ACCENT_COLOR: Color = Color::srgb(0.49, 0.55, 0.98);
const SHADOW_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.4);
const TEXT_COLOR: Color = Color::srgb(0.93, 0.94, 0.97);
const BUTTON_COLOR: Color = ACCENT_COLOR;
const BUTTON_TEXT_COLOR: Color = Color::srgb(0.98, 0.98, 1.0);

pub fn show_next_explain(
    mut commands: Commands,
    ca: Res<CommonAssets>,
    gd: Res<GraphDefinitionRes>,
    mut pending: ResMut<PendingExplain>,
    mut sim_time: ResMut<Time<Virtual>>,
    active: Query<(), With<ExplainBubble>>,
    nodes: Query<(Entity, &NodeMarker)>,
) {
    if !active.is_empty() {
        return;
    }
    let Some(entry) = pending.queue.pop_front() else {
        return;
    };
    // The anchor node may have despawned itself since `explain()` was called;
    // drop the entry rather than getting stuck waiting for a node that's gone.
    let Some((node_entity, _)) = nodes.iter().find(|(_, m)| m.node_name == entry.node_name) else {
        return;
    };

    sim_time.pause();
    pending.showing = Some(entry.node_name.clone());

    let theme = gd.graph_defn.graph_attrs.explain_theme.as_ref();
    let body_color = theme.and_then(|t| t.bg).unwrap_or(BODY_COLOR);
    let border_color = theme.and_then(|t| t.border).unwrap_or(BORDER_COLOR);
    let border_width = theme.map(|t| t.border_width).unwrap_or(1.5);
    let text_color = theme.and_then(|t| t.text_color).unwrap_or(TEXT_COLOR);
    let accent_color = theme.and_then(|t| t.accent).unwrap_or(ACCENT_COLOR);
    let button_color = accent_color;
    let button_text_color = theme
        .and_then(|t| t.button_text_color)
        .unwrap_or(BUTTON_TEXT_COLOR);
    let shadow_color = theme.and_then(|t| t.shadow_color).unwrap_or(SHADOW_COLOR);
    let backdrop_color = theme
        .and_then(|t| t.backdrop_color)
        .unwrap_or(BACKDROP_COLOR);
    let font_size = theme.map(|t| t.font_size).unwrap_or(13.5);
    let shape_kind = theme
        .map(|t| t.shape)
        .unwrap_or(MessageBubbleShape::Rounded);

    let mut font: Handle<Font> = Default::default();
    if let Some(ResourceType::FontHandle(f)) = ca.resource_map.get("default_font") {
        font = f.clone();
    }

    // Dim backdrop - a sibling of `root`, not a child of it, so it doesn't
    // inherit `root`'s pop-in scale.
    let backdrop_cmd = DrawCmd::Rect {
        x: 0.0,
        y: 0.0,
        w: BACKDROP_SIZE,
        h: BACKDROP_SIZE,
        radius: 0.0,
        paint: Paint {
            fill: Some(backdrop_color),
            stroke: None,
            stroke_width: 0.0,
        },
    };
    let backdrop = spawn_shape(&mut commands, &backdrop_cmd, BACKDROP_Z, &font, &ca, "")
        .insert((ExplainBubble, RenderLayers::layer(1)))
        .id();
    commands.entity(node_entity).add_child(backdrop);

    let root = commands
        .spawn((
            SpatialBundle::from_transform(
                Transform::from_xyz(0.0, 0.0, BUBBLE_Z).with_scale(Vec3::splat(POP_START_SCALE)),
            ),
            ExplainBubble,
            BubblePop { elapsed: 0.0 },
            RenderLayers::layer(1),
        ))
        .id();
    commands.entity(node_entity).add_child(root);

    let (ptr_w, ptr_h) = match shape_kind {
        MessageBubbleShape::Rounded => (11.0, 13.0),
        MessageBubbleShape::Box => (9.0, 10.0),
        MessageBubbleShape::Chamfered => (10.0, 13.0),
        MessageBubbleShape::Pill => (8.0, 10.0),
    };

    let shadow_pointer = DrawCmd::Polygon {
        points: vec![
            Vec2::new(-ptr_w, BUBBLE_BOTTOM) + SHADOW_OFFSET,
            Vec2::new(ptr_w, BUBBLE_BOTTOM) + SHADOW_OFFSET,
            Vec2::new(0.0, BUBBLE_BOTTOM - ptr_h) + SHADOW_OFFSET,
        ],
        closed: true,
        paint: Paint {
            fill: Some(shadow_color),
            stroke: None,
            stroke_width: 0.0,
        },
    };

    // Fills the pointer interior and overlaps slightly into the card body to seamlessly
    // mask the card's bottom border stroke between -ptr_w and +ptr_w.
    let pointer_fill = DrawCmd::Polygon {
        points: vec![
            Vec2::new(-ptr_w + 0.5, BUBBLE_BOTTOM + 2.5),
            Vec2::new(ptr_w - 0.5, BUBBLE_BOTTOM + 2.5),
            Vec2::new(0.0, BUBBLE_BOTTOM - ptr_h),
        ],
        closed: true,
        paint: Paint {
            fill: Some(body_color),
            stroke: None,
            stroke_width: 0.0,
        },
    };

    // Open polyline stroke for the left and right legs only (no horizontal line across the card).
    let pointer_stroke = DrawCmd::Polygon {
        points: vec![
            Vec2::new(-ptr_w, BUBBLE_BOTTOM),
            Vec2::new(0.0, BUBBLE_BOTTOM - ptr_h),
            Vec2::new(ptr_w, BUBBLE_BOTTOM),
        ],
        closed: false,
        paint: Paint {
            fill: None,
            stroke: Some(border_color),
            stroke_width: border_width,
        },
    };

    let (shadow_body, body, button_mesh) = match shape_kind {
        MessageBubbleShape::Rounded => {
            let s_body = DrawCmd::Rect {
                x: SHADOW_OFFSET.x,
                y: BUBBLE_CENTER_Y + SHADOW_OFFSET.y,
                w: BUBBLE_W,
                h: BUBBLE_H,
                radius: 18.0,
                paint: Paint {
                    fill: Some(shadow_color),
                    stroke: None,
                    stroke_width: 0.0,
                },
            };
            let b_body = DrawCmd::Rect {
                x: 0.0,
                y: BUBBLE_CENTER_Y,
                w: BUBBLE_W,
                h: BUBBLE_H,
                radius: 18.0,
                paint: Paint {
                    fill: Some(body_color),
                    stroke: Some(border_color),
                    stroke_width: border_width,
                },
            };
            let b_mesh = GeometryBuilder::build_as(&shapes::RoundedPolygon {
                points: vec![
                    Vec2::new(-BUTTON_W / 2.0, -BUTTON_H / 2.0),
                    Vec2::new(BUTTON_W / 2.0, -BUTTON_H / 2.0),
                    Vec2::new(BUTTON_W / 2.0, BUTTON_H / 2.0),
                    Vec2::new(-BUTTON_W / 2.0, BUTTON_H / 2.0),
                ],
                radius: 10.0,
                ..default()
            });
            (s_body, b_body, b_mesh)
        }
        MessageBubbleShape::Box => {
            let s_body = DrawCmd::Rect {
                x: SHADOW_OFFSET.x,
                y: BUBBLE_CENTER_Y + SHADOW_OFFSET.y,
                w: BUBBLE_W,
                h: BUBBLE_H,
                radius: 0.0,
                paint: Paint {
                    fill: Some(shadow_color),
                    stroke: None,
                    stroke_width: 0.0,
                },
            };
            let b_body = DrawCmd::Rect {
                x: 0.0,
                y: BUBBLE_CENTER_Y,
                w: BUBBLE_W,
                h: BUBBLE_H,
                radius: 0.0,
                paint: Paint {
                    fill: Some(body_color),
                    stroke: Some(border_color),
                    stroke_width: border_width,
                },
            };
            let b_mesh = GeometryBuilder::build_as(&shapes::RoundedPolygon {
                points: vec![
                    Vec2::new(-BUTTON_W / 2.0, -BUTTON_H / 2.0),
                    Vec2::new(BUTTON_W / 2.0, -BUTTON_H / 2.0),
                    Vec2::new(BUTTON_W / 2.0, BUTTON_H / 2.0),
                    Vec2::new(-BUTTON_W / 2.0, BUTTON_H / 2.0),
                ],
                radius: 0.0,
                ..default()
            });
            (s_body, b_body, b_mesh)
        }
        MessageBubbleShape::Chamfered => {
            let hw = BUBBLE_W / 2.0;
            let hh = BUBBLE_H / 2.0;
            let cut = 16.0;
            let s_pts = vec![
                Vec2::new(-hw + cut, BUBBLE_CENTER_Y - hh) + SHADOW_OFFSET,
                Vec2::new(hw - cut, BUBBLE_CENTER_Y - hh) + SHADOW_OFFSET,
                Vec2::new(hw, BUBBLE_CENTER_Y - hh + cut) + SHADOW_OFFSET,
                Vec2::new(hw, BUBBLE_CENTER_Y + hh - cut) + SHADOW_OFFSET,
                Vec2::new(hw - cut, BUBBLE_CENTER_Y + hh) + SHADOW_OFFSET,
                Vec2::new(-hw + cut, BUBBLE_CENTER_Y + hh) + SHADOW_OFFSET,
                Vec2::new(-hw, BUBBLE_CENTER_Y + hh - cut) + SHADOW_OFFSET,
                Vec2::new(-hw, BUBBLE_CENTER_Y - hh + cut) + SHADOW_OFFSET,
            ];
            let pts = vec![
                Vec2::new(-hw + cut, BUBBLE_CENTER_Y - hh),
                Vec2::new(hw - cut, BUBBLE_CENTER_Y - hh),
                Vec2::new(hw, BUBBLE_CENTER_Y - hh + cut),
                Vec2::new(hw, BUBBLE_CENTER_Y + hh - cut),
                Vec2::new(hw - cut, BUBBLE_CENTER_Y + hh),
                Vec2::new(-hw + cut, BUBBLE_CENTER_Y + hh),
                Vec2::new(-hw, BUBBLE_CENTER_Y + hh - cut),
                Vec2::new(-hw, BUBBLE_CENTER_Y - hh + cut),
            ];
            let s_body = DrawCmd::Polygon {
                points: s_pts,
                closed: true,
                paint: Paint {
                    fill: Some(shadow_color),
                    stroke: None,
                    stroke_width: 0.0,
                },
            };
            let b_body = DrawCmd::Polygon {
                points: pts,
                closed: true,
                paint: Paint {
                    fill: Some(body_color),
                    stroke: Some(border_color),
                    stroke_width: border_width,
                },
            };
            let b_cut = 6.0;
            let bhw = BUTTON_W / 2.0;
            let bhh = BUTTON_H / 2.0;
            let b_mesh = GeometryBuilder::build_as(&shapes::Polygon {
                points: vec![
                    Vec2::new(-bhw + b_cut, -bhh),
                    Vec2::new(bhw - b_cut, -bhh),
                    Vec2::new(bhw, -bhh + b_cut),
                    Vec2::new(bhw, bhh - b_cut),
                    Vec2::new(bhw - b_cut, bhh),
                    Vec2::new(-bhw + b_cut, bhh),
                    Vec2::new(-bhw, bhh - b_cut),
                    Vec2::new(-bhw, -bhh + b_cut),
                ],
                closed: true,
            });
            (s_body, b_body, b_mesh)
        }
        MessageBubbleShape::Pill => {
            let s_body = DrawCmd::Rect {
                x: SHADOW_OFFSET.x,
                y: BUBBLE_CENTER_Y + SHADOW_OFFSET.y,
                w: BUBBLE_W,
                h: BUBBLE_H,
                radius: 26.0,
                paint: Paint {
                    fill: Some(shadow_color),
                    stroke: None,
                    stroke_width: 0.0,
                },
            };
            let b_body = DrawCmd::Rect {
                x: 0.0,
                y: BUBBLE_CENTER_Y,
                w: BUBBLE_W,
                h: BUBBLE_H,
                radius: 26.0,
                paint: Paint {
                    fill: Some(body_color),
                    stroke: Some(border_color),
                    stroke_width: border_width,
                },
            };
            let b_mesh = GeometryBuilder::build_as(&shapes::RoundedPolygon {
                points: vec![
                    Vec2::new(-BUTTON_W / 2.0, -BUTTON_H / 2.0),
                    Vec2::new(BUTTON_W / 2.0, -BUTTON_H / 2.0),
                    Vec2::new(BUTTON_W / 2.0, BUTTON_H / 2.0),
                    Vec2::new(-BUTTON_W / 2.0, BUTTON_H / 2.0),
                ],
                radius: BUTTON_H / 2.0,
                ..default()
            });
            (s_body, b_body, b_mesh)
        }
    };

    for (i, cmd) in [
        shadow_pointer,
        shadow_body,
        body,
        pointer_fill,
        pointer_stroke,
    ]
    .iter()
    .enumerate()
    {
        let child = spawn_shape(&mut commands, cmd, i as f32 * 0.01, &font, &ca, "")
            .insert(RenderLayers::layer(1))
            .id();
        commands.entity(root).add_child(child);
    }

    let text_cmd = DrawCmd::Text {
        x: 0.0,
        y: BUBBLE_CENTER_Y + 24.0,
        text: entry.text,
        size: font_size,
        color: text_color,
    };
    let text_entity = spawn_shape(&mut commands, &text_cmd, 0.05, &font, &ca, "")
        .insert((
            Text2dBounds {
                size: Vec2::new(BUBBLE_W - 40.0, BUBBLE_H - 66.0) * TEXT_SUPERSAMPLE,
            },
            RenderLayers::layer(1),
        ))
        .id();
    commands.entity(root).add_child(text_entity);

    let button_y = BUBBLE_CENTER_Y - BUBBLE_H / 2.0 + 24.0;
    let button = commands
        .spawn((
            ShapeBundle {
                path: button_mesh,
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, button_y, 0.06)),
                ..default()
            },
            Fill::color(button_color),
            RenderLayers::layer(1),
        ))
        .id();
    commands.entity(root).add_child(button);
    let label = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Continue",
                    TextStyle {
                        font,
                        font_size: 14.0,
                        color: button_text_color,
                    },
                )
                .with_justify(JustifyText::Center),
                transform: Transform::from_xyz(0.0, button_y, 0.07),
                ..default()
            },
            RenderLayers::layer(1),
        ))
        .id();
    commands.entity(root).add_child(label);

    let mut icon: Handle<Image> = Default::default();
    if let Some(ResourceType::ImageHandle(i)) = ca.resource_map.get("default_system_icon") {
        icon = i.clone();
    }
    let hit_region = commands
        .spawn((
            SpriteBundle {
                texture: icon,
                sprite: Sprite {
                    color: Color::NONE,
                    custom_size: Some(Vec2::new(BUTTON_W, BUTTON_H)),
                    ..default()
                },
                transform: Transform::from_xyz(0.0, button_y, 0.08),
                ..default()
            },
            RenderLayers::layer(1),
            On::<Pointer<Click>>::run(dismiss_explain_bubble),
        ))
        .id();
    commands.entity(root).add_child(hit_region);
}

/// Marks a bubble root as still popping in; `elapsed` is real seconds since
/// spawn. Removed once the animation finishes.
#[derive(Component)]
pub struct BubblePop {
    elapsed: f32,
}

/// Eases `BubblePop` roots from `POP_START_SCALE` to full size over
/// `POP_IN_SECS`, using real (unpaused) time - see the module doc comment for
/// why this can't be a `bevy_tweening::Animator` like the tick pulse is.
pub fn animate_bubble_pop(
    real_time: Res<Time<Real>>,
    mut commands: Commands,
    mut bubbles: Query<(Entity, &mut Transform, &mut BubblePop)>,
) {
    for (entity, mut transform, mut pop) in bubbles.iter_mut() {
        pop.elapsed = (pop.elapsed + real_time.delta_seconds()).min(POP_IN_SECS);
        let t = pop.elapsed / POP_IN_SECS;
        // Standard "back out" ease: overshoots past 1.0 then settles, a livelier
        // pop than a plain ease-out.
        let c1 = 1.70158_f32;
        let c3 = c1 + 1.0;
        let overshoot = 1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2);
        let scale = POP_START_SCALE + (1.0 - POP_START_SCALE) * overshoot;
        transform.scale = Vec3::splat(scale.max(0.0));

        if pop.elapsed >= POP_IN_SECS {
            commands.entity(entity).remove::<BubblePop>();
        }
    }
}

/// Continue button click handler. Despawns the whole bubble (one
/// `despawn_recursive` on the marked root takes every child with it), then either
/// lets `show_next_explain` pick up the next queued entry next frame (still
/// paused) or unpauses if nothing else is waiting.
pub fn dismiss_explain_bubble(
    mut commands: Commands,
    bubble: Query<Entity, With<ExplainBubble>>,
    mut sim_time: ResMut<Time<Virtual>>,
    mut pending: ResMut<PendingExplain>,
) {
    for entity in bubble.iter() {
        commands.entity(entity).despawn_recursive();
    }
    pending.showing = None;
    if pending.queue.is_empty() {
        sim_time.unpause();
    }
}
