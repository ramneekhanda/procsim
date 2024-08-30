use bevy::prelude::*;

use crate::components::node::SelectedNode;
use bevy_egui::{
    egui::{self, epaint::color, widgets::Slider},
    EguiContexts,
};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};

use crate::resources::graph_def::GraphDefinitionRes;
use crate::{
    parser::graphv2::{parse_graph2, Node},
    resources::common_assets::{LoadingState, LoadingStateOpt},
};

#[derive(Resource)] //
pub struct CodeStorage {
    pub code: String,
}

impl Default for CodeStorage {
    fn default() -> Self {
        CodeStorage {
            code: include_str!("../examples/config/config.yaml").to_string(),
        }
    }
}

pub fn draw_codeviewer(
    mut contexts: EguiContexts,
    mut code_store: ResMut<CodeStorage>,
    mut graph_defn: ResMut<GraphDefinitionRes>,
    mut ls: ResMut<LoadingState>,
) {
    egui::Window::new("Hello").show(contexts.ctx_mut(), |ui| {
        ui.vertical_centered(|ui| {
            if ui.button("compile").clicked() {
                ls.state = LoadingStateOpt::Loading;
                let res = parse_graph2(&code_store.code);

                match res {
                    Ok(file) => {
                        graph_defn.graph_defn = file.graph_defn;
                    }
                    Err(e) => {
                        println!("{e}");
                        return;
                    }
                }
            }
        });
        CodeEditor::default()
            .with_rows(20)
            .with_fontsize(14.0)
            .with_theme(ColorTheme::GRUVBOX)
            .with_syntax(Syntax::lua())
            .with_numlines(true)
            .vscroll(true)
            .show(ui, &mut code_store.code);
    });
}

fn get_node_with_name_mut<'a>(
    name: &str,
    graph_defn: &'a mut GraphDefinitionRes,
) -> Option<&'a mut Node> {
    for node in graph_defn.graph_defn.nodes.iter_mut() {
        if node.name == name {
            return Some(node);
        }
    }
    None
}

fn get_node_with_name<'a>(name: &str, graph_defn: &'a GraphDefinitionRes) -> Option<&'a Node> {
    for node in graph_defn.graph_defn.nodes.iter() {
        if node.name == name {
            return Some(node);
        }
    }
    None
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

pub fn node_properties_viewer(
    mut contexts: EguiContexts,
    mut q_selected: Query<(Entity, &SelectedNode)>,
    mut graph_defn: ResMut<GraphDefinitionRes>,
) {
    use egui::widgets::color_picker;
    if q_selected.iter().count() == 0 {
        return;
    }

    for (entity, selected) in q_selected.iter() {
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
        });
    }
}

