use std::sync::{Arc, RwLock};

use crate::c_log;
use crate::components::message::{Message, Messages};
use crate::components::node_connector::NodeConnector;
use crate::parser::graphv2::{
    instantiate_node, resolve_ticks_secs, GraphDefinition, Node, NodeConnection, NodeParams,
    ParamType, Ticks,
};
use crate::resources::graph_def::{GraphDefinitionRes, NodeAdded, NodeRemoved};
use crate::resources::narration::{ExplainEntry, PendingExplain};
use bevy::prelude::*;
use rand::Rng;
use rhai::{CallFnOptions, Dynamic, Engine, Scope};
use std::time::Duration;

/// Hard cap on live nodes, so a runaway `spawn()` loop in a script can't lock up the tab.
const MAX_NODES: usize = 200;

pub fn execute_rhai_engine(
    mut graph_defn: ResMut<GraphDefinitionRes>,
    time: Res<Time>,
    mut query_conn: Query<(&mut Messages, &mut NodeConnector)>,
    mut node_added_writer: EventWriter<NodeAdded>,
    mut node_removed_writer: EventWriter<NodeRemoved>,
    mut pending_explain: ResMut<PendingExplain>,
    // TEMPORARY - see systems::profiling's doc comment.
    mut prof: ResMut<crate::systems::profiling::ProfilingStats>,
) {
    let __prof_t0 = web_time::Instant::now(); // TEMPORARY
    let message_store = Arc::new(RwLock::new(Vec::<(String, String, Dynamic)>::new()));
    let local_message_store = Arc::new(RwLock::new(Vec::<(String, Dynamic)>::new()));
    // Set by a handler calling `draw([...])`; `None` means the handler left the
    // node's existing overlay untouched. Drained after every handler invocation.
    let draw_store = Arc::new(RwLock::new(None::<rhai::Array>));
    // `spawn`/`despawn` are queued here (attributed to the calling node once its
    // handler call returns, same as `send`) and applied once at the very end of the
    // frame, after `node_map`'s `&mut Node` borrows into `graph_defn.node_instances`
    // are all dropped - the apply step pushes/removes elements from that Vec, which
    // would otherwise invalidate those live borrows.
    // The 4th element is an optional per-instance `fn` override - see
    // `spawn_node`'s 4-arg overload and `NodeConnection::func`'s doc comment.
    let local_spawn_store =
        Arc::new(RwLock::new(Vec::<(String, String, Vec<String>, Option<String>)>::new()));
    let local_despawn_store = Arc::new(RwLock::new(Vec::<String>::new()));
    let mut pending_spawns: Vec<(String, String, String, Vec<String>, Option<String>)> = Vec::new();
    let mut pending_despawns: Vec<(String, String)> = Vec::new();
    // `link`/`unlink` are self-scoped: applied immediately to the calling node right
    // after its handler call returns (mutating a field of an already-borrowed `Node`
    // is safe, unlike spawn/despawn above), same timing as `draw`.
    let local_link_store = Arc::new(RwLock::new(Vec::<(bool, String)>::new()));
    // `explain()` calls are attributed to the caller the same way spawn/despawn
    // are, but applied immediately (no deferral needed - narration never touches
    // `node_instances`); the once-per-run dedup happens against the persistent
    // `PendingExplain` resource once we have the caller's name in hand.
    let local_explain_store = Arc::new(RwLock::new(Vec::<(String, String)>::new()));
    let mut pending_explain_frame: Vec<(String, String, String)> = Vec::new();
    let local_log_store = Arc::new(RwLock::new(Vec::<String>::new()));
    // Set by a handler calling `update_node_params(#{...})`; drained after every
    // handler invocation the same way `draw_store` is - see
    // `parser::graphv2::apply_template_param_updates`.
    let update_params_store = Arc::new(RwLock::new(None::<rhai::Map>));

    let engine = initialize_engine(
        &local_message_store,
        &draw_store,
        &local_spawn_store,
        &local_despawn_store,
        &local_link_store,
        &local_explain_store,
        &local_log_store,
        &update_params_store,
    );
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
        if let Ticks::Range { jitter: true, .. } = &node.node_data.attrs.ticks {
            node.timer
                .set_duration(Duration::from_secs(resolve_ticks_secs(
                    &node.node_data.attrs.ticks,
                )));
        }
        if let Some(_) = &node.node_data.func {
            let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);
            *draw_store.write().unwrap() = None;
            let scope = &mut node.scope;
            let init_size = scope.len();
            populate_scope(scope, &node.node_data.params);
            // Move `state` into/out of the scope rather than cloning it -
            // `get_value`/`set_value` would deep-clone the whole `state` map on
            // every tick for every node; `push_dynamic`/`remove::<Dynamic>` instead
            // transfer ownership (the same move-not-clone trick `set_links_const`
            // below already uses for `links`), which is free regardless of how big
            // or nested a node's state is. Safe because the entry doesn't need to
            // survive the call - `scope.rewind` below discards it either way.
            scope.push_dynamic("state", std::mem::take(&mut node.state));

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
            for msg in local_log_store.write().unwrap().drain(..) {
                crate::wasm::browser::emit_log_event(&node.name, &msg);
            }
            node.state = scope.remove::<Dynamic>("state").unwrap_or_default();
            scope.rewind(init_size);
            if let Some(shapes) = draw_store.write().unwrap().take() {
                node.overlay = crate::parser::draw::parse_overlay(&shapes);
                node.overlay_dirty = true;
            }
            if let Some(map) = update_params_store.write().unwrap().take() {
                let updates = map.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                if let Err(msg) = crate::parser::graphv2::apply_template_param_updates(node, updates) {
                    crate::log_dsa_event!("WARN: {}", msg);
                }
            }
            drain_topology_ops(
                node,
                &local_spawn_store,
                &local_despawn_store,
                &local_link_store,
                &local_explain_store,
                &mut pending_spawns,
                &mut pending_despawns,
                &mut pending_explain_frame,
            );
        }
    }

    // process delivered messages
    for (mut msgs, _) in query_conn.iter_mut() {
        for msg in msgs.msg_delivered.iter_mut() {
            let _ = node_map.get_mut(&msg.node_to).is_some_and(|node| {
                if let Some(_) = &node.node_data.func {
                    *draw_store.write().unwrap() = None;
                    let scope = &mut node.scope;
                    let init_size = scope.len();
                    populate_scope(scope, &node.node_data.params);
                    scope.push_dynamic("state", std::mem::take(&mut node.state));
                    let mut call_res = engine.call_fn_with_options::<()>(
                        CallFnOptions::new().eval_ast(false).rewind_scope(false),
                        scope,
                        &node.ast,
                        "on_msg",
                        (msg.obj.clone(),),
                    );
                    if let Err(ref e) = call_res {
                        if let rhai::EvalAltResult::ErrorFunctionNotFound(f, _) = &**e {
                            if f.starts_with("on_msg") {
                                call_res = engine.call_fn_with_options::<()>(
                                    CallFnOptions::new().eval_ast(false).rewind_scope(false),
                                    scope,
                                    &node.ast,
                                    "on_message",
                                    (msg.obj.clone(),),
                                );
                            }
                        }
                    }
                    if let Err(ref e) = call_res {
                        if let rhai::EvalAltResult::ErrorFunctionNotFound(f, _) = &**e {
                            if f.starts_with("on_msg") || f.starts_with("on_message") {
                                call_res = engine.call_fn_with_options::<()>(
                                    CallFnOptions::new().eval_ast(false).rewind_scope(false),
                                    scope,
                                    &node.ast,
                                    "on_msg",
                                    (msg.node_from.clone(), msg.obj.clone()),
                                );
                            }
                        }
                    }
                    if let Err(ref e) = call_res {
                        if let rhai::EvalAltResult::ErrorFunctionNotFound(f, _) = &**e {
                            if f.starts_with("on_msg") || f.starts_with("on_message") {
                                call_res = engine.call_fn_with_options::<()>(
                                    CallFnOptions::new().eval_ast(false).rewind_scope(false),
                                    scope,
                                    &node.ast,
                                    "on_message",
                                    (msg.node_from.clone(), msg.obj.clone()),
                                );
                            }
                        }
                    }
                    if let Err(e) = call_res {
                        if let rhai::EvalAltResult::ErrorFunctionNotFound(_, _) = &*e {
                            // Function not defined on node, which is normal if node doesn't handle messages
                        } else {
                            c_log!("error running on_msg / on_message: {:?}", e);
                        }
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
                    for msg in local_log_store.write().unwrap().drain(..) {
                        crate::wasm::browser::emit_log_event(&node.name, &msg);
                    }
                    node.state = scope.remove::<Dynamic>("state").unwrap_or_default();
                    scope.rewind(init_size);
                    if let Some(shapes) = draw_store.write().unwrap().take() {
                        node.overlay = crate::parser::draw::parse_overlay(&shapes);
                        node.overlay_dirty = true;
                    }
                    if let Some(map) = update_params_store.write().unwrap().take() {
                        let updates = map.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                        if let Err(msg) = crate::parser::graphv2::apply_template_param_updates(node, updates) {
                            crate::log_dsa_event!("WARN: {}", msg);
                        }
                    }
                    drain_topology_ops(
                        node,
                        &local_spawn_store,
                        &local_despawn_store,
                        &local_link_store,
                        &local_explain_store,
                        &mut pending_spawns,
                        &mut pending_despawns,
                        &mut pending_explain_frame,
                    );
                    true
                } else {
                    false
                }
            });
        }
        msgs.msg_delivered.clear();
    }

    // `node_map`'s borrows into `gd.node_instances` must end before that Vec can be
    // pushed to or removed from.
    drop(node_map);

    for name in apply_despawns(gd, pending_despawns) {
        node_removed_writer.send(NodeRemoved { name });
    }
    for name in apply_spawns(gd, &engine, pending_spawns, &draw_store, &local_log_store, &update_params_store) {
        node_added_writer.send(NodeAdded { name });
    }
    for (node_name, key, text) in pending_explain_frame {
        if pending_explain.seen.insert(key.clone()) {
            pending_explain.queue.push_back(ExplainEntry {
                node_name,
                key,
                text,
            });
        }
    }

    send_messages(&message_store, &mut query_conn);
    prof.rhai_ms += __prof_t0.elapsed().as_secs_f64() * 1000.0; // TEMPORARY
}

