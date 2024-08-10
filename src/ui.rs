use bevy::prelude::*;
use bevy_egui::{
    egui::{self},
    EguiContexts,
};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};

use crate::parser::graphv2::parse_graph2;
use crate::parser::graphv2::GraphDefinition;

#[derive(Resource, Default, Debug)]
pub struct GraphDefinitionRes {
    pub graph_defn: GraphDefinition,
}

#[derive(Resource, Default)]
pub struct CodeStorage {
    pub code: String,
    pub console: String,
}

pub fn draw_codeviewer(
    mut contexts: EguiContexts,
    mut code_store: ResMut<CodeStorage>,
    mut graph_defn: ResMut<GraphDefinitionRes>,
) {
    egui::Window::new("Hello").show(contexts.ctx_mut(), |ui| {
        ui.vertical_centered(|ui| {
            if ui.button("compile").clicked() {
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
                for node in graph_defn.graph_defn.nodes.iter() {
                    //node.func
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

fn draw_logviewer(w: &mut World, ui: &mut egui::Ui) {
    w.resource_scope::<CodeStorage, _>(|_, mut code_store| {
        if ui.button("clear").clicked() {
            code_store.console.clear();
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_sized(
                ui.available_size(),
                egui::TextEdit::multiline(&mut code_store.console).frame(false),
            );
        });
    });
}
