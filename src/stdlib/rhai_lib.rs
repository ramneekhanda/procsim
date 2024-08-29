use crate::wasm::browser::console_log;

pub fn rhai_send(_msg: (String, String), _ctx: String) -> bool {
  console_log("fnction called");
  true
}

pub fn rhai_log(s: &str) {
  console_log(s);
}
