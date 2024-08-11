use crate::components::node::Node;
use crate::ui::GraphDefinitionRes;
use crate::{
    components::node_connector::*,
    parser::graphv2::GraphAttrs,
};
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
    let oga = &g.graph_defn.graph_attrs;
    let mut ga: &GraphAttrs = &GraphAttrs {
        ..Default::default()
    };
    let _ = oga.as_ref().is_some_and(|attrs| {
        ga = attrs;
        true
    });

    if !query_added.is_empty() && g.graph_defn.graph.iter().len() != 0 {
        let mut all_node_loc = HashMap::<String, Vec3>::new();

        for (entity, _path, _conn) in query_conn.iter_mut() {
            commands.entity(entity).despawn_recursive();
        }

        //insert in hash map location of all nodes
        for (node, transform) in query_all.iter() {
            println!("{:?}", node);
            let mut pos: Vec3 = transform.translation;
            pos.z = 50.;
            all_node_loc.insert(node.node_text.clone(), pos);
        }

        let mut done: HashSet<String> = HashSet::new();
        // for each node search its connecting entities
        // and make a line between current node and that node
        for (nodea_name, nodea_loc) in all_node_loc.iter() {
            let node_links = g.graph_defn.graph.get(nodea_name);
            match node_links {
                Some(nl) => {
                    for nodeb_name in nl.iter() {
                        if !done.contains(&(nodea_name.clone() + nodeb_name)) {
                            let nodeb_loc = all_node_loc.get(nodeb_name).unwrap();
                            done.insert(nodea_name.clone() + nodeb_name);
                            done.insert(nodeb_name.clone() + nodea_name);
                            let _ = generate_line(
                                nodea_loc,
                                nodeb_loc,
                                ga,
                                &nodea_name,
                                &nodeb_name,
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
            all_node_loc.insert(node.node_text.clone(), pos);
        }

        for (_, mut path, mut conn) in query_conn.iter_mut() {
            let node1_loc = all_node_loc.get(&conn.node1);
            let node2_loc = all_node_loc.get(&conn.node2);

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
    node1: &String,
    node2: &String,
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
    let occ = &ga.connection_color;
    let mut cc: &[f32; 4] = &[
        Color::BLACK.to_srgba().red,
        Color::BLACK.to_srgba().blue,
        Color::BLACK.to_srgba().green,
        Color::BLACK.to_srgba().alpha,
    ];
    let _ = occ.as_ref().is_some_and(|color| {
        cc = color;
        true
    });
    println!("color {:?}", cc);
    commands
        .spawn((
            ShapeBundle { path, ..default() },
            Stroke::new(Color::linear_rgba(cc[0], cc[1], cc[2], cc[3]), 3.0),
            NodeConnector {
                node1: node1.clone(),
                node2: node2.clone(),
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
