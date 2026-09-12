use bevy::prelude::*;

use crate::parser::graphv2::ParamType;
use bevy_egui::{
    egui::{self, widgets::Slider},
    EguiContexts,
};

use crate::parser::graphv2::Node;
use crate::resources::graph_def::GraphDefinitionRes;
use crate::resources::ui_state::{GraphPropertiesOpen, NodePropertiesPopup};

#[derive(Resource)] //
pub struct CodeStorage {
    pub code: String,
}

impl Default for CodeStorage {
    fn default() -> Self {
        CodeStorage {
            code: String::new(),
        }
    }
}

fn get_node_with_name_mut<'a>(
    name: &str,
    graph_defn: &'a mut GraphDefinitionRes,
) -> Option<&'a mut Node> {
    let idx = graph_defn
        .graph_defn
        .node_instances
        .iter()
        .position(|x| x.name == name);
    if let Some(idx) = idx {
        return Some(&mut graph_defn.graph_defn.node_instances[idx]);
    } else {
        return None;
    }
}

pub fn color_picker(ui: &mut egui::Ui, color: &mut bevy::color::Color, label: &str) {
    ui.collapsing(label, |ui| {
        egui::Grid::new("bgcolor").show(ui, |ui| {
            let mut bg_color = color.to_srgba();

            ui.label("Red:");
            let red = ui.add(Slider::new(&mut bg_color.red, 0. ..=1.0));
            ui.end_row();

            ui.label("Green:");
            let green = ui.add(Slider::new(&mut bg_color.green, 0. ..=1.0));
            ui.end_row();

            ui.label("Blue:");
            let blue = ui.add(Slider::new(&mut bg_color.blue, 0. ..=1.0));

            if red.changed() || green.changed() || blue.changed() {
                *color = bevy::color::Color::from(bg_color);
            };
            ui.end_row();
        });
    });
}

/// Renders one node's editable param grid - shared by `node_properties_popup`
/// so a node's params aren't laid out twice in two different places.
fn node_param_editor(ui: &mut egui::Ui, node: &mut Node) {
    egui::Grid::new(format!("params-{}", node.name)).show(ui, |ui| {
        for param in node.node_data.params.iter_mut().flatten() {
            if let ParamType::Bool { default } = &mut param.param_type {
                ui.label(param.name.clone());
                ui.checkbox(default, "");
                ui.end_row();
            } else if let ParamType::Integer { default, min, max } = &mut param.param_type {
                ui.label(param.name.clone());
                ui.add(Slider::new(
                    default,
                    std::ops::RangeInclusive::new(*min, *max),
                ));
                ui.end_row();
            } else if let ParamType::Float { default, min, max } = &mut param.param_type {
                ui.label(param.name.clone());
                ui.add(Slider::new(
                    default,
                    std::ops::RangeInclusive::new(*min, *max),
                ));
                ui.end_row();
            } else if let ParamType::String { default } = &mut param.param_type {
                ui.label(param.name.clone());
                ui.text_edit_singleline(default);
                ui.end_row();
            } else if let ParamType::Option { values, default } = &mut param.param_type {
                ui.label(param.name.clone());
                egui::ComboBox::from_label("")
                    .selected_text(format!("{}", default))
                    .show_ui(ui, |ui| {
                        for value in values.iter() {
                            ui.selectable_value(default, value.to_string(), value);
                        }
                    });
                ui.end_row();
            }
        }
    });
}

