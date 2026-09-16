use crate::c_log;
use crate::components::message::Messages;
use crate::components::node::{NodeMarker, SelectedNodeMarker};
use crate::resources::graph_def::GraphDefinitionRes;
use crate::{
    components::node_connector::*,
    parser::graphv2::{ConnectorStyle, GraphAttrs},
};
use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;
use std::collections::{HashMap, HashSet};

/// Roughly how many points `walk_path` samples along a connector, regardless
/// of how long it actually is in world units - see that function's doc
/// comment for why this needs to be length-independent.
const WALK_TARGET_POINTS: f32 = 120.0;
/// Floor on the sample spacing, so a very short (near-zero-length) connector
/// doesn't get a degenerate/huge point count.
const WALK_MIN_INTERVAL: f32 = 0.5;

/// Samples points along a path, spaced so the *count* stays roughly constant
/// (`WALK_TARGET_POINTS`) regardless of the path's actual length - used to
/// place an in-flight message along a connector's curve. Called only when a
/// connector's `Path` is (re)built, with the result cached on
/// `NodeConnector::walk_cache`, rather than every frame in the message
/// animation system (see that field's doc comment for why).
///
/// The interval used to be a fixed `0.1` world units regardless of path
/// length - fine for the small, tightly-packed example graphs this was
/// written against (connectors tens of units long), but on a larger/more
/// spread-out graph (a several-hundred-node stress-test graph's nodes are
/// spread across a much larger area - see `node_system::spawn_spread_radius`)
/// connectors can be many hundreds of units long, so a fixed `0.1` spacing
/// was generating thousands of sample points per connector - a real,
/// measurable cost distinct from (and larger than) the allocation cost fixed
/// alongside it, since this ran on every retrace (frequent - pulse
/// animations touch many connectors' endpoints across frames). A message
/// only needs on the order of a hundred points along its path for smooth
/// animation regardless of the path's physical length, so scaling the
/// interval to the path's own length keeps both quality and cost constant
/// across graph scales.
pub fn walk_path(path: &lyon_algorithms::path::Path) -> Vec<[f32; 2]> {
    use lyon_algorithms::length::approximate_length;
    use lyon_algorithms::walk::{walk_along_path, RegularPattern, WalkerEvent};

    let tolerance = 0.5; // The path flattening tolerance.
    let length = approximate_length(path.iter(), tolerance);
    let interval = (length / WALK_TARGET_POINTS).max(WALK_MIN_INTERVAL);

    let mut x: Vec<[f32; 2]> = vec![];
    let mut pattern = RegularPattern {
        callback: &mut |event: WalkerEvent| {
            x.push(event.position.to_array());
            true // Return true to continue walking the path.
        },
        interval,
    };

    let start_offset = 0.0; // Start walking at the beginning of the path.

    walk_along_path(path.iter(), start_offset, tolerance, &mut pattern);
    x
}

