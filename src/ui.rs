use bevy::prelude::*;

use bevy_egui::{
    egui::{self},
    EguiContexts,
};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};

use crate::resources::graph_def::GraphDefinitionRes;
use crate::{parser::graphv2::parse_graph2, resources::common_assets::{LoadingState, LoadingStateOpt}};

#[derive(Resource)] //
pub struct CodeStorage {
    pub code: String
}

impl Default for CodeStorage {
    fn default() -> Self {
        CodeStorage {
            code: include_str!("../examples/config/config.yaml").to_string()
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