/// Simulation-wide settings (speed, colors) - deliberately *not* the node
/// param editor (see `node_properties_popup`), so this window stays a fixed,
/// small size regardless of which/how many nodes exist. Hidden until
/// summoned by double-clicking empty canvas background
/// (`node_system::open_graph_properties_on_background_double_click`); closed
/// again by double-clicking background a second time or by clicking
/// anywhere outside the window (`Response::clicked_elsewhere` - no close
/// button of its own to keep the title bar uncluttered).
pub fn graph_properties_viewer(
    mut contexts: EguiContexts,
    mut graph_defn: ResMut<GraphDefinitionRes>,
    mut sim_time: ResMut<Time<Virtual>>,
    mut open: ResMut<GraphPropertiesOpen>,
    // Drives both the "just opened" skip (see below) and repositioning:
    // tracks the `click_pos` last used to place the window, in bit-pattern
    // form (see `ui::node_properties_popup`'s identical field for why not a
    // plain float comparison).
    mut last_shown: Local<Option<Option<(u32, u32)>>>,
) {
    if !open.open {
        *last_shown = None;
        return;
    }
    let click_key = open.click_pos.map(|p| (p.x.to_bits(), p.y.to_bits()));
    // Skips the "close on outside click" check on the exact frame the
    // window opens (or reopens at a new position) - otherwise the very
    // click that opened it, landing anywhere but this window's rect, would
    // register as "elsewhere" and close it again immediately, same frame.
    let just_opened = *last_shown != Some(click_key);
    *last_shown = Some(click_key);

    let mut window = egui::Window::new("Graph Properties").collapsible(false);
    // Opens centered right where the user double-clicked, rather than a
    // fixed default spot - on a large canvas, a fixed position is almost
    // never near a later click, so *any* subsequent click anywhere else
    // would immediately look like an "outside" click and close it.
    // Centering (rather than placing the click at the window's top-left
    // *corner*) matters here specifically: a corner sits exactly on the
    // interact rect's boundary, and floating-point/frame-margin rounding
    // can then land the click a fraction of a pixel outside it - "clicked
    // outside" was intermittently true on the very click that opened the
    // window. `pivot` has to be set on *every* frame the window is shown,
    // not only the opening one - egui only remembers the plain top-left
    // position across frames, so on any frame `pivot` isn't passed it
    // falls back to treating that remembered position as top-left again,
    // undoing the centering. `current_pos` (which actually moves the
    // window) stays gated to `just_opened` alone, same reasoning as
    // `node_properties_popup`: applied every frame it would fight the user
    // dragging the window while it's open.
    window = window.pivot(egui::Align2::CENTER_CENTER);
    if just_opened {
        if let Some(pos) = open.click_pos {
            window = window.current_pos(egui::pos2(pos.x, pos.y));
        }
    }

    let response = window.show(contexts.ctx_mut(), |ui| {
        let mut speed = sim_time.relative_speed();
        ui.horizontal(|ui| {
            ui.label("Simulation Speed:");
            if ui
                .add(Slider::new(&mut speed, 0.1..=5.0).suffix("x"))
                .changed()
            {
                sim_time.set_relative_speed(speed);
            }
        });
        ui.separator();

        color_picker(
            ui,
            &mut graph_defn.graph_defn.graph_attrs.background,
            "Background Color",
        );
        color_picker(
            ui,
            &mut graph_defn.graph_defn.graph_attrs.text_color,
            "Text Color",
        );
        color_picker(
            ui,
            &mut graph_defn.graph_defn.graph_attrs.connection_color,
            "Connection Color",
        );
    });
    if !just_opened {
        if let Some(response) = response {
            if response.response.clicked_elsewhere() {
                open.open = false;
            }
        }
    }
}

/// A node's params, in their own popup rather than tucked into Graph
/// Properties - opened by double-clicking the node (`node_system::on_click`),
/// closed by clicking anywhere outside it (`Response::clicked_elsewhere`, no
/// close button of its own), `NodePropertiesPopup` being reset when the
/// graph reloads, or the node itself being removed/despawned.
/// Gap between the node's icon and the bottom of the popup, in logical
/// pixels - big enough to clear the icon and its name label underneath it.
const NODE_POPUP_MARGIN_ABOVE: f32 = 48.0;

pub fn node_properties_popup(
    mut contexts: EguiContexts,
    mut graph_defn: ResMut<GraphDefinitionRes>,
    mut popup: ResMut<NodePropertiesPopup>,
    // Skips the "close on outside click" check on the exact frame the popup
    // opens - the double-click that opened it landed on the node/canvas, not
    // on this (not-yet-existing) window, so it would otherwise register as
    // an immediate outside click and close the popup the same frame it
    // opened. Also drives repositioning: tracks (node, anchor) so a fresh
    // double-click - on the same node after it's moved, on a different node,
    // or reopening after a close - always snaps the window back to the
    // node's current position instead of wherever egui last remembered it
    // (egui persists a window's position across opens, keyed by its title;
    // `default_pos` only ever applies the *very first* time a given title is
    // shown, so relying on it alone left every later open stuck at that
    // first position even as the node moved). `current_pos` forces the
    // position - but only on the frame something actually changed, so it
    // doesn't fight the user dragging the window while it's open.
    mut last_shown: Local<Option<(String, Option<(u32, u32)>)>>,
) {
    let Some(node_name) = popup.node_name.clone() else {
        *last_shown = None;
        return;
    };
    // Bit-pattern comparison (not float equality) - just needs to detect
    // "did this change since last frame", never arithmetic.
    let anchor_key = popup
        .anchor_screen_pos
        .map(|p| (p.x.to_bits(), p.y.to_bits()));
    let current_key = (node_name.clone(), anchor_key);
    let just_opened = last_shown.as_ref() != Some(&current_key);
    *last_shown = Some(current_key);

    let mut window = egui::Window::new(format!("Node - {node_name}")).collapsible(false);
    if let Some(pos) = popup.anchor_screen_pos {
        window = window.pivot(egui::Align2::CENTER_BOTTOM);
        // Anchors the window's bottom-center to just above the node. Forced
        // (via `current_pos`) only on the frame it just opened/moved/
        // switched nodes - otherwise left alone so the window stays
        // movable while open (see the doc comment above).
        if just_opened {
            window = window.current_pos(egui::pos2(pos.x, pos.y - NODE_POPUP_MARGIN_ABOVE));
        }
    }
    let response = window.show(contexts.ctx_mut(), |ui| {
        if let Some(node) = get_node_with_name_mut(&node_name, &mut graph_defn) {
            node_param_editor(ui, node);
        } else {
            ui.label("This node no longer exists.");
        }
    });
    if !just_opened {
        if let Some(response) = response {
            if response.response.clicked_elsewhere() {
                *popup = NodePropertiesPopup::default();
            }
        }
    }
}
