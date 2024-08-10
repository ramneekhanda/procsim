use bevy::prelude::*;
use rand::Rng;
use std::time::Duration;

use crate::components::message::*;
use crate::components::node_connector::*;

#[derive(Resource, Debug, Clone)]
pub struct DemoTimer {
    pub timer: Timer,
}

pub fn demo_send_message(
    time: Res<Time>,
    mut q: ResMut<DemoTimer>,
    mut query_messages: Query<(Entity, &Message)>,
    mut query_conn: Query<(Entity, &mut NodeConnector)>,
    mut commands: Commands,
) {
    if query_conn.iter().count() < 2 {
        return;
    }

    let mut rng = rand::thread_rng();
    q.as_mut().timer.tick(time.delta());

    if q.timer.finished() {
        //remove demo messages
        for entity in &mut query_messages.iter_mut() {
            commands.entity(entity.0).remove::<Message>();
        }

        for i in 0..2 {
            let pickable: usize = query_conn.iter().count() - 1;
            let picked = rng.gen_range(0..pickable);
            let nct = &mut query_conn.iter_mut().skip(picked).next().unwrap();
            let nc = &nct.1;
            let ent = nct.0;
            println!("spawned a message for {} - {}", nc.node1, nc.node2);
            commands.entity(ent).insert(Message {
                timer: Timer::new(Duration::from_secs(3), TimerMode::Once),
                node_from: nc.node1.clone(),
                node_to: nc.node2.clone(),
                str: "A message".to_string(),
            });
        }
    }
}