/// Normalizes an (a, b) pair so `edge_key(a, b) == edge_key(b, a)` - a connector is
/// undirected and only needs to exist once regardless of which side declared the link.
/// Borrows rather than allocates - see the doc comment on `update_connectors`'s
/// `desired`/`existing` maps for why that matters here.
fn edge_key<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Fallback half-width/half-height for a node with no `overlay` shapes (a
/// bare default icon + label) - close to the old fixed `NODE_RADIUS` this
/// replaced, just no longer pretending a node is a circle (see
/// `node_half_extents`'s doc comment for why that mattered). Node icons are
/// drawn at `ICON_WIDTH`/`ICON_HEIGHT` = 64px (see `node_system.rs`); the
/// extra height budgets for the name label rendered below the icon.
const DEFAULT_HALF_EXTENTS: Vec2 = Vec2::new(38.0, 44.0);

/// A connector endpoint's inset used to stop exactly at a node's real
/// footprint rather than a fixed circular "radius" from its center - a
/// node's on-canvas shape is a rectangle-ish card (its overlay/template, or
/// the bare icon+label), not a circle, so a single constant offset either
/// undershoots (cuts into a wide template card approached from the side) or
/// overshoots (leaves a gap approached from above/below a short-but-wide
/// one) depending on approach angle. Computed from the node's actual
/// `overlay` shapes: the tightest axis-aligned box (kept symmetric around
/// the node's local origin, since overlay shapes aren't guaranteed centered)
/// that contains every shape's own extent.
fn node_half_extents(node: &crate::parser::graphv2::Node) -> Vec2 {
    use crate::parser::draw::{DrawCmd, DrawProgressStyle};

    if node.overlay.is_empty() {
        return DEFAULT_HALF_EXTENTS;
    }

    let mut hx: f32 = 0.0;
    let mut hy: f32 = 0.0;
    let mut grow = |cx: f32, cy: f32, half_w: f32, half_h: f32| {
        hx = hx.max(cx.abs() + half_w);
        hy = hy.max(cy.abs() + half_h);
    };

    for cmd in &node.overlay {
        match cmd {
            DrawCmd::Rect { x, y, w, h, .. } => grow(*x, *y, w / 2.0, h / 2.0),
            DrawCmd::Circle { x, y, r, .. } => grow(*x, *y, *r, *r),
            DrawCmd::Icon { x, y, w, h, .. } => grow(*x, *y, w / 2.0, h / 2.0),
            DrawCmd::Line { x1, y1, x2, y2, .. } => {
                grow(*x1, *y1, 0.0, 0.0);
                grow(*x2, *y2, 0.0, 0.0);
            }
            DrawCmd::Polygon { points, .. } => {
                for p in points {
                    grow(p.x, p.y, 0.0, 0.0);
                }
            }
            DrawCmd::Text { x, y, text, size, .. } => {
                // Rough monospace-ish estimate - exact glyph metrics aren't
                // available here, and this only needs to be in the right
                // ballpark to keep a connector from cutting through a label.
                grow(*x, *y, text.len() as f32 * size * 0.3, size / 2.0);
            }
            DrawCmd::Progress { x, y, style, .. } => match style {
                DrawProgressStyle::Bar { w, h, .. } => grow(*x, *y, w / 2.0, h / 2.0),
                DrawProgressStyle::Segmented { w, h, .. } => grow(*x, *y, w / 2.0, h / 2.0),
                DrawProgressStyle::Ring { r, thickness, .. } => {
                    grow(*x, *y, r + thickness / 2.0, r + thickness / 2.0)
                }
                DrawProgressStyle::Pie { r, .. } => grow(*x, *y, *r, *r),
            },
        }
    }

    if hx <= 0.0 || hy <= 0.0 {
        return DEFAULT_HALF_EXTENTS;
    }
    Vec2::new(hx, hy)
}

/// Where a ray from a box's center (half-extents `half`) heading in
/// direction `dir` exits the box - the box is always exited on whichever
/// axis is reached first, so this needs no per-quadrant branching:
/// `half.x / |dir.x|` is the distance to the left/right edge, `half.y /
/// |dir.y|` to the top/bottom edge, and the smaller of the two is the real
/// exit point. When `dir`'s component on an axis is `0.0`, IEEE-754 float
/// division makes that axis's distance `f32::INFINITY`, which `.min()`
/// correctly discards - no explicit zero-check needed. `max_t` is the
/// existing short-link clamp (so two nearby/overlapping nodes' insets can't
/// cross past each other). Returns the exit point and whether the exit was
/// through the left/right edge (`true`) or the top/bottom edge (`false`) -
/// the two ends' exit sides are what `Step` uses to pick its route shape.
fn box_exit_point(center: Vec2, dir: Vec2, half: Vec2, max_t: f32) -> (Vec2, bool) {
    let t_x = half.x / dir.x.abs();
    let t_y = half.y / dir.y.abs();
    let horizontal_exit = t_x < t_y;
    let t = t_x.min(t_y).min(max_t);
    (center + dir * t, horizontal_exit)
}

/// How far the curve bows perpendicular to the A-B line, as a fraction of
/// the distance between the two node centers - clamped so very short links
/// don't degenerate and very long ones don't bow absurdly far.
const BOW_FRACTION: f32 = 0.12;
const BOW_MIN: f32 = 10.0;
const BOW_MAX: f32 = 48.0;

fn bow_amount(dist: f32) -> f32 {
    (dist * BOW_FRACTION).clamp(BOW_MIN, BOW_MAX)
}

/// How far back from each `Step` corner the rounding starts - a visibly
/// rounded fillet, not just the thin, stroke-width-scaled rounding
/// `LineJoin::Round` gives a sharp corner on its own. Clamped per-corner to
/// half the shorter of its two adjacent segments so a short leg (nodes close
/// together, or a corner near the midpoint elbow on a mostly-vertical link)
/// can't make the fillet overshoot past the corner or past the other end of
/// the path.
const STEP_CORNER_RADIUS: f32 = 22.0;

/// Draws `points` (already `move_to`'d to `points[0]`) as a polyline whose
/// interior corners are rounded fillets instead of sharp joins: pull back
/// `radius` along the incoming segment, quadratic-bezier through the
/// original corner point (as the control point) to `radius` along the
/// outgoing segment. Used by `Step`'s one-or-two-corner route; a 2-point
/// `points` (no interior corner) just draws the one straight segment.
fn line_through_rounded_corners(path_builder: &mut PathBuilder, points: &[Vec2], radius: f32) {
    if points.len() < 3 {
        if let Some(&last) = points.last() {
            path_builder.line_to(last);
        }
        return;
    }
    let mut cursor = points[0];
    for i in 1..points.len() - 1 {
        let corner = points[i];
        let next = points[i + 1];
        let in_len = (corner - cursor).length();
        let out_len = (next - corner).length();
        let in_dir = if in_len > 0.0 { (corner - cursor) / in_len } else { Vec2::ZERO };
        let out_dir = if out_len > 0.0 { (next - corner) / out_len } else { Vec2::ZERO };
        let r = radius.min(in_len * 0.5).min(out_len * 0.5);

        let before = corner - in_dir * r;
        let after = corner + out_dir * r;
        path_builder.line_to(before);
        path_builder.quadratic_bezier_to(corner, after);
        cursor = after;
    }
    path_builder.line_to(*points.last().unwrap());
}

/// Builds the connector path between node centers `a` and `b`, shaped
/// according to `graph_attrs.connector_style` (see `ConnectorStyle`'s doc
/// comment) - shared by both the initial spawn (`generate_line`) and the
/// per-frame retrace loop so newly-created and moving connectors can't
/// drift out of sync with each other.
///
/// `Curved` bows perpendicular to the A→B line (magnitude proportional to
/// distance, clamped) rather than always toward a fixed `(+50, +50)`
/// diagonal offset like before - that fixed offset could bow the "wrong"
/// way (e.g. for vertically stacked nodes) or look lopsided depending on
/// layout, since it didn't account for how the two nodes were actually
/// arranged. This way the arc direction and shape stay visually consistent
/// no matter how a graph is laid out or dragged.
///
/// `Straight` is a single line segment. `Step` picks the shortest orthogonal
/// route the two nodes' actual exit/entry sides allow (see `box_exit_point`
/// - `horizontal_exit`/`horizontal_entry` below come from which edge of each
/// node's box the connector actually leaves/arrives through, not a fixed
/// assumption):
/// - already aligned on one axis → 1 segment, straight.
/// - one side exits horizontal, the other enters vertical (or vice versa) →
///   2 segments, a clean L with no compromise on either end.
/// - both sides exit/enter on the *same* axis → 3 segments, a Z bridging
///   them (horizontal-vertical-horizontal, or vertical-horizontal-vertical).
///
/// These three cases cover every possible pairing of exit/entry sides, so a
/// `Step` connector is never more than 3 segments. Every corner is rounded
/// into a visible fillet by `line_through_rounded_corners` (a small
/// quadratic bezier through the corner point, not just the thin,
/// stroke-width-scaled rounding `LineJoin::Round` alone would give a sharp
/// corner).
fn build_connector_path(a: Vec2, a_half: Vec2, b: Vec2, b_half: Vec2, style: ConnectorStyle) -> Path {
    let delta = b - a;
    let dist = delta.length();
    let mut path_builder = PathBuilder::new();
    if dist < 1.0 {
        // Degenerate (nodes effectively on top of each other) - avoid a
        // divide-by-zero in the normalize below.
        path_builder.move_to(a);
        path_builder.line_to(b);
        return path_builder.build();
    }
    let dir = delta / dist;

    // Don't let either inset eat more than the two nodes' visual gap on
    // very short links (icons close together or briefly overlapping
    // mid-drag) - each side is independently capped at 45% of the total
    // distance, so together they can take at most 90%, always leaving some
    // visible gap between them.
    let max_t = dist * 0.45;
    let (start, horizontal_exit) = box_exit_point(a, dir, a_half, max_t);
    let (end, horizontal_entry) = box_exit_point(b, -dir, b_half, max_t);

    path_builder.move_to(start);
    match style {
        ConnectorStyle::Straight => {
            path_builder.line_to(end);
        }
        ConnectorStyle::Step => {
            if (end.y - start.y).abs() < 0.5 || (end.x - start.x).abs() < 0.5 {
                // Already aligned on one axis - a single straight segment,
                // whichever axis it is.
                path_builder.line_to(end);
            } else if horizontal_exit != horizontal_entry {
                // One end leaves/arrives horizontally, the other
                // vertically - a single corner connects them exactly, no
                // bridging segment needed.
                let corner = if horizontal_exit {
                    Vec2::new(end.x, start.y)
                } else {
                    Vec2::new(start.x, end.y)
                };
                line_through_rounded_corners(
                    &mut path_builder,
                    &[start, corner, end],
                    STEP_CORNER_RADIUS,
                );
            } else if horizontal_exit {
                // Both ends leave/arrive horizontally - bridge the y gap
                // with a vertical segment at the horizontal midpoint.
                let mid_x = (start.x + end.x) / 2.0;
                let points = [
                    start,
                    Vec2::new(mid_x, start.y),
                    Vec2::new(mid_x, end.y),
                    end,
                ];
                line_through_rounded_corners(&mut path_builder, &points, STEP_CORNER_RADIUS);
            } else {
                // Both ends leave/arrive vertically - bridge the x gap with
                // a horizontal segment at the vertical midpoint.
                let mid_y = (start.y + end.y) / 2.0;
                let points = [
                    start,
                    Vec2::new(start.x, mid_y),
                    Vec2::new(end.x, mid_y),
                    end,
                ];
                line_through_rounded_corners(&mut path_builder, &points, STEP_CORNER_RADIUS);
            }
        }
        ConnectorStyle::Curved => {
            let perp = Vec2::new(-dir.y, dir.x);
            let bow = bow_amount(dist);
            let c1 = start + (end - start) * 0.33 + perp * bow;
            let c2 = start + (end - start) * 0.66 + perp * bow;
            path_builder.cubic_bezier_to(c1, c2, end);
        }
    }
    path_builder.build()
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

/// Adds/removes connector entities to match the currently-declared `links`, and
/// retraces existing connectors' paths as their endpoints move. Runs as a
/// diff every frame (cheap at these graph sizes) rather than
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
    mut commands: Commands,
    query_changed: Query<(&NodeMarker, &Transform), Changed<Transform>>,
    query_all: Query<(&NodeMarker, &Transform)>,
    mut query_conn: Query<(Entity, &mut Path, &mut NodeConnector)>,
    // TEMPORARY - see systems::profiling's doc comment.
    mut prof: ResMut<crate::systems::profiling::ProfilingStats>,
) {
    let __prof_t0 = web_time::Instant::now(); // TEMPORARY
    (|| {
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

    // Each node's actual on-canvas footprint (see `node_half_extents`), keyed
    // by name so both the new-connector and retrace loops below can look up
    // either endpoint's real shape instead of assuming a fixed icon size.
    let extents: HashMap<&str, Vec2> = g
        .graph_defn
        .node_instances
        .iter()
        .map(|n| (n.name.as_str(), node_half_extents(n)))
        .collect();

    // edges that should exist right now, keyed so A-B and B-A collapse to one entry.
    // Keys/values borrow from `node_instances`/`query_conn` rather than cloning -
    // this diff runs unconditionally every frame (it has to: `link()`/`unlink()`
    // mutate a node's `links` directly with no event firing, so there's no cheap
    // "topology changed" signal to gate it behind), and on a graph with a few
    // hundred edges the old `(String, String)`-keyed version was allocating on the
    // order of a few thousand `String`s per frame just to build and throw away two
    // `HashMap`s - a real, measurable cost distinct from (and larger than) the
    // path-retrace cost fixed above.
    let mut desired: HashMap<(&str, &str), (&str, &str)> = HashMap::new();
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
            let key = edge_key(&node.name, peer);
            desired
                .entry(key)
                .or_insert_with(|| (node.name.as_str(), peer.as_str()));
        }
    }

    let mut existing: HashMap<(&str, &str), Entity> = HashMap::new();
    for (entity, _, conn) in query_conn.iter() {
        existing.insert(edge_key(&conn.id1, &conn.id2), entity);
    }

    // drop connectors for edges that are no longer declared by either side
    for (key, entity) in existing.iter() {
        if !desired.contains_key(key) {
            commands.entity(*entity).despawn_recursive();
        }
    }

    // add connectors for newly-declared edges
    for (key, &(a, b)) in desired.iter() {
        if existing.contains_key(key) {
            continue;
        }
        let a_loc = all_node_loc.get(a).unwrap();
        let b_loc = all_node_loc.get(b).unwrap();
        let a_half = extents.get(a).copied().unwrap_or(DEFAULT_HALF_EXTENTS);
        let b_half = extents.get(b).copied().unwrap_or(DEFAULT_HALF_EXTENTS);
        let _ = generate_line(
            a_loc,
            a_half,
            b_loc,
            b_half,
            &g.graph_defn.graph_attrs,
            a,
            b,
            &mut commands,
        );
    }

    // keep every surviving connector's path glued to its
    // endpoints as nodes move - but only the connectors actually touching a
    // node that moved *this frame*, not every connector in the graph.
    // Dragging, spawning, and layout recomputation all touch `Transform` on
    // whichever nodes are affected, so on a graph with many nodes
    // `query_changed` is non-empty often enough that retracing (bezier
    // rebuild + lyon re-tessellation) *every* connector on *every* frame
    // regardless of whether its own endpoints moved was a real, measurable
    // bottleneck at a few hundred nodes (a several-hundred-node stress-test
    // graph).
    if !query_changed.is_empty() {
        let moved: HashSet<&str> = query_changed
            .iter()
            .map(|(m, _)| m.node_name.as_str())
            .collect();
        for (_, mut path, mut conn) in query_conn.iter_mut() {
            if !moved.contains(conn.id1.as_str()) && !moved.contains(conn.id2.as_str()) {
                continue;
            }
            let node1_loc = all_node_loc.get(&conn.id1);
            let node2_loc = all_node_loc.get(&conn.id2);
            if node1_loc.is_none() || node2_loc.is_none() {
                c_log!("Node not found for connector: {}-{}", conn.id1, conn.id2);
                continue;
            }
            let a_half = extents.get(conn.id1.as_str()).copied().unwrap_or(DEFAULT_HALF_EXTENTS);
            let b_half = extents.get(conn.id2.as_str()).copied().unwrap_or(DEFAULT_HALF_EXTENTS);
            *path = build_connector_path(
                node1_loc.unwrap().truncate(),
                a_half,
                node2_loc.unwrap().truncate(),
                b_half,
                g.graph_defn.graph_attrs.connector_style,
            );
            conn.path = path.0.clone();
            conn.walk_cache = walk_path(&conn.path);
        }
    }
    })(); // TEMPORARY
    prof.update_connectors_ms += __prof_t0.elapsed().as_secs_f64() * 1000.0; // TEMPORARY
}

