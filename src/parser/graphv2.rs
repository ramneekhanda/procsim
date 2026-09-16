use crate::{c_log, log_dsa_event};
use bevy::{
    color::{Color, Srgba},
    prelude::*,
    time::Timer,
};
use rand::Rng;
use rhai::Dynamic;
use rhai::{CallFnOptions, Scope, AST};
use schemars::JsonSchema;
use serde::de::Error;
use serde::ser::Serializer;
use serde::{Deserialize, Deserializer, Serialize};
use std::time::Duration;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Debug,
};

fn white_color() -> Color {
    Srgba::hex("#FFFFFF").unwrap().into()
}

fn white_color_str() -> String {
    "#FFFFFF".to_string()
}

fn black_color() -> Color {
    Srgba::hex("#000000").unwrap().into()
}

fn black_color_str() -> String {
    "#000000".to_string()
}

pub fn parse_color_str(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Ok(c) = Srgba::hex(s) {
        return Some(c.into());
    }
    if s.starts_with("rgba(") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 4 {
            let r = parts[0].parse::<f32>().ok()? / 255.0;
            let g = parts[1].parse::<f32>().ok()? / 255.0;
            let b = parts[2].parse::<f32>().ok()? / 255.0;
            let a = parts[3].parse::<f32>().ok()?;
            return Some(Color::srgba(r, g, b, a));
        }
    } else if s.starts_with("rgb(") && s.ends_with(')') {
        let inner = &s[4..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<f32>().ok()? / 255.0;
            let g = parts[1].parse::<f32>().ok()? / 255.0;
            let b = parts[2].parse::<f32>().ok()? / 255.0;
            return Some(Color::srgb(r, g, b));
        }
    }
    None
}

fn deserialize_color<'de, D>(d: D) -> Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    parse_color_str(&s).ok_or_else(|| Error::custom(format!("Invalid color: {}", s)))
}

fn serialize_color<S>(c: &Color, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(format!("\"{}\"", c.to_srgba().to_hex()).as_str())
}

fn deserialize_optional_color<'de, D>(d: D) -> Result<Option<Color>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(d)?;
    match opt {
        Some(s) => parse_color_str(&s)
            .map(Some)
            .ok_or_else(|| Error::custom(format!("Invalid color: {}", s))),
        None => Ok(None),
    }
}

