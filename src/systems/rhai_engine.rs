use std::sync::{Arc, RwLock};

use crate::c_log;
use crate::components::message::{Message, Messages};
use crate::components::node_connector::NodeConnector;
use crate::parser::graphv2::{
    instantiate_node, resolve_ticks_secs, GraphDefinition, Node, NodeConnection, NodeParams,
    ParamType, Ticks,
};
use crate::resources::graph_def::{GraphDefinitionRes, NodeAdded, NodeRemoved, NodeTicked};
use crate::resources::narration::{ExplainEntry, PendingExplain};
use crate::stdlib::rhai_lib::rhai_log;
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
    mut ticked_writer: EventWriter<NodeTicked>,
    mut node_added_writer: EventWriter<NodeAdded>,
    mut node_removed_writer: EventWriter<NodeRemoved>,
    mut pending_explain: ResMut<PendingExplain>,
) {
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
    let local_spawn_store = Arc::new(RwLock::new(Vec::<(String, String, Vec<String>)>::new()));
    let local_despawn_store = Arc::new(RwLock::new(Vec::<String>::new()));
    let mut pending_spawns: Vec<(String, String, String, Vec<String>)> = Vec::new();
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

    let engine = initialize_engine(
        &local_message_store,
        &draw_store,
        &local_spawn_store,
        &local_despawn_store,
        &local_link_store,
        &local_explain_store,
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
        ticked_writer.send(NodeTicked {
            name: node.name.clone(),
        });
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
            // `get_value`/`set_value` would deep-clone the whole `globals` map on
            // every tick for every node; `push_dynamic`/`remove::<Dynamic>` instead
            // transfer ownership (the same move-not-clone trick `set_links_const`
            // below already uses for `links`), which is free regardless of how big
            // or nested a node's state is. Safe because the entry doesn't need to
            // survive the call - `scope.rewind` below discards it either way.
            scope.push_dynamic("globals", std::mem::take(&mut node.state));

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
            node.state = scope.remove::<Dynamic>("globals").unwrap_or_default();
            scope.rewind(init_size);
            if let Some(shapes) = draw_store.write().unwrap().take() {
                node.overlay = crate::parser::draw::parse_overlay(&shapes);
                node.overlay_dirty = true;
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
                    let options = CallFnOptions::new().eval_ast(false).rewind_scope(false);
                    *draw_store.write().unwrap() = None;
                    let scope = &mut node.scope;
                    let init_size = scope.len();
                    populate_scope(scope, &node.node_data.params);
                    // See the matching comment in the `on_timer` branch above - move,
                    // don't clone, `state` through the scope.
                    scope.push_dynamic("globals", std::mem::take(&mut node.state));
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
                    node.state = scope.remove::<Dynamic>("globals").unwrap_or_default();
                    scope.rewind(init_size);
                    if let Some(shapes) = draw_store.write().unwrap().take() {
                        node.overlay = crate::parser::draw::parse_overlay(&shapes);
                        node.overlay_dirty = true;
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
    for name in apply_spawns(gd, &engine, pending_spawns, &draw_store) {
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
}

/// Drains the `spawn`/`despawn`/`link`/`unlink` calls the handler that just returned
/// made. `link`/`unlink` mutate `node` directly since that's always safe; `spawn`/
/// `despawn` only get attributed to `node.name` here and queued for later.
fn drain_topology_ops(
    node: &mut Node,
    local_spawn_store: &Arc<RwLock<Vec<(String, String, Vec<String>)>>>,
    local_despawn_store: &Arc<RwLock<Vec<String>>>,
    local_link_store: &Arc<RwLock<Vec<(bool, String)>>>,
    local_explain_store: &Arc<RwLock<Vec<(String, String)>>>,
    pending_spawns: &mut Vec<(String, String, String, Vec<String>)>,
    pending_despawns: &mut Vec<(String, String)>,
    pending_explain_frame: &mut Vec<(String, String, String)>,
) {
    local_spawn_store
        .write()
        .unwrap()
        .drain(..)
        .for_each(|(name, node_type, links)| {
            pending_spawns.push((node.name.clone(), name, node_type, links));
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
/// across ticks (only `globals` is meant to be host-mutable); `Scope::set_value`
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
    pending: Vec<(String, String, String, Vec<String>)>,
    draw_store: &Arc<RwLock<Option<rhai::Array>>>,
) -> Vec<String> {
    let mut spawned = Vec::new();
    for (requester, name, node_type_id, links) in pending {
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

        *draw_store.write().unwrap() = None;
        let mut node = instantiate_node(engine, &node_type, name.clone(), valid_links.clone());
        if let Some(shapes) = draw_store.write().unwrap().take() {
            node.overlay = crate::parser::draw::parse_overlay(&shapes);
            node.overlay_dirty = true;
        }

        gd.graph.push(NodeConnection {
            name: name.clone(),
            node_type: node_type_id,
            links: valid_links,
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
    let mut msg_display: String;
    for (from, to, msg) in store.iter_mut() {
        if msg.is_map() {
            let set_display;
            let icon: Option<String>;
            {
                let mut val = msg.write_lock::<rhai::Map>().unwrap();
                msg_display = "".to_string();

                val.insert("from".into(), from.clone().into());

                set_display = val.get("display").is_some_and(|v| {
                    msg_display = v.to_string();
                    true
                });

                icon = val.get("icon").map(|v| v.to_string());
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
                        icon: icon.clone(),
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

fn initialize_engine(
    message_store: &Arc<RwLock<Vec<(String, Dynamic)>>>,
    draw_store: &Arc<RwLock<Option<rhai::Array>>>,
    spawn_store: &Arc<RwLock<Vec<(String, String, Vec<String>)>>>,
    despawn_store: &Arc<RwLock<Vec<String>>>,
    link_store: &Arc<RwLock<Vec<(bool, String)>>>,
    explain_store: &Arc<RwLock<Vec<(String, String)>>>,
) -> Engine {
    let mut engine = Engine::new();
    // Match the compile-time limit raised in `parser::graphv2::parse_graph2`.
    engine.set_max_expr_depths(256, 256);
    let ms = message_store.clone();
    let ds = draw_store.clone();
    let ds_one = draw_store.clone();
    let sp = spawn_store.clone();
    let dsp = despawn_store.clone();
    let lk_add = link_store.clone();
    let lk_rm = link_store.clone();
    let ex = explain_store.clone();
    engine
        .register_fn("log", rhai_log)
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
                sp.write().unwrap().push((name, node_type, links));
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
        })
        // Show a pausing narration bubble pointing at the calling node, the first
        // time `key` is ever seen this run - every later call with that key,
        // from any node, is a no-op. See `resources::narration::PendingExplain`
        // and `systems::explain_bubble`.
        .register_fn("explain", move |key: String, text: String| {
            ex.write().unwrap().push((key, text));
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
