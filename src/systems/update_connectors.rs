use crate::c_log;
use crate::components::message::Messages;
use crate::components::node::{NodeMarker, SelectedNodeMarker};
use crate::resources::common_assets::{CommonAssets, ResourceType};
use crate::resources::graph_def::GraphDefinitionRes;
use crate::{components::node_connector::*, parser::graphv2::GraphAttrs};
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_prototype_lyon::prelude::*;
use std::collections::{HashMap, HashSet};

/// Normalizes an (a, b) pair so `edge_key(a, b) == edge_key(b, a)` - a connector is
/// undirected and only needs to exist once regardless of which side declared the link.
fn edge_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// Distance from a node's center to inset each connector endpoint by, so the
/// line visibly stops just outside the icon's edge instead of running into
/// or under it. Node icons are drawn at `ICON_WIDTH`/`ICON_HEIGHT` = 64px
/// (see `node_system.rs`) - half that, plus a small margin.
const NODE_RADIUS: f32 = 38.0;
/// How far the curve bows perpendicular to the A-B line, as a fraction of
/// the distance between the two node centers - clamped so very short links
/// don't degenerate and very long ones don't bow absurdly far.
const BOW_FRACTION: f32 = 0.12;
const BOW_MIN: f32 = 10.0;
const BOW_MAX: f32 = 48.0;

fn bow_amount(dist: f32) -> f32 {
    (dist * BOW_FRACTION).clamp(BOW_MIN, BOW_MAX)
}

/// Computes the inset start/end points and the two cubic-bezier control
/// points for the connector between node centers `a` and `b`.
///
/// The curve bows perpendicular to the A→B line (magnitude proportional to
/// distance, clamped) rather than always toward a fixed `(+50, +50)`
/// diagonal offset like before - that fixed offset could bow the "wrong"
/// way (e.g. for vertically stacked nodes) or look lopsided depending on
/// layout, since it didn't account for how the two nodes were actually
/// arranged. This way the arc direction and shape stay visually consistent
/// no matter how a graph is laid out or dragged.
fn connector_geometry(a: Vec2, b: Vec2) -> (Vec2, Vec2, Vec2, Vec2) {
    let delta = b - a;
    let dist = delta.length();
    if dist < 1.0 {
        // Degenerate (nodes effectively on top of each other) - avoid a
        // divide-by-zero in the normalize below.
        return (a, a, b, b);
    }
    let dir = delta / dist;
    let perp = Vec2::new(-dir.y, dir.x);
    let bow = bow_amount(dist);

    // Don't let the inset eat more than the two nodes' visual gap on very
    // short links (icons close together or briefly overlapping mid-drag).
    let inset = NODE_RADIUS.min(dist * 0.45);
    let start = a + dir * inset;
    let end = b - dir * inset;

    let c1 = start + (end - start) * 0.33 + perp * bow;
    let c2 = start + (end - start) * 0.66 + perp * bow;
    (start, c1, c2, end)
}

/// A connector's rounded-cap, rounded-join stroke - shared by both
/// `generate_line` and the retrace loop so newly-spawned and moving
/// connectors always look the same.
fn connector_stroke(color: Color) -> Stroke {
    Stroke {
        options: StrokeOptions::default()
            .with_line_width(BASE_WIDTH)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round),
        color,
    }
}

/// Extra half-thickness (beyond the curve's own bow) the hit-region gets on
/// each side, in pixels - generous since it only needs to be a comfortable
/// click/hover target, not pixel-exact against the tessellated curve.
const HIT_REGION_PADDING: f32 = 16.0;
/// z the hit-region sprite sits at - below every node icon (spawned at
/// z = 0, 1, 2, ... by spawn index, see `node_system::create_nodes`), so a
/// node always wins picking over a connector hit-region it happens to
/// overlap near the node's edge, but *above* `background_grid`'s one giant
/// 4000x4000 sprite at z = -1.0 (`background_grid::setup_grid`). Sitting at
/// the same z as that sprite was a real bug: `bevy_mod_picking`'s sprite
/// backend sorts hits by z descending and the first opaque hit blocks every
/// tied-or-lower one, so on a z tie the grid - which covers the entire
/// visible world - won essentially every time and no connector hover ever
/// registered.
const HIT_REGION_Z: f32 = -0.5;

/// A rectangle covering the connector's chord - close enough to its actual
/// (modestly bowed) curve to be a comfortable pointer target - sized and
/// rotated to match, at `HIT_REGION_Z`.
fn hit_region_transform_and_size(start: Vec2, end: Vec2) -> (Transform, Vec2) {
    let delta = end - start;
    let dist = delta.length().max(1.0);
    let angle = delta.y.atan2(delta.x);
    let mid = (start + end) / 2.0;
    let half_h = bow_amount(dist) + HIT_REGION_PADDING;
    let transform = Transform::from_translation(mid.extend(HIT_REGION_Z))
        .with_rotation(Quat::from_rotation_z(angle));
    (transform, Vec2::new(dist, half_h * 2.0))
}

