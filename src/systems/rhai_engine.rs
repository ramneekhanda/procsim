use crate::parser::graphv2::GraphDefinition;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::stdlib::rhai_lib::rhai_log;
use crate::c_log;
use bevy::prelude::*;

use bevy::utils::tracing::event;
use rhai::{Engine, Scope};

pub fn execute_rhai_engine(
  mut graph_defn: ResMut<GraphDefinitionRes>,
) {    
      let gd = &graph_defn.graph_defn;
      for node in gd.node_instances.iter() {
          let _ = node.node_data.func.as_ref().is_some_and(|f| {
              let mut engine = Engine::new(); // TODO: Optimization - store on heap and initialize once
              engine.register_fn("log", rhai_log);
              match engine.run_ast(&node.ast) {
                  Ok(res) => true,
                  Err(e) => {
                    c_log!("error running");
                      false
                  }
              }
          });
      }
}