/// Drains the `spawn`/`despawn`/`link`/`unlink` calls the handler that just returned
/// made. `link`/`unlink` mutate `node` directly since that's always safe; `spawn`/
/// `despawn` only get attributed to `node.name` here and queued for later.
fn drain_topology_ops(
    node: &mut Node,
    local_spawn_store: &Arc<RwLock<Vec<(String, String, Vec<String>, Option<String>)>>>,
    local_despawn_store: &Arc<RwLock<Vec<String>>>,
    local_link_store: &Arc<RwLock<Vec<(bool, String)>>>,
    local_explain_store: &Arc<RwLock<Vec<(String, String)>>>,
    pending_spawns: &mut Vec<(String, String, String, Vec<String>, Option<String>)>,
    pending_despawns: &mut Vec<(String, String)>,
    pending_explain_frame: &mut Vec<(String, String, String)>,
) {
    local_spawn_store
        .write()
        .unwrap()
        .drain(..)
        .for_each(|(name, node_type, links, func_override)| {
            pending_spawns.push((node.name.clone(), name, node_type, links, func_override));
        });
    local_despawn_store
        .write()
        .unwrap()
        .drain(..)
        .for_each(|name| {
            pending_despawns.push((node.name.clone(), name));
        });
    local_explain_store
        .write()
        .unwrap()
        .drain(..)
        .for_each(|(key, text)| {
            pending_explain_frame.push((node.name.clone(), key, text));
        });

    for (add, peer) in local_link_store.write().unwrap().drain(..) {
        if add {
            if !node.links.contains(&peer) {
                node.links.push(peer);
            }
        } else {
            node.links.retain(|p| p != &peer);
        }
        set_links_const(&mut node.scope, &node.links);
    }
}

