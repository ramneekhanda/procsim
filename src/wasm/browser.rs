#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

// Native builds (cargo build/test on the host) don't have a JS console; fall back to stdout
// so the shared c_log!/log_dsa_event! call sites don't need to be target-gated everywhere.
#[cfg(not(target_arch = "wasm32"))]
pub fn log(s: &str) {
    println!("{}", s);
}

#[macro_export]
macro_rules! c_log {
  ($($arg:tt)*) => {
      crate::wasm::browser::log(&format!($($arg)*).as_str());
  };
}

#[cfg(target_arch = "wasm32")]
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
    let _ = log_ev_listener.dispatch_event(&event);
  };
}

#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! log_dsa_event {
  ($($arg:tt)*) => {
      c_log!($($arg)*);
  };
}