/// `bevy_mod_picking` here only hit-tests `Sprite` entities (see
/// `Cargo.toml`'s `backend_sprite`-only feature set), so a connector's own
/// lyon `Mesh2d` can never receive pointer events directly - these two
/// handlers live on its invisible `ConnectorHitRegion` sprite child instead
/// and reach back to the connector via `Parent`. This is the same pattern
/// `explain_bubble`'s Continue button uses for the same reason.
fn connector_hover_start(
    e: Listener<Pointer<Over>>,
    parents: Query<&Parent>,
    mut connectors: Query<&mut NodeConnector>,
) {
    if let Ok(parent) = parents.get(e.target) {
        if let Ok(mut conn) = connectors.get_mut(parent.get()) {
            conn.hovered = true;
        }
    }
}

fn connector_hover_end(
    e: Listener<Pointer<Out>>,
    parents: Query<&Parent>,
    mut connectors: Query<&mut NodeConnector>,
) {
    if let Ok(parent) = parents.get(e.target) {
        if let Ok(mut conn) = connectors.get_mut(parent.get()) {
            conn.hovered = false;
        }
    }
}

/// Adds/removes connector entities to match the currently-declared `links`, and
/// retraces existing connectors' (and their hit-regions') paths as their endpoints
/// move. Runs as a diff every frame (cheap at these graph sizes) rather than
/// despawning and rebuilding every connector whenever any single node is added or
/// removed - that used to happen on every `spawn()`/`despawn()` (and every YAML
/// edit), destroying every *other* connector's in-flight `Messages` queue along
/// with it.
///
/// Links are read from `node_instances` (the live, runtime-mutable copy `link()`/
/// `unlink()` update) rather than the YAML-shaped `graph_defn.graph`, so there's a
/// single source of truth for "who is connected to whom" while the sim is running.
pub fn update_connectors(
    g: Res<GraphDefinitionRes>,
    ca: Res<CommonAssets>,
    mut commands: Commands,
    query_changed: Query<(&NodeMarker, &Transform), Changed<Transform>>,
    query_all: Query<(&NodeMarker, &Transform)>,
    mut query_conn: Query<(Entity, &mut Path, &mut NodeConnector)>,
    mut query_hit_region: Query<
        (&mut Transform, &mut Sprite),
        (With<ConnectorHitRegion>, Without<NodeMarker>),
    >,
) {
    if g.graph_defn.node_instances.is_empty() {
        for (entity, _, _) in query_conn.iter_mut() {
            commands.entity(entity).despawn_recursive();
        }
        return;
    }

    let mut all_node_loc = HashMap::<String, Vec3>::new();
    for (node, transform) in query_all.iter() {
        let mut pos: Vec3 = transform.translation;
        pos.z = 50.;
        all_node_loc.insert(node.node_name.clone(), pos);
    }

    // edges that should exist right now, keyed so A-B and B-A collapse to one entry
    let mut desired: HashMap<(String, String), (String, String)> = HashMap::new();
    for node in g.graph_defn.node_instances.iter() {
        for peer in node.links.iter() {
            if &node.name == peer {
                c_log!("Ignoring loopback: {}-{}", node.name, peer);
                continue;
            }
            if !all_node_loc.contains_key(&node.name) || !all_node_loc.contains_key(peer) {
                // one (or both) endpoints haven't been spawned as an entity yet -
                // pick this edge up on a later frame once they have been.
                continue;
            }
            desired
                .entry(edge_key(&node.name, peer))
                .or_insert_with(|| (node.name.clone(), peer.clone()));
        }
    }

    let mut existing: HashMap<(String, String), Entity> = HashMap::new();
    for (entity, _, conn) in query_conn.iter() {
        existing.insert(edge_key(&conn.id1, &conn.id2), entity);
    }

    // drop connectors for edges that are no longer declared by either side
    // (the hit-region child despawns along with it via despawn_recursive)
    for (key, entity) in existing.iter() {
        if !desired.contains_key(key) {
            commands.entity(*entity).despawn_recursive();
        }
    }

    // add connectors for newly-declared edges
    for (key, (a, b)) in desired.iter() {
        if existing.contains_key(key) {
            continue;
        }
        let a_loc = all_node_loc.get(a).unwrap();
        let b_loc = all_node_loc.get(b).unwrap();
        let _ = generate_line(a_loc, b_loc, &g.graph_defn.graph_attrs, a, b, &ca, &mut commands);
    }

    // keep every surviving connector's path (and hit-region) glued to its
    // endpoints as nodes move
    if !query_changed.is_empty() {
        for (_, mut path, mut conn) in query_conn.iter_mut() {
            let node1_loc = all_node_loc.get(&conn.id1);
            let node2_loc = all_node_loc.get(&conn.id2);
            if node1_loc.is_none() || node2_loc.is_none() {
                c_log!("Node not found for connector: {}-{}", conn.id1, conn.id2);
                continue;
            }
            let (start, c1, c2, end) =
                connector_geometry(node1_loc.unwrap().truncate(), node2_loc.unwrap().truncate());
            let mut path_builder = PathBuilder::new();
            path_builder.move_to(start);
            path_builder.cubic_bezier_to(c1, c2, end);

            *path = path_builder.build();
            conn.path = path.0.clone();

            if let Ok((mut transform, mut sprite)) = query_hit_region.get_mut(conn.hit_region) {
                let (t, size) = hit_region_transform_and_size(start, end);
                *transform = t;
                sprite.custom_size = Some(size);
            }
        }
    }
}

