use crate::c_log;
use crate::components::message::Messages;
use crate::components::node::{NodeMarker, SelectedNodeMarker};
use crate::resources::graph_def::GraphDefinitionRes;
use crate::{
    components::node_connector::*,
    parser::graphv2::ConnectorStyle,
};
use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;
use rand::Rng;
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
/// one) depending on approach angle.
///
/// Returns `(offset, half)`: the box's true center relative to the node's
/// own local origin, and its half-extents - the real min/max bounding box
/// of whatever is actually rendered, not a box forced symmetric around
/// `(0, 0)`.
///
/// Mirrors `node_system.rs`'s selection-highlight box (`icon_label_box` +
/// `overlay_box`, unioned): the default icon+label footprint only drops out
/// when a `template` suppresses it, so a node whose script calls `draw()`
/// *without* a template (e.g. a status card floated above/beside the icon
/// via a fixed `y` offset - see `web/static/tutorial/ch5/04_two_phase_commit.yml`'s
/// `update_ui`) still has its icon+label rendered underneath, and the
/// connector box has to cover both, not just the overlay. Using only the
/// overlay's own tight box here previously mispositioned the connector
/// endpoint anywhere the overlay didn't already happen to cover the icon -
/// which the old symmetric-around-origin approximation coincidentally
/// masked (mirroring an off-center overlay's extent onto the origin side
/// too, which usually swallowed the icon along with it) but a true tight
/// box does not.
///
/// An overlay shape is frequently *not* centered on the node's origin (e.g.
/// a template card drawn from `x=0` to `x=160`) - real min/max (`parser::
/// draw::bounds`, the same per-shape math the highlight box uses) handles
/// that correctly where a symmetric approximation couldn't. Unlike
/// `node_system.rs`'s highlight box, a `Text` shape's width here (both the
/// name label and any overlay `Text`) is still `parser::draw::bounds`'s
/// character-count guess rather than a measured `TextLayoutInfo` - this
/// system only has each node's `Transform` to work with (see
/// `update_connectors`'s query), not its overlay child entities.
fn node_half_extents(node: &crate::parser::graphv2::Node) -> (Vec2, Vec2) {
    use crate::parser::draw::bounds;
    use crate::systems::node_system::{FONT_SIZE, ICON_HEIGHT, ICON_WIDTH, TEXT_DISTANCE_FROM_BOTTOM};

    let has_template = node.node_data.attrs.template.is_some();
    let icon_label_box = (!has_template).then(|| {
        // No live label-text entity to measure here (unlike the
        // highlight box) - guess its width the same way `bounds()` guesses
        // an overlay `Text` shape's.
        let text_width_guess = node.name.chars().count() as f32 * FONT_SIZE * 0.6;
        let half_x = f32::max(ICON_WIDTH, text_width_guess) / 2.0;
        let y_transform = -1.0 * (FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM) / 2.0;
        let half_y = (ICON_HEIGHT + FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM) / 2.0;
        (
            Vec2::new(-half_x, y_transform - half_y),
            Vec2::new(half_x, y_transform + half_y),
        )
    });

    let overlay_box = node
        .overlay
        .iter()
        .map(bounds)
        .reduce(|(min1, max1), (min2, max2)| (min1.min(min2), max1.max(max2)));

    let (min, max) = match (icon_label_box, overlay_box) {
        (Some((min1, max1)), Some((min2, max2))) => (min1.min(min2), max1.max(max2)),
        (Some(b), None) | (None, Some(b)) => b,
        // A template that resolved to zero overlay shapes (not yet rendered
        // this frame, say) - fall back to a small fixed box rather than a
        // degenerate zero-size one.
        (None, None) => (
            Vec2::splat(-ICON_WIDTH / 2.0),
            Vec2::splat(ICON_WIDTH / 2.0),
        ),
    };

    let half = (max - min) / 2.0;
    if half.x <= 0.0 || half.y <= 0.0 {
        return (Vec2::ZERO, DEFAULT_HALF_EXTENTS);
    }
    ((min + max) / 2.0, half)
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

/// Radius of the small semicircular hop drawn where a `Straight`/`Step`
/// connector crosses another one (see `compute_crossing_bumps`) - the
/// classic schematic "these wires don't connect" hint. Kept small and
/// constant so it reads as a hop over a normal run of the connector rather
/// than visually dominating it.
const BRIDGE_RADIUS: f32 = 9.0;

/// Standard segment-segment intersection via the parametric cross-product
/// form. Returns the intersection point only if it falls strictly inside
/// both *closed* segments (not on their infinite extensions and not right at
/// an endpoint), `None` for parallel or non-crossing segments.
fn segment_intersection(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> Option<Vec2> {
    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let denom = d1.x * d2.y - d1.y * d2.x;
    if denom.abs() < 1e-6 {
        return None; // parallel, or one of the segments is degenerate
    }
    let t = ((p3.x - p1.x) * d2.y - (p3.y - p1.y) * d2.x) / denom;
    let u = ((p3.x - p1.x) * d1.y - (p3.y - p1.y) * d1.x) / denom;
    if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
        return None;
    }
    Some(p1 + d1 * t)
}

/// Finds, for every pair of *different* connectors' straight legs, the
/// points where they cross - at any angle, not just perpendicular - and
/// assigns the bridge hop to exactly one side of each crossing.
///
/// Ownership is decided per crossing by comparing the two connectors' own
/// (already `edge_key`-normalized) keys: the lexicographically greater one
/// always draws the hop over the lesser one. Nothing about "greater" is
/// meaningful here - the point is only that it's *stable*, so a crossing
/// gets exactly one bump regardless of which connector happens to retrace
/// first, and the same pair keeps the same owner from frame to frame instead
/// of visibly swapping sides as nodes move.
///
/// `polylines` holds each connector's un-rounded corner points (see
/// `connector_corner_points`) - only `Straight`/`Step` connectors contribute
/// one, since a bowed `Curved` connector has no straight legs to bridge.
fn compute_crossing_bumps<'a>(
    polylines: &[((&'a str, &'a str), Vec<Vec2>)],
) -> HashMap<(&'a str, &'a str), Vec<Vec2>> {
    let mut bumps: HashMap<(&'a str, &'a str), Vec<Vec2>> = HashMap::new();
    for i in 0..polylines.len() {
        let (key_i, pts_i) = &polylines[i];
        for (key_j, pts_j) in &polylines[i + 1..] {
            // Connectors sharing an endpoint node meet there by design -
            // not a crossing to bridge.
            if key_i.0 == key_j.0 || key_i.0 == key_j.1 || key_i.1 == key_j.0 || key_i.1 == key_j.1
            {
                continue;
            }
            for a in pts_i.windows(2) {
                for b in pts_j.windows(2) {
                    let Some(p) = segment_intersection(a[0], a[1], b[0], b[1]) else {
                        continue;
                    };
                    let owner = if key_i > key_j { *key_i } else { *key_j };
                    bumps.entry(owner).or_default().push(p);
                }
            }
        }
    }
    bumps
}

