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
//! - `#{ shape: "circle", x, y, r, fill, stroke, stroke_width, opacity }`
//! - `#{ shape: "line",   x1, y1, x2, y2, stroke, stroke_width }`
//! - `#{ shape: "polygon"/"polyline", points: [[x,y], ...], fill, stroke, stroke_width }`
//! - `#{ shape: "text",   x, y, text, size, color }`
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

fn parse_color(s: &str) -> Option<Color> {
    Srgba::hex(s).ok().map(Into::into)
}

fn color_of(map: &rhai::Map, key: &str) -> Option<Color> {
    map.get(key)
        .and_then(|v| v.clone().into_string().ok())
        .and_then(|s| parse_color(&s))
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
    let opacity = num(map, "opacity", 1.0);
    Paint {
        fill: with_opacity(color_of(map, "fill"), opacity),
        stroke: with_opacity(color_of(map, "stroke"), opacity),
        stroke_width: num(map, "stroke_width", 1.0).max(0.0),
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
            color: color_of(&map, "stroke").unwrap_or(Color::BLACK),
            width: num(&map, "stroke_width", 1.0).max(0.0),
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
            size: num(&map, "size", 14.0).max(1.0),
            color: color_of(&map, "color").unwrap_or(Color::BLACK),
        }),
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
