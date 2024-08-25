use serde::{Deserialize, Serialize};
use serde_yaml::Error;
use std::collections::{HashMap, HashSet};
use schemars::JsonSchema;

type GraphType = HashMap<String, HashSet<String>>;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct GraphAttrs {
    pub background: Option<[f32; 4]>,
    pub connection_color: Option<[f32; 4]>,
    pub title: Option<String>,
    pub text_color: Option<[f32; 4]>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct Attrs {
    pub ticks: Option<String>,
    pub color: Option<[f32; 4]>,
    pub icon: Option<String>
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    pub name: String,
    #[serde(rename = "fn")]
    pub func: Option<String>,
    pub attrs: Option<Attrs>,
}

#[derive(Default, Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct GraphDefinition {
    /// this is name commentary
    pub name: String,
    pub nodes: Vec<Node>,
    pub graph: GraphType,
    pub graph_attrs: Option<GraphAttrs>,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct File {
    #[serde(skip)]
    #[serde(rename = "fns")]
    func: Option<String>,
    pub graph_defn: GraphDefinition,
}

pub fn parse_graph2(graph_code: &String) -> Result<File, Error> {
    let data: Result<File, Error> = serde_yaml::from_str(&graph_code);
    data
}

pub fn parse_graph(graph_code: &String) -> Result<GraphType, Error> {
    let data: Result<File, Error> = serde_yaml::from_str(&graph_code);
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
      fn: *server_fn
      attrs:
        tick: 5s
        color: [1., 0., 1., 0.5]
    - name: client1
      fn: *client_fn
      attrs:
        ticks: 10s
        color: [1., 1., 0., 0.5]
    - name: client2
      fn: *client_fn
  allowed_connections:
    server: [client1, client2]
  graph:
    server: [client1, client2, client4]
  graph_attrs:
      title: "A Sample Graph!"
      background: [0.1, 0.1, 0.1, 1.]
      connection_color: [0.9, 0.9, 0.9, 1.]
      text_color: [0.5, 0.5, 0.5, 1.0]
"#;

    let res = parse_graph(&code.to_string());
    println!("{:?}", res);
    assert!(res.is_ok());
}
