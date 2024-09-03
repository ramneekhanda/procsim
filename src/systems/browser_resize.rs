use wasm_bindgen::prelude::*;
use crate::c_log;
pub fn handle_browser_resize(
    mut primary_query: bevy::ecs::system::Query<
        &mut bevy::window::Window,
        bevy::ecs::query::With<bevy::window::PrimaryWindow>,
    >,
) {
    let Some(wasm_window) = web_sys::window() else {
        return;
    };

    let document = wasm_window.document().unwrap();
    let canvas = document.get_element_by_id("bevy-canvas").unwrap();
    let canvas_parent = canvas.parent_element().unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();
    
    let canvas_parent = canvas_parent
      .dyn_into::<web_sys::HtmlDivElement>()
      .map_err(|_| ())
      .unwrap();
    let ht = canvas.height();
    let wd = canvas.width();

    let inner_width = canvas_parent.client_width();
    let inner_height = canvas_parent.client_height();
    
    let target_width = inner_width;
    let target_height = inner_height;

    for mut window in &mut primary_query {
        if window.resolution.width() != target_width as f32 || window.resolution.height() != target_height as f32{
            window.resolution.set(target_width as f32, target_height as f32);
            c_log!("Resizing window to: {}x{}", target_width, target_height);
        }
    }
}
