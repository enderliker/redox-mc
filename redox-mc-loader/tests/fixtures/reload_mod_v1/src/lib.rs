//! First generation of the hot-reload integration fixture.
//!
//! The chat callback cancels messages prefixed with `!`. The v2 fixture cancels
//! messages prefixed with `?` instead, which makes a successful hot reload
//! observable without restarting the JVM.

use redox_mc_abi::types::{FfiResult, FfiStatus, FfiStr};

#[no_mangle]
pub extern "C" fn redox_mod_init() -> i32 {
    println!("[reload-fixture-v1] initialized");
    0
}

#[no_mangle]
pub extern "C" fn redox_mod_on_bootstrap() -> FfiStatus {
    println!("[reload-fixture-v1] bootstrap hook");
    FfiStatus::Success
}

#[no_mangle]
pub extern "C" fn redox_mod_on_server_chat(
    _player_name: FfiStr,
    message: FfiStr,
) -> FfiResult<bool> {
    let message = unsafe { message.as_str().unwrap_or("") };
    println!("[reload-fixture-v1] chat: {message}");
    FfiResult::ok(!message.starts_with('!'))
}
