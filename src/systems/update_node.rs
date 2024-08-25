use crate::components::node::Node;
use crate::components::node::NodeTimers;
use crate::parser::graphv2::Attrs;
use crate::resources::common_assets::CommonAssets;
use crate::resources::common_assets::ResourceType;
use crate::systems::drag;
use crate::resources::graph_def::GraphDefinitionRes;
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_tweening::{lens::*, *};
use rand::Rng;
use std::collections::HashSet;
use std::time::Duration;

pub fn update_nodes(
    mut commands: Commands,
    ca: Res<CommonAssets>,
    g: Res<GraphDefinitionRes>,
    query: Query<Entity, With<Node>>
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
                        &ca,
                        node_d.attrs.clone()
                    );
                    z += 1.;
                    drawn = true;
                }
            }
            if !drawn {
                spawn_node(z, node.clone(), &mut commands, &ca, None);
                z += 1.;
            }
        }
    } else {
    }
}

use wasm_bindgen::prelude::*;
#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

fn spawn_node(
    z: f32,
    node_name: String,
    commands: &mut Commands,
    ca: &Res<CommonAssets>,
    o_attrs: Option<Attrs>,
) {
    //TODO move this to setup
    let mut font: Handle<Font> = Default::default();
    if let Some(ResourceType::FontHandle(f1)) = ca.resource_map.get("default_font"){
        font = f1.clone();
    };
    
    let mut def_icon: Handle<Image> = Default::default();
    if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get("default_system_icon"){
        def_icon = img.clone();
    };

    let text_style = TextStyle {
        font: font.clone(),
        font_size: 16.0,
        color: Color::BLACK,
    };

    let mut color: [f32; 4] = [1.0, 0.0, 0.5, 1.0];
    let _ = o_attrs.clone().is_some_and(|a| {
        a.color.is_some_and(|c| {
            color.clone_from(&c);
            true
        })
    });

    let mut node_icon: Handle<Image> = def_icon;
    let _ = o_attrs.is_some_and(|a| {
      a.icon.is_some_and(|icon_txt| {
        if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get(&icon_txt) {
          node_icon = img.clone();
        };
        true
      })
    });


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
            SpriteBundle {
                // Simply use a url where you would normally use an asset folder relative path
                texture: node_icon.clone(),
                sprite: Sprite {
                    custom_size: Vec2::new(32., 32.).into(),
                    ..Default::default()
                },
                ..default()
            },
            On::<Pointer<DragStart>>::target_insert(Pickable::IGNORE),
            On::<Pointer<DragEnd>>::target_insert(Pickable::default()),
            On::<Pointer<Drag>>::run(drag::drag),
            On::<Pointer<Out>>::target_remove::<NodeTimers>(),
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

#[test]
fn did_spawn_node() {  
  use crate::parse_graph2;
  let mut app = App::new();
  let res = parse_graph2(&include_str!("../../examples/tests/update_node.yaml").to_string());
  
  let mut graph_defn = GraphDefinitionRes::default();
  graph_defn.graph_defn = res.unwrap().graph_defn;
  
  app.add_plugins((MinimalPlugins, AssetPlugin::default(), ImagePlugin::default()));
  let _assets = app.world().resource::<AssetServer>();
  app.init_asset::<bevy::text::Font>();
  app.insert_resource(graph_defn);

  app.add_systems(Update, update_nodes);
  app.update();
  
  assert_eq!(app.world_mut().query::<&Node>().iter(&app.world()).count(), 3); // check all the nodes have been spawned
  assert_eq!(app.world_mut().query::<Entity>().iter(&app.world()).count(), 9); // check that three entities are created per node
  app.world_mut().resource_mut::<GraphDefinitionRes>().graph_defn.graph.clear();
  app.update();
  assert_eq!(app.world_mut().query::<&Node>().iter(&app.world()).count(), 0); // check if we change the graph the response is acceptable
  assert_eq!(app.world_mut().query::<Entity>().iter(&app.world()).count(), 0); // check that entities are deleted as expected

}