/// Replaces the `links` constant in a node's Rhai scope with `links`'s current
/// value. `links` was pushed as a Rhai *constant* in `init_scope` and stays that way
/// across ticks (only `state` is meant to be host-mutable); `Scope::set_value`
/// panics on a read-only entry, so the entry has to be removed and reinserted -
/// `Scope::remove` is a plain Rust API and doesn't go through Rhai's read-only
/// enforcement (that lives in the interpreter's eval path, not in the `Scope` data
/// structure), so this is safe. The `Vec<String> -> Dynamic` conversion has to go
/// through `.into()`, not `push_constant`'s generic parameter directly:
/// `push_constant` funnels its argument through `Dynamic::from`'s *inherent* method,
/// which only special-cases the exact type `Array` (`Vec<Dynamic>`) and otherwise
/// opaque-wraps it (not iterable from script); `.into()` resolves to the separate
/// `impl From<Vec<T>> for Dynamic`, which converts element-by-element into a real
/// Rhai array.
fn set_links_const(scope: &mut Scope, links: &[String]) {
    let _ = scope.remove::<Dynamic>("links");
    let links: Dynamic = links.to_vec().into();
    scope.push_constant("links", links);
}

/// Applies queued `despawn()` calls: drops the named nodes, and scrubs them out of
/// every surviving node's `links` (both the bookkeeping `Vec` and that node's own
/// Rhai scope). Returns the names actually removed, for `NodeRemoved` events.
fn apply_despawns(gd: &mut GraphDefinition, pending: Vec<(String, String)>) -> Vec<String> {
    let mut removed = Vec::new();
    for (requester, name) in pending {
        let before = gd.node_instances.len();
        gd.node_instances.retain(|n| n.name != name);
        gd.graph.retain(|n| n.name != name);

        if gd.node_instances.len() == before {
            c_log!("{} tried to despawn unknown node {}", requester, name);
            continue;
        }

        for n in gd.node_instances.iter_mut() {
            if n.links.contains(&name) {
                n.links.retain(|p| p != &name);
                set_links_const(&mut n.scope, &n.links);
            }
        }

        c_log!("{} despawned {}", requester, name);
        removed.push(name);
    }
    removed
}