/// Draws a straight run from `from` to `to`, inserting a small semicircular
/// hop over each point in `bumps` that falls on this segment (strictly
/// between its endpoints, with room for a full `BRIDGE_RADIUS` on both
/// sides, and actually on the segment's own line rather than just near it).
/// Used for a `Straight` connector's single segment and each straight leg of
/// a `Step` connector's rounded-corner route (`line_through_rounded_corners`).
/// `bumps` is the connector's full crossing list, not pre-split per leg, so
/// this is what decides which of them belong to *this* leg.
fn line_to_with_bumps(path_builder: &mut PathBuilder, from: Vec2, to: Vec2, bumps: &[Vec2]) {
    let delta = to - from;
    let len = delta.length();
    if len < 1.0 || bumps.is_empty() {
        path_builder.line_to(to);
        return;
    }
    let dir = delta / len;

    // (distance along the segment, point snapped onto it) for every bump
    // that lands on this leg, in travel order.
    let mut on_segment: Vec<(f32, Vec2)> = bumps
        .iter()
        .filter_map(|&p| {
            let t = (p - from).dot(dir);
            if t <= BRIDGE_RADIUS || t >= len - BRIDGE_RADIUS {
                return None; // too close to either endpoint to fit a hop
            }
            let closest = from + dir * t;
            if (p - closest).length() > 1.5 {
                return None; // not actually on this leg
            }
            Some((t, closest))
        })
        .collect();
    on_segment.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let perp = Vec2::new(-dir.y, dir.x);
    for (t, point) in on_segment {
        let before = from + dir * (t - BRIDGE_RADIUS);
        let after = from + dir * (t + BRIDGE_RADIUS);
        path_builder.line_to(before);
        // Approximates a semicircular hop as two quadratic beziers through a
        // shared apex, rather than pulling in a dedicated arc primitive.
        let apex = point + perp * BRIDGE_RADIUS;
        let ctrl1 = before + perp * BRIDGE_RADIUS;
        let ctrl2 = after + perp * BRIDGE_RADIUS;
        path_builder.quadratic_bezier_to(ctrl1, apex);
        path_builder.quadratic_bezier_to(ctrl2, after);
    }
    path_builder.line_to(to);
}

/// Target spacing, in world units, between two sibling edges' anchor points
/// on the same side of a shared node - see `compute_lane_offsets`.
const LANE_STEP: f32 = 16.0;

