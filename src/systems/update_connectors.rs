use crate::components::node::Node;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::c_log;
use crate::{components::node_connector::*, parser::graphv2::GraphAttrs};
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_prototype_lyon::prelude::*;
use std::collections::{HashMap, HashSet};

pub fn update_connectors(
    g: Res<GraphDefinitionRes>,
    mut commands: Commands,
    query_changed: Query<(&Node, &Transform), Changed<Transform>>,
    query_added: Query<(&Node, &Transform), Added<Node>>,
    query_all: Query<(&Node, &Transform)>,
    mut query_conn: Query<(Entity, &mut Path, &mut NodeConnector)>,
) {
    if g.graph_defn.graph.iter().len() == 0 {
        for (entity, _path, _conn) in query_conn.iter_mut() {
            commands.entity(entity).despawn_recursive();
        }
        return;
    }

    if !query_added.is_empty() && g.graph_defn.nodes.iter().len() != 0 {
        let mut all_node_loc = HashMap::<String, Vec3>::new();

        for (entity, _path, _conn) in query_conn.iter_mut() {
            commands.entity(entity).despawn_recursive();
        }

        //insert in hash map location of all nodes
        for (node, transform) in query_all.iter() {
            let mut pos: Vec3 = transform.translation;
            pos.z = 50.;
            all_node_loc.insert(node.node_id.clone(), pos);
        }

        let mut done: HashSet<String> = HashSet::new();
        // for each node search its connecting entities
        // and make a line between current node and that node
        for (nodea_id, nodea_loc) in all_node_loc.iter() {
            let node_links: Option<&HashSet<String>> = g.graph_defn.graph.get(nodea_id);
            match node_links {
                Some(nl) => {
                    for nodeb_id in nl.iter() {
                        let s: String;
                        let s2: String;
                        s = format!("{}-{}", nodea_id, nodeb_id);
                        s2 = format!("{}-{}", nodeb_id, nodea_id);
                        if nodea_id == nodeb_id {
                            c_log!(
                                "Ignoring loopback: {}-{}", nodea_id, nodeb_id
                            );
                            continue;
                        }
                        if all_node_loc.get(nodeb_id).is_none() {
                            c_log!(
                                "Node not found for connector: {}-{}", nodea_id, nodeb_id
                            );
                            continue;
                        }
                        if !done.contains(&s) {
                            let nodeb_loc = all_node_loc.get(nodeb_id).unwrap();
                            done.insert(s);
                            done.insert(s2);
                            let _ = generate_line(
                                nodea_loc,
                                nodeb_loc,
                                &g.graph_defn.graph_attrs,
                                &nodea_id,
                                &nodeb_id,
                                &mut commands,
                            );
                        }
                    }
                }
                None => {}
            }
        }
    } else if !query_changed.is_empty() {
        let mut all_node_loc = HashMap::<String, Vec3>::new();

        for (node, transform) in query_all.iter() {
            let mut pos: Vec3 = transform.translation;
            pos.z = 50.;
            all_node_loc.insert(node.node_id.clone(), pos);
        }

        for (_, mut path, mut conn) in query_conn.iter_mut() {
            let node1_loc = all_node_loc.get(&conn.id1);
            let node2_loc = all_node_loc.get(&conn.id2);
            if node1_loc.is_none() || node2_loc.is_none() {
                c_log!(
                    "Node not found for connector: {}-{}", conn.id1, conn.id2
                );
                continue;
            }
            let mut path_builder = PathBuilder::new();

            path_builder.move_to(Vec2 {
                x: node1_loc.unwrap().x,
                y: node1_loc.unwrap().y,
            });
            path_builder.cubic_bezier_to(
                Vec2::new(node1_loc.unwrap().x + 50., node1_loc.unwrap().y + 50.),
                Vec2::new(node2_loc.unwrap().x + 50., node2_loc.unwrap().y + 50.),
                Vec2::new(node2_loc.unwrap().x, node2_loc.unwrap().y),
            );

            *path = path_builder.build();
            (*conn).path = path.0.clone();
        }
    } 
}

fn generate_line(
    a: &Vec3,
    b: &Vec3,
    ga: &GraphAttrs,
    id1: &String,
    id2: &String,
    commands: &mut Commands,
) -> Entity {
    let mut path_builder = PathBuilder::new();

    path_builder.move_to(Vec2 { x: a.x, y: a.y });
    path_builder.cubic_bezier_to(
        Vec2::new(a.x + 50., a.y + 50.),
        Vec2::new(b.x + 50., b.y + 50.),
        Vec2::new(b.x, b.y),
    );

    let path = path_builder.build();
    let walking_path = path.0.clone();
    let cc = ga.connection_color;

    commands
        .spawn((
            ShapeBundle { path, ..default() },
            Stroke::new(cc, 3.0),
            NodeConnector {
                id1: id1.clone(),
                id2: id2.clone(),
                path: walking_path,
            },
            On::<Pointer<Over>>::target_component_mut::<Stroke>(|_, s| {
                s.options.line_width = 7.;
            }),
            On::<Pointer<Out>>::target_component_mut::<Stroke>(|_, s| {
                s.options.line_width = 3.;
            }),
        ))
        .id()
}

//#[test]
// fn did_spawn_connectors() {
//   use std::collections::HashSet;
//   use crate::parser::graphv2::Node as NodeData;
//   use crate::ui::*;
//   use crate::systems::update_node::update_nodes;
//   use bevy::asset::AssetServer;
//   use bevy::asset::FileAssetIo;
//   use bevy::tasks::IoTaskPool;
//   let mut app = App::new();
//   let mut graph = NodeDepsMap::new();
//   let mut hs = HashSet::new();

//   hs.insert("b".to_string());
//   hs.insert("c".to_string());

//   graph.insert("a".to_string(), hs.clone());
//   let mut graph_defn = GraphDefinition::default();
//   graph_defn.graph = graph;
//   graph_defn.nodes = vec!(NodeData {
//     name: "a".to_string(),
//     ..Default::default()
//   }, NodeData {
//     name: "b".to_string(),
//     ..Default::default()
//   }, NodeData {
//     name: "c".to_string(),
//     ..Default::default()
//   });
//   app.insert_resource(GraphDefinitionRes {
//     graph_defn
//   });
//   IoTaskPool::init(Default::default);
//   app.insert_resource(AssetServer::new(FileAssetIo::new("./assets", &None)));
//   app.add_systems(Update, (update_nodes, update_connectors));
//   app.update();
//   app.update();
//   assert_eq!(app.world.query::<&Node>().iter(&app.world).count(), 3); // check all the keys have been spawned
//   assert_eq!(app.world.query::<&NodeConnector>().iter(&app.world).count(), 2);

//   assert_eq!(app.world.query::<Entity>().iter(&app.world).count(), 11); //check entity count = 3 * nodes + connectors
//   app.world.resource_mut::<GraphDefinitionRes>().graph_defn.graph.clear();
//   app.update();
//   app.update();
//   assert_eq!(app.world.query::<&Node>().iter(&app.world).count(), 0); // check if we change the graph the response is acceptable
//   assert_eq!(app.world.query::<&NodeConnector>().iter(&app.world).count(), 0);
// }