/// Applies queued `spawn()` calls: validates the request, instantiates the node
/// (running its `on_init` through the live, fully-registered `engine` so `draw()`/
/// `log()` in `on_init` work immediately), and appends it to the graph. Returns the
/// names actually created, for `NodeAdded` events.
fn apply_spawns(
    gd: &mut GraphDefinition,
    engine: &Engine,
    pending: Vec<(String, String, String, Vec<String>, Option<String>)>,
    draw_store: &Arc<RwLock<Option<rhai::Array>>>,
    local_log_store: &Arc<RwLock<Vec<String>>>,
    update_params_store: &Arc<RwLock<Option<rhai::Map>>>,
) -> Vec<String> {
    let mut spawned = Vec::new();
    for (requester, name, node_type_id, links, func_override) in pending {
        if gd.node_instances.len() >= MAX_NODES {
            c_log!(
                "{} tried to spawn {} but the node cap ({}) is reached",
                requester,
                name,
                MAX_NODES
            );
            continue;
        }
        if name.is_empty() || gd.node_instances.iter().any(|n| n.name == name) {
            c_log!(
                "{} tried to spawn {} but that name is empty or already in use",
                requester,
                name
            );
            continue;
        }
        let Some(node_type) = gd.node_types.iter().find(|t| t.id == node_type_id).cloned() else {
            c_log!(
                "{} tried to spawn {} with unknown node type {}",
                requester,
                name,
                node_type_id
            );
            continue;
        };
        // Only link to peers that already exist - keeps this a same-frame operation
        // with no forward references; a script can just retry the link next tick.
        let valid_links: Vec<String> = links
            .into_iter()
            .filter(|peer| gd.node_instances.iter().any(|n| &n.name == peer))
            .collect();

        // A 4-arg `spawn_node(name, type, links, fn)` call overrides just this
        // instance's script, the same way a static `graph:` entry's own `fn:`
        // does in `parser::graphv2::parse_graph2_with_sources` - the type's
        // icon/template/params are untouched.
        let effective_type = if let Some(script) = &func_override {
            let mut overridden = node_type.clone();
            overridden.func = Some(script.clone());
            if let Err(e) = crate::parser::graphv2::compile_ast(engine, &mut overridden) {
                c_log!(
                    "{} tried to spawn {} with an invalid fn override: {:?}",
                    requester,
                    name,
                    e
                );
                continue;
            }
            overridden
        } else {
            node_type
        };

        let node = instantiate_node(
            engine,
            &effective_type,
            name.clone(),
            valid_links.clone(),
            draw_store,
            update_params_store,
        );
        for msg in local_log_store.write().unwrap().drain(..) {
            crate::wasm::browser::emit_log_event(&node.name, &msg);
        }

        gd.graph.push(NodeConnection {
            name: name.clone(),
            node_type: node_type_id,
            func: func_override,
            links: valid_links,
            group: None,
            rank: None,
            order: None,
            offset: None,
            pos: None,
            draggable: None,
        });
        gd.node_instances.push(node);
        c_log!("{} spawned {}", requester, name);
        spawned.push(name);
    }
    spawned
}

