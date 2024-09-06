use std::any::Any;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::hash::Hash;
use std::cell::RefCell;
use std::sync::{Arc, RwLock};

use crate::c_log;
use crate::parser::graphv2::{GraphDefinition, ParamType, NodeParams};
use crate::resources::graph_def::GraphDefinitionRes;
use crate::stdlib::rhai_lib::rhai_log;
use bevy::prelude::*;
use bevy::utils::tracing::event;
use rhai::{Dynamic, Engine, Scope};

pub fn execute_rhai_engine(mut graph_defn: ResMut<GraphDefinitionRes>, time: Res<Time>) {
    let mut message_store = Arc::new(RwLock::new(Vec::<Dynamic>::new()));
    let mut engine = initialize_engine(&message_store);

    let gd = &mut graph_defn.graph_defn;
    for node in gd.node_instances.iter_mut() {
        node.timer.tick(time.delta());
        if !node.timer.finished() {
            continue;
        }

        if let Some(func) = &node.node_data.func {
            let scope = &mut node.scope;
            populate_scope(scope, &node.node_data.params);
            if let Err(e) = engine.run_ast_with_scope(scope, &node.ast) {
                c_log!("error running: {:?}", e);
            } else {
              c_log!("size of message_store: {:?}", message_store.read().unwrap().len());
            }

        }
        
    }
}

fn initialize_engine(message_store:  &Arc<RwLock<Vec<Dynamic>>>) -> Engine {
    let mut engine = Engine::new();
    let ms = message_store.clone();
    engine.register_fn("log", rhai_log)
          .register_fn("send", move |msg: Dynamic| {
              let mut store = ms.write().unwrap();
              store.push(msg);
              c_log!("pushed a message");
    });
    
    engine
}

fn populate_scope(scope: &mut Scope, params: &Vec::<NodeParams>) {
    for param in params {
        match &param.param_type {
            ParamType::Bool { default } => scope.push_constant(param.name.clone(), *default),
            ParamType::Float { default, .. } => scope.push_constant(param.name.clone(), *default),
            ParamType::Integer { default, .. } => scope.push_constant(param.name.clone(), *default),
            ParamType::String { default } => scope.push_constant(param.name.clone(), default.clone()),
            ParamType::Option { default, .. } => scope.push_constant(param.name.clone(), default.clone()),
        };
    }
}
