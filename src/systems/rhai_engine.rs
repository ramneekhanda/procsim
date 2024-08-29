use crate::wasm::browser::console_log;
use crate::parser::graphv2::GraphDefinition;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::stdlib::rhai_lib::rhai_log;

use bevy::prelude::*;

use rhai::Engine;

fn recompile(gd: &mut GraphDefinition) {
  let engine = Engine::new();
  for node in gd.nodes.iter_mut() {
    let _ = node.func.as_ref().is_some_and(|f| {
      match engine.compile(f) {
        Ok(res) => {
          node.ast = res;
          console_log(format!("{} - {}", "compiled successfully",  f).as_str());
          true
        },
        Err(e) => {
          console_log(format!("{} - {}", "failed to compile",  f).as_str());
          false
        }
      }
    });
  }
}

pub fn execute_rhai_engine(
  mut graph_defn: ResMut<GraphDefinitionRes>
) {
  if graph_defn.is_changed() {
    recompile(&mut graph_defn.graph_defn);
  } else {
    let gd = &graph_defn.graph_defn;
    for node in gd.nodes.iter() {
      let _ = node.func.as_ref().is_some_and(|f| {
        let mut engine = Engine::new(); // TODO: Optimization - store on heap and initialize once
        engine.register_fn("log", rhai_log);
        match engine.run_ast(&node.ast) {
          Ok(res) => {
            true
          },
          Err(e) => {
            console_log("error running");
            false
          }
        }
      });
    }

  }

}