/// A node with several edges leaving/arriving through the *same* side of its
/// box (a fan-out/fan-in - e.g. two load-balanced service instances each
/// calling the same three downstream services) used to have every one of
/// those edges computed independently from nothing but its own two
/// endpoints (`build_connector_path` has no idea any other connector
/// exists), so they all left from essentially the same point on the box and
/// drew on top of each other. This assigns each such edge a small, stable
/// tangential offset - "which lane on this side of the box does this edge
/// use" - so siblings fan out along the box edge instead of bunching at one
/// point.
///
/// Grouping is per (node, side) - side meaning left/right ("horizontal exit",
/// the same classification `box_exit_point` makes) vs top/bottom - and
/// within a group, edges are ordered by their peer's position along the
/// tangent axis (a horizontal-exit group orders by the peer's *y*; a
/// vertical-exit group by the peer's *x*), so lane order visually matches
/// where each edge is actually headed rather than being an arbitrary
/// tie-break. The offset step is capped so the total spread can't exceed the
/// node's own box on that axis (an edge's anchor should never wander past
/// the box's own corner).
///
/// Returned keyed by `(node, peer)` (both directions get their own,
/// independent entry) so a single edge's two ends can each look up their own
/// node's assignment - `build_connector_path`'s `a_lane`/`b_lane` - without
/// needing to know anything about the other end's node.
fn compute_lane_offsets<'a>(
    desired: &HashMap<(&'a str, &'a str), (&'a str, &'a str)>,
    all_node_loc: &HashMap<String, Vec3>,
    extents: &HashMap<&'a str, (Vec2, Vec2)>,
) -> HashMap<(&'a str, &'a str), f32> {
    let mut incident: HashMap<&str, Vec<&str>> = HashMap::new();
    for &(a, b) in desired.values() {
        incident.entry(a).or_default().push(b);
        incident.entry(b).or_default().push(a);
    }

    let mut offsets = HashMap::new();
    for (node, peers) in incident.iter() {
        if peers.len() < 2 {
            continue; // no siblings to spread apart from
        }
        let Some(node_pos) = all_node_loc.get(*node) else {
            continue;
        };
        let (offset, half) = extents
            .get(node)
            .copied()
            .unwrap_or((Vec2::ZERO, DEFAULT_HALF_EXTENTS));
        // The box's actual center, not the node's raw transform origin - see
        // `node_half_extents`'s doc comment for why they can differ.
        let center = node_pos.truncate() + offset;

        // (peer, tangent coordinate) - horizontal_group orders by the peer's
        // y (tangent to a left/right exit), vertical_group by the peer's x.
        let mut horizontal_group: Vec<(&str, f32)> = Vec::new();
        let mut vertical_group: Vec<(&str, f32)> = Vec::new();
        for peer in peers {
            let Some(p) = all_node_loc.get(*peer) else {
                continue;
            };
            let delta = p.truncate() - center;
            if delta.length() < 1.0 {
                continue;
            }
            let t_x = half.x / delta.x.abs();
            let t_y = half.y / delta.y.abs();
            if t_x < t_y {
                horizontal_group.push((peer, p.y));
            } else {
                vertical_group.push((peer, p.x));
            }
        }

        for (group, half_extent) in [
            (&mut horizontal_group, half.y),
            (&mut vertical_group, half.x),
        ] {
            if group.len() < 2 {
                continue;
            }
            group.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            let n = group.len();
            // Leaves a margin inside the box edge rather than spreading lanes
            // flush to the corner.
            let max_span = half_extent * 1.6;
            let step = LANE_STEP.min(max_span / (n - 1) as f32);
            for (i, (peer, _)) in group.iter().enumerate() {
                let offset = (i as f32 - (n - 1) as f32 / 2.0) * step;
                offsets.insert((*node, *peer), offset);
            }
        }
    }
    offsets
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
fn line_through_rounded_corners(
    path_builder: &mut PathBuilder,
    points: &[Vec2],
    radius: f32,
    bumps: &[Vec2],
) {
    if points.len() < 3 {
        if let Some(&last) = points.last() {
            line_to_with_bumps(path_builder, points[0], last, bumps);
        }
        return;
    }
    let mut cursor = points[0];
    for i in 1..points.len() - 1 {
        let corner = points[i];
        let next = points[i + 1];
        let in_len = (corner - cursor).length();
        let out_len = (next - corner).length();
        let in_dir = if in_len > 0.0 {
            (corner - cursor) / in_len
        } else {
            Vec2::ZERO
        };
        let out_dir = if out_len > 0.0 {
            (next - corner) / out_len
        } else {
            Vec2::ZERO
        };
        let r = radius.min(in_len * 0.5).min(out_len * 0.5);

        let before = corner - in_dir * r;
        let after = corner + out_dir * r;
        line_to_with_bumps(path_builder, cursor, before, bumps);
        path_builder.quadratic_bezier_to(corner, after);
        cursor = after;
    }
    line_to_with_bumps(path_builder, cursor, *points.last().unwrap(), bumps);
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
///
/// `a_lane`/`b_lane` are each endpoint's tangential anchor offset from
/// `compute_lane_offsets` (`0.0` for a node with no sibling edges sharing its
/// exit side) - applied to `start`/`end` right after the box-exit inset, so
/// every downstream computation (corner points, bezier control points, the
/// `Step` bus position) naturally uses the already-spread point instead of
/// needing its own separate adjustment.
/// Computes each end's actual on-box anchor point (`box_exit_point` inset,
/// then the `a_lane`/`b_lane` tangential nudge) plus which side of each box
/// it exits/enters through - the part of path-building every `ConnectorStyle`
/// shares, factored out so `build_connector_path`'s `Curved` branch and
/// `connector_corner_points`'s `Straight`/`Step` branches compute the exact
/// same anchors. Returns `None` in the degenerate case (`a`/`b` effectively
/// on top of each other), which the caller handles as a bare two-point line.
fn connector_endpoints(
    a: Vec2,
    a_offset: Vec2,
    a_half: Vec2,
    a_lane: f32,
    b: Vec2,
    b_offset: Vec2,
    b_half: Vec2,
    b_lane: f32,
) -> Option<(Vec2, Vec2, bool, bool)> {
    // The box's actual center, not the node's raw transform origin - see
    // `node_half_extents`'s doc comment for why they can differ.
    let a_center = a + a_offset;
    let b_center = b + b_offset;
    let delta = b_center - a_center;
    let dist = delta.length();
    if dist < 1.0 {
        // Avoid a divide-by-zero in the normalize below.
        return None;
    }
    let dir = delta / dist;

    // Don't let either inset eat more than the two nodes' visual gap on
    // very short links (icons close together or briefly overlapping
    // mid-drag) - each side is independently capped at 45% of the total
    // distance, so together they can take at most 90%, always leaving some
    // visible gap between them.
    let max_t = dist * 0.45;
    let (mut start, horizontal_exit) = box_exit_point(a_center, dir, a_half, max_t);
    let (mut end, horizontal_entry) = box_exit_point(b_center, -dir, b_half, max_t);

    // Nudge each endpoint along the tangent of whichever side it actually
    // exits/enters through (see `compute_lane_offsets`), clamped so the
    // offset can't push the point past that box's own corner.
    if horizontal_exit {
        start.y = (start.y + a_lane).clamp(
            a_center.y - a_half.y * 0.95,
            a_center.y + a_half.y * 0.95,
        );
    } else {
        start.x = (start.x + a_lane).clamp(
            a_center.x - a_half.x * 0.95,
            a_center.x + a_half.x * 0.95,
        );
    }
    if horizontal_entry {
        end.y = (end.y + b_lane).clamp(
            b_center.y - b_half.y * 0.95,
            b_center.y + b_half.y * 0.95,
        );
    } else {
        end.x = (end.x + b_lane).clamp(
            b_center.x - b_half.x * 0.95,
            b_center.x + b_half.x * 0.95,
        );
    }

    Some((start, end, horizontal_exit, horizontal_entry))
}

/// Returns a `Straight`/`Step` connector's un-rounded corner points (`start`,
/// any interior corner(s), `end`) - the same route `build_connector_path`
/// turns into a rounded, bump-adorned path, but as raw points so crossing
/// detection (`compute_crossing_bumps`) can test straight segments against
/// every other connector's without needing a real `lyon::Path`. Returns
/// `None` for `Curved` - a bowed bezier has no straight legs to bridge, so
/// it never contributes crossing candidates (see `compute_crossing_bumps`'s
/// doc comment) - and is never called for it in practice.
fn connector_corner_points(
    a: Vec2,
    a_offset: Vec2,
    a_half: Vec2,
    a_lane: f32,
    b: Vec2,
    b_offset: Vec2,
    b_half: Vec2,
    b_lane: f32,
    style: ConnectorStyle,
) -> Option<Vec<Vec2>> {
    if style == ConnectorStyle::Curved {
        return None;
    }
    let Some((start, end, horizontal_exit, horizontal_entry)) =
        connector_endpoints(a, a_offset, a_half, a_lane, b, b_offset, b_half, b_lane)
    else {
        return Some(vec![a, b]);
    };

    Some(match style {
        ConnectorStyle::Straight => vec![start, end],
        ConnectorStyle::Step => {
            if (end.y - start.y).abs() < 0.5 || (end.x - start.x).abs() < 0.5 {
                vec![start, end]
            } else if horizontal_exit != horizontal_entry {
                let corner = if horizontal_exit {
                    Vec2::new(end.x, start.y)
                } else {
                    Vec2::new(start.x, end.y)
                };
                vec![start, corner, end]
            } else if horizontal_exit {
                let bus_offset = (a_lane + b_lane) * 0.5;
                let mid_x = (start.x + end.x) / 2.0 + bus_offset;
                vec![
                    start,
                    Vec2::new(mid_x, start.y),
                    Vec2::new(mid_x, end.y),
                    end,
                ]
            } else {
                let bus_offset = (a_lane + b_lane) * 0.5;
                let mid_y = (start.y + end.y) / 2.0 + bus_offset;
                vec![
                    start,
                    Vec2::new(start.x, mid_y),
                    Vec2::new(end.x, mid_y),
                    end,
                ]
            }
        }
        ConnectorStyle::Curved => unreachable!("returned above"),
    })
}

/// Minimum perpendicular gap (world units, ~= px) two *different*
/// connectors' parallel, axis-aligned segments must keep from each other
/// wherever they run alongside one another - any closer and they visually
/// merge into what looks like a single, thicker line. Every axis-aligned
/// segment of every connector participates (see `segment_runs`), not just a
/// designated "long run" - a single-segment aligned route, either leg of an
/// `L`-route, and a Z-route's bus segment are all equally eligible.
const MIN_PARALLEL_GAP: f32 = 10.0;

/// One axis-aligned segment of a connector's `connector_corner_points`
/// output - `points[seg_idx]` -> `points[seg_idx + 1]` - eligible to be
/// nudged apart from another connector's parallel segment running alongside
/// it within `MIN_PARALLEL_GAP`. `horizontal` is `true` for a horizontal
/// segment (constant y, nudged along y), `false` for vertical (constant x,
/// nudged along x); `span` is the segment's extent along the *other* axis,
/// used to test whether two same-coordinate segments actually overlap in
/// the range they run alongside each other, not just happen to share a
/// coordinate far apart. A diagonal `Straight` route's one segment has no
/// entry - see `segment_runs`.
struct SegmentRun<'a> {
    key: (&'a str, &'a str),
    seg_idx: usize,
    horizontal: bool,
    coord: f32,
    span: (f32, f32),
}

/// Every axis-aligned segment of a connector's corner-point polyline, as a
/// `SegmentRun` - every `Step`/`Straight` segment is horizontal or vertical
/// by construction, *except* a diagonal `Straight` route (the two nodes
/// aren't aligned on either axis), which contributes nothing since there's
/// no coordinate to de-conflict it on.
fn segment_runs<'a>(key: (&'a str, &'a str), points: &[Vec2]) -> Vec<SegmentRun<'a>> {
    points
        .windows(2)
        .enumerate()
        .filter_map(|(seg_idx, w)| {
            let (p, q) = (w[0], w[1]);
            if (p.y - q.y).abs() < 0.5 {
                Some(SegmentRun {
                    key,
                    seg_idx,
                    horizontal: true,
                    coord: p.y,
                    span: (p.x.min(q.x), p.x.max(q.x)),
                })
            } else if (p.x - q.x).abs() < 0.5 {
                Some(SegmentRun {
                    key,
                    seg_idx,
                    horizontal: false,
                    coord: p.x,
                    span: (p.y.min(q.y), p.y.max(q.y)),
                })
            } else {
                None // a diagonal `Straight` route's one segment
            }
        })
        .collect()
}

/// Finds every group of 2+ *different* connectors' axis-aligned segments
/// (see `segment_runs`) that are parallel, within `MIN_PARALLEL_GAP` of each
/// other, and actually overlap in the range they run alongside each other -
/// then spreads each such group evenly around its own mean coordinate, a
/// `MIN_PARALLEL_GAP` step apart, so every adjacent pair in the spread ends
/// up at least that far apart. Grouping is a straightforward union-find
/// rather than picking pairwise "owners" the way `compute_crossing_bumps`
/// does, because unlike a crossing (an isolated point, always exactly two
/// connectors), a parallel-proximity conflict is often a whole *chain* of
/// three or more mutually-close connectors that all need to move together
/// to end up evenly spread.
///
/// Keyed by `(edge_key, seg_idx)` rather than just `edge_key` - a single
/// connector can have more than one axis-aligned segment (an `L`-route has
/// two, a Z-route three), each nudged independently of the others on the
/// same connector - see `apply_segment_offsets`.
fn compute_parallel_offsets<'a>(
    polylines: &[((&'a str, &'a str), Vec<Vec2>)],
) -> HashMap<((&'a str, &'a str), usize), f32> {
    let runs: Vec<SegmentRun<'a>> = polylines
        .iter()
        .flat_map(|(key, pts)| segment_runs(*key, pts))
        .collect();

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    let mut offsets = HashMap::new();
    for horizontal in [true, false] {
        let group: Vec<&SegmentRun> = runs.iter().filter(|r| r.horizontal == horizontal).collect();
        let n = group.len();
        if n < 2 {
            continue;
        }

        let mut parent: Vec<usize> = (0..n).collect();
        for i in 0..n {
            for j in (i + 1)..n {
                // Two segments of the *same* connector are already joined
                // by construction (they share a corner point) - not a
                // proximity conflict to resolve by nudging them apart.
                if group[i].key == group[j].key {
                    continue;
                }
                let close = (group[i].coord - group[j].coord).abs() < MIN_PARALLEL_GAP;
                let overlaps = group[i].span.0 < group[j].span.1 && group[j].span.0 < group[i].span.1;
                if close && overlaps {
                    let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                    if ri != rj {
                        parent[ri] = rj;
                    }
                }
            }
        }

        let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            clusters.entry(root).or_default().push(i);
        }

        for members in clusters.into_values() {
            if members.len() < 2 {
                continue;
            }
            let mut idxs = members;
            idxs.sort_by(|&i, &j| group[i].coord.partial_cmp(&group[j].coord).unwrap());
            let mean: f32 =
                idxs.iter().map(|&i| group[i].coord).sum::<f32>() / idxs.len() as f32;
            let m = idxs.len();
            for (rank, &idx) in idxs.iter().enumerate() {
                let target = mean + (rank as f32 - (m - 1) as f32 / 2.0) * MIN_PARALLEL_GAP;
                offsets.insert((group[idx].key, group[idx].seg_idx), target - group[idx].coord);
            }
        }
    }
    offsets
}

