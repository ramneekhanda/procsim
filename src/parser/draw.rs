//! Author-facing drawing primitives.
//!
//! A node's Rhai handler can call the host function `draw([...])` with a list of
//! shape maps to paint a custom overlay on top of that node on the canvas. The
//! overlay is drawn in the node's local coordinate space: origin at the node
//! centre, `x` to the right, `y` up (Bevy convention), units are world pixels at
//! 1x zoom. Shapes are painted in list order (later shapes on top).
//!
//! Supported shapes (the `shape` key selects which):
//! - `#{ shape: "rect",   x, y, w, h, fill, stroke, stroke_width, radius, opacity }`
//! - `#{ shape: "roundedrect", ... }` - identical to `rect`, `radius` just defaults
//!   to `8.0` instead of `0.0`
//! - `#{ shape: "circle", x, y, r, fill, stroke, stroke_width, opacity }`
//! - `#{ shape: "line",   x1, y1, x2, y2, stroke, stroke_width }`
//! - `#{ shape: "polygon"/"polyline", points: [[x,y], ...], fill, stroke, stroke_width }`
//! - `#{ shape: "text",   x, y, text, size, color }`
//! - `#{ shape: "icon",   x, y, w, h, icon }` - `icon` is an id from the graph's
//!   top-level `icons` list (the same id `attrs.icon`/a `send()` payload's `icon`
//!   key reference); falls back to the default system icon if not found.
//!
//! `fill`/`stroke`/`color` are `#rrggbb` (or `#rgb` / `#rrggbbaa`) strings. Numbers
//! may be ints or floats. Unknown shapes and malformed maps are skipped.

use bevy::color::Srgba;
use bevy::prelude::*;
use rhai::Dynamic;

/// Hard cap on shapes kept per node, to bound the cost of a runaway script.
pub const MAX_SHAPES_PER_NODE: usize = 512;

