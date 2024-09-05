use crate::c_log;
use crate::parser::graphv2::{GraphDefinition, ParamType};
use crate::resources::graph_def::GraphDefinitionRes;
use crate::stdlib::rhai_lib::rhai_log;
use bevy::prelude::*;

use bevy::utils::tracing::event;
use rhai::{Engine, Scope};

pub fn execute_rhai_engine(mut graph_defn: ResMut<GraphDefinitionRes>, time: Res<Time>) {
    //let mut message_store = Vec<rhai::Map>::new();
    let mut engine = Engine::new(); // TODO: Optimization - store on heap and initialize once
    engine.register_fn("log", rhai_log);
    engine.register_fn("send", |from: String, to: String, msg: String| -> bool {
        rhai_log(format!("send called from {} to {} with msg {}", from, to, msg).as_str());
        true
    });

    let gd = &mut graph_defn.graph_defn;
    for node in gd.node_instances.iter_mut() {
        node.timer.tick(time.delta());
        if !(node.timer.finished()) {
            continue;
        }
        let _ = node.node_data.func.as_ref().is_some_and(|f| {        
            let scope = &mut node.scope;
            for node_params in node.node_data.params.iter() {
                if let ParamType::Bool { default } = node_params.param_type {
                    scope.push_constant(node_params.name.clone(), default);
                } else if let ParamType::Float { default, min, max } = node_params.param_type {
                    scope.push_constant(node_params.name.clone(), default);
                } else if let ParamType::Integer { default, min, max } = node_params.param_type {
                    scope.push_constant(node_params.name.clone(), default);
                } else if let ParamType::String { default } = &node_params.param_type {
                    scope.push_constant(node_params.name.clone(), default.clone());
                } else if let ParamType::Option { default, values } = &node_params.param_type {
                    scope.push_constant(node_params.name.clone(), default.clone());
                }
            }
            match engine.run_ast_with_scope(scope, &node.ast) {
                Ok(res) => true,
                Err(e) => {
                    c_log!("error running");
                    false
                }
            }
        });
    }
}