/// Applies `compute_parallel_offsets`'s per-segment nudges directly onto a
/// connector's own corner points, shifting each axis-aligned segment's
/// constant coordinate on both its endpoints. Safe to do segment-by-segment
/// in any order: consecutive `Step` segments always alternate axis
/// (horizontal then vertical or vice versa - see `ConnectorStyle`'s doc
/// comment), so a shared corner point only ever has *one* of its two
/// coordinates touched by any given segment, never both - the incoming
/// segment's offset and the outgoing segment's offset can never fight over
/// the same field. The two endpoints (`points[0]`/`points[last]`, anchored
/// to each node's box) can end up nudged too, exactly like this - the same
/// small-scale tradeoff `a_lane`/`b_lane` already makes for sibling
/// spreading.
fn apply_segment_offsets(
    points: &mut [Vec2],
    key: (&str, &str),
    offsets: &HashMap<((&str, &str), usize), f32>,
) {
    for seg_idx in 0..points.len().saturating_sub(1) {
        let Some(&offset) = offsets.get(&(key, seg_idx)) else {
            continue;
        };
        let (p, q) = (points[seg_idx], points[seg_idx + 1]);
        if (p.y - q.y).abs() < 0.5 {
            points[seg_idx].y += offset;
            points[seg_idx + 1].y += offset;
        } else if (p.x - q.x).abs() < 0.5 {
            points[seg_idx].x += offset;
            points[seg_idx + 1].x += offset;
        }
    }
}

/// Builds a `Straight`/`Step` connector's final `lyon::Path` from its
/// already-fully-resolved corner points - `connector_corner_points`'s output
/// with `compute_parallel_offsets`'s per-segment spacing nudges already
/// applied (`apply_segment_offsets`) - plus its crossing-bridge hop points
/// (`compute_crossing_bumps`). Every corner is rounded into a visible fillet
/// and every bump spliced in by `line_through_rounded_corners`/
/// `line_to_with_bumps`, not left as the thin, stroke-width-scaled rounding
/// `LineJoin::Round` alone would give a sharp corner.
///
/// Shared by both the initial spawn (`generate_line`) and the per-frame
/// retrace loop (`update_connectors`) so newly-created and moving connectors
/// can't drift out of sync with each other.
fn build_connector_path_from_points(points: &[Vec2], bumps: &[Vec2]) -> Path {
    let mut path_builder = PathBuilder::new();
    path_builder.move_to(points[0]);
    line_through_rounded_corners(&mut path_builder, points, STEP_CORNER_RADIUS, bumps);
    path_builder.build()
}

