use crate::c_log;
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

fn deserialize_color<'de, D>(d: D) -> Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(d).unwrap();

    let srgba = Srgba::hex(s.as_str());
    if srgba.is_ok() {
        let color = srgba.unwrap().into();
        Ok(color)
    } else {
        Err(Error::custom("Invalid color"))
    }
}

fn serialize_color<S>(c: &Color, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(format!("\"{}\"", c.to_srgba().to_hex()).as_str())
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

    pub title: String,

    #[schemars(with = "String", default = "black_color_str")]
    #[serde(
        default = "black_color",
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub text_color: Color,
}

impl Default for GraphAttrs {
    fn default() -> Self {
        GraphAttrs {
            background: white_color(),
            connection_color: black_color(),
            title: String::new(),
            text_color: black_color(),
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct Attrs {
    #[serde(default)]
    pub ticks: Ticks,
    /// Id of an entry in the graph's top-level `icons` list, used as this node type's
    /// on-canvas icon.
    pub icon: Option<String>,
}

impl Default for Attrs {
    fn default() -> Self {
        Attrs {
            ticks: Ticks::default(),
            icon: None,
        }
    }
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
}

impl std::cmp::PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.node_data == other.node_data
    }
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct NodeConnection {
    pub name: String,
    pub node_type: String,
    pub links: Vec<String>,
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct IconDef {
    pub id: String,
    pub url: String,
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct GraphDefinition {
    pub node_types: Vec<NodeType>,
    pub graph: Vec<NodeConnection>,
    pub graph_attrs: GraphAttrs,
    #[serde(default)]
    pub icons: Vec<IconDef>,

    #[serde(skip)]
    pub node_instances: Vec<Node>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct File {
    #[serde(default)]
    pub graph_defn: GraphDefinition,

    #[serde(skip)]
    pub timers: HashMap<String, Timer>,
}

pub fn parse_graph2(graph_code: &String) -> Result<File, serde_yaml::Error> {
    let mut engine = rhai::Engine::new();
    // Rhai's default nesting limit inside a function is only 32 levels, which a
    // realistic handler (a map literal with an inline `if`, say) can trip. Raise
    // it well clear of hand-written scripts while still bounding pathological input.
    engine.set_max_expr_depths(256, 256);

    let data: Result<File, serde_yaml::Error> = serde_yaml::from_str(&graph_code);
    if let Ok(mut m_data) = data {
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
                let n =
                    instantiate_node(&engine, data_w_type, node.name.clone(), node.links.clone());
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
/// both at initial YAML parse time (a bare, unconfigured `engine` - `log`/`send`/
/// `draw`/`spawn`/etc all silently no-op there, matching prior behavior) and by
/// `rhai_engine::apply_spawns` for a script's runtime `spawn()` (the fully-registered
/// engine there, so `draw()` in `on_init` paints the new node's overlay immediately).
pub fn instantiate_node(
    engine: &rhai::Engine,
    node_type: &NodeType,
    name: String,
    links: Vec<String>,
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
    };
    init_scope(engine, &mut n);
    n
}

pub fn init_scope(engine: &rhai::Engine, node: &mut Node) {
    let scope = &mut node.scope;
    scope.push_constant("node_name", node.name.clone());
    let links: Dynamic = node.links.clone().into();
    scope.push_constant("links", links);
    let init_size = scope.len();
    let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);

    scope.set_value("globals", node.state.clone());
    let _ = engine.call_fn_with_options::<()>(options, scope, &node.ast, "on_init", ());
    node.state = scope.get_value::<Dynamic>("globals").unwrap();

    c_log!("scope size: {}", scope.len());
    c_log!("scope: {:?}", scope);
    c_log!("globals {:?}", node.state);
    scope.rewind(init_size);
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