fn serialize_optional_color<S>(c: &Option<Color>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match c {
        Some(col) => s.serialize_str(format!("\"{}\"", col.to_srgba().to_hex()).as_str()),
        None => s.serialize_none(),
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MessageBubbleShape {
    #[default]
    Rounded,
    Pill,
    Box,
    Chamfered,
}

/// Shape of the connector line drawn between two linked nodes (see
/// `systems::update_connectors::build_connector_path`, the single function
/// shared by the initial connector spawn and the per-frame retrace loop).
/// One value for the whole graph, same scope as `connection_color` - not
/// per-connector/per-link. `Step`'s right-angle corner gets rounded for
/// free by the connector's existing `LineJoin::Round` stroke - no arc math
/// needed, it's built from plain straight segments.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorStyle {
    /// Bows perpendicular to the A→B line, magnitude proportional to
    /// node distance (clamped).
    Curved,
    /// A single straight segment between the two (inset) node edges.
    Straight,
    /// A three-segment orthogonal route: horizontal out from the start,
    /// vertical to align, horizontal in to the end (collapses to a single
    /// segment when the two nodes are already level). Default when
    /// `graph_attrs.connector_style` is unset.
    #[default]
    Step,
}

fn default_message_stroke_width() -> f32 {
    1.5
}
fn default_message_font_size() -> f32 {
    16.0
}
fn default_message_icon_size() -> f32 {
    20.0
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct MessageTheme {
    #[serde(default)]
    pub shape: MessageBubbleShape,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub bg: Option<Color>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub stroke: Option<Color>,

    #[serde(default = "default_message_stroke_width")]
    pub stroke_width: f32,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub text_color: Option<Color>,

    #[serde(default = "default_message_font_size")]
    pub font_size: f32,

    #[serde(default = "default_message_icon_size")]
    pub icon_size: f32,
}

impl Default for MessageTheme {
    fn default() -> Self {
        Self {
            shape: MessageBubbleShape::default(),
            bg: None,
            stroke: None,
            stroke_width: default_message_stroke_width(),
            text_color: None,
            font_size: default_message_font_size(),
            icon_size: default_message_icon_size(),
        }
    }
}

fn default_explain_border_width() -> f32 {
    1.5
}
fn default_explain_font_size() -> f32 {
    13.5
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ExplainTheme {
    #[serde(default)]
    pub shape: MessageBubbleShape,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub bg: Option<Color>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub border: Option<Color>,

    #[serde(default = "default_explain_border_width")]
    pub border_width: f32,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub text_color: Option<Color>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub accent: Option<Color>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub button_text_color: Option<Color>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub shadow_color: Option<Color>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_color",
        deserialize_with = "deserialize_optional_color"
    )]
    #[schemars(with = "Option<String>")]
    pub backdrop_color: Option<Color>,

    #[serde(default = "default_explain_font_size")]
    pub font_size: f32,
}

impl Default for ExplainTheme {
    fn default() -> Self {
        Self {
            shape: MessageBubbleShape::default(),
            bg: None,
            border: None,
            border_width: default_explain_border_width(),
            text_color: None,
            accent: None,
            button_text_color: None,
            shadow_color: None,
            backdrop_color: None,
            font_size: default_explain_font_size(),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct GraphAttrs {
    #[schemars(with = "String", default = "white_color_str")]
    #[serde(
        default = "white_color",
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub background: Color,

    #[schemars(with = "String", default = "black_color_str")]
    #[serde(
        default = "black_color",
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub connection_color: Color,

    #[serde(default)]
    pub title: String,

    #[schemars(with = "String", default = "black_color_str")]
    #[serde(
        default = "black_color",
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub text_color: Color,

    /// Shape of every connector line in the graph - see `ConnectorStyle`'s
    /// doc comment. Defaults to `step` (right-angle routing) when unset.
    #[serde(default)]
    pub connector_style: ConnectorStyle,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_theme: Option<MessageTheme>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain_theme: Option<ExplainTheme>,

    /// URL of a font file to use for every piece of canvas text (node labels,
    /// message/connector text, draw()/template text shapes, group titles,
    /// explain bubbles - everything rendered through the single shared
    /// `"default_font"` resource, see `systems::resource_loader`). `None`
    /// keeps the app-wide default (Comic Neue). Like every other `GraphAttrs`
    /// field, this is one value for the whole graph, not a per-node/per-shape
    /// choice - a local `graph_attrs.font` always wins over an imported
    /// theme's, and among imports the first one in `imports:` order that sets
    /// it wins (see `merge_graph_definitions`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

impl Default for GraphAttrs {
    fn default() -> Self {
        GraphAttrs {
            background: white_color(),
            connection_color: black_color(),
            title: String::new(),
            text_color: black_color(),
            connector_style: ConnectorStyle::default(),
            message_theme: None,
            explain_theme: None,
            font: None,
        }
    }
}

/// Either a fixed tick interval in seconds, or a `{min, max}` range. A range with
/// `jitter: false` (the default) is resolved to one random value once, at node
/// creation, and stays constant for that instance's lifetime. `jitter: true` instead
/// re-picks a fresh random value within the range every time the timer fires.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum Ticks {
    Fixed(u64),
    Range {
        min: u64,
        max: u64,
        #[serde(default)]
        jitter: bool,
    },
}

impl Default for Ticks {
    fn default() -> Self {
        Ticks::Fixed(2)
    }
}

/// Resolves a `Ticks` value to a concrete interval in seconds, picking a random value
/// within the range for `Ticks::Range`.
pub fn resolve_ticks_secs(ticks: &Ticks) -> u64 {
    match ticks {
        Ticks::Fixed(n) => *n,
        Ticks::Range { min, max, .. } => rand::thread_rng().gen_range(*min..=*max),
    }
}

fn default_stroke_width() -> f32 {
    1.0
}

fn default_opacity() -> f32 {
    1.0
}

fn default_rounded_rect_radius() -> f32 {
    8.0
}

fn default_text_size() -> f32 {
    14.0
}

fn default_text_color() -> String {
    "#000000".to_string()
}

/// Matches `systems::node_system`'s `ICON_WIDTH`/`ICON_HEIGHT` - a
/// `TemplateShape::Icon` with no explicit `w`/`h` reads the same size as a
/// node's normal default icon sprite.
fn default_icon_size() -> f32 {
    64.0
}

fn default_progress_style() -> String {
    "bar".to_string()
}

fn default_bar_width() -> f32 {
    32.0
}

fn default_bar_height() -> f32 {
    3.0
}

fn default_progress_radius() -> f32 {
    16.0
}

fn default_progress_thickness() -> f32 {
    3.0
}

fn default_progress_start_angle() -> f32 {
    90.0
}

fn default_true() -> bool {
    true
}

fn default_progress_segments() -> usize {
    5
}

fn default_progress_gap() -> f32 {
    2.0
}

/// A YAML-declarable equivalent of one shape in a Rhai handler's `draw([...])`
/// call (see `parser::draw`'s module doc for the shapes/fields this mirrors) -
/// lets a node type get a custom on-canvas look with no script at all, via
/// `Attrs::template`. Colors are the same `#rrggbb`/`#rgb`/`#rrggbbaa` hex
/// strings `draw()` accepts, parsed by the same `parser::draw::parse_color`.
///
/// String-valued fields (`text`, `fill`, `stroke`, `color`, `icon`) may embed
/// `{{param}}` placeholders, substituted per node instance before the shape is
/// drawn - see `NodeTemplateDef`'s doc comment for where `param`'s value comes
/// from. A template with no placeholders at all still works exactly as before
/// (the substitution pass is a no-op on a string containing no `{{`).
///
/// Otherwise static - a template can't reference a node's `state` or branch
/// on logic the way a live `draw()` call can (there's no script evaluation
/// involved, only string substitution). A node type that also has a Rhai `fn`
/// and calls `draw()` from it will have that call *replace* the
/// template-seeded overlay the first time it runs (same "draw() replaces the
/// overlay" rule documented on `Node::overlay`), so a template is really a
/// default/initial look, not a persistent base layer underneath dynamic
/// shapes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum TemplateShape {
    Rect {
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        #[serde(default)]
        w: f32,
        #[serde(default)]
        h: f32,
        #[serde(default)]
        radius: f32,
        #[serde(default, alias = "bg", alias = "color")]
        fill: Option<String>,
        #[serde(default, alias = "border")]
        stroke: Option<String>,
        #[serde(default = "default_stroke_width", alias = "border_width")]
        stroke_width: f32,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    /// Identical to `Rect` - `radius` just defaults to `8.0` instead of
    /// `0.0`, so `shape: roundedrect` alone (no `radius` given) still reads
    /// as rounded. `radius: 0` on a `roundedrect` is a valid (if pointless)
    /// way to square it back off.
    #[serde(rename = "roundedrect", alias = "rounded_rect")]
    RoundedRect {
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        #[serde(default)]
        w: f32,
        #[serde(default)]
        h: f32,
        #[serde(default = "default_rounded_rect_radius")]
        radius: f32,
        #[serde(default, alias = "bg", alias = "color")]
        fill: Option<String>,
        #[serde(default, alias = "border")]
        stroke: Option<String>,
        #[serde(default = "default_stroke_width", alias = "border_width")]
        stroke_width: f32,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    Circle {
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        #[serde(default = "default_progress_radius")]
        r: f32,
        #[serde(default, alias = "bg", alias = "color")]
        fill: Option<String>,
        #[serde(default, alias = "border")]
        stroke: Option<String>,
        #[serde(default = "default_stroke_width", alias = "border_width")]
        stroke_width: f32,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    Line {
        #[serde(default)]
        x1: f32,
        #[serde(default)]
        y1: f32,
        #[serde(default)]
        x2: f32,
        #[serde(default)]
        y2: f32,
        #[serde(alias = "color")]
        stroke: String,
        #[serde(default = "default_stroke_width", alias = "border_width")]
        stroke_width: f32,
    },
    Polygon {
        points: Vec<[f32; 2]>,
        /// `true` connects the last point back to the first (a closed
        /// polygon); `false` leaves it open (a polyline) - the two shape
        /// names `draw()` accepts as sugar for this are collapsed to one
        /// explicit flag here.
        #[serde(default)]
        closed: bool,
        #[serde(default, alias = "bg", alias = "color")]
        fill: Option<String>,
        #[serde(default, alias = "border")]
        stroke: Option<String>,
        #[serde(default = "default_stroke_width", alias = "border_width")]
        stroke_width: f32,
    },
    Text {
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        text: String,
        #[serde(default = "default_text_size", alias = "font_size")]
        size: f32,
        #[serde(default = "default_text_color")]
        color: String,
        #[serde(default)]
        bold: bool,
    },
    /// An icon-registry image at an arbitrary node-local position/size -
    /// typically `icon: "{{icon}}"`, so the template draws whichever icon
    /// each node type/instance actually binds (see `NodeTemplateDef`), rather
    /// than one hardcoded icon shared by every node type that uses it.
    ///
    /// A template is the *whole* node visual once it's set at all (see
    /// `Attrs::template`'s doc comment) - the default icon sprite
    /// `node_system::spawn_node` would otherwise draw is skipped entirely,
    /// so a template with no `Icon` shape of its own means that node type
    /// simply has no icon. And since a script's own `draw()` call replaces
    /// the *entire* overlay a template seeded, not just the parts that
    /// call is meant to update, a handler that calls `draw()` to refresh
    /// some other live bit (a status badge, say) needs to re-include its own `Icon`
    /// shape in that same call, or the icon disappears the moment the
    /// handler first runs.
    Icon {
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        #[serde(default = "default_icon_size")]
        w: f32,
        #[serde(default = "default_icon_size")]
        h: f32,
        icon: String,
    },
    /// A timer progress indicator (linear bar, circular ring, pie wedge, or segmented meter)
    /// positioned at arbitrary node-local coordinates with configurable dimensions and theme colors.
    Progress {
        #[serde(default = "default_progress_style")]
        style: String,
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
        #[serde(default = "default_bar_width")]
        w: f32,
        #[serde(default = "default_bar_height")]
        h: f32,
        #[serde(default = "default_progress_radius")]
        r: f32,
        #[serde(default = "default_progress_thickness")]
        thickness: f32,
        #[serde(default = "default_progress_start_angle")]
        start_angle: f32,
        #[serde(default = "default_true")]
        clockwise: bool,
        #[serde(default)]
        radius: f32,
        #[serde(default = "default_progress_segments")]
        segments: usize,
        #[serde(default = "default_progress_gap")]
        gap: f32,
        #[serde(default)]
        track_color: Option<String>,
        #[serde(default)]
        fill_color: Option<String>,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
}

impl TemplateShape {
    /// Converts to the same `DrawCmd` a parsed `draw()` call produces, so a
    /// YAML template renders through the exact same
    /// `systems::node_overlay` pipeline. Returns `None` only for an
    /// unparsable color string (the YAML schema already rules out anything
    /// else malformed, unlike `draw()`'s free-form Rhai maps).
    pub fn to_draw_cmd(&self) -> Option<crate::parser::draw::DrawCmd> {
        use crate::parser::draw::{parse_color, DrawCmd};
        Some(match self {
            TemplateShape::Rect {
                x,
                y,
                w,
                h,
                radius,
                fill,
                stroke,
                stroke_width,
                opacity,
            }
            | TemplateShape::RoundedRect {
                x,
                y,
                w,
                h,
                radius,
                fill,
                stroke,
                stroke_width,
                opacity,
            } => DrawCmd::Rect {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                radius: *radius,
                paint: paint_of(fill, stroke, *stroke_width, *opacity)?,
            },
            TemplateShape::Circle {
                x,
                y,
                r,
                fill,
                stroke,
                stroke_width,
                opacity,
            } => DrawCmd::Circle {
                x: *x,
                y: *y,
                r: *r,
                paint: paint_of(fill, stroke, *stroke_width, *opacity)?,
            },
            TemplateShape::Line {
                x1,
                y1,
                x2,
                y2,
                stroke,
                stroke_width,
            } => DrawCmd::Line {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                color: parse_color(stroke)?,
                width: *stroke_width,
            },
            TemplateShape::Polygon {
                points,
                closed,
                fill,
                stroke,
                stroke_width,
            } => DrawCmd::Polygon {
                points: points.iter().map(|[x, y]| Vec2::new(*x, *y)).collect(),
                closed: *closed,
                paint: paint_of(fill, stroke, *stroke_width, 1.0)?,
            },
            TemplateShape::Text {
                x,
                y,
                text,
                size,
                color,
                bold: _,
            } => DrawCmd::Text {
                x: *x,
                y: *y,
                text: text.clone(),
                size: *size,
                color: parse_color(color)?,
            },
            TemplateShape::Icon { x, y, w, h, icon } => DrawCmd::Icon {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                icon: icon.clone(),
            },
            TemplateShape::Progress {
                style,
                x,
                y,
                w,
                h,
                r,
                thickness,
                start_angle,
                clockwise,
                radius,
                segments,
                gap,
                track_color,
                fill_color,
                opacity,
            } => {
                let opacity = opacity.clamp(0.0, 1.0);
                let with_op = |c: Color| {
                    let mut s = c.to_srgba();
                    s.alpha *= opacity;
                    Color::from(s)
                };
                let track_color = match track_color {
                    Some(s) => parse_color(s).map(with_op),
                    None => None,
                };
                let fill_color = match fill_color {
                    Some(s) => parse_color(s).map(with_op),
                    None => None,
                };
                let style = match style.as_str() {
                    "ring" | "radial" | "circle" => crate::parser::draw::DrawProgressStyle::Ring {
                        r: *r,
                        thickness: *thickness,
                        start_angle: *start_angle,
                        clockwise: *clockwise,
                    },
                    "pie" => crate::parser::draw::DrawProgressStyle::Pie {
                        r: *r,
                        start_angle: *start_angle,
                        clockwise: *clockwise,
                    },
                    "segmented" => crate::parser::draw::DrawProgressStyle::Segmented {
                        w: *w,
                        h: *h,
                        segments: *segments,
                        gap: *gap,
                    },
                    _ => crate::parser::draw::DrawProgressStyle::Bar {
                        w: *w,
                        h: *h,
                        radius: *radius,
                    },
                };
                DrawCmd::Progress {
                    x: *x,
                    y: *y,
                    style,
                    track_color,
                    fill_color,
                }
            }
        })
    }

    /// Returns a copy of `self` with every `{{param}}` placeholder in its
    /// string-valued fields replaced by `params[param]` - unmatched
    /// placeholders (a typo'd param name, say) are left verbatim rather than
    /// silently dropped, so the mistake is visible on canvas instead of
    /// invisible. A field with no `{{` at all is untouched.
    pub fn substituted(&self, params: &HashMap<String, String>) -> TemplateShape {
        fn sub(s: &str, params: &HashMap<String, String>) -> String {
            if !s.contains("{{") {
                return s.to_string();
            }
            let mut out = s.to_string();
            for (k, v) in params {
                out = out.replace(&format!("{{{{{}}}}}", k), v);
            }
            out
        }
        match self {
            TemplateShape::Rect {
                x,
                y,
                w,
                h,
                radius,
                fill,
                stroke,
                stroke_width,
                opacity,
            } => TemplateShape::Rect {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                radius: *radius,
                fill: fill.as_deref().map(|s| sub(s, params)),
                stroke: stroke.as_deref().map(|s| sub(s, params)),
                stroke_width: *stroke_width,
                opacity: *opacity,
            },
            TemplateShape::RoundedRect {
                x,
                y,
                w,
                h,
                radius,
                fill,
                stroke,
                stroke_width,
                opacity,
            } => TemplateShape::RoundedRect {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                radius: *radius,
                fill: fill.as_deref().map(|s| sub(s, params)),
                stroke: stroke.as_deref().map(|s| sub(s, params)),
                stroke_width: *stroke_width,
                opacity: *opacity,
            },
            TemplateShape::Circle {
                x,
                y,
                r,
                fill,
                stroke,
                stroke_width,
                opacity,
            } => TemplateShape::Circle {
                x: *x,
                y: *y,
                r: *r,
                fill: fill.as_deref().map(|s| sub(s, params)),
                stroke: stroke.as_deref().map(|s| sub(s, params)),
                stroke_width: *stroke_width,
                opacity: *opacity,
            },
            TemplateShape::Line {
                x1,
                y1,
                x2,
                y2,
                stroke,
                stroke_width,
            } => TemplateShape::Line {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                stroke: sub(stroke, params),
                stroke_width: *stroke_width,
            },
            TemplateShape::Polygon {
                points,
                closed,
                fill,
                stroke,
                stroke_width,
            } => TemplateShape::Polygon {
                points: points.clone(),
                closed: *closed,
                fill: fill.as_deref().map(|s| sub(s, params)),
                stroke: stroke.as_deref().map(|s| sub(s, params)),
                stroke_width: *stroke_width,
            },
            TemplateShape::Text {
                x,
                y,
                text,
                size,
                color,
                bold,
            } => TemplateShape::Text {
                x: *x,
                y: *y,
                text: sub(text, params),
                size: *size,
                color: sub(color, params),
                bold: *bold,
            },
            TemplateShape::Icon { x, y, w, h, icon } => TemplateShape::Icon {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                icon: sub(icon, params),
            },
            TemplateShape::Progress {
                style,
                x,
                y,
                w,
                h,
                r,
                thickness,
                start_angle,
                clockwise,
                radius,
                segments,
                gap,
                track_color,
                fill_color,
                opacity,
            } => TemplateShape::Progress {
                style: sub(style, params),
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                r: *r,
                thickness: *thickness,
                start_angle: *start_angle,
                clockwise: *clockwise,
                radius: *radius,
                segments: *segments,
                gap: *gap,
                track_color: track_color.as_deref().map(|s| sub(s, params)),
                fill_color: fill_color.as_deref().map(|s| sub(s, params)),
                opacity: *opacity,
            },
        }
    }
}

/// Shared by every `TemplateShape` variant with fill/stroke - parses both
/// (either may be absent) and applies `opacity` the same way
/// `parser::draw::paint_of` does for a Rhai `draw()` call.
fn paint_of(
    fill: &Option<String>,
    stroke: &Option<String>,
    stroke_width: f32,
    opacity: f32,
) -> Option<crate::parser::draw::Paint> {
    use crate::parser::draw::{parse_color, Paint};
    let opacity = opacity.clamp(0.0, 1.0);
    let with_opacity = |c: Color| {
        let mut s = c.to_srgba();
        s.alpha *= opacity;
        Color::from(s)
    };
    let fill = match fill {
        Some(s) => Some(with_opacity(parse_color(s)?)),
        None => None,
    };
    let stroke = match stroke {
        Some(s) => Some(with_opacity(parse_color(s)?)),
        None => None,
    };
    Some(Paint {
        fill,
        stroke,
        stroke_width: stroke_width.max(0.0),
    })
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct Attrs {
    #[serde(default)]
    pub ticks: Ticks,
    /// Id of an entry in the graph's top-level `icons` list, used as this node type's
    /// on-canvas icon.
    pub icon: Option<String>,
    /// A static custom look for this node type, declared directly in YAML
    /// instead of via a Rhai `draw()` call - see `TemplateShape`'s doc
    /// comment. Renders on top of the icon/label, in the same node-local
    /// coordinate space `draw()` uses (origin at node center, y up). Can be
    /// combined with `template_ref` (that template's shapes are applied
    /// first, these on top of them) or used alone for a one-off look not
    /// worth naming and sharing.
    #[serde(default)]
    pub template: Option<Vec<TemplateShape>>,
    /// Id of an entry in the graph's top-level `node_templates` list (see
    /// `NodeTemplateDef`) - reuses that named template's shapes for this
    /// node type instead of (or as a base for, if `template` is also set)
    /// writing them out again inline. Resolved once at parse time
    /// (`parse_graph2`), the same way `icon` is validated against `icons`.
    #[serde(default)]
    pub template_ref: Option<String>,
    /// Explicit `{{param}}` -> value bindings for this node type's template
    /// (own `template` and/or a referenced `template_ref`) - merged over the
    /// two builtin auto-bound params every node gets for free: `node_name`
    /// (this specific node *instance*'s own name, so a template shared by
    /// several instances still labels each one correctly) and `icon` (this
    /// node type's own `attrs.icon`, if set). An entry here overrides the
    /// matching builtin if both set the same key - e.g. to have a template
    /// show a different icon than the node's actual on-canvas icon. See
    /// `NodeTemplateDef`'s doc comment for the overall mechanism.
    #[serde(default, alias = "params")]
    pub template_params: Option<HashMap<String, String>>,
}

impl Default for Attrs {
    fn default() -> Self {
        Attrs {
            ticks: Ticks::default(),
            icon: None,
            template: None,
            template_ref: None,
            template_params: None,
        }
    }
}

/// One reusable named entry in the graph's top-level `node_templates` list -
/// referenced by `Attrs::template_ref`, the same pattern `IconDef`/`icons`
/// already uses for sharing an icon across node types instead of repeating a
/// URL. Exists purely to be referenced; has no meaning on its own.
///
/// A single template can give a consistent look to every node type that uses
/// it - e.g. an icon plus a name label laid out the same way for every node
/// in the graph - by writing its shapes generically, in terms of `{{param}}`
/// placeholders (see `TemplateShape`'s doc comment), instead of one specific
/// icon/label. `params` documents which placeholders the template expects,
/// for the YAML author's own reference and for the JSON-schema-driven editor;
/// it isn't itself checked against what a node type actually binds - an
/// unresolved `{{param}}` just renders literally rather than erroring, so a
/// typo is a visible canvas bug, not a load-time failure.
///
/// Two params are always available with no binding needed at all -
/// `node_name` (the specific node instance's own name) and `icon` (that node
/// type's own `attrs.icon`) - see `Attrs::template_params` for how those
/// auto-bound values combine with any explicit bindings.
#[derive(Default, Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct NodeTemplateDef {
    pub id: String,
    /// Documents the `{{param}}` names this template's shapes reference,
    /// beyond the always-available `node_name`/`icon` builtins - purely
    /// informational, not validated.
    #[serde(default)]
    pub params: Vec<String>,
    #[serde(alias = "template")]
    pub shapes: Vec<TemplateShape>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum ParamType {
    Bool {
        default: bool,
    },
    Float {
        min: f64,
        max: f64,
        default: f64,
    },
    Integer {
        min: i64,
        max: i64,
        default: i64,
    },
    String {
        default: String,
    },
    Option {
        values: HashSet<String>,
        default: String,
    },
}

impl Default for ParamType {
    fn default() -> Self {
        ParamType::String {
            default: "".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct NodeParams {
    pub name: String,
    #[serde(flatten)]
    pub param_type: ParamType,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct NodeType {
    #[serde(alias = "name")]
    pub id: String,
    #[serde(rename = "fn")]
    pub func: Option<String>,
    #[serde(default)]
    pub attrs: Attrs,
    pub params: Option<Vec<NodeParams>>,
    #[serde(skip)]
    pub ast: AST,
}

impl std::cmp::PartialEq for NodeType {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.func == other.func
            && self.attrs == other.attrs
            && self.params == other.params
    }
}

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub node_data: NodeType,
    pub links: Vec<String>,
    pub timer: Timer,

    pub ast: AST, //TODO: change this to reference
    pub scope: Scope<'static>,
    pub state: Dynamic,

    /// Custom canvas overlay for this node, set from a Rhai handler via `draw([...])`.
    /// Persists until the node calls `draw()` again; `overlay_dirty` flags a pending
    /// re-render for `systems::node_overlay`.
    pub overlay: Vec<crate::parser::draw::DrawCmd>,
    pub overlay_dirty: bool,
    /// `{{param}}` -> value overrides set at runtime via `update_node_params(#{...})`
    /// (see `apply_template_param_updates`), layered on top of `template_params()`'s
    /// defaults/YAML `template_params` and persisted across calls so a later update
    /// only needs to name the fields that actually changed. Only meaningful for a
    /// node type with `attrs.template`/`template_ref` - a plain node has nothing to
    /// re-substitute these against.
    pub template_overrides: HashMap<String, String>,
}

impl std::cmp::PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.node_data == other.node_data
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LayoutType {
    #[default]
    Hierarchical,
    Grid,
    Circular,
    Manual,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LayoutDirection {
    #[default]
    Lr,
    Tb,
    Rl,
    Bt,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct LayoutConfig {
    #[serde(default)]
    pub r#type: LayoutType,
    #[serde(default)]
    pub direction: LayoutDirection,
    #[serde(
        default,
        alias = "rank_spacing",
        alias = "rank spacing",
        alias = "rank-spacing",
        alias = "rank_separation",
        alias = "rankSep",
        alias = "rankSpacing",
        alias = "rank sep",
        alias = "rank-sep"
    )]
    pub rank_sep: Option<f32>,
    #[serde(
        default,
        alias = "node_spacing",
        alias = "node spacing",
        alias = "node-spacing",
        alias = "node_separation",
        alias = "nodeSep",
        alias = "nodeSpacing",
        alias = "node sep",
        alias = "node-sep"
    )]
    pub node_sep: Option<f32>,
    #[serde(
        default,
        alias = "group_spacing",
        alias = "group spacing",
        alias = "group-spacing",
        alias = "group_separation",
        alias = "groupSep",
        alias = "groupSpacing",
        alias = "group sep",
        alias = "group-sep"
    )]
    pub group_sep: Option<f32>,
    #[serde(default)]
    pub draggable: Option<bool>,
}

impl LayoutConfig {
    pub fn get_rank_sep(&self) -> f32 {
        self.rank_sep.unwrap_or(260.0)
    }

    pub fn get_node_sep(&self) -> f32 {
        self.node_sep.unwrap_or(140.0)
    }

    pub fn get_group_sep(&self) -> f32 {
        self.group_sep.unwrap_or(320.0)
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            r#type: LayoutType::Hierarchical,
            direction: LayoutDirection::Lr,
            rank_sep: Some(260.0),
            node_sep: Some(140.0),
            group_sep: Some(320.0),
            draggable: Some(false),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema, Default)]
pub struct GroupLayoutConfig {
    #[serde(default)]
    pub direction: Option<LayoutDirection>,
    #[serde(
        default,
        alias = "node_sep",
        alias = "node sep",
        alias = "node-sep",
        alias = "node_spacing",
        alias = "node spacing",
        alias = "node-spacing",
        alias = "nodeSep",
        alias = "nodeSpacing",
        alias = "spacing",
        alias = "sep"
    )]
    pub sep: Option<f32>,
    #[serde(default, alias = "cols", alias = "col")]
    pub columns: Option<usize>,
}

impl GroupLayoutConfig {
    pub fn get_spacing(&self, default_val: f32) -> f32 {
        self.sep.unwrap_or(default_val)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema, Default)]
pub struct GroupStyle {
    #[serde(default, rename = "box")]
    pub r#box: Option<bool>,
    #[serde(default)]
    pub border: Option<String>,
    #[serde(default)]
    pub border_width: Option<f32>,
    #[serde(default)]
    pub border_style: Option<String>,
    #[serde(default)]
    pub bg: Option<String>,
    #[serde(default)]
    pub padding: Option<f32>,
    #[serde(default)]
    pub radius: Option<f32>,
}

#[derive(Deserialize)]
struct GroupDefHelper {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    direction: Option<LayoutDirection>,
    #[serde(
        default,
        alias = "sep",
        alias = "node_sep",
        alias = "node_spacing",
        alias = "nodeSep"
    )]
    spacing: Option<f32>,
    #[serde(default)]
    layout: Option<GroupLayoutConfig>,
    #[serde(default)]
    style: Option<GroupStyle>,
    #[serde(default)]
    nodes: Vec<String>,
}

pub fn deserialize_groups<'de, D>(deserializer: D) -> Result<Vec<GroupDef>, D::Error>
where
    D: Deserializer<'de>,
{
    struct GroupsVisitor;

    impl<'de> serde::de::Visitor<'de> for GroupsVisitor {
        type Value = Vec<GroupDef>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a list or map of group definitions")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut groups = Vec::new();
            while let Some(raw) = seq.next_element::<serde_yaml::Value>()? {
                if let Ok(helper) = serde_yaml::from_value::<GroupDefHelper>(raw.clone()) {
                    if let Some(id) = helper.id {
                        let mut layout = helper.layout.unwrap_or_default();
                        if helper.direction.is_some() {
                            layout.direction = helper.direction;
                        }
                        if helper.spacing.is_some() {
                            layout.sep = helper.spacing;
                        }
                        let layout_opt = if layout.direction.is_some()
                            || layout.sep.is_some()
                            || layout.columns.is_some()
                        {
                            Some(layout)
                        } else {
                            None
                        };
                        groups.push(GroupDef {
                            id,
                            title: helper.title,
                            layout: layout_opt,
                            style: helper.style,
                            nodes: helper.nodes,
                        });
                        continue;
                    }
                }
                if let Some(map) = raw.as_mapping() {
                    for (k, v) in map {
                        let id = k.as_str().unwrap_or_default().to_string();
                        if let Ok(helper) = serde_yaml::from_value::<GroupDefHelper>(v.clone()) {
                            let mut layout = helper.layout.unwrap_or_default();
                            if helper.direction.is_some() {
                                layout.direction = helper.direction;
                            }
                            if helper.spacing.is_some() {
                                layout.sep = helper.spacing;
                            }
                            let layout_opt = if layout.direction.is_some()
                                || layout.sep.is_some()
                                || layout.columns.is_some()
                            {
                                Some(layout)
                            } else {
                                None
                            };
                            groups.push(GroupDef {
                                id,
                                title: helper.title,
                                layout: layout_opt,
                                style: helper.style,
                                nodes: helper.nodes,
                            });
                        }
                    }
                }
            }
            Ok(groups)
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut groups = Vec::new();
            while let Some((key, val)) = access.next_entry::<String, serde_yaml::Value>()? {
                if let Ok(helper) = serde_yaml::from_value::<GroupDefHelper>(val.clone()) {
                    let mut layout = helper.layout.unwrap_or_default();
                    if helper.direction.is_some() {
                        layout.direction = helper.direction;
                    }
                    if helper.spacing.is_some() {
                        layout.sep = helper.spacing;
                    }
                    let layout_opt = if layout.direction.is_some()
                        || layout.sep.is_some()
                        || layout.columns.is_some()
                    {
                        Some(layout)
                    } else {
                        None
                    };
                    groups.push(GroupDef {
                        id: key,
                        title: helper.title,
                        layout: layout_opt,
                        style: helper.style,
                        nodes: helper.nodes,
                    });
                } else if let Ok(mut g) = serde_yaml::from_value::<GroupDef>(val.clone()) {
                    g.id = key;
                    groups.push(g);
                }
            }
            Ok(groups)
        }
    }

    deserializer.deserialize_any(GroupsVisitor)
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema, Default)]
pub struct GroupDef {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub layout: Option<GroupLayoutConfig>,
    #[serde(default)]
    pub style: Option<GroupStyle>,
    #[serde(default)]
    pub nodes: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct NodeConnection {
    pub name: String,
    pub node_type: String,
    pub links: Vec<String>,
    /// Per-instance Rhai script override - when set, only *this* node runs it
    /// instead of `node_type`'s own `fn`. Everything else (icon, template,
    /// params, ticks) still comes from `node_type` - this exists so one
    /// instance of a shared/imported type (e.g. a plib's `lambda_func`) can
    /// behave differently without copy-pasting the whole type definition
    /// just to change its script. Compiled once, the same way a node type's
    /// own `fn` is (see `parse_graph2_with_sources`).
    #[serde(rename = "fn", default, skip_serializing_if = "Option::is_none")]
    pub func: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draggable: Option<bool>,
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct IconDef {
    pub id: String,
    pub url: String,
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct GraphDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<crate::parser::imports::ImportDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConfig>,
    #[serde(
        default,
        deserialize_with = "deserialize_groups",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub groups: Vec<GroupDef>,
    #[serde(default)]
    pub node_types: Vec<NodeType>,
    #[serde(default)]
    pub graph: Vec<NodeConnection>,
    #[serde(default)]
    pub graph_attrs: GraphAttrs,
    #[serde(default)]
    pub icons: Vec<IconDef>,
    /// Named, reusable shape lists a node type's `attrs.template_ref` can
    /// point at instead of repeating the same `attrs.template` shapes
    /// inline across several node types - see `NodeTemplateDef`.
    #[serde(default)]
    pub node_templates: Vec<NodeTemplateDef>,

    #[serde(skip)]
    pub node_instances: Vec<Node>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct File {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<crate::parser::imports::ImportDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConfig>,
    #[serde(
        default,
        deserialize_with = "deserialize_groups",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub groups: Vec<GroupDef>,
    #[serde(default)]
    pub graph_defn: GraphDefinition,

    #[serde(skip)]
    pub timers: HashMap<String, Timer>,
}

pub fn create_rhai_engine(
    draw_store: std::sync::Arc<std::sync::RwLock<Option<rhai::Array>>>,
    update_params_store: std::sync::Arc<std::sync::RwLock<Option<rhai::Map>>>,
) -> rhai::Engine {
    let mut engine = rhai::Engine::new();
    engine.set_max_expr_depths(256, 256);
    let ds = draw_store.clone();
    let ds_one = draw_store.clone();
    let up = update_params_store.clone();
    engine
        .register_fn("log", |_s: String| {})
        .register_fn("log", |_s: Dynamic| {})
        .register_fn("log", |_a: Dynamic, _b: Dynamic| {})
        .register_fn("send", |_to: String, _msg: Dynamic| {})
        .register_fn("draw", move |shapes: rhai::Array| {
            *ds.write().unwrap() = Some(shapes);
        })
        .register_fn("draw", move |shape: rhai::Map| {
            *ds_one.write().unwrap() = Some(vec![Dynamic::from_map(shape)]);
        })
        .register_fn("random_chance", |percent: i64| -> bool {
            rand::thread_rng().gen_range(0..100) < percent
        })
        .register_fn("random_int", |min: i64, max: i64| -> i64 {
            if max <= min {
                min
            } else {
                rand::thread_rng().gen_range(min..max)
            }
        })
        .register_fn(
            "spawn_node",
            |_name: String, _node_type: String, _links: rhai::Array| {},
        )
        .register_fn(
            "spawn_node",
            |_name: String, _node_type: String, _links: rhai::Array, _fn_override: String| {},
        )
        .register_fn("despawn", |_name: String| {})
        .register_fn("link", |_peer: String| {})
        .register_fn("unlink", |_peer: String| {})
        .register_fn("explain", |_key: String, _text: String| {})
        .register_fn("explain", |_text: String| {})
        // Genuinely wired (not a no-op), same reasoning as `draw` above - an
        // `on_init` that calls `update_node_params(#{...})` should take effect
        // immediately, before there's a live ECS world.
        .register_fn("update_node_params", move |p: rhai::Map| {
            *up.write().unwrap() = Some(p);
        });
    engine
}

pub fn parse_graph2(graph_code: &String) -> Result<File, serde_yaml::Error> {
    parse_graph2_with_sources(graph_code, &HashMap::new())
}

pub fn parse_graph2_with_sources(
    graph_code: &String,
    external_sources: &HashMap<String, String>,
) -> Result<File, serde_yaml::Error> {
    let draw_store = std::sync::Arc::new(std::sync::RwLock::new(None));
    let update_params_store = std::sync::Arc::new(std::sync::RwLock::new(None));
    let engine = create_rhai_engine(draw_store.clone(), update_params_store.clone());

    let data = crate::parser::imports::resolve_file_imports(graph_code, external_sources);
    if let Ok(mut m_data) = data {
        if m_data.graph_defn.layout.is_none() && m_data.layout.is_some() {
            m_data.graph_defn.layout = m_data.layout.clone();
        }
        if m_data.graph_defn.groups.is_empty() && !m_data.groups.is_empty() {
            m_data.graph_defn.groups = m_data.groups.clone();
        }
        for node_type in m_data.graph_defn.node_types.iter_mut() {
            let res = compile_ast(&engine, node_type);
            if res.is_err() {
                let e = res.err().unwrap();
                return Err(serde_yaml::Error::custom(
                    format!(
                        "Error compiling function for node type {} with error: {:?}",
                        node_type.id, e
                    )
                    .as_str(),
                ));
            }
            if let Some(icon_id) = &node_type.attrs.icon {
                if !m_data.graph_defn.icons.iter().any(|i| &i.id == icon_id) {
                    return Err(serde_yaml::Error::custom(
                        format!(
                            "Icon '{}' referenced by node type '{}' not found in icons list",
                            icon_id, node_type.id
                        )
                        .as_str(),
                    ));
                }
            }
            if node_type.attrs.template.is_none() && node_type.attrs.template_ref.is_none() {
                if m_data
                    .graph_defn
                    .node_templates
                    .iter()
                    .any(|t| t.id == "default")
                {
                    node_type.attrs.template_ref = Some("default".to_string());
                }
            }
            if let Some(template_ref) = &node_type.attrs.template_ref {
                let Some(named) = m_data
                    .graph_defn
                    .node_templates
                    .iter()
                    .find(|t| &t.id == template_ref)
                else {
                    return Err(serde_yaml::Error::custom(
                        format!(
                            "Template '{}' referenced by node type '{}' not found in node_templates list",
                            template_ref, node_type.id
                        )
                        .as_str(),
                    ));
                };
                // The named template's shapes first, then any inline
                // `attrs.template` shapes on top of them - lets a node type
                // start from a shared base and add its own per-type touches
                // without repeating the base itself.
                let mut shapes = named.shapes.clone();
                if let Some(inline) = node_type.attrs.template.take() {
                    shapes.extend(inline);
                }
                node_type.attrs.template = Some(shapes);
            }
            if let Ticks::Range { min, max, .. } = &node_type.attrs.ticks {
                if min > max {
                    return Err(serde_yaml::Error::custom(
                        format!(
                            "Invalid ticks range for node type '{}': min ({}) > max ({})",
                            node_type.id, min, max
                        )
                        .as_str(),
                    ));
                }
            }
        }
        for node in m_data.graph_defn.graph.iter() {
            let type_data = m_data
                .graph_defn
                .node_types
                .iter()
                .find(|x| x.id == node.node_type);
            if let Some(data_w_type) = type_data {
                let effective_type = if let Some(script) = &node.func {
                    let mut overridden = data_w_type.clone();
                    overridden.func = Some(script.clone());
                    if let Err(e) = compile_ast(&engine, &mut overridden) {
                        return Err(serde_yaml::Error::custom(
                            format!(
                                "Error compiling per-instance fn override for node '{}': {:?}",
                                node.name, e
                            )
                            .as_str(),
                        ));
                    }
                    overridden
                } else {
                    data_w_type.clone()
                };
                let n = instantiate_node(
                    &engine,
                    &effective_type,
                    node.name.clone(),
                    node.links.clone(),
                    &draw_store,
                    &update_params_store,
                );
                m_data.graph_defn.node_instances.push(n);
            } else {
                return Err(serde_yaml::Error::custom(
                    format!("Node type not found for type {}", node.node_type).as_str(),
                ));
            }
        }
        return Ok(m_data);
    }
    data
}

/// Builds a fresh `Node` instance of `node_type` and runs its `on_init` once. Used
/// both at initial YAML parse time (with draw_store recording any initial draw() call)
/// and by `rhai_engine::apply_spawns` for a script's runtime `spawn()`.
pub fn instantiate_node(
    engine: &rhai::Engine,
    node_type: &NodeType,
    name: String,
    links: Vec<String>,
    draw_store: &std::sync::Arc<std::sync::RwLock<Option<rhai::Array>>>,
    update_params_store: &std::sync::Arc<std::sync::RwLock<Option<rhai::Map>>>,
) -> Node {
    let mut n = Node {
        name,
        node_data: node_type.clone(),
        timer: Timer::new(
            Duration::from_secs(resolve_ticks_secs(&node_type.attrs.ticks)),
            TimerMode::Repeating,
        ),
        links,
        ast: node_type.ast.clone(),
        scope: Scope::new(),
        state: Dynamic::from_map(BTreeMap::new()),
        overlay: Vec::new(),
        overlay_dirty: false,
        template_overrides: HashMap::new(),
    };
    // Seed the overlay from a YAML-declared `attrs.template` (see
    // `TemplateShape`'s doc comment) *before* `on_init` runs - a script's
    // own `draw()` call in `on_init` replaces this outright (same as any
    // later `draw()` call), so a template only actually shows for a node
    // type that never calls `draw()` at all, or hasn't yet by the time
    // this first render happens.
    if let Some(template) = &node_type.attrs.template {
        let params = template_params(&n.name, &node_type.attrs);
        n.overlay = template
            .iter()
            .map(|s| s.substituted(&params))
            .filter_map(|s| s.to_draw_cmd())
            .take(crate::parser::draw::MAX_SHAPES_PER_NODE)
            .collect();
        n.overlay_dirty = true;
    }
    init_scope(engine, &mut n, draw_store, update_params_store);
    n
}

/// Builds the `{{param}}` -> value map used to render `instance_name`'s
/// template (see `TemplateShape::substituted` / `NodeTemplateDef`'s doc
/// comment): the two auto-bound builtins (`node_name`, `icon`) first, then
/// any explicit `attrs.template_params` layered on top so they can override
/// either builtin (or add params of their own the template refers to).
pub(crate) fn template_params(instance_name: &str, attrs: &Attrs) -> HashMap<String, String> {
    let mut params = HashMap::new();
    // Default fallback values for standard theme parameters so templates render cleanly
    params.insert("accent_color".to_string(), "#38bdf8".to_string());
    params.insert("service_type".to_string(), "Cloud Node".to_string());
    params.insert("status_bg".to_string(), "#dbeafe".to_string());
    params.insert("status_color".to_string(), "#1e40af".to_string());
    params.insert("status_text".to_string(), "HEALTHY • ACTIVE".to_string());
    params.insert("neon_border".to_string(), "#00f5ff".to_string());
    params.insert("neon_accent".to_string(), "#00f5ff".to_string());
    params.insert("telemetry".to_string(), "SYSTEM // ONLINE".to_string());
    params.insert("badge_bg".to_string(), "#083344".to_string());
    params.insert("badge_color".to_string(), "#67e8f9".to_string());
    params.insert("status_code".to_string(), "ONLINE".to_string());
    params.insert("unit_id".to_string(), "1U RACK UNIT".to_string());
    params.insert("led_pwr".to_string(), "#22c55e".to_string());
    params.insert("led_net".to_string(), "#38bdf8".to_string());
    params.insert("led_act".to_string(), "#f59e0b".to_string());
    params.insert("disc_color".to_string(), "#dbeafe".to_string());
    params.insert("role_tag".to_string(), "NODE".to_string());
    params.insert("tag_color".to_string(), "#2563eb".to_string());

    params.insert("node_name".to_string(), instance_name.to_string());
    if let Some(icon) = &attrs.icon {
        params.insert("icon".to_string(), icon.clone());
    }
    if let Some(explicit) = &attrs.template_params {
        for (k, v) in explicit {
            params.insert(k.clone(), v.clone());
        }
    }
    params
}

/// Applies a runtime `update_node_params(#{...})` call (see `rhai_engine`'s
/// `update_params_store`): merges `updates` into `node.template_overrides`,
/// then re-renders `node.overlay` from the node type's own `attrs.template`
/// shapes (the same ones `instantiate_node` seeded it from) using the merged
/// param map, so a later call only needs to name what changed - unrelated
/// shapes/params are untouched, not painted over. Returns `Err` (a message
/// meant for the caller to log as a warning, not a hard failure) when there's
/// nothing to substitute against: no `attrs.template` at all, or an empty
/// `updates` map.
pub(crate) fn apply_template_param_updates(
    node: &mut Node,
    updates: HashMap<String, String>,
) -> Result<(), String> {
    let Some(template) = &node.node_data.attrs.template else {
        return Err(format!(
            "update_node_params called on '{}', which has no template_ref/template to update",
            node.name
        ));
    };
    if updates.is_empty() {
        return Err(format!(
            "update_node_params called on '{}' with no params",
            node.name
        ));
    }
    node.template_overrides.extend(updates);
    let mut params = template_params(&node.name, &node.node_data.attrs);
    params.extend(node.template_overrides.clone());
    node.overlay = template
        .iter()
        .map(|s| s.substituted(&params))
        .filter_map(|s| s.to_draw_cmd())
        .take(crate::parser::draw::MAX_SHAPES_PER_NODE)
        .collect();
    node.overlay_dirty = true;
    Ok(())
}

pub fn init_scope(
    engine: &rhai::Engine,
    node: &mut Node,
    draw_store: &std::sync::Arc<std::sync::RwLock<Option<rhai::Array>>>,
    update_params_store: &std::sync::Arc<std::sync::RwLock<Option<rhai::Map>>>,
) {
    let scope = &mut node.scope;
    scope.push_constant("node_name", node.name.clone());
    let links: Dynamic = node.links.clone().into();
    scope.push_constant("links", links);
    let init_size = scope.len();
    let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);

    scope.push_dynamic("state", std::mem::take(&mut node.state));
    let _ = engine.call_fn_with_options::<()>(options, scope, &node.ast, "on_init", ());
    node.state = scope.remove::<Dynamic>("state").unwrap_or_default();

    if let Some(shapes) = draw_store.write().unwrap().take() {
        node.overlay = crate::parser::draw::parse_overlay(&shapes);
        node.overlay_dirty = true;
    }

    c_log!("scope size: {}", scope.len());
    c_log!("scope: {:?}", scope);
    c_log!("state {:?}", node.state);
    scope.rewind(init_size);

    if let Some(map) = update_params_store.write().unwrap().take() {
        let updates = map
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        if let Err(msg) = apply_template_param_updates(node, updates) {
            log_dsa_event!("WARN: {}", msg);
        }
    }
}

pub fn compile_ast(engine: &rhai::Engine, node: &mut NodeType) -> Result<bool, rhai::ParseError> {
    c_log!("Compiling code for node type {}", node.id);
    if node.func.is_none() {
        c_log!("No code for node type {}", node.id);
        return Ok(true);
    }
    let ast = engine.compile(node.func.as_ref().unwrap());
    if ast.is_ok() {
        c_log!("code compiled for node type {}", node.id);
        node.ast = ast.unwrap();
        return Ok(true);
    }
    Err(ast.err().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message_theme() {
        let yaml = r##"
graph_defn:
  graph_attrs:
    background: "#0a0e17"
    text_color: "#f8fafc"
    message_theme:
      shape: pill
      bg: "#1e293b"
      stroke: "#38bdf8"
      stroke_width: 2.0
      text_color: "#ffffff"
      font_size: 14.0
      icon_size: 18.0
  node_types:
    - id: worker
  graph:
    - name: w1
      node_type: worker
      links: []
"##
        .to_string();

        let parsed = parse_graph2(&yaml).expect("Should parse valid YAML with message_theme");
        let theme = parsed
            .graph_defn
            .graph_attrs
            .message_theme
            .expect("message_theme should exist");
        assert_eq!(theme.shape, MessageBubbleShape::Pill);
        assert_eq!(theme.stroke_width, 2.0);
        assert_eq!(theme.font_size, 14.0);
        assert_eq!(theme.icon_size, 18.0);
        assert!(theme.bg.is_some());
        assert!(theme.stroke.is_some());
        assert!(theme.text_color.is_some());
    }

    /// `connector_style` defaults to `Step` when unset, and each explicit
    /// value round-trips through the YAML tag correctly.
    #[test]
    fn test_connector_style_defaults_and_parses() {
        let unset = r##"
graph_defn:
  node_types:
    - id: worker
  graph:
    - name: w1
      node_type: worker
      links: []
"##
        .to_string();
        let parsed = parse_graph2(&unset).expect("Should parse YAML with no connector_style");
        assert_eq!(
            parsed.graph_defn.graph_attrs.connector_style,
            ConnectorStyle::Step
        );

        for (tag, expected) in [
            ("curved", ConnectorStyle::Curved),
            ("straight", ConnectorStyle::Straight),
            ("step", ConnectorStyle::Step),
        ] {
            let yaml = format!(
                "graph_defn:\n  graph_attrs:\n    connector_style: {}\n  node_types:\n    - id: worker\n  graph:\n    - name: w1\n      node_type: worker\n      links: []\n",
                tag
            );
            let parsed = parse_graph2(&yaml)
                .unwrap_or_else(|e| panic!("Should parse connector_style: {}: {}", tag, e));
            assert_eq!(parsed.graph_defn.graph_attrs.connector_style, expected);
        }
    }

    #[test]
    fn test_parse_message_theme_shapes() {
        for (shape_str, expected_shape) in [
            ("rounded", MessageBubbleShape::Rounded),
            ("pill", MessageBubbleShape::Pill),
            ("box", MessageBubbleShape::Box),
            ("chamfered", MessageBubbleShape::Chamfered),
        ] {
            let yaml = format!(
                r#"
graph_defn:
  graph_attrs:
    message_theme:
      shape: {}
  node_types:
    - id: worker
  graph:
    - name: w1
      node_type: worker
      links: []
"#,
                shape_str
            );
            let parsed = parse_graph2(&yaml).expect("Should parse valid shape");
            let theme = parsed.graph_defn.graph_attrs.message_theme.expect("theme");
            assert_eq!(theme.shape, expected_shape);
        }
    }

    #[test]
    fn test_parse_progress_shapes() {
        let yaml = r##"
graph_defn:
  node_types:
    - id: server
      attrs:
        template:
          - shape: progress
            style: ring
            x: 10
            y: 20
            r: 16
            thickness: 3.0
            start_angle: 90
            clockwise: true
            track_color: "#1e293b"
            fill_color: "#00f5ff"
          - shape: progress
            style: bar
            x: 0
            y: -30
            w: 100
            h: 6
            track_color: "#334155"
            fill_color: "#22c55e"
          - shape: progress
            style: pie
            r: 12
            fill_color: "#f59e0b"
          - shape: progress
            style: segmented
            w: 80
            h: 8
            segments: 5
            gap: 2
            fill_color: "#ec4899"
  graph:
    - name: s1
      node_type: server
      links: []
"##
        .to_string();

        let parsed = parse_graph2(&yaml).expect("Should parse progress shapes in template");
        let node_type = &parsed.graph_defn.node_types[0];
        let template = node_type.attrs.template.as_ref().expect("template");
        assert_eq!(template.len(), 4);
    }

    #[test]
    fn test_parse_all_tutorial_files() {
        let tutorial_files = [
            "web/static/tutorial/ch1/02_first_graph.yml",
            "web/static/tutorial/ch1/03_messaging.yml",
            "web/static/tutorial/ch1/04_params_and_icons.yml",
            "web/static/tutorial/ch1/05_state_and_logging.yml",
            "web/static/tutorial/ch1/06_multilevel_routing.yml",
            "web/static/tutorial/ch1/07_interactive_narration.yml",
            "web/static/tutorial/ch1/08_runtime_topology.yml",
            "web/static/tutorial/ch2/01_hierarchical_layouts.yml",
            "web/static/tutorial/ch2/02_groups_and_tiers.yml",
            "web/static/tutorial/ch2/03_grid_and_circular.yml",
            "web/static/tutorial/ch2/04_manual_and_offsets.yml",
            "web/static/tutorial/ch3/01_draw_basics.yml",
            "web/static/tutorial/ch3/02_shapes.yml",
            "web/static/tutorial/ch3/03_node_templates.yml",
            "web/static/tutorial/ch3/04_state_gauges.yml",
            "web/static/tutorial/ch4/01_cloud_cards.yml",
            "web/static/tutorial/ch4/02_datacenter_rack.yml",
            "web/static/tutorial/ch4/03_cyberpunk_hud.yml",
            "web/static/tutorial/ch4/04_capsule_pills.yml",
            "web/static/tutorial/ch4/05_layered_theming.yml",
            "web/static/tutorial/ch5/01_intro_to_plibs.yml",
            "web/static/tutorial/ch5/02_token_ring.yml",
            "web/static/tutorial/ch5/03_load_balancer.yml",
            "web/static/tutorial/ch5/04_two_phase_commit.yml",
            "web/static/tutorial/ch5/05_primary_backup.yml",
            "web/static/tutorial/ch5/06_aws_cloud_architecture.yml",
        ];

        for path in tutorial_files {
            let content = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
            let parsed = parse_graph2(&content)
                .unwrap_or_else(|e| panic!("Failed to parse tutorial YAML {}: {}", path, e));
            assert!(
                !parsed.graph_defn.node_instances.is_empty(),
                "Tutorial {} should instantiate at least one node",
                path
            );
        }
    }

    #[test]
    fn test_parse_explain_theme() {
        let yaml = r##"
graph_defn:
  graph_attrs:
    explain_theme:
      shape: chamfered
      bg: "#060d17"
      border: "#00f5ff"
      border_width: 2.0
      text_color: "#e0f7fa"
      accent: "#00f5ff"
      button_text_color: "#060d17"
      shadow_color: "rgba(0, 245, 255, 0.3)"
      backdrop_color: "rgba(7, 11, 25, 0.8)"
      font_size: 14.5
  node_types:
    - id: test_node
  graph:
    - name: n1
      node_type: test_node
      links: []
"##
        .to_string();

        let parsed = parse_graph2(&yaml).expect("Should parse explain_theme in graph_attrs");
        let explain_theme = parsed
            .graph_defn
            .graph_attrs
            .explain_theme
            .expect("explain_theme should be present");
        assert_eq!(explain_theme.shape, MessageBubbleShape::Chamfered);
        assert_eq!(explain_theme.border_width, 2.0);
        assert_eq!(explain_theme.font_size, 14.5);
        assert!(explain_theme.bg.is_some());
        assert!(explain_theme.border.is_some());
        assert!(explain_theme.accent.is_some());
        assert!(explain_theme.shadow_color.is_some());
        assert!(explain_theme.backdrop_color.is_some());
    }

    #[test]
    fn test_parse_groups_mapping_and_sequence() {
        let yaml_map = r##"
layout:
  type: hierarchical
  direction: lr

groups:
  frontend:
    title: "Edge Tier"
    direction: tb
    style:
      bg: "#1e293b"
      border: "#38bdf8"
  backend:
    title: "Application Core"
    direction: lr

graph:
  - name: api
    type: api
    group: frontend
    links: []
  - name: srv
    type: srv
    group: backend
    links: []
"##
        .to_string();

        let parsed_map = parse_graph2(&yaml_map).expect("Should parse map syntax for groups");
        assert_eq!(parsed_map.graph_defn.groups.len(), 2);
        assert_eq!(parsed_map.graph_defn.groups[0].id, "frontend");
        assert_eq!(
            parsed_map.graph_defn.groups[0].title.as_deref(),
            Some("Edge Tier")
        );
        assert_eq!(
            parsed_map.graph_defn.groups[0]
                .layout
                .as_ref()
                .and_then(|l| l.direction),
            Some(LayoutDirection::Tb)
        );

        let yaml_seq = r##"
groups:
  - id: g1
    title: "Group 1"
    direction: rl
graph:
  - name: n1
    type: t1
    group: g1
    links: []
"##
        .to_string();

        let parsed_seq = parse_graph2(&yaml_seq).expect("Should parse sequence syntax for groups");
        assert_eq!(parsed_seq.graph_defn.groups.len(), 1);
        assert_eq!(parsed_seq.graph_defn.groups[0].id, "g1");
        assert_eq!(
            parsed_seq.graph_defn.groups[0]
                .layout
                .as_ref()
                .and_then(|l| l.direction),
            Some(LayoutDirection::Rl)
        );
    }

    #[test]
    fn test_init_scope_draw_and_state() {
        let yaml = r##"
graph_defn:
  node_types:
    - id: worker
      fn: |
        fn on_init() {
          state.queue = [1, 2, 3];
          redraw(state.queue.len());
        }
        fn redraw(count) {
          draw([
            #{ shape: "rect", w: 100, h: 40, fill: "#ffffff", stroke: "#10b981", stroke_width: 2 },
            #{ shape: "text", text: "Count: " + count, color: "#1e293b", size: 12 }
          ]);
        }
  graph:
    - name: w1
      node_type: worker
      links: []
"##
        .to_string();

        let parsed = parse_graph2(&yaml).expect("Should parse graph with on_init draw()");
        let w1 = &parsed.graph_defn.node_instances[0];
        assert_eq!(w1.overlay.len(), 2);
        assert!(w1.overlay_dirty);
    }

    /// `update_node_params(#{...})` should re-render the node's existing
    /// `attrs.template` shapes with the new param merged in - not require
    /// restating the whole shape list the way `draw()` does - and persist
    /// that override in `template_overrides` for a later, unrelated update to
    /// build on.
    #[test]
    fn test_update_node_params_merges_and_rerenders_template() {
        let yaml = r##"
graph_defn:
  node_templates:
    - id: card
      shapes:
        - shape: text
          x: 0
          y: 0
          text: "{{status_text}}"
          size: 10
          color: "{{status_color}}"
  node_types:
    - id: worker
      attrs:
        template_ref: card
      fn: |
        fn on_init() {
          update_node_params(#{ status_text: "BOOTING" });
        }
  graph:
    - name: w1
      node_type: worker
      links: []
"##
        .to_string();

        let parsed = parse_graph2(&yaml).expect("Should parse graph");
        let w1 = &parsed.graph_defn.node_instances[0];
        assert_eq!(
            w1.template_overrides.get("status_text").map(String::as_str),
            Some("BOOTING")
        );
        assert_eq!(w1.overlay.len(), 1);
        match &w1.overlay[0] {
            crate::parser::draw::DrawCmd::Text { text, .. } => assert_eq!(text, "BOOTING"),
            other => panic!("expected a Text draw cmd, got {:?}", other),
        }
    }

    /// A node with no `template_ref`/`template` has nothing for
    /// `update_node_params` to re-substitute - this should be a harmless
    /// no-op (not a panic, not a fabricated overlay), with the failure
    /// surfaced as a warning `apply_template_param_updates` returns rather
    /// than silently swallowed.
    #[test]
    fn test_update_node_params_without_template_is_noop() {
        let mut node = Node {
            name: "plain".to_string(),
            node_data: NodeType {
                id: "plain".to_string(),
                func: None,
                ast: rhai::AST::empty(),
                attrs: Attrs::default(),
                params: None,
            },
            timer: Timer::new(Duration::from_secs(1), TimerMode::Repeating),
            links: vec![],
            ast: rhai::AST::empty(),
            scope: Scope::new(),
            state: Dynamic::from_map(BTreeMap::new()),
            overlay: Vec::new(),
            overlay_dirty: false,
            template_overrides: HashMap::new(),
        };
        let mut updates = HashMap::new();
        updates.insert("status_text".to_string(), "X".to_string());
        let err = apply_template_param_updates(&mut node, updates)
            .expect_err("node with no template should return a warning, not Ok");
        assert!(err.contains("no template_ref/template"));
        assert!(node.overlay.is_empty());
    }

    /// A `graph:` entry's own `fn:` should override its node type's script
    /// for that one instance only - a sibling instance of the same type with
    /// no override keeps running the type's default script untouched.
    #[test]
    fn test_graph_entry_fn_override() {
        let yaml = r##"
graph_defn:
  node_types:
    - id: worker
      fn: |
        fn on_init() {
          state.tag = "default";
        }
  graph:
    - name: default_worker
      node_type: worker
      links: []
    - name: custom_worker
      node_type: worker
      links: []
      fn: |
        fn on_init() {
          state.tag = "custom";
        }
"##
        .to_string();

        let parsed = parse_graph2(&yaml).expect("Should parse graph with per-instance fn override");
        let default_worker = parsed
            .graph_defn
            .node_instances
            .iter()
            .find(|n| n.name == "default_worker")
            .unwrap();
        let custom_worker = parsed
            .graph_defn
            .node_instances
            .iter()
            .find(|n| n.name == "custom_worker")
            .unwrap();
        assert_eq!(
            default_worker
                .state
                .read_lock::<rhai::Map>()
                .and_then(|m| m.get("tag").map(|v| v.to_string())),
            Some("default".to_string())
        );
        assert_eq!(
            custom_worker
                .state
                .read_lock::<rhai::Map>()
                .and_then(|m| m.get("tag").map(|v| v.to_string())),
            Some("custom".to_string())
        );
    }

    /// Regression test for the `globals`/`state` dual-binding bug: `on_init`
    /// used to push the node's persisted state into scope under two names
    /// (`globals` and `state`, both seeded from the same clone) and guess
    /// afterward which one to keep by checking whether `state` looked like a
    /// non-empty map - a heuristic that only worked on the very first
    /// state-mutating call, since `state` stayed non-empty (just stale) on
    /// every call after that regardless of whether the script touched it.
    /// This drives a second call the same way `rhai_engine.rs`'s `on_timer`
    /// path does (push `state` by move, call, pop, assign back) to prove a
    /// count keeps incrementing past the first tick instead of freezing.
    #[test]
    fn test_state_round_trips_across_multiple_calls() {
        let yaml = r##"
graph_defn:
  node_types:
    - id: counter
      fn: |
        fn on_init() { state.count = 0; }
        fn on_timer() { state.count += 1; }
  graph:
    - name: c1
      node_type: counter
      links: []
"##
        .to_string();

        let parsed = parse_graph2(&yaml).expect("Should parse graph with state counter");
        let mut c1 = parsed.graph_defn.node_instances.into_iter().next().unwrap();
        assert_eq!(
            c1.state
                .read_lock::<rhai::Map>()
                .unwrap()
                .get("count")
                .unwrap()
                .as_int()
                .unwrap(),
            0,
            "on_init should have set count to 0"
        );

        let engine = rhai::Engine::new();
        for expected in 1..=3 {
            let scope = &mut c1.scope;
            let init_size = scope.len();
            let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);
            scope.push_dynamic("state", std::mem::take(&mut c1.state));
            engine
                .call_fn_with_options::<()>(options, scope, &c1.ast, "on_timer", ())
                .expect("on_timer should run without error");
            c1.state = scope.remove::<Dynamic>("state").unwrap_or_default();
            scope.rewind(init_size);

            let count = c1
                .state
                .read_lock::<rhai::Map>()
                .unwrap()
                .get("count")
                .unwrap()
                .as_int()
                .unwrap();
            assert_eq!(
                count, expected,
                "count should keep incrementing across calls, not freeze after the first"
            );
        }
    }
}
