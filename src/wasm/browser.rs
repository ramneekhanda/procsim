use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

use web_sys::{CustomEvent, CustomEventInit};
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

#[macro_export]
macro_rules! log_dsa_event {
  ($($arg:tt)*) => {
    let Some(wasm_window) = web_sys::window() else {
      c_log!("error getting window");
      return;
    };
    let Some(document) = wasm_window.document() else {
      c_log!("error getting document");
      return;
    };
    let Some(log_ev_listener) = document.get_element_by_id("dsa-log-event-listener") else {
      c_log!("error getting log event listener");
      return;
    };
    let custom_event_init = web_sys::CustomEventInit::new();
    custom_event_init.set_detail(&wasm_bindgen::JsValue::from_str(&format!($($arg)*)));
    let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("dsa-log-event", &custom_event_init) else {
      c_log!("error creating custom event");
      return;
    };
    log_ev_listener.dispatch_event(&event);
  };
}