#[derive(Clone, Debug, PartialEq)]
pub struct Paint {
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DrawCmd {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        paint: Paint,
    },
    Circle {
        x: f32,
        y: f32,
        r: f32,
        paint: Paint,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        width: f32,
    },
    Polygon {
        points: Vec<Vec2>,
        closed: bool,
        paint: Paint,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: Color,
    },
    /// An icon-registry image, drawn at an arbitrary node-local position/size -
    /// lets a node template (or a script) place the node's own icon (or any
    /// other registered icon) as part of a custom look instead of relying on
    /// the node's fixed default icon sprite. `icon` is resolved against
    /// `CommonAssets.resource_map` by `systems::node_overlay::spawn_shape`,
    /// the same lookup `systems::node_system::spawn_node` uses for the
    /// default icon, falling back to `"default_system_icon"` if not found.
    Icon {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        icon: String,
    },
    /// A timer progress indicator (bar, ring, pie, or segmented meter) positioned
    /// at an arbitrary node-local position and customized to match the node's theme.
    Progress {
        x: f32,
        y: f32,
        style: DrawProgressStyle,
        track_color: Option<Color>,
        fill_color: Option<Color>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DrawProgressStyle {
    Bar {
        w: f32,
        h: f32,
        radius: f32,
    },
    Ring {
        r: f32,
        thickness: f32,
        start_angle: f32,
        clockwise: bool,
    },
    Pie {
        r: f32,
        start_angle: f32,
        clockwise: bool,
    },
    Segmented {
        w: f32,
        h: f32,
        segments: usize,
        gap: f32,
    },
}

fn num(map: &rhai::Map, key: &str, default: f32) -> f32 {
    match map.get(key) {
        Some(v) => {
            if let Ok(f) = v.as_float() {
                f as f32
            } else if let Ok(i) = v.as_int() {
                i as f32
            } else {
                default
            }
        }
        None => default,
    }
}

fn text_of(map: &rhai::Map, key: &str) -> Option<String> {
    map.get(key).map(|v| v.to_string())
}

/// `#rrggbb` / `#rgb` / `#rrggbbaa` hex string to `Color` - also reused by
/// `graphv2::TemplateShape::to_draw_cmd` so a YAML-declared node template
/// parses colors with the exact same rules `draw()` does.
pub fn parse_color(s: &str) -> Option<Color> {
    Srgba::hex(s).ok().map(Into::into)
}

fn color_of(map: &rhai::Map, key: &str) -> Option<Color> {
    map.get(key)
        .and_then(|v| v.clone().into_string().ok())
        .and_then(|s| parse_color(&s))
}

fn color_of_aliases(map: &rhai::Map, keys: &[&str]) -> Option<Color> {
    for &k in keys {
        if let Some(c) = color_of(map, k) {
            return Some(c);
        }
    }
    None
}

fn num_aliases(map: &rhai::Map, keys: &[&str], default: f32) -> f32 {
    for &k in keys {
        if map.contains_key(k) {
            return num(map, k, default);
        }
    }
    default
}

/// Applies an optional `opacity` multiplier (0..=1, default 1) to a colour's alpha.
fn with_opacity(color: Option<Color>, opacity: f32) -> Option<Color> {
    color.map(|c| {
        let mut s = c.to_srgba();
        s.alpha *= opacity.clamp(0.0, 1.0);
        s.into()
    })
}

fn paint_of(map: &rhai::Map) -> Paint {
    let opacity = num_aliases(map, &["opacity"], 1.0);
    Paint {
        fill: with_opacity(color_of_aliases(map, &["fill", "bg", "color"]), opacity),
        stroke: with_opacity(
            color_of_aliases(map, &["stroke", "border", "border_color"]),
            opacity,
        ),
        stroke_width: num_aliases(map, &["stroke_width", "border_width", "width"], 1.0).max(0.0),
    }
}

fn points_of(map: &rhai::Map) -> Vec<Vec2> {
    let Some(arr) = map
        .get("points")
        .and_then(|v| v.clone().try_cast::<rhai::Array>())
    else {
        return Vec::new();
    };
    arr.into_iter()
        .filter_map(|p| {
            let pair = p.try_cast::<rhai::Array>()?;
            if pair.len() < 2 {
                return None;
            }
            let x = pair[0]
                .as_float()
                .map(|f| f as f32)
                .or_else(|_| pair[0].as_int().map(|i| i as f32))
                .ok()?;
            let y = pair[1]
                .as_float()
                .map(|f| f as f32)
                .or_else(|_| pair[1].as_int().map(|i| i as f32))
                .ok()?;
            Some(Vec2::new(x, y))
        })
        .collect()
}

/// Parses one shape map. Returns `None` for anything not understood.
pub fn parse_draw_cmd(value: &Dynamic) -> Option<DrawCmd> {
    let map = value.read_lock::<rhai::Map>()?;
    let shape = map.get("shape")?.clone().into_string().ok()?;
    match shape.as_str() {
        "rect" => Some(DrawCmd::Rect {
            x: num(&map, "x", 0.0),
            y: num(&map, "y", 0.0),
            w: num(&map, "w", 10.0),
            h: num(&map, "h", 10.0),
            radius: num(&map, "radius", 0.0).max(0.0),
            paint: paint_of(&map),
        }),
        // Same shape as "rect" - just defaults `radius` to something
        // visibly rounded (8.0) instead of 0.0, so the name's promise holds
        // even if the caller doesn't pass `radius` at all. `rect` and
        // `roundedrect` are otherwise identical; `radius: 0` on a
        // `roundedrect` is a valid (if pointless) way to square it back off.
        "roundedrect" => Some(DrawCmd::Rect {
            x: num(&map, "x", 0.0),
            y: num(&map, "y", 0.0),
            w: num(&map, "w", 10.0),
            h: num(&map, "h", 10.0),
            radius: num(&map, "radius", 8.0).max(0.0),
            paint: paint_of(&map),
        }),
        "circle" => Some(DrawCmd::Circle {
            x: num(&map, "x", 0.0),
            y: num(&map, "y", 0.0),
            r: num(&map, "r", 5.0).max(0.0),
            paint: paint_of(&map),
        }),
        "line" => Some(DrawCmd::Line {
            x1: num(&map, "x1", 0.0),
            y1: num(&map, "y1", 0.0),
            x2: num(&map, "x2", 0.0),
            y2: num(&map, "y2", 0.0),
            color: color_of_aliases(&map, &["stroke", "border", "color"]).unwrap_or(Color::BLACK),
            width: num_aliases(&map, &["stroke_width", "border_width", "width"], 1.0).max(0.0),
        }),
        "polygon" | "polyline" => Some(DrawCmd::Polygon {
            points: points_of(&map),
            closed: shape == "polygon",
            paint: paint_of(&map),
        }),
        "text" => Some(DrawCmd::Text {
            x: num(&map, "x", 0.0),
            y: num(&map, "y", 0.0),
            text: text_of(&map, "text").unwrap_or_default(),
            size: num_aliases(&map, &["size", "font_size"], 14.0).max(1.0),
            color: color_of_aliases(&map, &["color", "text_color", "fill"]).unwrap_or(Color::BLACK),
        }),
        "icon" => Some(DrawCmd::Icon {
            x: num(&map, "x", 0.0),
            y: num(&map, "y", 0.0),
            w: num(&map, "w", 64.0).max(1.0),
            h: num(&map, "h", 64.0).max(1.0),
            icon: text_of(&map, "icon").unwrap_or_default(),
        }),
        "progress" => {
            let style_str = text_of(&map, "style").unwrap_or_else(|| "bar".to_string());
            let style = match style_str.as_str() {
                "ring" | "radial" | "circle" => DrawProgressStyle::Ring {
                    r: num(&map, "r", 16.0).max(1.0),
                    thickness: num(&map, "thickness", 3.0).max(0.5),
                    start_angle: num(&map, "start_angle", 90.0),
                    clockwise: map
                        .get("clockwise")
                        .and_then(|v| v.clone().try_cast::<bool>())
                        .unwrap_or(true),
                },
                "pie" => DrawProgressStyle::Pie {
                    r: num(&map, "r", 16.0).max(1.0),
                    start_angle: num(&map, "start_angle", 90.0),
                    clockwise: map
                        .get("clockwise")
                        .and_then(|v| v.clone().try_cast::<bool>())
                        .unwrap_or(true),
                },
                "segmented" => DrawProgressStyle::Segmented {
                    w: num(&map, "w", 32.0).max(1.0),
                    h: num(&map, "h", 4.0).max(1.0),
                    segments: num(&map, "segments", 5.0).max(1.0) as usize,
                    gap: num(&map, "gap", 2.0).max(0.0),
                },
                _ => DrawProgressStyle::Bar {
                    w: num(&map, "w", 32.0).max(1.0),
                    h: num(&map, "h", 3.0).max(1.0),
                    radius: num(&map, "radius", 0.0).max(0.0),
                },
            };
            let opacity = num(&map, "opacity", 1.0);
            Some(DrawCmd::Progress {
                x: num(&map, "x", 0.0),
                y: num(&map, "y", 0.0),
                style,
                track_color: with_opacity(color_of(&map, "track_color"), opacity),
                fill_color: with_opacity(color_of(&map, "fill_color"), opacity),
            })
        }
        _ => None,
    }
}

/// Parses a whole `draw()` payload (a Rhai array of shape maps) into draw commands,
/// dropping malformed entries and clamping to [`MAX_SHAPES_PER_NODE`].
pub fn parse_overlay(shapes: &rhai::Array) -> Vec<DrawCmd> {
    shapes
        .iter()
        .filter_map(parse_draw_cmd)
        .take(MAX_SHAPES_PER_NODE)
        .collect()
}

/// Local-space axis-aligned bounding box (min, max corners) of one drawn
/// shape - used by `node_system::on_click` to size the selection-highlight
/// rectangle around whatever a node's `draw()`/template overlay actually
/// occupies, instead of assuming every node is exactly the default
/// icon+label footprint (a template/`draw()` call can paint well outside
/// it - a wide badge, a shape gallery, anything). Exact for every shape
/// except `Text`, whose box here is only a character-count estimate (no
/// font metrics are available in this pure-data function) - `on_click`
/// prefers the real, already-measured size of the spawned overlay text
/// entity when it can find one, falling back to this guess only if it
/// can't (e.g. the overlay hasn't rendered yet this frame).
pub fn bounds(cmd: &DrawCmd) -> (Vec2, Vec2) {
    match cmd {
        DrawCmd::Rect { x, y, w, h, .. } | DrawCmd::Icon { x, y, w, h, .. } => (
            Vec2::new(x - w / 2.0, y - h / 2.0),
            Vec2::new(x + w / 2.0, y + h / 2.0),
        ),
        DrawCmd::Progress { x, y, style, .. } => match style {
            DrawProgressStyle::Bar { w, h, .. } | DrawProgressStyle::Segmented { w, h, .. } => (
                Vec2::new(x - w / 2.0, y - h / 2.0),
                Vec2::new(x + w / 2.0, y + h / 2.0),
            ),
            DrawProgressStyle::Ring { r, .. } | DrawProgressStyle::Pie { r, .. } => {
                (Vec2::new(x - r, y - r), Vec2::new(x + r, y + r))
            }
        },
        DrawCmd::Circle { x, y, r, .. } => (Vec2::new(x - r, y - r), Vec2::new(x + r, y + r)),
        DrawCmd::Line { x1, y1, x2, y2, .. } => (
            Vec2::new(x1.min(*x2), y1.min(*y2)),
            Vec2::new(x1.max(*x2), y1.max(*y2)),
        ),
        DrawCmd::Polygon { points, .. } => {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for p in points {
                min = min.min(*p);
                max = max.max(*p);
            }
            if points.is_empty() {
                (Vec2::ZERO, Vec2::ZERO)
            } else {
                (min, max)
            }
        }
        DrawCmd::Text {
            x, y, text, size, ..
        } => {
            let half_w = text.chars().count() as f32 * size * 0.3;
            let half_h = size * 0.7;
            (
                Vec2::new(x - half_w, y - half_h),
                Vec2::new(x + half_w, y + half_h),
            )
        }
    }
}
