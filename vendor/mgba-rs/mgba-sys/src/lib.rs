#![allow(clippy::all)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// The wall clock `shim.c` asks the Rust side for.
///
/// wasm32-unknown-unknown has no `gettimeofday`, so the shim implements
/// libmgba's three clock reads (savestate stamps, the cart RTC's default)
/// on top of this one symbol. It belongs here, with the shim that declares
/// it: a host that links this crate for wasm shouldn't have to know the
/// C side left a symbol undefined, and every one of them would answer it
/// the same way.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn mgba_sys_now_unix_ms() -> f64 {
    js_sys::Date::now()
}
