use crate::components::node::{ProgressKind, TickProgressFill};
use crate::resources::graph_def::GraphDefinitionRes;
use bevy::prelude::*;
use bevy_prototype_lyon::prelude::*;

pub fn update_tick_progress(
    gd: Res<GraphDefinitionRes>,
    mut bars: Query<(
        &TickProgressFill,
        Option<&mut Sprite>,
        Option<&mut Transform>,
        Option<&mut Path>,
    )>,
) {
    for (bar, mut sprite, mut transform, mut path) in bars.iter_mut() {
        let Some(node) = gd
            .graph_defn
            .node_instances
            .iter()
            .find(|n| n.name == bar.node_name)
        else {
            continue;
        };

        let remaining = node.timer.fraction_remaining().clamp(0.0, 1.0);

        match &bar.kind {
            ProgressKind::Bar { origin_x, w } => {
                if let (Some(ref mut s), Some(ref mut t)) = (&mut sprite, &mut transform) {
                    let cur_w = (w * remaining).max(0.0);
                    s.custom_size = Some(Vec2::new(cur_w, s.custom_size.unwrap_or_default().y));
                    t.translation.x = -w / 2.0 + cur_w / 2.0 + origin_x;
                }
            }
            ProgressKind::Ring {
                center,
                r,
                start_angle,
                clockwise,
            } => {
                if let Some(ref mut p) = path {
                    let sweep_deg = 360.0 * remaining;
                    let mut pb = PathBuilder::new();
                    if sweep_deg > 0.5 {
                        let start_rad = start_angle.to_radians();
                        let start_pt =
                            *center + Vec2::new(r * start_rad.cos(), r * start_rad.sin());
                        let sweep_sign = if *clockwise { -1.0 } else { 1.0 };
                        let sweep_angle = sweep_sign * sweep_deg.to_radians();
                        pb.move_to(start_pt);
                        pb.arc(*center, Vec2::splat(*r), sweep_angle, 0.0);
                    }
                    **p = pb.build();
                }
            }
            ProgressKind::Pie {
                center,
                r,
                start_angle,
                clockwise,
            } => {
                if let Some(ref mut p) = path {
                    let sweep_deg = 360.0 * remaining;
                    let mut pb = PathBuilder::new();
                    if sweep_deg > 0.5 {
                        let start_rad = start_angle.to_radians();
                        let start_pt =
                            *center + Vec2::new(r * start_rad.cos(), r * start_rad.sin());
                        let sweep_sign = if *clockwise { -1.0 } else { 1.0 };
                        let sweep_angle = sweep_sign * sweep_deg.to_radians();
                        pb.move_to(*center);
                        pb.line_to(start_pt);
                        pb.arc(*center, Vec2::splat(*r), sweep_angle, 0.0);
                        pb.close();
                    }
                    **p = pb.build();
                }
            }
            ProgressKind::Segmented {
                segment_index,
                total_segments,
                active_color,
                inactive_color,
            } => {
                if let Some(ref mut s) = sprite {
                    let active_limit = (remaining * *total_segments as f32).ceil() as usize;
                    if *segment_index < active_limit {
                        s.color = *active_color;
                    } else {
                        s.color = *inactive_color;
                    }
                }
            }
        }
    }
}
