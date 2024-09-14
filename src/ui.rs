use bevy::prelude::*;

use crate::{components::node::SelectedNodeMarker, parser::graphv2::ParamType};
use bevy_egui::{
    egui::{self, widgets::Slider},
    EguiContexts,
};

use crate::parser::graphv2::Node;
use crate::resources::graph_def::GraphDefinitionRes;

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
/*
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

pub fn graph_properties_viewer(
    mut contexts: EguiContexts,
    mut graph_defn: ResMut<GraphDefinitionRes>,
    q_selected: Query<(Entity, &SelectedNodeMarker)>,
) {
    egui::Window::new("Graph Properties").show(contexts.ctx_mut(), |ui| {
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
        if q_selected.iter().count() == 0 {
            return;
        }

        for (entity, selected) in q_selected.iter() {
            egui::CollapsingHeader::new(format!("Selected Node - {}", selected.node_name).as_str())
                .show(ui, |ui| {
                    if let Some(node) = get_node_with_name_mut(&selected.node_name, &mut graph_defn)
                    {
                        egui::Grid::new("Grid - Selected").show(ui, |ui| {
                            for param in &mut node.node_data.params {
                                if let ParamType::Bool { default } = &mut param.param_type {
                                    ui.label(param.name.clone());
                                    ui.checkbox(default, "");
                                    ui.end_row();
                                } else if let ParamType::Integer { default, min, max } =
                                    &mut param.param_type
                                {
                                    ui.label(param.name.clone());
                                    ui.add(Slider::new(
                                        default,
                                        std::ops::RangeInclusive::new(*min, *max),
                                    ));
                                    ui.end_row();
                                } else if let ParamType::Float { default, min, max } =
                                    &mut param.param_type
                                {
                                    ui.label(param.name.clone());
                                    ui.add(Slider::new(
                                        default,
                                        std::ops::RangeInclusive::new(*min, *max),
                                    ));
                                    ui.end_row();
                                } else if let ParamType::String { default } = &mut param.param_type
                                {
                                    ui.label(param.name.clone());
                                    ui.text_edit_singleline(default);
                                    ui.end_row();
                                } else if let ParamType::Option { values, default } =
                                    &mut param.param_type
                                {
                                    ui.label(param.name.clone());
                                    egui::ComboBox::from_label("")
                                        .selected_text(format!("{}", default))
                                        .show_ui(ui, |ui| {
                                            for value in values.iter() {
                                                ui.selectable_value(
                                                    default,
                                                    value.to_string(),
                                                    value,
                                                );
                                            }
                                        });
                                    ui.end_row();
                                }
                            }
                        });
                    }
                });
        }
    });
}
 */
