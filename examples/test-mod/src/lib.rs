//! Example native Redox-MC mod written in 100% Rust.

use redox_mc_api::redox_export;
use redox_mc_api::types::FfiStatus;

#[redox_export]
pub fn redox_mod_init() -> i32 {
    println!("[test-mod] Native Rust mod initialized successfully via redox_mod_init()!");
    0
}

#[redox_export]
pub fn redox_mod_on_bootstrap() {
    println!("[test-mod] Hook received: Bootstrap.bootStrap() entered! Game is bootstrapping vanilla registries.");
}

#[redox_export]
pub fn redox_mod_on_register_content() -> FfiStatus {
    println!(
        "[test-mod] Hook received: BuiltInRegistries.freeze()! Registries open for custom content."
    );
    FfiStatus::Success
}
