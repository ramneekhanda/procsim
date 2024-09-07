use crate::c_log;
use bevy::{
    color::{Color, Srgba},
    time::Timer,
    prelude::*,
};
use rhai::{Scope, AST};
use schemars::JsonSchema;
use serde::de::Error;
use serde::ser::Serializer;
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::{HashMap, HashSet}, fmt::Debug};
use std::time::Duration;
use rhai::Dynamic;

fn gray_color() -> Color {
    Srgba::hex("#D3D3D3").unwrap().into()
}

fn gray_color_str() -> String {
    "#D3D3D3".to_string()
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
    #[schemars(with = "String", default = "gray_color_str")]
    #[serde(
        default = "gray_color",
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
            background: gray_color(),
            connection_color: black_color(),
            title: String::new(),
            text_color: black_color(),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct Attrs {
    pub ticks: u64,
    pub icon: Option<String>,
}

impl Default for Attrs {
    fn default() -> Self {
        Attrs {
            ticks: 2,
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
    pub params: Vec<NodeParams>,
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
pub struct GraphDefinition {
    pub node_types: Vec<NodeType>,
    pub graph: Vec<NodeConnection>,
    pub graph_attrs: GraphAttrs,

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
    let data: Result<File, serde_yaml::Error> = serde_yaml::from_str(&graph_code);
    if let Ok(mut m_data) = data {
        for node_type in m_data.graph_defn.node_types.iter_mut() {
            let res = compile_ast(node_type);
            if res.is_err() {
                let e = res.err().unwrap();
                return Err(serde_yaml::Error::custom(
                    format!("Error compiling function for node type {} with error: {:?}", node_type.id, e).as_str(),
                ));
            }
        }
        let scope = Scope::new();
        for node in m_data.graph_defn.graph.iter() {
            let type_data = m_data
                .graph_defn
                .node_types
                .iter()
                .find(|x| x.id == node.node_type);
            if let Some(data_w_type) = type_data {
                let mut n = Node {
                    name: node.name.clone(),
                    node_data: data_w_type.clone(),
                    timer: Timer::new(
                        Duration::from_secs(data_w_type.attrs.ticks),
                        TimerMode::Repeating,
                    ),
                    links: node.links.clone(),
                    ast: data_w_type.ast.clone(),
                    scope: scope.clone(),
                };
                let links: Dynamic = node.links.clone().into();
                n.scope.push_constant("node_name", n.name.clone());
                n.scope.push_constant("links", links);
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

pub fn compile_ast(node: &mut NodeType) -> Result<bool, rhai::ParseError> {
    let engine = rhai::Engine::new();

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
