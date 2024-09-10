use std::sync::{Arc, RwLock};

use crate::c_log;
use crate::components::message::{Message, Messages};
use crate::components::node_connector::NodeConnector;
use crate::parser::graphv2::{Node, NodeParams, ParamType};
use crate::resources::graph_def::GraphDefinitionRes;
use crate::stdlib::rhai_lib::rhai_log;
use bevy::prelude::*;
use rhai::{CallFnOptions, Dynamic, Engine, Scope};
use std::time::Duration;

pub fn execute_rhai_engine(
    mut graph_defn: ResMut<GraphDefinitionRes>,
    time: Res<Time>,
    mut query_conn: Query<(&mut Messages, &mut NodeConnector)>,
) {
    let message_store = Arc::new(RwLock::new(Vec::<(String, String, Dynamic)>::new()));
    let local_message_store = Arc::new(RwLock::new(Vec::<(String, Dynamic)>::new()));
    let engine = initialize_engine(&local_message_store);
    let gd = &mut graph_defn.graph_defn;

    let mut node_map = gd
        .node_instances
        .iter_mut()
        .map(|node| (node.name.clone(), node))
        .collect::<std::collections::HashMap<String, &mut Node>>();

    // process all timers
    for (_, node) in node_map.iter_mut() {
        node.timer.tick(time.delta());
        if !node.timer.finished() {
            continue;
        }
        if let Some(_) = &node.node_data.func {
            let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);
            let scope = &mut node.scope;
            let init_size = scope.len();
            populate_scope(scope, &node.node_data.params);
            scope.set_value("globals", node.state.clone());

            if let Err(e) =
                engine.call_fn_with_options::<()>(options, scope, &node.ast, "on_timer", ())
            {
                c_log!("error running: {:?}", e);
            } else {
                local_message_store.read().unwrap().iter().for_each(|msg| {
                    message_store.write().unwrap().push((
                        node.name.clone(),
                        msg.0.clone(),
                        msg.1.clone(),
                    ));
                });
                local_message_store.write().unwrap().clear();
            }
            node.state = scope.get_value("globals").unwrap();
            scope.rewind(init_size);
        }
    }

    // process delivered messages
    for (mut msgs, _) in query_conn.iter_mut() {
        for msg in msgs.msg_delivered.iter_mut() {
            let _ = node_map.get_mut(&msg.node_to).is_some_and(|node| {
                if let Some(_) = &node.node_data.func {
                    let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);
                    let scope = &mut node.scope;
                    let init_size = scope.len();
                    populate_scope(scope, &node.node_data.params);
                    scope.set_value("globals", node.state.clone());
                    if let Err(e) = engine.call_fn_with_options::<()>(
                        options,
                        scope,
                        &node.ast,
                        "on_msg",
                        (msg.obj.clone(),),
                    ) {
                        c_log!("error running: {:?}", e);
                    } else {
                        local_message_store.read().unwrap().iter().for_each(|msg| {
                            message_store.write().unwrap().push((
                                node.name.clone(),
                                msg.0.clone(),
                                msg.1.clone(),
                            ));
                        });
                        local_message_store.write().unwrap().clear();
                    }
                    node.state = scope.get_value("globals").unwrap();
                    scope.rewind(init_size);
                    true
                } else {
                    false
                }
            });
        }
        msgs.msg_delivered.clear();
    }
    send_messages(&message_store, &mut query_conn);
}

fn send_messages(
    message_store: &Arc<RwLock<Vec<(String, String, Dynamic)>>>,
    q: &mut Query<(&mut Messages, &mut NodeConnector)>,
) {
    let mut store = message_store.write().unwrap();
    let mut msg_display: String;
    for (from, to, msg) in store.iter_mut() {
        if msg.is_map() {
            let mut set_display = false;
            {
                let mut val = msg.write_lock::<rhai::Map>().unwrap();
                msg_display = "".to_string();

                val.insert("from".into(), from.clone().into());

                set_display = val.get("display").is_some_and(|v| {
                    msg_display = v.to_string();
                    true
                });
            }

            if !set_display {
                msg_display = format!("{}", msg);
            }

            for (mut msgs, nc) in q.iter_mut() {
                if nc.id1.eq(from) && nc.id2.eq(to) || nc.id2.eq(from) && nc.id1.eq(to) {
                    c_log!("sending message from {} to {}", from, to);

                    msgs.msg_inflight.push(Message {
                        timer: Timer::new(Duration::from_secs(3), TimerMode::Once),
                        node_from: from.clone(),
                        node_to: to.clone(),
                        str: msg_display.to_string(),
                        obj: msg.clone(),
                    });
                }
            }
        } else {
            c_log!("message is not a map");
            continue;
        }
    }
    store.clear();
}

fn initialize_engine(message_store: &Arc<RwLock<Vec<(String, Dynamic)>>>) -> Engine {
    let mut engine = Engine::new();
    let ms = message_store.clone();
    engine
        .register_fn("log", rhai_log)
        .register_fn("send", move |to: String, msg: Dynamic| {
            let mut store = ms.write().unwrap();
            store.push((to, msg));
        });

    engine
}

fn populate_scope(scope: &mut Scope, params: &Vec<NodeParams>) {
    for param in params {
        match &param.param_type {
            ParamType::Bool { default } => scope.push_constant(param.name.clone(), *default),
            ParamType::Float { default, .. } => scope.push_constant(param.name.clone(), *default),
            ParamType::Integer { default, .. } => scope.push_constant(param.name.clone(), *default),
            ParamType::String { default } => {
                scope.push_constant(param.name.clone(), default.clone())
            }
            ParamType::Option { default, .. } => {
                scope.push_constant(param.name.clone(), default.clone())
            }
        };
    }
}
