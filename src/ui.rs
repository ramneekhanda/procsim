use std::{string, sync::Mutex};

use lazy_static::lazy_static;

use bevy::prelude::*;
use bevy_egui::{
    egui::{self},
    EguiContexts,
};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};

use crate::{parser::graphv2::parse_graph2, resources::common_assets::{LoadingState, LoadingStateOpt}};
use crate::parser::graphv2::GraphDefinition;
use wasm_bindgen::prelude::*;

lazy_static! {
    static ref E_CODE: Mutex<String> = Mutex::new(String::new());
}

#[derive(Resource, Default, Debug)]
pub struct GraphDefinitionRes {
    pub graph_defn: GraphDefinition,
}

#[derive(Resource)] //
pub struct CodeStorage {
    pub code: String,
    pub console: String,
}

#[wasm_bindgen]
struct CompileResult {
    result: bool,
    errorLog: String,
}

#[wasm_bindgen]
impl CompileResult {
    #[wasm_bindgen(getter)]
    pub fn errorLog(&self) -> String {
        self.errorLog.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn result(&self) -> bool {
        self.result.clone()
    }
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn compile_code(s: String) -> CompileResult {
    let ret = parse_graph2(&s);
    match ret {
        Ok(_file) => {
            //graph_defn.graph_defn = file.graph_defn;
            let mut code = E_CODE.lock().unwrap();
            *code = s;
            return CompileResult {
                result: true,
                errorLog: "".to_string(),
            };
        }
        Err(e) => {
            alert("Please resolve the errors!");
            return CompileResult {
                result: false,
                errorLog: e.to_string(),
            };
        }
    }
} //

impl Default for CodeStorage {
    fn default() -> Self {
        CodeStorage {
            code: include_str!("../examples/config/config.yaml").to_string(),
            console: "".to_string(),
        }
    }
}

pub fn ingest_codechange(
    mut code_store: ResMut<CodeStorage>,
    mut graph_defn: ResMut<GraphDefinitionRes>,
    mut ls: ResMut<LoadingState>,
) {
    let code = E_CODE.lock().unwrap();

    if code_store.code != *code {
        ls.state = LoadingStateOpt::Loading;
        code_store.code = (*code).clone();

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
