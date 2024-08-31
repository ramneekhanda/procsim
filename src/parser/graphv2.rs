use bevy::{
    color::{Color, Srgba},
    time::Timer,
};
use rhai::{Scope, AST};
use schemars::JsonSchema;
use serde::de::Error;
use serde::ser::Serializer;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};


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

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct Attrs {
    #[serde(default)]
    pub ticks: i32,
    pub icon: Option<String>,
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum ParamType {
    Bool { default: bool },
    Float { min: f64, max: f64, default: f64 },
    Integer { min: i64, max: i64, default: i64 },
    String { default: String },
}

impl Default for ParamType {
    fn default() -> Self {
        ParamType::String {
            default: "".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Default, Serialize, Deserialize, JsonSchema)]
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
        self.id == other.id && self.func == other.func && self.attrs == other.attrs && self.params == other.params
    }
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct Node {
    pub name: String,
    pub node_data: NodeType,
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
        let mut scope = Scope::new();
        for node in m_data.graph_defn.graph.iter() {
            m_data.graph_defn.node_instances.push(Node {
                name: node.name.clone(),
                node_data: m_data.graph_defn.node_types.iter().find(|x| x.id == node.node_type).unwrap().clone(),
            });
        }
        return Ok(m_data);
    } 
    data
}
