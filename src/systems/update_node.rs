use crate::components::node::Node;
use crate::components::node::NodeTimers;
use crate::parser::graphv2::Attrs;
use crate::systems::drag;
use crate::ui::GraphDefinitionRes;
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_prototype_lyon::prelude::*;
use bevy_tweening::{lens::*, *};
use rand::Rng;
use std::collections::HashSet;
use std::time::Duration;

pub fn update_nodes(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    g: Res<GraphDefinitionRes>,
    query: Query<Entity, With<Node>>,
) {
    if g.is_changed() {
        for entity in query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        let mut z = 0.;
        let mut node_set = HashSet::<String>::new();
        for node in g.graph_defn.graph.keys() {
            node_set.insert(node.clone());
            let v = g.graph_defn.graph.get(node);
            match v {
                Some(val) => {
                    for conn_node in val.iter() {
                        node_set.insert(conn_node.clone());
                    }
                }
                None => {}
            }
        }

        for node in node_set.iter() {
            let mut drawn = false;
            for node_d in g.graph_defn.nodes.iter() {
                if node_d.name == *node {
                    spawn_node(
                        z,
                        node.clone(),
                        &mut commands,
                        &asset_server,
                        node_d.attrs.clone(),
                    );
                    z += 1.;
                    drawn = true;
                }
            }
            if !drawn {
                spawn_node(z, node.clone(), &mut commands, &asset_server, None);
                z += 1.;
            }
        }
    } else {
    }
}

fn spawn_node(
    z: f32,
    node_name: String,
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    o_attrs: Option<Attrs>,
) {
    //TODO move this to setup
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    let text_style = TextStyle {
        font: font.clone(),
        font_size: 16.0,
        color: Color::WHITE,
    };
    let mut color: [f32; 4] = [1.0, 0.0, 0.5, 1.0];
    let _ = o_attrs.is_some_and(|a| {
        a.color.is_some_and(|c| {
            color.clone_from(&c);
            true
        })
    });

    let shape = shapes::RegularPolygon {
        sides: 4,
        feature: shapes::RegularPolygonFeature::Radius(30.0),
        ..shapes::RegularPolygon::default()
    };

    let mut rng = rand::thread_rng();
    let x = rng.gen_range(-250.0..250.0);
    let y = rng.gen_range(-250.0..250.0);

    let tween: Tween<Transform> = Tween::new(
        EaseFunction::QuadraticInOut,
        Duration::from_millis(500),
        TransformPositionLens {
            start: Vec3::ZERO,
            end: Vec3::new(x, y, z),
        },
    );

    let parent = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(0., 0., 100.)),
                ..Default::default()
            },
            Node {
                node_text: node_name.clone(),
                ..Default::default()
            },
            Animator::new(tween),
        ))
        .id();

    let icon_child = commands
        .spawn((
            ShapeBundle {
                path: GeometryBuilder::build_as(&shape),
                ..default()
            },
            On::<Pointer<DragStart>>::target_insert(Pickable::IGNORE),
            On::<Pointer<DragEnd>>::target_insert(Pickable::default()),
            On::<Pointer<Drag>>::run(drag::drag),
            On::<Pointer<Out>>::target_remove::<NodeTimers>(),
            Fill::color(Color::linear_rgba(color[0], color[1], color[2], color[3])),
            Stroke::new(Color::BLACK, 3.0),
        ))
        .id();

    let text_child = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(node_name, text_style).with_justify(JustifyText::Center),
                transform: Transform::from_translation(Vec3::new(0.0, -35., 100.)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();

    commands
        .entity(parent)
        .push_children(&[icon_child, text_child]);
}

// #[test]
// fn did_spawn_node() {
//   use std::collections::HashSet;
//   use bevy::asset::AssetServer;
//   use bevy::asset::FileAssetIo;
//   use bevy::tasks::IoTaskPool;
//   use crate::parser::graphv2::Node as NodeData;

//   let mut app = App::new();
//   let mut graph = NodeDepsMap::new();
//   let mut hs = HashSet::new();

//   hs.insert("b".to_string());
//   graph.insert("a".to_string(), hs);

//   let mut graph_defn = GraphDefinitionRes::default();
//   graph_defn.graph_defn.graph = graph;
//   // graph_defn.graph_defn.nodes = vec!(NodeData {
//   //   name: "a".to_string(),
//   //   ..Default::default()
//   // }, NodeData {
//   //   name: "b".to_string(),
//   //   ..Default::default()
//   // });
//   // println!("{:?}", graph_defn);

//   app.insert_resource(graph_defn);
//   IoTaskPool::init(Default::default);
//   app.insert_resource(AssetServer::new(FileAssetIo::new("./assets", &None)));
//   app.add_systems(Update, update_nodes);
//   app.update();
//   assert_eq!(app.world.query::<&Node>().iter(&app.world).count(), 2); // check all the nodes have been spawned
//   assert_eq!(app.world.query::<Entity>().iter(&app.world).count(), 6); // check that three entities are created per node
//   app.world.resource_mut::<GraphDefinitionRes>().graph_defn.graph.clear();
//   app.update();
//   assert_eq!(app.world.query::<&Node>().iter(&app.world).count(), 0); // check if we change the graph the response is acceptable
//   assert_eq!(app.world.query::<Entity>().iter(&app.world).count(), 0); // check that entities are deleted as expected

// }
