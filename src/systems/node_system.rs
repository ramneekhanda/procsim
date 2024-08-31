use crate::components::node::{SelectedNodeMarker,NodeMarker};
use crate::parser::graphv2::{GraphAttrs, Node};
use crate::resources::common_assets::CommonAssets;
use crate::resources::common_assets::ResourceType;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::systems::drag;
use crate::c_log;
use bevy::text::TextLayoutInfo;
use bevy::transform::commands;
use bevy_prototype_lyon::prelude::*;

use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use bevy_tweening::{lens::*, *};
use rand::Rng;
use std::time::Duration;
use crate::resources::graph_def::GraphChange;

pub fn create_nodes(
    mut commands: Commands,
    ca: Res<CommonAssets>,
    g: Res<GraphDefinitionRes>,
    query: Query<Entity, With<NodeMarker>>,
    mut event_reader: EventReader<GraphChange>,
) {
  if event_reader.read().count() > 0 {
    c_log!("Graph Change Event");
    for entity in query.iter() {
      commands.entity(entity).despawn_recursive();
    }
    let mut z = 0.;
    let g_attrs: &GraphAttrs = &g.graph_defn.graph_attrs;
    for node in g.graph_defn.node_instances.iter() {
        spawn_node(z, node, &mut commands, &ca, g_attrs);
        z += 1.;
    }
  }
}

use wasm_bindgen::prelude::*;
#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

pub fn on_click(
    e: Listener<Pointer<Click>>,
    mut commands: Commands,
    mut q_selected: Query<(Entity, &SelectedNodeMarker)>,
    mut q: Query<(Entity, &mut Transform, &mut Children, &NodeMarker)>,
    text_query: Query<&TextLayoutInfo>,
) {
    if q.iter().count() == 0 {
        return;
    }
    for (entity, _) in q_selected.iter_mut() {
        commands.entity(entity).despawn_recursive();
    }

    for (mut entity, mut transform, children, node) in q.iter_mut() {
        let mut selected: bool = false;
        let mut scale = 1.0;
        for child in children.iter() {
            if *child == e.target() {
                if selected == false {
                    selected = true;
                }
            }
        }
        if selected {
            scale = 1.25;
            let id = commands.spawn(SelectedNodeMarker {
              node_name: node.node_name.clone(),
              node_type: node.node_type.clone(),
            }).id();
            commands.entity(entity).push_children(&[id]);
        } 
        transform.scale = Vec3::new(scale, scale, 1.0);
    }
}

fn spawn_node(
    z: f32,
    node: &Node,
    commands: &mut Commands,
    ca: &Res<CommonAssets>,
    g_attrs: &GraphAttrs,
) {
    //TODO move this to setup
    let mut font: Handle<Font> = Default::default();
    if let Some(ResourceType::FontHandle(f1)) = ca.resource_map.get("default_font") {
        font = f1.clone();
    };

    let mut def_icon: Handle<Image> = Default::default();
    if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get("default_system_icon") {
        def_icon = img.clone();
    };
    //TODO move end

    let text_style = TextStyle {
        font: font.clone(),
        font_size: 16.0,
        color: g_attrs.text_color,
    };
    let node_attrs = &node.node_data.attrs;
    let mut node_icon: Handle<Image> = def_icon;
    let _ = node_attrs.icon.as_ref().is_some_and(|icon_txt| {
        if let Some(ResourceType::ImageHandle(img)) = ca.resource_map.get(icon_txt) {
            node_icon = img.clone();
        };
        true
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

    let shape = shapes::RegularPolygon {
        sides: 4,
        feature: shapes::RegularPolygonFeature::Radius(22.0),
        ..shapes::RegularPolygon::default()
    };

    let txt_bndl = Text2dBundle {
        text: Text::from_section(&node.name, text_style).with_justify(JustifyText::Center),
        transform: Transform::from_translation(Vec3::new(0.0, -24., 100.)),
        ..default()
    };

    let parent = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(0., 0., 100.)),
                ..Default::default()
            },
            Stroke::new(Color::BLACK, 1.5),
            NodeMarker {
                node_type: node.node_data.id.clone(),
                node_name: node.name.clone(),
                ..Default::default()
            },
            On::<Pointer<Drag>>::run(drag::drag),
            On::<Pointer<Click>>::run(on_click),
            Animator::new(tween),
        ))
        .id();

    let icon_child = commands
        .spawn((SpriteBundle {
            texture: node_icon.clone(),
            sprite: Sprite {
                custom_size: Vec2::new(32., 32.).into(),
                ..Default::default()
            },
            ..default()
        },))
        .id();

    let text_child = commands.spawn(txt_bndl).id();

    commands
        .entity(parent)
        .push_children(&[icon_child, text_child]);
    c_log!(
        "Entities are Parent {}, Icon {}, Text {}",
        parent, icon_child, text_child
    );
}

// #[test]
// fn did_spawn_node() {
//     use crate::parser::graphv2::parse_graph2;
//     let mut app = App::new();
//     let res = parse_graph2(&include_str!("../../examples/tests/update_node.yaml").to_string());

//     let mut graph_defn = GraphDefinitionRes::default();
//     graph_defn.graph_defn = res.unwrap().graph_defn;

//     app.add_plugins((
//         MinimalPlugins,
//         AssetPlugin::default(),
//         ImagePlugin::default(),
//     ));
//     let _assets = app.world().resource::<AssetServer>();
//     app.init_asset::<bevy::text::Font>();
//     app.insert_resource(graph_defn);

//     app.add_systems(Update, update_nodes);
//     app.update();

//     assert_eq!(
//         app.world_mut().query::<&Node>().iter(&app.world()).count(),
//         3
//     ); // check all the nodes have been spawned
//     assert_eq!(
//         app.world_mut().query::<Entity>().iter(&app.world()).count(),
//         9
//     ); // check that three entities are created per node
//     app.world_mut()
//         .resource_mut::<GraphDefinitionRes>()
//         .graph_defn
//         .graph
//         .clear();
//     app.update();
//     assert_eq!(
//         app.world_mut().query::<&Node>().iter(&app.world()).count(),
//         0
//     ); // check if we change the graph the response is acceptable
//     assert_eq!(
//         app.world_mut().query::<Entity>().iter(&app.world()).count(),
//         0
//     ); // check that entities are deleted as expected
// }
