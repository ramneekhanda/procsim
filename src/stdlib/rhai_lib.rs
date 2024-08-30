use crate::c_log;
pub fn rhai_send(_msg: (String, String), _ctx: String) -> bool {
  c_log!("fnction called");
    true
}

pub fn rhai_log(s: &str) {
    c_log!("{}", s);
}