/// Builds a `Curved` connector's path: a single cubic bezier that bows
/// perpendicular to the A→B line (magnitude proportional to distance,
/// clamped) rather than always toward a fixed `(+50, +50)` diagonal offset
/// like before - that fixed offset could bow the "wrong" way (e.g. for
/// vertically stacked nodes) or look lopsided depending on layout, since it
/// didn't account for how the two nodes were actually arranged. This way the
/// arc direction and shape stay visually consistent no matter how a graph is
/// laid out or dragged.
///
/// `a_lane`/`b_lane` are each endpoint's tangential anchor offset from
/// `compute_lane_offsets` (`0.0` for a node with no sibling edges sharing its
/// exit side). `Curved` connectors get neither crossing bumps nor
/// parallel-run spacing (see `connector_corner_points`'s doc comment) - a
/// bowed bezier has no straight, axis-aligned run for either mechanism to
/// act on.
fn build_curved_connector_path(
    a: Vec2,
    a_offset: Vec2,
    a_half: Vec2,
    a_lane: f32,
    b: Vec2,
    b_offset: Vec2,
    b_half: Vec2,
    b_lane: f32,
) -> Path {
    let mut path_builder = PathBuilder::new();
    let Some((start, end, _, _)) =
        connector_endpoints(a, a_offset, a_half, a_lane, b, b_offset, b_half, b_lane)
    else {
        // Degenerate (nodes effectively on top of each other).
        path_builder.move_to(a);
        path_builder.line_to(b);
        return path_builder.build();
    };
    let dir = (end - start).normalize_or_zero();
    let dist = (end - start).length();

    path_builder.move_to(start);
    let perp = Vec2::new(-dir.y, dir.x);
    let bow = bow_amount(dist);
    let c1 = start + (end - start) * 0.33 + perp * bow;
    let c2 = start + (end - start) * 0.66 + perp * bow;
    path_builder.cubic_bezier_to(c1, c2, end);
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

        // Each node's actual on-canvas footprint - (box center offset, half
        // extents) - see `node_half_extents`, keyed by name so both the
        // new-connector and retrace loops below can look up either
        // endpoint's real shape instead of assuming a fixed icon size
        // centered on the node's own transform.
        let extents: HashMap<&str, (Vec2, Vec2)> = g
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

        // Only worth computing lane offsets (see `compute_lane_offsets`) on a
        // frame that's actually spawning a new connector or retracing an
        // existing one - a fully idle frame (nothing moved, nothing new)
        // should stay as cheap as it was before this existed.
        let has_new_edges = desired.keys().any(|k| !existing.contains_key(k));
        let recompute = has_new_edges || !query_changed.is_empty();
        let lane_offsets = if recompute {
            compute_lane_offsets(&desired, &all_node_loc, &extents)
        } else {
            HashMap::new()
        };

        // `Straight`/`Step` connectors' final corner points (box-exit insets,
        // lane nudges, and `compute_parallel_offsets`'s per-segment spacing
        // all applied - see `apply_segment_offsets`) plus crossing bridge
        // hops (`compute_crossing_bumps`), keyed by the same normalized
        // `edge_key` `desired`/`existing` use. Recomputed on the same gate
        // as `lane_offsets`: geometry only actually needs this when
        // something moved or a new edge showed up, and a connector that
        // isn't retraced this frame keeps whatever its `Path` was last built
        // with anyway (see the retrace loop below). `Curved` connectors sit
        // outside this - they're rebuilt directly from their endpoints by
        // `build_curved_connector_path`, with neither bumps nor spacing (see
        // that function's doc comment).
        let (final_points, crossing_bumps) = if recompute {
            let raw_polylines: Vec<((&str, &str), Vec<Vec2>)> = desired
                .iter()
                .filter_map(|(&key, &(a, b))| {
                    let a_loc = all_node_loc.get(a)?.truncate();
                    let b_loc = all_node_loc.get(b)?.truncate();
                    let (a_offset, a_half) = extents
                        .get(a)
                        .copied()
                        .unwrap_or((Vec2::ZERO, DEFAULT_HALF_EXTENTS));
                    let (b_offset, b_half) = extents
                        .get(b)
                        .copied()
                        .unwrap_or((Vec2::ZERO, DEFAULT_HALF_EXTENTS));
                    let a_lane = lane_offsets.get(&(a, b)).copied().unwrap_or(0.0);
                    let b_lane = lane_offsets.get(&(b, a)).copied().unwrap_or(0.0);
                    let points = connector_corner_points(
                        a_loc,
                        a_offset,
                        a_half,
                        a_lane,
                        b_loc,
                        b_offset,
                        b_half,
                        b_lane,
                        g.graph_defn.graph_attrs.connector_style,
                    )?;
                    Some((key, points))
                })
                .collect();
            let seg_offsets = compute_parallel_offsets(&raw_polylines);

            let final_points: HashMap<(&str, &str), Vec<Vec2>> = raw_polylines
                .into_iter()
                .map(|(key, mut points)| {
                    apply_segment_offsets(&mut points, key, &seg_offsets);
                    (key, points)
                })
                .collect();

            let adjusted_polylines: Vec<((&str, &str), Vec<Vec2>)> =
                final_points.iter().map(|(&key, pts)| (key, pts.clone())).collect();
            let crossing_bumps = compute_crossing_bumps(&adjusted_polylines);

            (final_points, crossing_bumps)
        } else {
            (HashMap::new(), HashMap::new())
        };
        let no_bumps: Vec<Vec2> = Vec::new();
        let no_points: Vec<Vec2> = Vec::new();

        // add connectors for newly-declared edges
        for (key, &(a, b)) in desired.iter() {
            if existing.contains_key(key) {
                continue;
            }
            let a_loc = all_node_loc.get(a).unwrap().truncate();
            let b_loc = all_node_loc.get(b).unwrap().truncate();
            let style = g.graph_defn.graph_attrs.connector_style;
            let path = if style == ConnectorStyle::Curved {
                let (a_offset, a_half) = extents
                    .get(a)
                    .copied()
                    .unwrap_or((Vec2::ZERO, DEFAULT_HALF_EXTENTS));
                let (b_offset, b_half) = extents
                    .get(b)
                    .copied()
                    .unwrap_or((Vec2::ZERO, DEFAULT_HALF_EXTENTS));
                let a_lane = lane_offsets.get(&(a, b)).copied().unwrap_or(0.0);
                let b_lane = lane_offsets.get(&(b, a)).copied().unwrap_or(0.0);
                build_curved_connector_path(
                    a_loc, a_offset, a_half, a_lane, b_loc, b_offset, b_half, b_lane,
                )
            } else {
                let points = final_points.get(key).unwrap_or(&no_points);
                let bumps = crossing_bumps.get(key).unwrap_or(&no_bumps);
                build_connector_path_from_points(points, bumps)
            };
            let _ = generate_line(path, a, b, &mut commands);
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
                let style = g.graph_defn.graph_attrs.connector_style;
                let ek = edge_key(&conn.id1, &conn.id2);
                *path = if style == ConnectorStyle::Curved {
                    let (a_offset, a_half) = extents
                        .get(conn.id1.as_str())
                        .copied()
                        .unwrap_or((Vec2::ZERO, DEFAULT_HALF_EXTENTS));
                    let (b_offset, b_half) = extents
                        .get(conn.id2.as_str())
                        .copied()
                        .unwrap_or((Vec2::ZERO, DEFAULT_HALF_EXTENTS));
                    let a_lane = lane_offsets
                        .get(&(conn.id1.as_str(), conn.id2.as_str()))
                        .copied()
                        .unwrap_or(0.0);
                    let b_lane = lane_offsets
                        .get(&(conn.id2.as_str(), conn.id1.as_str()))
                        .copied()
                        .unwrap_or(0.0);
                    build_curved_connector_path(
                        node1_loc.unwrap().truncate(),
                        a_offset,
                        a_half,
                        a_lane,
                        node2_loc.unwrap().truncate(),
                        b_offset,
                        b_half,
                        b_lane,
                    )
                } else {
                    let points = final_points.get(&ek).unwrap_or(&no_points);
                    let bumps = crossing_bumps.get(&ek).unwrap_or(&no_bumps);
                    build_connector_path_from_points(points, bumps)
                };
                conn.path = path.0.clone();
                conn.walk_cache = walk_path(&conn.path);
            }
        }
    })(); // TEMPORARY
    prof.update_connectors_ms += __prof_t0.elapsed().as_secs_f64() * 1000.0; // TEMPORARY
}

fn generate_line(path: Path, id1: &str, id2: &str, commands: &mut Commands) -> Entity {
    let walking_path = path.0.clone();
    let walk_cache = walk_path(&walking_path);
    let base_color = random_connector_color();

    commands
        .spawn((
            ShapeBundle {
                path,
                spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 10.0)),
                ..default()
            },
            connector_stroke(base_color),
            NodeConnector {
                id1: id1.to_string(),
                id2: id2.to_string(),
                path: walking_path,
                walk_cache,
                flash: 0.0,
                base_color,
            },
            Messages::default(),
        ))
        .id()
}