fn generate_line(
    a: &Vec3,
    a_half: Vec2,
    b: &Vec3,
    b_half: Vec2,
    ga: &GraphAttrs,
    id1: &str,
    id2: &str,
    commands: &mut Commands,
) -> Entity {
    let path = build_connector_path(a.truncate(), a_half, b.truncate(), b_half, ga.connector_style);
    let walking_path = path.0.clone();
    let walk_cache = walk_path(&walking_path);
    let cc = ga.connection_color;

    commands
        .spawn((
            ShapeBundle {
                path,
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 10.0)),
                ..default()
            },
            connector_stroke(cc),
            NodeConnector {
                id1: id1.to_string(),
                id2: id2.to_string(),
                path: walking_path,
                walk_cache,
                flash: 0.0,
            },
            Messages::default(),
        ))
        .id()
}

const BASE_WIDTH: f32 = 3.0;
const TRAFFIC_WIDTH_BONUS: f32 = 2.0;
const SELECTED_WIDTH_BONUS: f32 = 1.5;
const FLASH_WIDTH_BONUS: f32 = 2.0;
/// How long a delivery flash (`NodeConnector::flash`) takes to fully decay.
const FLASH_DECAY_SECS: f32 = 0.5;
/// Echoes the narration-bubble's indigo accent (see `explain_bubble`'s
/// `ACCENT_COLOR`) so a highlighted connector reads as the same app-wide
/// accent rather than an unrelated color.
const ACCENT_COLOR: Color = Color::srgb(0.49, 0.55, 0.98);