fn generate_line(
    a: &Vec3,
    b: &Vec3,
    ga: &GraphAttrs,
    id1: &String,
    id2: &String,
    ca: &Res<CommonAssets>,
    commands: &mut Commands,
) -> Entity {
    let (start, c1, c2, end) = connector_geometry(a.truncate(), b.truncate());
    let mut path_builder = PathBuilder::new();
    path_builder.move_to(start);
    path_builder.cubic_bezier_to(c1, c2, end);

    let path = path_builder.build();
    let walking_path = path.0.clone();
    let cc = ga.connection_color;

    // Reuses whatever texture is already loaded for node icons purely as a
    // sprite to hang a hit-test on - `sprite.color = Color::NONE` makes it
    // fully invisible. Same trick `explain_bubble`'s Continue button uses.
    let mut icon: Handle<Image> = Default::default();
    if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get("default_system_icon") {
        icon = img.clone();
    }
    let (hit_transform, hit_size) = hit_region_transform_and_size(start, end);
    let hit_region = commands
        .spawn((
            SpriteBundle {
                texture: icon,
                sprite: Sprite {
                    color: Color::NONE,
                    custom_size: Some(hit_size),
                    ..default()
                },
                transform: hit_transform,
                ..default()
            },
            ConnectorHitRegion,
            On::<Pointer<Over>>::run(connector_hover_start),
            On::<Pointer<Out>>::run(connector_hover_end),
        ))
        .id();

    let connector = commands
        .spawn((
            ShapeBundle { path, ..default() },
            connector_stroke(cc),
            NodeConnector {
                id1: id1.clone(),
                id2: id2.clone(),
                path: walking_path,
                hit_region,
                hovered: false,
                flash: 0.0,
            },
            Messages::default(),
        ))
        .id();
    commands.entity(connector).add_child(hit_region);
    connector
}

const BASE_WIDTH: f32 = 3.0;
const TRAFFIC_WIDTH_BONUS: f32 = 2.0;
const HOVER_WIDTH_BONUS: f32 = 2.5;
const SELECTED_WIDTH_BONUS: f32 = 1.5;
const FLASH_WIDTH_BONUS: f32 = 2.0;
/// How long a delivery flash (`NodeConnector::flash`) takes to fully decay.
const FLASH_DECAY_SECS: f32 = 0.5;
/// Echoes the narration-bubble's indigo accent (see `explain_bubble`'s
/// `ACCENT_COLOR`) so a highlighted connector reads as the same app-wide
/// accent rather than an unrelated color.
const ACCENT_COLOR: Color = Color::srgb(0.49, 0.55, 0.98);

/// Recolors/re-weights each connector's stroke from its current state -
/// hover, adjacency to the selected node, in-flight message traffic, and a
/// brief flash on delivery - by blending from the user-configured
/// `connection_color` (see `ui::graph_properties_viewer`'s color picker)
/// toward `ACCENT_COLOR`, rather than replacing it outright, so a custom
/// base color still comes through under the highlight.
pub fn update_connector_style(
    time: Res<Time>,
    g: Res<GraphDefinitionRes>,
    selected: Query<&SelectedNodeMarker>,
    mut connectors: Query<(&Messages, &mut NodeConnector, &mut Stroke)>,
) {
    let selected_names: HashSet<&str> = selected.iter().map(|s| s.node_name.as_str()).collect();
    let base = g.graph_defn.graph_attrs.connection_color;

    for (messages, mut conn, mut stroke) in connectors.iter_mut() {
        conn.flash = (conn.flash - time.delta_seconds() / FLASH_DECAY_SECS).max(0.0);

        let adjacent_selected = selected_names.contains(conn.id1.as_str())
            || selected_names.contains(conn.id2.as_str());
        let traffic = (messages.msg_inflight.len() as f32 / 3.0).min(1.0);

        let mut width = BASE_WIDTH + traffic * TRAFFIC_WIDTH_BONUS;
        let mut accent = traffic * 0.25;
        if conn.hovered {
            width += HOVER_WIDTH_BONUS;
            accent = accent.max(0.55);
        }
        if adjacent_selected {
            width += SELECTED_WIDTH_BONUS;
            accent = accent.max(0.4);
        }
        width += conn.flash * FLASH_WIDTH_BONUS;
        accent = accent.max(conn.flash * 0.9);

        stroke.options.line_width = width;
        stroke.color = base.mix(&ACCENT_COLOR, accent.min(1.0));
    }
}