fn send_messages(
    message_store: &Arc<RwLock<Vec<(String, String, Dynamic)>>>,
    q: &mut Query<(&mut Messages, &mut NodeConnector)>,
) {
    let mut store = message_store.write().unwrap();
    for (from, to, msg) in store.iter_mut() {
        let (msg_display, icon) = if msg.is_map() {
            let (display, icon) = {
                let mut val = msg.write_lock::<rhai::Map>().unwrap();
                val.insert("from".into(), from.clone().into());
                let icon = val.get("icon").map(|v| v.to_string());
                let display = val.get("display").map(|d| d.to_string());
                (display, icon)
            };
            let display = display.unwrap_or_else(|| format!("{}", msg));
            (display, icon)
        } else if msg.is_string() {
            (
                msg.clone().into_string().unwrap_or_else(|_| format!("{}", msg)),
                None,
            )
        } else {
            (format!("{}", msg), None)
        };

        for (mut msgs, nc) in q.iter_mut() {
            if nc.id1.eq(from) && nc.id2.eq(to) || nc.id2.eq(from) && nc.id1.eq(to) {
                c_log!("sending message from {} to {}", from, to);

                msgs.msg_inflight.push(Message {
                    timer: Timer::new(Duration::from_secs(3), TimerMode::Once),
                    node_from: from.clone(),
                    node_to: to.clone(),
                    str: msg_display.clone(),
                    obj: msg.clone(),
                    icon: icon.clone(),
                    bubble_entity: None,
                });
            }
        }
    }
    store.clear();
}

