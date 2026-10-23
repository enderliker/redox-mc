//! Second generation of the hot-reload integration fixture.
//!
//! Its chat callback cancels messages prefixed with `?`, proving that a reload
//! replaced the callback table rather than keeping the v1 closure alive.

use redox_mc_abi::types::{FfiResult, FfiStatus, FfiStr};

#[no_mangle]
pub extern "C" fn redox_mod_init() -> i32 {
    println!("[reload-fixture-v2] initialized");
    0
}

#[no_mangle]
pub extern "C" fn redox_mod_on_bootstrap() -> FfiStatus {
    println!("[reload-fixture-v2] bootstrap hook");
    FfiStatus::Success
}

#[no_mangle]
pub extern "C" fn redox_mod_on_server_chat(
    _player_name: FfiStr,
    message: FfiStr,
) -> FfiResult<bool> {
    let message = unsafe { message.as_str().unwrap_or("") };
    println!("[reload-fixture-v2] chat: {message}");
    FfiResult::ok(!message.starts_with('?'))
}
