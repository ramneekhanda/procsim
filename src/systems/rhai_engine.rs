use crate::parser::graphv2::GraphDefinition;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::stdlib::rhai_lib::rhai_log;
use crate::c_log;
use bevy::prelude::*;

use bevy::utils::tracing::event;
use rhai::Engine;

fn recompile(gd: &mut GraphDefinition) {
    let engine = Engine::new();
    for node in gd.nodes.iter_mut() {
        let _ = node.func.as_ref().is_some_and(|f| match engine.compile(f) {
            Ok(res) => {
                node.ast = res;
                c_log!("{} - {}", "compiled successfully", f);
                true
            }
            Err(e) => {
                c_log!("{} - {}", "failed to compile", f);
                false
            }
        });
    }
}

pub fn execute_rhai_engine(
  mut graph_defn: ResMut<GraphDefinitionRes>,
  mut event_reader: EventReader<crate::resources::graph_def::GraphChange>,
) {
    let evnts = event_reader.read();
    if evnts.count() > 0 {
        recompile(&mut graph_defn.graph_defn);
    } else {
        let gd = &graph_defn.graph_defn;
        for node in gd.nodes.iter() {
            let _ = node.func.as_ref().is_some_and(|f| {
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
}
