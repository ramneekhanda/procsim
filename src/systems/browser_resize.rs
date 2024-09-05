use crate::c_log;
use wasm_bindgen::prelude::*;

pub fn handle_browser_resize(
    mut primary_query: bevy::ecs::system::Query<
        &mut bevy::window::Window,
        bevy::ecs::query::With<bevy::window::PrimaryWindow>,
    >,
) {
    let Some(wasm_window) = web_sys::window() else {
        return;
    };
    let Some(document) = wasm_window.document() else {
        return;
    };
    let Some(canvas) = document.get_element_by_id("bevy-canvas") else {
        return;
    };
    let Some(canvas_parent) = canvas.parent_element() else {
        return;
    };

    let Ok(canvas_parent) = canvas_parent
        .dyn_into::<web_sys::HtmlDivElement>()
        .map_err(|_| ())
    else {
        return;
    };

    let inner_width = canvas_parent.client_width();
    let inner_height = canvas_parent.client_height();

    let target_width = inner_width;
    let target_height = inner_height;

    for mut window in &mut primary_query {
        if window.resolution.width() != target_width as f32
            || window.resolution.height() != target_height as f32
        {
            window
                .resolution
                .set(target_width as f32, target_height as f32);
        }
    }
}
