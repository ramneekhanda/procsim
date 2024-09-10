use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;

use crate::components::message::*;
use crate::components::node_connector::*;
use crate::resources::common_assets::CommonAssets;
use crate::resources::common_assets::ResourceType;
use crate::resources::graph_def::GraphDefinitionRes;

fn walk_message(path: &lyon_algorithms::path::Path) -> Vec<[f32; 2]> {
    use lyon_algorithms::walk::{walk_along_path, RegularPattern, WalkerEvent};

    let mut x: Vec<[f32; 2]> = vec![];
    let mut pattern = RegularPattern {
        callback: &mut |event: WalkerEvent| {
            x.push(event.position.to_array());
            true // Return true to continue walking the path.
        },
        interval: 0.1,
    };

    let tolerance = 0.1; // The path flattening tolerance.
    let start_offset = 0.0; // Start walking at the beginning of the path.

    walk_along_path(path.iter(), start_offset, tolerance, &mut pattern);
    x
}

pub fn update_message_path(
    mut query_conn: Query<(&mut Messages, &NodeConnector)>,
    mut query_marker: Query<(Entity, &mut MessageMarker)>,
    ca: Res<CommonAssets>,
    gd: Res<GraphDefinitionRes>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let mut font: Handle<Font> = Default::default();
    if let Some(ResourceType::FontHandle(f1)) = ca.resource_map.get("default_font") {
        font = f1.clone();
    };

    let text_style = TextStyle {
        font: font.clone(),
        font_size: 16.0,
        color: gd.graph_defn.graph_attrs.text_color,
    };

    for (entity, _marker) in query_marker.iter_mut() {
        commands.entity(entity).despawn_recursive();
    }

    for (mut msgs, nc) in query_conn.iter_mut() {
        let v_points = walk_message(&nc.path); //TODO: cache this - put it in NodeConnector
        let mut msg_finished = Vec::<Message>::new();

        msgs.msg_inflight.retain(|m: &Message| {
            if m.timer
                .duration()
                .as_millis()
                .abs_diff(m.timer.elapsed().as_millis())
                > 10
            {
                return true;
            } else {
                msg_finished.push(m.clone());
                return false;
            }
        });

        msgs.msg_delivered.append(&mut msg_finished);

        for mesg in msgs.msg_inflight.iter_mut() {
            mesg.timer.tick(time.delta());

            let mut loc = ((v_points.len() as f32 * mesg.timer.elapsed().as_millis() as f32)
                / mesg.timer.duration().as_millis() as f32) as usize;

            if mesg.node_from == nc.id2 {
                loc = v_points.len() - loc;
            }
            if loc >= v_points.len() {
                continue;
            }
            let parent = commands
                .spawn((
                    SpatialBundle {
                        transform: Transform::from_translation(Vec3::new(
                            v_points[loc][0],
                            v_points[loc][1],
                            100.,
                        )),
                        ..Default::default()
                    },
                    MessageMarker {},
                ))
                .id();

            let shape = shapes::RegularPolygon {
                sides: 4,
                feature: shapes::RegularPolygonFeature::Radius(4.0),
                ..shapes::RegularPolygon::default()
            };
            let icon_child = commands
                .spawn((
                    ShapeBundle {
                        path: GeometryBuilder::build_as(&shape),
                        ..default()
                    },
                    Stroke::new(gd.graph_defn.graph_attrs.text_color, 3.0),
                ))
                .id();

            let text_child = commands
                .spawn(Text2dBundle {
                    text: Text::from_section(mesg.str.clone(), text_style.clone())
                        .with_justify(JustifyText::Center), //.with_alignment(text_alignment),
                    transform: Transform::from_translation(Vec3::new(0.0, -20., 100.)),
                    ..default()
                })
                .id();

            commands
                .entity(parent)
                .push_children(&[icon_child, text_child]);
        }
    }
}
