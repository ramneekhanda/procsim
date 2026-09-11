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
use crate::resources::common_assets::{CommonAssets, ResourceType};
use crate::resources::narration::PendingExplain;
use crate::systems::node_overlay::spawn_shape;

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
/// (y-up) units - down and to the right, like a light source from the
/// top-left.
const SHADOW_OFFSET: Vec2 = Vec2::new(5.0, -7.0);
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

// A quiet charcoal card rather than the previous navy/gold combo - a soft
// indigo accent (border, pointer stroke, button) against near-black, plus a
// faint drop shadow (`SHADOW_COLOR`) so the card reads as floating above the
// blurred world instead of pasted flat onto it.
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

    let mut font: Handle<Font> = Default::default();
    if let Some(ResourceType::FontHandle(f)) = ca.resource_map.get("default_font") {
        font = f.clone();
    }

    // Dim backdrop - a sibling of `root`, not a child of it, so it doesn't
    // inherit `root`'s pop-in scale (a huge rect visibly growing from a point
    // would look broken, not like a dialog appearing).
    let backdrop_cmd = DrawCmd::Rect {
        x: 0.0,
        y: 0.0,
        w: BACKDROP_SIZE,
        h: BACKDROP_SIZE,
        radius: 0.0,
        paint: Paint {
            fill: Some(BACKDROP_COLOR),
            stroke: None,
            stroke_width: 0.0,
        },
    };
    let backdrop = spawn_shape(&mut commands, &backdrop_cmd, BACKDROP_Z, &font)
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

    // Fake drop shadow: the same body+pointer silhouette, solid and offset,
    // drawn first (lowest z) so it peeks out from behind the card instead of
    // the card sitting flat on the blurred backdrop.
    let shadow_pointer = DrawCmd::Polygon {
        points: vec![
            Vec2::new(-14.0, BUBBLE_BOTTOM) + SHADOW_OFFSET,
            Vec2::new(14.0, BUBBLE_BOTTOM) + SHADOW_OFFSET,
            Vec2::new(0.0, BUBBLE_BOTTOM - 25.0) + SHADOW_OFFSET,
        ],
        closed: true,
        paint: Paint {
            fill: Some(SHADOW_COLOR),
            stroke: None,
            stroke_width: 0.0,
        },
    };
    let shadow_body = DrawCmd::Rect {
        x: SHADOW_OFFSET.x,
        y: BUBBLE_CENTER_Y + SHADOW_OFFSET.y,
        w: BUBBLE_W,
        h: BUBBLE_H,
        radius: 20.0,
        paint: Paint {
            fill: Some(SHADOW_COLOR),
            stroke: None,
            stroke_width: 0.0,
        },
    };
    // Pointer triangle, bubble body, and narration text all reuse the same
    // shape primitives an author's own `draw()` calls use.
    let pointer = DrawCmd::Polygon {
        points: vec![
            Vec2::new(-14.0, BUBBLE_BOTTOM),
            Vec2::new(14.0, BUBBLE_BOTTOM),
            Vec2::new(0.0, BUBBLE_BOTTOM - 25.0),
        ],
        closed: true,
        paint: Paint {
            fill: Some(BODY_COLOR),
            stroke: Some(BORDER_COLOR),
            stroke_width: 1.5,
        },
    };
    let body = DrawCmd::Rect {
        x: 0.0,
        y: BUBBLE_CENTER_Y,
        w: BUBBLE_W,
        h: BUBBLE_H,
        radius: 20.0,
        paint: Paint {
            fill: Some(BODY_COLOR),
            stroke: Some(BORDER_COLOR),
            stroke_width: 1.5,
        },
    };
    for (i, cmd) in [shadow_pointer, shadow_body, pointer, body]
        .iter()
        .enumerate()
    {
        let child = spawn_shape(&mut commands, cmd, i as f32 * 0.01, &font)
            .insert(RenderLayers::layer(1))
            .id();
        commands.entity(root).add_child(child);
    }

    // The narration text specifically needs word-wrap, unlike the short single-line
    // labels `draw()` overlays normally carry - `spawn_shape` leaves text unbounded,
    // so bound this one instance directly rather than changing that shared default.
    let text_cmd = DrawCmd::Text {
        x: 0.0,
        y: BUBBLE_CENTER_Y + 24.0,
        text: entry.text,
        size: 13.5,
        color: TEXT_COLOR,
    };
    // z must clear every shape in the shadow/pointer/body loop above (which
    // now runs up to index 3, i.e. z 0.03) or the opaque body rect draws over
    // the text and hides it entirely.
    let text_entity = spawn_shape(&mut commands, &text_cmd, 0.05, &font)
        .insert((
            Text2dBounds {
                size: Vec2::new(BUBBLE_W - 40.0, BUBBLE_H - 66.0),
            },
            RenderLayers::layer(1),
        ))
        .id();
    commands.entity(root).add_child(text_entity);

    // Continue button. The visible rounded rect + label are `bevy_prototype_lyon`/
    // `Text2d`, same as the rest of the bubble - but this project's
    // `bevy_mod_picking` is built with only `backend_sprite` enabled (see
    // Cargo.toml), which hit-tests `Sprite` entities and nothing else; lyon
    // shapes render as `Mesh2d` and are never picked by it (this is also why
    // connector hover has never actually worked - a pre-existing, unrelated gap).
    // So the actual clickable surface is a separate, fully transparent `Sprite`
    // layered on top, sized to the button - invisible, but real to the backend
    // that's proven to work (it's what makes node icons clickable).
    let button_shape = shapes::RoundedPolygon {
        points: vec![
            Vec2::new(-BUTTON_W / 2.0, -BUTTON_H / 2.0),
            Vec2::new(BUTTON_W / 2.0, -BUTTON_H / 2.0),
            Vec2::new(BUTTON_W / 2.0, BUTTON_H / 2.0),
            Vec2::new(-BUTTON_W / 2.0, BUTTON_H / 2.0),
        ],
        radius: 10.0,
        ..default()
    };
    let button_y = BUBBLE_CENTER_Y - BUBBLE_H / 2.0 + 24.0;
    let button = commands
        .spawn((
            ShapeBundle {
                path: GeometryBuilder::build_as(&button_shape),
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, button_y, 0.06)),
                ..default()
            },
            Fill::color(BUTTON_COLOR),
            RenderLayers::layer(1),
        ))
        .id();
    commands.entity(root).add_child(button);
    let label = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Continue \u{25b8}",
                    TextStyle {
                        font,
                        font_size: 14.0,
                        color: BUTTON_TEXT_COLOR,
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
    pending: Res<PendingExplain>,
) {
    for entity in bubble.iter() {
        commands.entity(entity).despawn_recursive();
    }
    if pending.queue.is_empty() {
        sim_time.unpause();
    }
}