fn initialize_engine(
    message_store: &Arc<RwLock<Vec<(String, Dynamic)>>>,
    draw_store: &Arc<RwLock<Option<rhai::Array>>>,
    spawn_store: &Arc<RwLock<Vec<(String, String, Vec<String>, Option<String>)>>>,
    despawn_store: &Arc<RwLock<Vec<String>>>,
    link_store: &Arc<RwLock<Vec<(bool, String)>>>,
    explain_store: &Arc<RwLock<Vec<(String, String)>>>,
    log_store: &Arc<RwLock<Vec<String>>>,
    update_params_store: &Arc<RwLock<Option<rhai::Map>>>,
) -> Engine {
    let mut engine = Engine::new();
    // Match the compile-time limit raised in `parser::graphv2::parse_graph2`.
    engine.set_max_expr_depths(256, 256);
    let ms = message_store.clone();
    let ds = draw_store.clone();
    let ds_one = draw_store.clone();
    let sp = spawn_store.clone();
    let sp_override = spawn_store.clone();
    let dsp = despawn_store.clone();
    let lk_add = link_store.clone();
    let lk_rm = link_store.clone();
    let ls1 = log_store.clone();
    let ls2 = log_store.clone();
    let ls3 = log_store.clone();
    engine
        .register_fn("log", move |s: String| {
            ls1.write().unwrap().push(s);
        })
        .register_fn("log", move |s: Dynamic| {
            ls2.write().unwrap().push(s.to_string());
        })
        .register_fn("log", move |a: Dynamic, b: Dynamic| {
            ls3.write().unwrap().push(format!("{} {}", a, b));
        })
        .register_fn("send", move |to: String, msg: Dynamic| {
            let mut store = ms.write().unwrap();
            store.push((to, msg));
        })
        // Replace the calling node's canvas overlay with `shapes` (a list of shape
        // maps - see `parser::draw`). Persists until the node calls `draw()` again;
        // `draw([])` clears it.
        .register_fn("draw", move |shapes: rhai::Array| {
            *ds.write().unwrap() = Some(shapes);
        })
        .register_fn("draw", move |shape: rhai::Map| {
            *ds_one.write().unwrap() = Some(vec![Dynamic::from_map(shape)]);
        })
        // Returns true with roughly `percent` probability (0-100). Lets scripts
        // simulate flaky/failing behavior (e.g. a participant randomly voting no)
        // without needing a manually-toggled param for every failure scenario.
        .register_fn("random_chance", |percent: i64| -> bool {
            rand::thread_rng().gen_range(0..100) < percent
        })
        // Returns a random integer in [min, max) - e.g. `random_int(0, links.len())`
        // to pick a random peer out of a node's own `links`. `max <= min` returns
        // `min` rather than panicking on an empty/degenerate range (a node with no
        // links calling `random_int(0, links.len())` would otherwise crash it).
        .register_fn("random_int", |min: i64, max: i64| -> i64 {
            if max <= min {
                min
            } else {
                rand::thread_rng().gen_range(min..max)
            }
        })
        // Create a new node instance of `node_type` named `name`, linked to `links`
        // (peers that must already exist - no forward references). Applied once at
        // the end of the frame; its `on_init` runs immediately when applied.
        // Named `spawn_node` (not `spawn`) because Rhai reserves the bare word
        // `spawn`, presumably for a future async/threading feature - `spawn(...)` is
        // a parse error no matter what a host registers under that name.
        .register_fn(
            "spawn_node",
            move |name: String, node_type: String, links: rhai::Array| {
                let links: Vec<String> = links
                    .into_iter()
                    .filter_map(|v| v.into_string().ok())
                    .collect();
                sp.write().unwrap().push((name, node_type, links, None));
            },
        )
        // 4-arg overload: `fn_override` replaces just this new instance's
        // script - the type's icon/template/params are untouched. Same
        // mechanism a static `graph:` entry's own `fn:` uses (see
        // `NodeConnection::func`).
        .register_fn(
            "spawn_node",
            move |name: String, node_type: String, links: rhai::Array, fn_override: String| {
                let links: Vec<String> = links
                    .into_iter()
                    .filter_map(|v| v.into_string().ok())
                    .collect();
                sp_override
                    .write()
                    .unwrap()
                    .push((name, node_type, links, Some(fn_override)));
            },
        )
        // Remove any node by name, including the calling node itself.
        .register_fn("despawn", move |name: String| {
            dsp.write().unwrap().push(name);
        })
        // Add `peer` to the calling node's own `links` - self-scoped, takes effect
        // starting with this node's very next `on_timer`/`on_msg` call.
        .register_fn("link", move |peer: String| {
            lk_add.write().unwrap().push((true, peer));
        })
        .register_fn("unlink", move |peer: String| {
            lk_rm.write().unwrap().push((false, peer));
        });
        let up = update_params_store.clone();
        engine
            // Update just the calling node's `{{param}}` -> value bindings and
            // re-render its overlay from its own `attrs.template` shapes -
            // unlike `draw()`, this doesn't require restating the whole shape
            // list, only what changed. No-op (with a log warning - see
            // `apply_template_param_updates`) on a node with no
            // `template_ref`/`template` at all, or an empty `#{...}`.
            .register_fn("update_node_params", move |p: rhai::Map| {
                *up.write().unwrap() = Some(p);
            });
        let ex1 = explain_store.clone();
        let ex2 = explain_store.clone();
        let ex3 = explain_store.clone();
        let ex4 = explain_store.clone();
        engine
            .register_fn("explain", move |key: String, text: String| {
                ex1.write().unwrap().push((key, text));
            })
            .register_fn("explain", move |key: String, text: String, _opts: Dynamic| {
                ex2.write().unwrap().push((key, text));
            })
            .register_fn("explain", move |text: String| {
                let key = text.clone();
                ex3.write().unwrap().push((key, text));
            })
            .register_fn("explain", move |text: String, _opts: Dynamic| {
                let key = text.clone();
                ex4.write().unwrap().push((key, text));
            });

    engine
}

fn populate_scope(scope: &mut Scope, params: &Option<Vec<NodeParams>>) {
    if let None = params {
        return;
    }
    for param in params.as_ref().unwrap() {
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
