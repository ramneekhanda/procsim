use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

#[cfg(target_arch = "wasm32")]
pub fn console_log(s: &str) {
    log(s);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn console_log(s: &str) {
  println!("{}", s);
}