/// Picks a random but readable connector color - each connector gets its own,
/// assigned once at spawn (`generate_line`) rather than every connector
/// sharing the graph-wide `graph_attrs.connection_color`. Random hue at a
/// fixed saturation/lightness keeps every connector legible against the
/// canvas background regardless of which hue it lands on, rather than a
/// uniform-RGB random color that could come out too dark, too light, or
/// muddy.
fn random_connector_color() -> Color {
    let hue = rand::thread_rng().gen_range(0.0..360.0);
    Color::hsl(hue, 0.65, 0.55)
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
/// brief flash on delivery - by blending from the connector's own
/// `base_color` (randomized once per connector at spawn time - see
/// `generate_line`/`random_connector_color`) toward `ACCENT_COLOR`, rather
/// than replacing it outright, so each connector's own color still comes
/// through under the highlight.
pub fn update_connector_style(
    time: Res<Time>,
    selected: Query<&SelectedNodeMarker>,
    mut connectors: Query<(&Messages, &mut NodeConnector, &mut Stroke)>,
    // TEMPORARY - see systems::profiling's doc comment.
    mut prof: ResMut<crate::systems::profiling::ProfilingStats>,
) {
    let __prof_t0 = web_time::Instant::now(); // TEMPORARY
    let selected_names: HashSet<&str> = selected.iter().map(|s| s.node_name.as_str()).collect();

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
        let color = conn.base_color.mix(&ACCENT_COLOR, accent.min(1.0));

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

    /// Builds a connector's path directly from its endpoints with no
    /// crossing bumps or parallel-spacing nudges applied - production code
    /// (`update_connectors`) instead goes through `connector_corner_points`/
    /// `apply_segment_offsets`/`build_connector_path_from_points` (or
    /// `build_curved_connector_path`) directly, since it has real bump/
    /// offset maps spanning every connector in the graph to apply. This is
    /// just what the tests below that only care about single-connector
    /// geometry (lane offsets, box-exit insets) need.
    fn build_connector_path(
        a: Vec2,
        a_half: Vec2,
        a_lane: f32,
        b: Vec2,
        b_half: Vec2,
        b_lane: f32,
        style: ConnectorStyle,
    ) -> Path {
        if style == ConnectorStyle::Curved {
            return build_curved_connector_path(
                a,
                Vec2::ZERO,
                a_half,
                a_lane,
                b,
                Vec2::ZERO,
                b_half,
                b_lane,
            );
        }
        let points = connector_corner_points(
            a,
            Vec2::ZERO,
            a_half,
            a_lane,
            b,
            Vec2::ZERO,
            b_half,
            b_lane,
            style,
        )
        .expect("Straight/Step always return corner points");
        build_connector_path_from_points(&points, &[])
    }

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
    fn test_compute_lane_offsets_spreads_horizontal_fanout() {
        let hub = "hub".to_string();
        let peer_a = "peer_a".to_string();
        let peer_b = "peer_b".to_string();
        let peer_c = "peer_c".to_string();

        let mut all_node_loc: HashMap<String, Vec3> = HashMap::new();
        all_node_loc.insert(hub.clone(), Vec3::new(0.0, 0.0, 0.0));
        // All three peers sit well to the right of the hub at different
        // heights, so all three edges exit the hub's right (horizontal) side.
        all_node_loc.insert(peer_a.clone(), Vec3::new(200.0, -60.0, 0.0));
        all_node_loc.insert(peer_b.clone(), Vec3::new(200.0, 0.0, 0.0));
        all_node_loc.insert(peer_c.clone(), Vec3::new(200.0, 60.0, 0.0));

        let mut extents: HashMap<&str, (Vec2, Vec2)> = HashMap::new();
        extents.insert(hub.as_str(), (Vec2::ZERO, DEFAULT_HALF_EXTENTS));
        extents.insert(peer_a.as_str(), (Vec2::ZERO, DEFAULT_HALF_EXTENTS));
        extents.insert(peer_b.as_str(), (Vec2::ZERO, DEFAULT_HALF_EXTENTS));
        extents.insert(peer_c.as_str(), (Vec2::ZERO, DEFAULT_HALF_EXTENTS));

        let mut desired: HashMap<(&str, &str), (&str, &str)> = HashMap::new();
        for peer in [&peer_a, &peer_b, &peer_c] {
            desired.insert(
                edge_key(hub.as_str(), peer.as_str()),
                (hub.as_str(), peer.as_str()),
            );
        }

        let offsets = compute_lane_offsets(&desired, &all_node_loc, &extents);

        let a = offsets[&(hub.as_str(), peer_a.as_str())];
        let b = offsets[&(hub.as_str(), peer_b.as_str())];
        let c = offsets[&(hub.as_str(), peer_c.as_str())];

        // Ordered along the tangent axis (peer y) the same way the peers
        // themselves are ordered: peer_a (y=-60) < peer_b (y=0) < peer_c (y=60).
        assert!(a < b, "peer_a's lane should be below peer_b's: {a} vs {b}");
        assert!(b < c, "peer_b's lane should be below peer_c's: {b} vs {c}");
        // Symmetric around zero for an evenly-spaced group of three.
        assert!((a + c).abs() < 0.001);
        assert!(b.abs() < 0.001);

        // Each peer only has this one connector touching it, so it gets no
        // lane assignment of its own back toward the hub.
        assert_eq!(offsets.get(&(peer_a.as_str(), hub.as_str())), None);
    }

    #[test]
    fn test_compute_lane_offsets_stays_within_box_bounds() {
        // A tall, narrow hub with many siblings on one side - the assigned
        // offsets must never spread wider than the box itself allows.
        let hub = "hub".to_string();
        let half = Vec2::new(30.0, 20.0);
        let mut all_node_loc: HashMap<String, Vec3> = HashMap::new();
        all_node_loc.insert(hub.clone(), Vec3::new(0.0, 0.0, 0.0));
        let mut extents: HashMap<&str, (Vec2, Vec2)> = HashMap::new();
        extents.insert(hub.as_str(), (Vec2::ZERO, half));

        let peers: Vec<String> = (0..8).map(|i| format!("peer_{i}")).collect();
        for (i, peer) in peers.iter().enumerate() {
            // Spread the peers out vertically but keep them clearly to the
            // right of the hub, so every edge exits horizontally.
            all_node_loc.insert(peer.clone(), Vec3::new(200.0, i as f32 * 10.0 - 35.0, 0.0));
            extents.insert(peer.as_str(), (Vec2::ZERO, DEFAULT_HALF_EXTENTS));
        }

        let mut desired: HashMap<(&str, &str), (&str, &str)> = HashMap::new();
        for peer in &peers {
            desired.insert(
                edge_key(hub.as_str(), peer.as_str()),
                (hub.as_str(), peer.as_str()),
            );
        }

        let offsets = compute_lane_offsets(&desired, &all_node_loc, &extents);
        for peer in &peers {
            let offset = offsets[&(hub.as_str(), peer.as_str())];
            assert!(
                offset.abs() <= half.y * 0.8 + 0.001,
                "offset {offset} escaped the hub's own half-height ({})",
                half.y
            );
        }
    }

    #[test]
    fn test_build_connector_path_anchor_lane_shifts_start_point() {
        let half = Vec2::new(40.0, 40.0);
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(300.0, 0.0); // due east - both ends exit horizontally

        let no_offset = build_connector_path(a, half, 0.0, b, half, 0.0, ConnectorStyle::Straight);
        let offset = build_connector_path(a, half, 20.0, b, half, 0.0, ConnectorStyle::Straight);

        let no_offset_start = walk_path(&no_offset.0)[0];
        let offset_start = walk_path(&offset.0)[0];

        // A due-east link exits `a`'s right edge, so the lane offset (a
        // tangent-axis nudge) should move the start point's y, not its x.
        assert!((no_offset_start[0] - offset_start[0]).abs() < 0.001);
        assert!((offset_start[1] - no_offset_start[1] - 20.0).abs() < 0.5);
    }

    #[test]
    fn test_build_connector_path_lane_offset_clamped_to_box() {
        let half = Vec2::new(20.0, 20.0);
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(300.0, 0.0);

        // An absurdly large lane offset must not push the anchor past the
        // node's own half-extent on that axis.
        let path = build_connector_path(a, half, 500.0, b, half, 0.0, ConnectorStyle::Straight);
        let start = walk_path(&path.0)[0];
        assert!(start[1] <= half.y + 0.01);
    }

    #[test]
    fn test_segment_intersection_finds_crossing_point() {
        let p = segment_intersection(
            Vec2::new(0.0, 5.0),
            Vec2::new(10.0, 5.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(5.0, 10.0),
        );
        let p = p.expect("perpendicular segments crossing mid-span should intersect");
        assert!((p - Vec2::new(5.0, 5.0)).length() < 0.001);
    }

    #[test]
    fn test_segment_intersection_ignores_parallel_and_non_crossing() {
        // Parallel, never meet.
        assert!(segment_intersection(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(0.0, 5.0),
            Vec2::new(10.0, 5.0),
        )
        .is_none());

        // Perpendicular, but the vertical segment sits past the end of the
        // horizontal one - their infinite extensions cross, the segments don't.
        assert!(segment_intersection(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(20.0, -5.0),
            Vec2::new(20.0, 5.0),
        )
        .is_none());
    }

    #[test]
    fn test_compute_crossing_bumps_assigns_exactly_one_owner() {
        // Two unrelated (no shared node) connectors crossing at a right angle.
        let horizontal = (
            ("a", "b"),
            vec![Vec2::new(-50.0, 0.0), Vec2::new(50.0, 0.0)],
        );
        let vertical = (
            ("c", "d"),
            vec![Vec2::new(0.0, -50.0), Vec2::new(0.0, 50.0)],
        );
        let polylines = vec![horizontal.clone(), vertical.clone()];
        let bumps = compute_crossing_bumps(&polylines);

        let owner_has_bump = bumps.contains_key(&horizontal.0);
        let other_has_bump = bumps.contains_key(&vertical.0);
        assert!(
            owner_has_bump ^ other_has_bump,
            "exactly one side of a crossing should own the bridge hop"
        );
        let owned = if owner_has_bump {
            &bumps[&horizontal.0]
        } else {
            &bumps[&vertical.0]
        };
        assert_eq!(owned.len(), 1);
        assert!((owned[0] - Vec2::ZERO).length() < 0.001);
    }

    #[test]
    fn test_compute_crossing_bumps_ignores_connectors_sharing_a_node() {
        // These two "connectors" both touch node "a", and their straight legs
        // do cross at a right angle away from it - but sharing an endpoint
        // means it's not a real crossing to bridge.
        let polylines = vec![
            (("a", "b"), vec![Vec2::new(-50.0, 0.0), Vec2::new(50.0, 0.0)]),
            (("a", "c"), vec![Vec2::new(10.0, -50.0), Vec2::new(10.0, 50.0)]),
        ];
        let bumps = compute_crossing_bumps(&polylines);
        assert!(bumps.is_empty());
    }

    #[test]
    fn test_compute_crossing_bumps_bumps_shallow_angle_crossings_too() {
        // These cross at a shallow angle (~20 degrees from parallel), not a
        // right angle - a bridge hop should still be assigned, since any
        // crossing gets one regardless of angle.
        let polylines = vec![
            (("a", "b"), vec![Vec2::new(-50.0, 0.0), Vec2::new(50.0, 0.0)]),
            (
                ("c", "d"),
                vec![Vec2::new(-10.0, -20.0), Vec2::new(10.0, 20.0)],
            ),
        ];
        let bumps = compute_crossing_bumps(&polylines);
        let total: usize = bumps.values().map(|v| v.len()).sum();
        assert_eq!(total, 1, "the one crossing should get exactly one bump");
    }

    #[test]
    fn test_line_to_with_bumps_detours_around_the_bump_point() {
        let from = Vec2::new(0.0, 0.0);
        let to = Vec2::new(100.0, 0.0);
        let bump = Vec2::new(50.0, 0.0);

        let mut path_builder = PathBuilder::new();
        path_builder.move_to(from);
        line_to_with_bumps(&mut path_builder, from, to, &[bump]);
        let path = path_builder.build();

        let points = walk_path(&path.0);
        assert!((points[0][0] - from.x).abs() < 0.5 && points[0][1].abs() < 0.5);
        let last = points.last().unwrap();
        // `walk_path` samples at a fixed interval and doesn't necessarily
        // land exactly on the final point - just check it's close to `to`,
        // not still mid-hop.
        assert!((last[0] - to.x).abs() < 5.0 && last[1].abs() < 0.5);

        // Somewhere near the bump point, the path should have visibly left
        // the straight y=0 line to hop over the crossing connector.
        let max_deviation = points
            .iter()
            .filter(|p| (p[0] - bump.x).abs() < BRIDGE_RADIUS)
            .map(|p| p[1].abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_deviation > 1.0,
            "expected a visible hop near the bump point, got max deviation {max_deviation}"
        );
    }

    #[test]
    fn test_segment_runs_covers_every_axis_aligned_segment() {
        // An already-aligned single-segment route: one run.
        let aligned = segment_runs(("a", "b"), &[Vec2::new(0.0, 5.0), Vec2::new(50.0, 5.0)]);
        assert_eq!(aligned.len(), 1);
        assert!(aligned[0].horizontal);
        assert_eq!(aligned[0].seg_idx, 0);
        assert!((aligned[0].coord - 5.0).abs() < 0.001);

        // A `Step` Z-route (4 points, 3 segments): horizontal, vertical,
        // horizontal - every one of them is a run now, not just the bus.
        let z_route = segment_runs(
            ("c", "d"),
            &[
                Vec2::new(0.0, 0.0),
                Vec2::new(20.0, 0.0),
                Vec2::new(20.0, 40.0),
                Vec2::new(50.0, 40.0),
            ],
        );
        assert_eq!(z_route.len(), 3);
        assert!(z_route[0].horizontal && (z_route[0].coord - 0.0).abs() < 0.001);
        assert!(!z_route[1].horizontal && (z_route[1].coord - 20.0).abs() < 0.001);
        assert!(z_route[2].horizontal && (z_route[2].coord - 40.0).abs() < 0.001);

        // An `L`-route (3 points, 2 segments): both legs are runs too.
        let l_route = segment_runs(
            ("g", "h"),
            &[Vec2::new(0.0, 0.0), Vec2::new(30.0, 0.0), Vec2::new(30.0, 40.0)],
        );
        assert_eq!(l_route.len(), 2);
        assert!(l_route[0].horizontal);
        assert!(!l_route[1].horizontal);

        // A diagonal `Straight` route's one segment - nothing to nudge.
        assert!(segment_runs(("e", "f"), &[Vec2::new(0.0, 0.0), Vec2::new(30.0, 40.0)]).is_empty());
    }

    #[test]
    fn test_compute_parallel_offsets_spreads_two_close_runs() {
        // Two unrelated horizontal single-segment routes only 4px apart,
        // overlapping in x - well inside `MIN_PARALLEL_GAP`.
        let polylines = vec![
            (("a", "b"), vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)]),
            (("c", "d"), vec![Vec2::new(0.0, 4.0), Vec2::new(100.0, 4.0)]),
        ];
        let offsets = compute_parallel_offsets(&polylines);
        assert_eq!(offsets.len(), 2);

        let new_a = 0.0 + offsets[&(("a", "b"), 0)];
        let new_c = 4.0 + offsets[&(("c", "d"), 0)];
        assert!(
            (new_a - new_c).abs() >= MIN_PARALLEL_GAP - 0.001,
            "spread runs should end up at least MIN_PARALLEL_GAP apart: {new_a} vs {new_c}"
        );
        // Spread symmetrically around the original mean (2.0), not shoved
        // entirely toward one side.
        assert!((new_a + new_c - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_parallel_offsets_covers_l_route_legs() {
        // Two unrelated `L`-routes whose *first* (horizontal) legs run
        // alongside each other only 3px apart - not a designated "bus"
        // segment, just an ordinary leg, and it should still get spread.
        let polylines = vec![
            (
                ("a", "b"),
                vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(50.0, 0.0),
                    Vec2::new(50.0, 40.0),
                ],
            ),
            (
                ("c", "d"),
                vec![
                    Vec2::new(0.0, 3.0),
                    // Vertical leg at a different x than "a"/"b"'s -
                    // shouldn't conflict with it.
                    Vec2::new(90.0, 3.0),
                    Vec2::new(90.0, -40.0),
                ],
            ),
        ];
        let offsets = compute_parallel_offsets(&polylines);
        let new_a = 0.0 + offsets[&(("a", "b"), 0)];
        let new_c = 3.0 + offsets[&(("c", "d"), 0)];
        assert!(
            (new_a - new_c).abs() >= MIN_PARALLEL_GAP - 0.001,
            "L-route legs should be spread apart same as any other run: {new_a} vs {new_c}"
        );
        // Neither route's *second* (vertical) leg was anywhere near another
        // vertical leg, so it should be left untouched.
        assert!(!offsets.contains_key(&(("a", "b"), 1)));
        assert!(!offsets.contains_key(&(("c", "d"), 1)));
    }

    #[test]
    fn test_compute_parallel_offsets_ignores_non_overlapping_runs() {
        // Same y, well within MIN_PARALLEL_GAP, but their x-ranges don't
        // overlap at all - they never actually run alongside each other.
        let polylines = vec![
            (("a", "b"), vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)]),
            (("c", "d"), vec![Vec2::new(20.0, 2.0), Vec2::new(30.0, 2.0)]),
        ];
        let offsets = compute_parallel_offsets(&polylines);
        assert!(offsets.is_empty());
    }

    #[test]
    fn test_compute_parallel_offsets_leaves_well_spaced_runs_alone() {
        let polylines = vec![
            (("a", "b"), vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)]),
            (("c", "d"), vec![Vec2::new(0.0, 50.0), Vec2::new(100.0, 50.0)]),
        ];
        let offsets = compute_parallel_offsets(&polylines);
        assert!(offsets.is_empty());
    }

    #[test]
    fn test_apply_segment_offsets_shifts_only_the_owning_axis() {
        // A Z-route: horizontal, vertical, horizontal.
        let mut points = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 0.0),
            Vec2::new(20.0, 40.0),
            Vec2::new(50.0, 40.0),
        ];
        let mut offsets: HashMap<((&str, &str), usize), f32> = HashMap::new();
        offsets.insert((("a", "b"), 1), 12.0); // nudge only the vertical bus

        apply_segment_offsets(&mut points, ("a", "b"), &offsets);

        // The bus segment's x moved by the offset on both its endpoints...
        assert!((points[1].x - 32.0).abs() < 0.001);
        assert!((points[2].x - 32.0).abs() < 0.001);
        // ...and nothing else did (y coordinates, and the two horizontal
        // legs' endpoints untouched by this segment's own offset).
        assert!((points[0].y - 0.0).abs() < 0.001);
        assert!((points[3].y - 40.0).abs() < 0.001);
        assert!((points[0].x - 0.0).abs() < 0.001);
        assert!((points[3].x - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_node_half_extents_defaults_to_icon_label_box_without_overlay() {
        // No overlay and no template - the default icon+label footprint is
        // what's actually on screen, so that's what the box should be, not
        // the old flat `DEFAULT_HALF_EXTENTS` constant (still used only as a
        // last-resort fallback for a genuinely degenerate box).
        let node = test_node_with_overlay(vec![], false);
        let (offset, half) = node_half_extents(&node);
        // name = "n": half_x = max(ICON_WIDTH, text guess)/2 = 64/2 = 32;
        // half_y = (ICON_HEIGHT + FONT_SIZE + TEXT_DISTANCE_FROM_BOTTOM)/2 = 48,
        // centered 16 units below the node's own origin (label sits under
        // the icon) - see `node_system.rs`'s `icon_label_box`.
        assert!((half.x - 32.0).abs() < 0.001);
        assert!((half.y - 48.0).abs() < 0.001);
        assert!(offset.x.abs() < 0.001);
        assert!((offset.y + 16.0).abs() < 0.001);
    }

    #[test]
    fn test_node_half_extents_from_rect_overlay_with_template() {
        // `template` suppresses the default icon+label, so a templated
        // node's box is the overlay's own tight bounding box alone.
        let node = test_node_with_overlay(
            vec![DrawCmd::Rect {
                x: 0.0,
                y: 0.0,
                w: 160.0,
                h: 50.0,
                radius: 8.0,
                paint: Paint {
                    fill: None,
                    stroke: None,
                    stroke_width: 0.0,
                },
            }],
            true,
        );
        let (offset, half) = node_half_extents(&node);
        assert!((offset.length()) < 0.001, "a centered shape has no offset");
        assert!((half.x - 80.0).abs() < 0.001);
        assert!((half.y - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_node_half_extents_offset_for_off_center_overlay_with_template() {
        // A card drawn from x=0 to x=160 - not centered on the node's own
        // local origin. The old symmetric-around-origin computation would
        // report half.x = 160 (mirroring the shape's far edge onto the
        // opposite, empty side); the real box is 80 wide, centered at x=80.
        let node = test_node_with_overlay(
            vec![DrawCmd::Rect {
                x: 80.0,
                y: 0.0,
                w: 160.0,
                h: 50.0,
                radius: 8.0,
                paint: Paint {
                    fill: None,
                    stroke: None,
                    stroke_width: 0.0,
                },
            }],
            true,
        );
        let (offset, half) = node_half_extents(&node);
        assert!((offset.x - 80.0).abs() < 0.001);
        assert!(offset.y.abs() < 0.001);
        assert!((half.x - 80.0).abs() < 0.001);
        assert!((half.y - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_node_half_extents_unions_overlay_with_icon_when_no_template() {
        // Reproduces `web/static/tutorial/ch5/04_two_phase_commit.yml`'s
        // `update_ui`: a status card floated well clear of the icon
        // (`y: CARD_Y = 80`) via a plain `draw()` call, with no `template`
        // set - so the default icon+label is still rendered underneath it.
        // The box must cover both, not just the floating card, or a
        // connector aims at the card alone and its line cuts straight
        // through the icon to get there.
        let node = test_node_with_overlay(
            vec![DrawCmd::Rect {
                x: 0.0,
                y: 80.0,
                w: 150.0,
                h: 60.0,
                radius: 8.0,
                paint: Paint {
                    fill: None,
                    stroke: None,
                    stroke_width: 0.0,
                },
            }],
            false,
        );
        let (offset, half) = node_half_extents(&node);
        // icon+label box (name "n"): y from -64 to 32. Overlay card: y from
        // 50 to 110. Union: y from -64 to 110 - a 174-tall box, not the
        // card's own 60-tall one.
        let min_y = offset.y - half.y;
        let max_y = offset.y + half.y;
        assert!(
            min_y <= -60.0,
            "box must still reach down to the icon/label, got min_y={min_y}"
        );
        assert!(
            max_y >= 105.0,
            "box must still reach up to the overlay card, got max_y={max_y}"
        );
    }

    /// Builds a minimal `Node` carrying only the `overlay`/`template`
    /// fields these tests care about - the rest are irrelevant to
    /// `node_half_extents`.
    fn test_node_with_overlay(
        overlay: Vec<DrawCmd>,
        has_template: bool,
    ) -> crate::parser::graphv2::Node {
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
                attrs: Attrs {
                    template: has_template.then(Vec::new),
                    ..Attrs::default()
                },
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
