use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

pub fn zoom_panel(
    mut evr_scroll: EventReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query_camera: Query<&mut OrthographicProjection, With<Camera2d>>,
) {
    for ev in evr_scroll.read() {
        if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::SuperLeft) {
            let mut projection = query_camera.single_mut();
            if ev.y > 0.0 {
                projection.scale /= 1.1;
                // zoom in
            } else {
                projection.scale *= 1.1;
                // zoom out
            }
        } else {
        }
    }
}
