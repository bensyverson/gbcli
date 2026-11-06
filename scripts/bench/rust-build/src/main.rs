use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize)] struct Req { id: u64, cmd: String }
extern "C" { fn GB_alloc() -> *mut std::ffi::c_void; }
fn main() {
    let r: Req = serde_json::from_str(r#"{"id":1,"cmd":"ping"}"#).unwrap();
    let p = unsafe { GB_alloc() };
    println!("{} {} {:?}", r.id, serde_json::to_string(&r).unwrap(), p.is_null());
}
