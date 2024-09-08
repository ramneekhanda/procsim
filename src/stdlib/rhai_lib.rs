use crate::{c_log, log_dsa_event};
pub fn rhai_send(_msg: (String, String), _ctx: String) -> bool {
    c_log!("fnction called");
    true
}

pub fn rhai_log(s: &str) {
    log_dsa_event!("{}", s);
}