/// Recolors/re-weights each connector's stroke from its current state -
/// adjacency to the selected node, in-flight message traffic, and a
/// brief flash on delivery - by blending from the user-configured
/// `connection_color` (see `ui::graph_properties_viewer`'s color picker)
/// toward `ACCENT_COLOR`, rather than replacing it outright, so a custom
/// base color still comes through under the highlight.
pub fn update_connector_style(
    time: Res<Time>,
    g: Res<GraphDefinitionRes>,
    selected: Query<&SelectedNodeMarker>,
    mut connectors: Query<(&Messages, &mut NodeConnector, &mut Stroke)>,
    // TEMPORARY - see systems::profiling's doc comment.
    mut prof: ResMut<crate::systems::profiling::ProfilingStats>,
) {
    let __prof_t0 = web_time::Instant::now(); // TEMPORARY
    let selected_names: HashSet<&str> = selected.iter().map(|s| s.node_name.as_str()).collect();
    let base = g.graph_defn.graph_attrs.connection_color;

    for (messages, mut conn, mut stroke) in connectors.iter_mut() {
        conn.flash = (conn.flash - time.delta_seconds() / FLASH_DECAY_SECS).max(0.0);

        let adjacent_selected = selected_names.contains(conn.id1.as_str())
            || selected_names.contains(conn.id2.as_str());
        let traffic = (messages.msg_inflight.len() as f32 / 3.0).min(1.0);

        let mut width = BASE_WIDTH + traffic * TRAFFIC_WIDTH_BONUS;
        let mut accent = traffic * 0.25;
        if adjacent_selected {
            width += SELECTED_WIDTH_BONUS;
            accent = accent.max(0.4);
        }
        width += conn.flash * FLASH_WIDTH_BONUS;
        accent = accent.max(conn.flash * 0.9);
        let color = base.mix(&ACCENT_COLOR, accent.min(1.0));

        // `bevy_prototype_lyon` fully re-tessellates and allocates a brand-new
        // `Mesh` asset on *any* write to `Stroke` (it reacts to `Changed<Stroke>`,
        // and `Mut` derefs mark a component changed regardless of whether the
        // value actually differs) - on an idle connector (by far the common case
        // on a large graph) both `width` and `color` are identical to last
        // frame's, so writing unconditionally was re-tessellating every
        // connector's geometry every frame for nothing. Only touch `Stroke` when
        // something actually changed.
        if (stroke.options.line_width - width).abs() > 0.01 || stroke.color != color {
            stroke.options.line_width = width;
            stroke.color = color;
        }
    }
    prof.connector_style_ms += __prof_t0.elapsed().as_secs_f64() * 1000.0; // TEMPORARY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::draw::{DrawCmd, Paint};

    #[test]
    fn test_box_exit_point_picks_nearer_edge() {
        let half = Vec2::new(80.0, 25.0); // a wide, short card

        // Approaching mostly sideways - should exit the left/right edge.
        let (p, horizontal) = box_exit_point(Vec2::ZERO, Vec2::new(1.0, 0.0), half, 1000.0);
        assert!(horizontal);
        assert!((p.x - half.x).abs() < 0.001);
        assert!(p.y.abs() < 0.001);

        // Approaching mostly vertically - should exit the top/bottom edge.
        let (p, horizontal) = box_exit_point(Vec2::ZERO, Vec2::new(0.0, 1.0), half, 1000.0);
        assert!(!horizontal);
        assert!((p.y - half.y).abs() < 0.001);
        assert!(p.x.abs() < 0.001);

        // A diagonal shallow enough that the wide card's side edge is still
        // reached first (dx dominates relative to the card's aspect ratio).
        let dir = Vec2::new(1.0, 0.3).normalize();
        let (_, horizontal) = box_exit_point(Vec2::ZERO, dir, half, 1000.0);
        assert!(horizontal);
    }

    #[test]
    fn test_box_exit_point_clamped_on_short_links() {
        let half = Vec2::new(80.0, 25.0);
        let (p, _) = box_exit_point(Vec2::ZERO, Vec2::new(1.0, 0.0), half, 10.0);
        // max_t (10.0) is smaller than the box's own half-width (80.0) - the
        // short-link clamp must win, not the box edge.
        assert!((p.x - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_node_half_extents_defaults_without_overlay() {
        let node = test_node_with_overlay(vec![]);
        assert_eq!(node_half_extents(&node), DEFAULT_HALF_EXTENTS);
    }

    #[test]
    fn test_node_half_extents_from_rect_overlay() {
        let node = test_node_with_overlay(vec![DrawCmd::Rect {
            x: 0.0,
            y: 0.0,
            w: 160.0,
            h: 50.0,
            radius: 8.0,
            paint: Paint { fill: None, stroke: None, stroke_width: 0.0 },
        }]);
        let half = node_half_extents(&node);
        assert!((half.x - 80.0).abs() < 0.001);
        assert!((half.y - 25.0).abs() < 0.001);
    }

    /// Builds a minimal `Node` carrying only the `overlay` field these tests
    /// care about - the rest are irrelevant to `node_half_extents`, which
    /// only ever reads `overlay`.
    fn test_node_with_overlay(overlay: Vec<DrawCmd>) -> crate::parser::graphv2::Node {
        use crate::parser::graphv2::{Attrs, Node, NodeType};
        use bevy::time::{Timer, TimerMode};
        use rhai::Dynamic;
        use std::collections::BTreeMap;
        use std::time::Duration;

        Node {
            name: "n".to_string(),
            node_data: NodeType {
                id: "n".to_string(),
                func: None,
                ast: rhai::AST::empty(),
                attrs: Attrs::default(),
                params: None,
            },
            timer: Timer::new(Duration::from_secs(1), TimerMode::Repeating),
            links: vec![],
            ast: rhai::AST::empty(),
            scope: rhai::Scope::new(),
            state: Dynamic::from_map(BTreeMap::new()),
            overlay,
            overlay_dirty: false,
            template_overrides: std::collections::HashMap::new(),
        }
    }
}
