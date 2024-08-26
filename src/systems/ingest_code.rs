use bevy::prelude::*;
use crate::{parser::graphv2::{parse_graph2, File}, resources::{common_assets::{LoadingState, LoadingStateOpt}, graph_def::GraphDefinitionRes}, ui::CodeStorage};

use wasm_bindgen::prelude::*;
use lazy_static::lazy_static;
use std::sync::Mutex;
use schemars::schema_for;
use serde_json;

lazy_static! {
    static ref E_CODE: Mutex<String> = Mutex::new(String::new());
}


#[wasm_bindgen]
pub struct CompileResult {
    result: bool,
    error_log: String,
}

#[wasm_bindgen]
impl CompileResult {
    #[wasm_bindgen(getter)]
    pub fn error_log(&self) -> String {
        self.error_log.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn result(&self) -> bool {
        self.result.clone()
    }
}

#[wasm_bindgen]
pub fn get_code_schema() -> String {
    let schema = schema_for!(File);
    return serde_json::to_string_pretty(&schema).unwrap();
}

#[wasm_bindgen]
#[allow(dead_code)]
pub fn compile_code(s: String) -> CompileResult {
    let ret = parse_graph2(&s);
    match ret {
        Ok(_file) => {
            //graph_defn.graph_defn = file.graph_defn;
            let mut code = E_CODE.lock().unwrap();
            *code = s;
            return CompileResult {
                result: true,
                error_log: "".to_string(),
            };
        }
        Err(e) => {
            return CompileResult {
                result: false,
                error_log: e.to_string(),
            };
        }
    }
} //

#[allow(dead_code)]
pub fn ingest_codechange(
    mut code_store: ResMut<CodeStorage>,
    mut graph_defn: ResMut<GraphDefinitionRes>,
    mut ls: ResMut<LoadingState>,
) {
    let code = E_CODE.lock().unwrap();

    if code_store.code != *code { // TODO: can improve performance by checking a boolean instead
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
