use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

#[macro_export]
macro_rules! c_log {
  ($($arg:tt)*) => {
      crate::wasm::browser::log(&format!($($arg)*).as_str());
  };
}
