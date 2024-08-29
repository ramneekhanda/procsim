use rhai::AST;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use schemars::JsonSchema;
use bevy::{color::{Color, Srgba}, time::Timer};
use serde::de::Error;
use serde::ser::Serializer;

type GraphType = HashMap<String, HashSet<String>>;

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

  #[schemars(with = "String", default="gray_color_str")]
  #[serde(default="gray_color", serialize_with = "serialize_color", deserialize_with="deserialize_color")]
  pub background: Color,

  #[schemars(with = "String", default="black_color_str")]
  #[serde(default="black_color", serialize_with = "serialize_color", deserialize_with="deserialize_color")]
  pub connection_color: Color,

  pub title: String,

  #[schemars(with = "String", default="black_color_str")]
  #[serde(default="black_color", serialize_with = "serialize_color", deserialize_with="deserialize_color")]
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
  pub icon: Option<String>
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Node {
  pub id: String,
  pub name: String,
  #[serde(rename = "fn")]
  pub func: Option<String>,
  #[serde(default)]
  pub attrs: Attrs,
  #[serde(skip)]
  pub ast: AST,
}

impl std::cmp::PartialEq for Node {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name && self.func == other.func && self.attrs == other.attrs
  }
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct GraphDefinition {
  pub nodes: Vec<Node>,
  pub graph: GraphType,
  pub graph_attrs: GraphAttrs,
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
  data
}

pub fn parse_graph(graph_code: &String) -> Result<GraphType, serde_yaml::Error> {
  let data: Result<File, serde_yaml::Error> = serde_yaml::from_str(&graph_code);
  match data {
    Ok(d) => return Ok(d.graph_defn.graph),
    Err(e) => return Err(e),
  }
}

#[test]
fn parse_graph_test() {
  let code = r#"
fns:
  - &server_fn |
      def fn():
        log("hello server")

  - &client_fn |
      def fn():
        log("hello client")

graph_defn:
  name: "test graph"
  nodes:
    - name: server
      id: server
      fn: *server_fn
      attrs:
        tick: 5
        color: [1., 0., 1., 0.5]
    - name: client1
      id: client1
      fn: *client_fn
      attrs:
        ticks: 10
        color: [1., 1., 0., 0.5]
    - name: client2
      id: client2
      fn: *client_fn
  allowed_connections:
    server: [client1, client2]
  graph:
    server: [client1, client2, client4]
  graph_attrs:
      title: "A Sample Graph!"
"#;

  let res = parse_graph(&code.to_string());
  println!("{:?}", res);
  assert!(res.is_ok());
}
